//! Gate 64 must name the file it accuses, and must not accuse a fix.
//!
//! Both defects lived in one branch. It ran `unbounded_alloc_re` over
//! `diff_ctx.diff_content` — the whole diff, removals included — and filed the
//! result under `file_path: "hotpath"`, which is not a file: it is the gate's
//! own word for the category, published where a reviewer reads a path.
//!
//! Reading the whole diff is the same inversion found and fixed in the
//! credential scanner. It was still live here.

use anvil::criterion_bench_ratchet::CriterionBenchRatchet;
use anvil::git_manager::diff_context::PrDiffContext;
use std::path::{Path, PathBuf};

const BAD: &str = "for x in xs { let mut v = Vec::new(); v.push(x); }";

fn eval(diff: &str) -> anvil::criterion_bench_ratchet::BenchmarkReport {
    let ctx = PrDiffContext {
        repo: "oyatie/anvil".into(),
        pr_number: 1,
        base_branch: "dev".into(),
        base_sha: "a".into(),
        head_sha: "b".into(),
        is_incremental: false,
        previous_head_sha: None,
        diff_content: diff.to_string(),
        changed_files: vec!["src/crypto/token.rs".to_string()],
        repo_working_dir: anvil::git_manager::SubjectRoot::asserted(
            PathBuf::from("."),
            anvil::git_manager::Uncloned::TestFixture,
        ),
    };
    CriterionBenchRatchet
        .evaluate_benchmarks(Path::new("."), &ctx)
        .expect("evaluate")
}

#[test]
fn an_added_unbounded_allocation_is_named_against_its_own_file() {
    let report = eval(&format!(
        "--- a/src/crypto/token.rs\n+++ b/src/crypto/token.rs\n+{BAD}\n"
    ));
    let v = report
        .violations
        .iter()
        .find(|v| v.metric == "UNBOUNDED_LOOP_ALLOCATION")
        .expect("the added allocation must still be found");
    assert_eq!(
        v.file_path, "src/crypto/token.rs",
        "the finding used to be filed under the literal `hotpath`, which is not a file"
    );
}

#[test]
fn removing_an_unbounded_allocation_is_not_adding_one() {
    let report = eval(&format!(
        "--- a/src/crypto/token.rs\n+++ b/src/crypto/token.rs\n-{BAD}\n\
         +for x in xs {{ let mut v = Vec::with_capacity(xs.len()); v.push(x); }}\n"
    ));
    assert!(
        !report
            .violations
            .iter()
            .any(|v| v.metric == "UNBOUNDED_LOOP_ALLOCATION"),
        "the pull request that fixes the allocation was refused for it: {:?}",
        report.violations
    );
}

#[test]
fn the_pattern_does_not_straddle_two_files() {
    // The whole-diff scan could match a `for` in one file against a
    // `Vec::new()` in the next, because the diff is one string. Per-file added
    // text is what makes that impossible rather than unlikely.
    let report = eval(
        "--- a/src/crypto/a.rs\n+++ b/src/crypto/a.rs\n+for x in xs {\n\
         --- a/src/crypto/b.rs\n+++ b/src/crypto/b.rs\n+let mut v = Vec::new();\n",
    );
    assert!(
        !report
            .violations
            .iter()
            .any(|v| v.metric == "UNBOUNDED_LOOP_ALLOCATION"),
        "matched across a file boundary: {:?}",
        report.violations
    );
}

#[test]
fn a_finding_without_a_file_header_is_not_filed_against_nothing() {
    // `current_file` was a `String` defaulting to empty, so a `+` line arriving
    // before any header produced a violation whose path was "".
    let report = eval("+let data = payload.clone(); // hotpath\n");
    assert!(
        report.violations.iter().all(|v| !v.file_path.is_empty()),
        "a violation was filed against the empty path: {:?}",
        report.violations
    );
}

#[test]
fn the_clone_rule_still_fires_when_the_file_is_known() {
    let report = eval(
        "--- a/src/crypto/token.rs\n+++ b/src/crypto/token.rs\n\
         +let data = payload.clone(); // hotpath\n",
    );
    let v = report
        .violations
        .iter()
        .find(|v| v.metric == "EXCESSIVE_HOTPATH_CLONE")
        .expect("the clone rule must not have been made inert");
    assert_eq!(v.file_path, "src/crypto/token.rs");
}
