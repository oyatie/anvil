//! A rule cannot read the removed side of a diff without saying why.
//!
//! Seven times a scanner read the whole diff and refused the pull request that
//! DELETED the thing it was looking for. It was fixed in the credential
//! scanner, then in gate 41, then in gate 64, then in three more gates -- and
//! then written FRESH with the same defect in #115, after all of those.
//!
//! That recurrence is the argument. Reading a diff whole is what you get by not
//! thinking about it: a unified diff is one string containing both sides, so
//! the correct behaviour is the deliberate act and the defect is the default.
//! Detection could not hold it, because every detection came after someone had
//! already written the default again.
//!
//! So the default is gone. `FileDiff` hands out `added()` and `after_change()`,
//! neither of which contains a removed line, and the only corpus that does is
//! reachable through `both_sides(BothSides::…)` -- a closed set of reasons.
//! Asking for removals is now a named act that appears in review.

use anvil::git_manager::diff_context::{BothSides, diffs_by_path};
use std::fs;
use std::path::PathBuf;

/// A real hunk. The leading space on a context line is significant -- it is
/// what marks the line as present in the file but untouched by the change --
/// so this is built by concatenation rather than a `\`-continued literal,
/// which strips exactly that space and silently turns context into nothing.
const DIFF: &str = concat!(
    "diff --git a/src/a.rs b/src/a.rs\n",
    "--- a/src/a.rs\n",
    "+++ b/src/a.rs\n",
    "@@ -1,3 +1,3 @@\n",
    " let context = 0;\n",
    "-let removed = 1;\n",
    "+let added = 2;\n",
);

#[test]
fn neither_ordinary_corpus_contains_a_removed_line() {
    // The property the whole class turns on. A rule working from either of
    // these cannot refuse the change that deletes what it looks for, because
    // the deleted text is not in front of it.
    let files = diffs_by_path(DIFF);
    let f = &files[0];

    assert!(f.added().contains("let added = 2;"));
    assert!(!f.added().contains("let removed = 1;"));
    assert!(
        !f.added().contains("let context = 0;"),
        "context is not an addition"
    );

    assert!(f.after_change().contains("let added = 2;"));
    assert!(f.after_change().contains("let context = 0;"));
    assert!(
        !f.after_change().contains("let removed = 1;"),
        "the file after the change does not contain what the change removed"
    );
}

#[test]
fn the_removed_side_is_reachable_only_by_naming_a_reason() {
    // It is still reachable -- some rules genuinely need it -- but not by
    // accident. The reason is a value the caller has to write down.
    let files = diffs_by_path(DIFF);
    let both = files[0].both_sides(BothSides::ContractComparesRemovedFields);
    assert!(both.contains("-let removed = 1;"));
    assert!(both.contains("+let added = 2;"));
}

fn rust_sources() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::from("src")];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let body = fs::read_to_string(&p).unwrap_or_default();
                out.push((p.display().to_string(), body));
            }
        }
    }
    out
}

#[test]
fn only_the_sanctioned_rules_read_removals_and_the_set_is_small() {
    // The reason enum is the review surface. If this list grows, someone added
    // a variant or a caller, and that is exactly the change that should be
    // argued about rather than slipped in.
    let callers: Vec<String> = rust_sources()
        .into_iter()
        // Code, not commentary, and WHOLE-SOURCE rather than line by line.
        // `postmortem/mod.rs` describes this mechanism in its ledger prose, and
        // a substring scan read the description as a call site. A line-wise
        // scan did not fix it either: the mention sits inside a string literal
        // spanning several lines, and a scanner looking at one line at a time
        // cannot know it is inside one.
        .filter(|(path, body)| {
            !path.ends_with("diff_context.rs")
                && anvil::source_scan::code_only(body).contains("both_sides(")
        })
        .map(|(path, _)| path)
        .collect();

    assert_eq!(
        callers.len(),
        1,
        "{} rule(s) read the removed side of a diff. Each one must have a \
         subject that IS the removal -- a wire contract losing a required \
         field -- and not merely find it convenient:\n  {}",
        callers.len(),
        callers.join("\n  ")
    );
    assert!(
        callers[0].contains("cross_service_impact"),
        "the sanctioned reader changed: {}",
        callers[0]
    );
}

#[test]
fn the_reason_set_stays_closed() {
    // One variant today. A second is a design decision, not a refactor: it
    // means a second kind of rule legitimately needs the removed side, and the
    // burden is to say which and why.
    let src = fs::read_to_string("src/git_manager/diff_context.rs").expect("source");
    let block = src
        .split("pub enum BothSides {")
        .nth(1)
        .and_then(|s| s.split('}').next())
        .expect("the reason enum is declared");
    let variants = block
        .lines()
        .map(str::trim)
        .filter(|l| l.ends_with(',') && !l.starts_with("///") && !l.starts_with("//"))
        .count();
    assert_eq!(
        variants, 1,
        "the sanctioned-reason set has {variants} variants. Each one is a rule \
         allowed to read what a change deletes, so each needs an argument:\n{block}"
    );
}
