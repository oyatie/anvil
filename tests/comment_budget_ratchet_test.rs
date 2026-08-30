//! A comment inside a function body is bounded. A blob is not a comment.
//!
//! Prose in source is checked by nothing. It does not compile, no test asserts
//! it, and it goes stale silently the moment the code moves. In this repository
//! it is worse than unverified: the guards read the source as text, so a
//! sentence can *satisfy* the check written to catch its subject. All of these
//! happened, and each was caught only by seeding the guard afterwards:
//!
//!   - `stack_whitelist_guard` refused the pull request that wired it, because
//!     the comment explaining the exclusion contained ``a real `use redis::…` ``
//!     and the scan read the sentence as an adoption of Redis;
//!   - the autonomous-door scan was satisfied by the word "pause" appearing in
//!     the comment beside a door, rather than by a read of the pause;
//!   - `clear_reviewed_sha`'s reason was written out at three call sites, so
//!     one claim had three copies and one maintainer.
//!
//! Length is the symptom; the disease is explaining in prose what a name should
//! carry. A body comment that needs more than [`MAX_BODY_COMMENT_LINES`] lines
//! is a decomposition failure: extract the block and let the function name say
//! it. Doc comments are deliberately exempt -- `///` is the contract, rustdoc
//! renders it, and `# Errors`/`# Panics`/`# Examples` are multi-line by nature.
//! Genre is bounded separately by `comment_genre_ratchet_test`.
//!
//! Width is bounded too, because "one line" and "fixed width" cannot both hold:
//! rustfmt wraps at its max, so an over-wide comment becomes several lines and
//! the pair of rules fights the formatter. One rule, two dimensions.

use anvil::source_scan::code_only;
use std::fs;
use std::path::{Path, PathBuf};

/// Lines a `//` block inside a function body may occupy.
///
/// Three: enough for a claim and its consequence, not enough for a story.
const MAX_BODY_COMMENT_LINES: usize = 3;

/// Columns any comment line may occupy, matching rustfmt's default `max_width`.
const MAX_COMMENT_WIDTH: usize = 100;

/// A floor, not the assertion. The bound that matters is derived from this
/// change's own merge-base below; this one keeps the check alive in a checkout
/// with no git, and catches a catastrophic regression even there.
const OVER_BUDGET_COMMENT_LINES_CEILING: usize = 1156;

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Lines this file spends over budget: body-comment overflow plus over-wide
/// comment lines.
///
/// Depth comes from `code_only`, so a brace inside a string or a comment cannot
/// move it. A `//` line at depth zero is an item-level comment and is left
/// alone; only what is inside a body is bounded.
pub fn over_budget_lines(body: &str) -> usize {
    let code = code_only(body);
    let mut depth = 0i32;
    let mut run = 0usize;
    let mut over = 0usize;

    for (raw, stripped) in body.lines().zip(code.lines()) {
        let t = raw.trim_start();
        let is_doc = t.starts_with("///") || t.starts_with("//!");
        let is_line_comment = t.starts_with("//") && !is_doc;

        if (is_doc || is_line_comment) && raw.chars().count() > MAX_COMMENT_WIDTH {
            over += 1;
        }

        if is_line_comment && depth > 0 {
            run += 1;
        } else {
            over += run.saturating_sub(MAX_BODY_COMMENT_LINES);
            run = 0;
        }

        for c in stripped.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
    }
    over + run.saturating_sub(MAX_BODY_COMMENT_LINES)
}

fn offenders() -> Vec<(PathBuf, usize)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_sources(&root, &mut files);
    let mut out: Vec<(PathBuf, usize)> = files
        .into_iter()
        .filter_map(|p| {
            let body = fs::read_to_string(&p).ok()?;
            let n = over_budget_lines(&body);
            (n > 0).then_some((p, n))
        })
        .collect();
    out.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    out
}

/// The bound that matters: derived from this change's own merge-base, so
/// nothing is written down and two branches that both improve it cannot
/// disagree.
#[test]
fn comment_blobs_do_not_grow_against_the_merge_base() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let derived = rt.block_on(anvil::ratchet::facade::derived::at_merge_base(
        repo,
        "origin/dev",
        "HEAD",
        |p| p.starts_with("src/") && p.ends_with(".rs"),
        |tree| {
            tree.paths()
                .iter()
                .filter(|p| p.starts_with("src/") && p.ends_with(".rs"))
                .filter_map(|p| tree.read(p).ok().flatten())
                .filter_map(|b| std::str::from_utf8(b).ok())
                .map(over_budget_lines)
                .sum::<usize>()
        },
    ));
    let Ok(base) = derived else {
        eprintln!("skipped: no merge-base against origin/dev in this checkout");
        return;
    };

    let now: usize = offenders().iter().map(|(_, n)| n).sum();
    assert!(
        now <= base.at_merge_base,
        "comment lines over budget grew from {} at merge-base {} to {} here.\n\
         A `//` block inside a function body may be {} lines; any comment line \
         may be {} columns. Neither number needs editing when this falls.\n\
         If a body comment needs more, the block it describes wants a name: \
         extract it. If a doc comment needs more, it is a doc comment and this \
         rule does not touch it.\n\
         Worst files here:\n{}",
        base.at_merge_base,
        &base.merge_base[..12],
        now,
        MAX_BODY_COMMENT_LINES,
        MAX_COMMENT_WIDTH,
        offenders()
            .iter()
            .take(8)
            .map(|(p, n)| format!("  {:>4}  {}", n, p.display()))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The floor, for a checkout with no git. Withheld rather than passed is the
/// derived test's job; this one only refuses a catastrophe.
#[test]
fn the_committed_floor_still_holds() {
    let now: usize = offenders().iter().map(|(_, n)| n).sum();
    assert!(
        now <= OVER_BUDGET_COMMENT_LINES_CEILING,
        "{now} comment lines over budget, ceiling {OVER_BUDGET_COMMENT_LINES_CEILING}"
    );
}

/// What the rule counts, stated as fixtures rather than described.
#[test]
fn the_budget_bounds_bodies_and_width_and_nothing_else() {
    // A doc comment of any length is a contract, not a blob.
    let doc = format!("{}\nfn f() {{}}\n", "/// line\n".repeat(20));
    assert_eq!(over_budget_lines(&doc), 0, "doc comments are not bounded");

    // An item-level `//` block is outside every body.
    let item = format!("{}\nfn f() {{}}\n", "// line\n".repeat(20));
    assert_eq!(
        over_budget_lines(&item),
        0,
        "item-level comments are not bounded"
    );

    // Inside a body, three lines are free and the fourth is not.
    let three = "fn f() {\n    // a\n    // b\n    // c\n    g();\n}\n";
    assert_eq!(over_budget_lines(three), 0);
    let four = "fn f() {\n    // a\n    // b\n    // c\n    // d\n    g();\n}\n";
    assert_eq!(
        over_budget_lines(four),
        1,
        "the fourth body line is over budget"
    );

    // A brace inside a string literal must not open a body.
    let braced = "fn f() {\n    let s = \"{\";\n}\n// a\n// b\n// c\n// d\n";
    assert_eq!(
        over_budget_lines(braced),
        0,
        "a brace in a literal moved the depth, so item comments were read as body comments"
    );

    // Width is counted for doc comments too.
    let wide = format!("/// {}\n", "x".repeat(120));
    assert_eq!(
        over_budget_lines(&wide),
        1,
        "an over-wide comment line is over budget"
    );
}
