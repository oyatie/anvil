//! A finding must be able to name the file it is about.
//!
//! # The defect
//!
//! Two guards iterated `changed_files` and, for every path that matched,
//! scanned the WHOLE diff. The path decided whether to look; the diff decided
//! what was found; nothing joined the two. `cloud_native_guard` reported an
//! SDK import added in an adapter as a violation of every `/core/` file in the
//! change, by name. `stack_whitelist_guard` was worse: its `[dependencies]`
//! section flag, raised in one file's hunk, stayed raised through every
//! following file, so three functions added to `src/lib.rs` were reported as
//! three unauthorised dependencies.
//!
//! Both were found the same way -- by reading a nested loop, not by any test
//! failing. Both shipped with tests that passed, because both fixtures were
//! synthetic diff fragments with no `diff --git` header, which the guards'
//! path-from-elsewhere reading tolerated and real `git diff` output never
//! produces.
//!
//! # The two rules
//!
//! The first is absolute and closes what was fixed: a loop over
//! `changed_files` may not read `diff_content` inside it. There is no correct
//! version of that shape -- if the body needs the diff, it needs THAT FILE's
//! hunk, which is what `diffs_by_path` returns.
//!
//! The second is a ratchet, because a whole-diff line scan is not always
//! wrong: a rule asking "does this change mention X anywhere" is legitimately
//! diff-wide. But it cannot attribute what it finds, so a guard that reports
//! a per-file finding from one is guessing. Nineteen remain; the number must
//! fall as rules move onto the attributing parser.
//!
//! Nineteen, not the twenty-one a first hand-written census reported. That one
//! truncated each file at the first `#[cfg(test)]` and matched raw text, so it
//! counted two occurrences that live inside COMMENTARY. Using `code_only` and
//! `without_test_modules` -- the strippers the rest of this codebase already
//! relies on -- is what made the answer right, and is the same argument
//! `stage_liveness` records for the same reason.

use anvil::source_scan::code_only;
use std::fs;
use std::path::{Path, PathBuf};

/// Whole-diff line scans left in production code. EXACT, and it must fall.
const WHOLE_DIFF_LINE_SCANS: usize = 19;

fn production_sources() -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    collect(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src").as_path(),
        &mut out,
    );
    out
}

fn collect(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            let Ok(raw) = fs::read_to_string(&p) else {
                continue;
            };
            // Test modules are stripped two ways: an inline `#[cfg(test)] mod`,
            // and a whole file that its parent declares under `#[cfg(test)]`.
            // A fixture is allowed to spell anything.
            if anvil::source_scan::is_cfg_test_module_file(&p) {
                continue;
            }
            let prod = anvil::source_scan::without_test_modules(&raw);
            // Commentary and string literals are not code. This module's own
            // Commentary and string literals are not code. This module's own
            // doc comment names the forbidden shape repeatedly; reading prose
            // as code is a class this repository has recorded three times.
            out.push((p, code_only(&prod)));
        }
    }
}

#[test]
fn no_loop_over_changed_files_reads_the_whole_diff_inside_it() {
    let mut offenders = Vec::new();
    for (path, src) in production_sources() {
        let lines: Vec<&str> = src.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if !(line.contains("for ") && line.contains("changed_files")) {
                continue;
            }
            // Walk the loop body by brace depth from this line.
            let mut depth: i32 = 0;
            let mut started = false;
            for l in &lines[i..] {
                depth += l.matches('{').count() as i32;
                depth -= l.matches('}').count() as i32;
                if depth > 0 {
                    started = true;
                }
                if started && l.contains("diff_content") {
                    offenders.push(format!("{}:{}", path.display(), i + 1));
                    break;
                }
                if started && depth <= 0 {
                    break;
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a loop over `changed_files` reads `diff_content` inside it. The path \
         chooses whether to look and the diff chooses what is found, so the \
         finding is attributed to whichever path happened to match. Use \
         `diffs_by_path`, whose `FileDiff::path` and `FileDiff::added` come \
         from the same hunk.\n  {}",
        offenders.join("\n  ")
    );
}

/// A ceiling, not an equality.
///
/// An exact literal is the hazard it was meant to prevent: two branches that
/// both lower it write the same line, git merges them cleanly, and the merged
/// tree carries a number that is wrong with no conflict to catch it. The
/// monotone bound is derived below; this only floors a checkout with no
/// merge-base.
#[test]
fn whole_diff_line_scans_do_not_exceed_the_recorded_ceiling() {
    let found: usize = production_sources()
        .iter()
        .map(|(_, s)| {
            // Flattened before matching. rustfmt breaks a long receiver across
            // lines, so `ctx.diff_content.lines()` can be spelled with the
            // `.lines()` on its own line, and a literal matcher misses it. Two
            // such splits exist in this tree; the first draft of this ratchet
            // counted seventeen instead of nineteen because of them -- the same
            // blind-spot shape that made an earlier ratchet miss
            // `pub(super) fn`.
            let flat = s.lines().map(str::trim).collect::<Vec<_>>().join(" ");
            flat.matches("diff_content .lines()").count()
                + flat.matches("diff_content.lines()").count()
        })
        .sum();
    assert!(
        found <= WHOLE_DIFF_LINE_SCANS,
        "whole-diff line scans moved. Falling is the work: a rule that scans \
         the whole diff cannot say which file its finding is in, so any \
         per-file message it produces is a guess. Rising means a new rule was \
         written that cannot attribute what it finds."
    );
}

/// The bound that holds without a committed literal.
///
/// `WHOLE_DIFF_LINE_SCANS` above is a floor kept for checkouts with no
/// merge-base. This is the real rule: no growth against this change's own
/// base, so nothing is written down and two branches that both improve it
/// cannot disagree about the number.
#[test]
fn whole_diff_line_scans_do_not_grow_against_the_merge_base() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let Some(base) = rt.block_on(anvil::ratchet::facade::derived::source_sites_at_merge_base(
        repo,
        "origin/dev",
        "HEAD",
        |_, body| {
            let stripped = anvil::source_scan::without_test_modules(body);
            let flat = anvil::source_scan::code_only(&stripped)
                .lines()
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(" ");
            flat.matches("diff_content .lines()").count()
                + flat.matches("diff_content.lines()").count()
        },
    )) else {
        eprintln!("skipped: no merge-base against origin/dev");
        return;
    };
    let now: usize = production_sources()
        .iter()
        .map(|(_, s)| {
            let flat = s.lines().map(str::trim).collect::<Vec<_>>().join(" ");
            flat.matches("diff_content .lines()").count()
                + flat.matches("diff_content.lines()").count()
        })
        .sum();
    assert!(
        now <= base.at_merge_base,
        "whole-diff line scans grew from {} at merge-base {} to {}. A rule that \
         scans the whole diff cannot say which file its finding is in.",
        base.at_merge_base,
        &base.merge_base[..12],
        now
    );
}
