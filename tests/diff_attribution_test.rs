//! A finding must name the file the diff names, and must not accuse a deletion.
//!
//! Three gates each carried their own copy of this block:
//!
//! ```text
//! let mut current_file = "unknown.rs".to_string();
//! if let Some(first_line) = lines.first()
//!     && let Some(path) = first_line.split_whitespace().last()
//! { current_file = path.trim_start_matches("b/").to_string(); }
//! ```
//!
//! Three copies, the same two defects in each, because they were the same
//! lines pasted three times. Both were measured, not read:
//!
//! ```text
//! constant_work_guard   add_flagged=true  del_flagged=true
//! idempotency_guard     add_flagged=true  del_flagged=true
//! invented-path   path = "registry.rs_lookup(\"a.rs\");"
//! empty-first-line path = "unknown.rs"
//! ```
//!
//! The parsing now lives in one place, `diff_context::diffs_by_path`, and takes
//! the path from the `+++ b/` header -- the only thing in a diff that states it.

use anvil::constant_work_guard::ConstantWorkGuard;
use anvil::finops_ratchet::FinOpsUnitCostRatchet;
use anvil::git_manager::diff_context::{PrDiffContext, diffs_by_path};
use anvil::idempotency_guard::IdempotencyGuard;
use std::path::{Path, PathBuf};

fn ctx(diff: &str, files: &[&str]) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".into(),
        pr_number: 1,
        base_branch: "dev".into(),
        base_sha: "a".into(),
        head_sha: "b".into(),
        is_incremental: false,
        previous_head_sha: None,
        diff_content: diff.to_string(),
        changed_files: files.iter().map(|s| s.to_string()).collect(),
        repo_working_dir: anvil::git_manager::SubjectRoot::asserted(
            PathBuf::from("."),
            anvil::git_manager::Uncloned::TestFixture,
        ),
    }
}

fn hunk(path: &str, sign: char, body: &str) -> String {
    format!("diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n{sign}{body}\n")
}

const UNBOUNDED: &str = "let (tx, rx) = tokio::sync::mpsc::unbounded_channel();";
const ROUTE: &str = r#"route("/orders", post(create_order));"#;

// ---------------------------------------------------------------- the parser

#[test]
fn a_hunk_with_no_header_is_attributed_to_nothing() {
    // The old block guessed a path from the last token of a chunk's first
    // line, and on ordinary code that produced `registry.rs_lookup("a.rs");` --
    // a finding filed against a path invented out of the code it was reading.
    let files = diffs_by_path("+ let handler = registry.rs_lookup(\"a.rs\");\n");
    assert!(
        files.is_empty(),
        "a hunk that names no file must yield no file: {:?}",
        files.iter().map(|f| &f.path).collect::<Vec<_>>()
    );
}

#[test]
fn additions_context_and_removals_are_kept_apart() {
    let files = diffs_by_path(
        "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n\
         @@ -1,3 +1,3 @@\n let kept = 1;\n-let gone = 2;\n+let fresh = 3;\n",
    );
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "src/a.rs");
    assert_eq!(files[0].added(), "let fresh = 3;\n");
    assert!(files[0].after_change().contains("let kept = 1;"));
    assert!(files[0].after_change().contains("let fresh = 3;"));
    assert!(
        !files[0].after_change().contains("let gone = 2;"),
        "a removed line belongs to neither corpus"
    );
}

#[test]
fn two_hunks_of_one_file_land_in_one_entry() {
    let files =
        diffs_by_path("+++ b/src/a.rs\n+one\n+++ b/src/b.rs\n+two\n+++ b/src/a.rs\n+three\n");
    assert_eq!(
        files.len(),
        2,
        "{:?}",
        files.iter().map(|f| &f.path).collect::<Vec<_>>()
    );
    let a = files.iter().find(|f| f.path == "src/a.rs").unwrap();
    assert_eq!(a.added(), "one\nthree\n");
}

// ------------------------------------------------------------------ the gates

#[test]
fn removing_an_unbounded_channel_is_not_adding_one() {
    let g = ConstantWorkGuard::new();
    let added = g
        .evaluate_constant_work(
            Path::new("."),
            &ctx(&hunk("src/q.rs", '+', UNBOUNDED), &["src/q.rs"]),
        )
        .unwrap();
    assert!(!added.is_bounded, "the red half must still fire");
    assert_eq!(added.unbounded_findings[0].file_path, "src/q.rs");

    let removed = g
        .evaluate_constant_work(
            Path::new("."),
            &ctx(&hunk("src/q.rs", '-', UNBOUNDED), &["src/q.rs"]),
        )
        .unwrap();
    assert!(
        removed.is_bounded,
        "the change that REMOVES the unbounded channel was refused for it: {:?}",
        removed.unbounded_findings
    );
}

#[test]
fn removing_a_mutating_route_is_not_declaring_one() {
    let g = IdempotencyGuard::new();
    let added = g
        .evaluate_idempotency(
            Path::new("."),
            &ctx(&hunk("src/api.rs", '+', ROUTE), &["src/api.rs"]),
        )
        .unwrap();
    assert!(!added.is_idempotent, "the red half must still fire");
    assert_eq!(added.findings[0].file_path, "src/api.rs");
    assert_eq!(added.findings[0].endpoint, "/orders");

    let removed = g
        .evaluate_idempotency(
            Path::new("."),
            &ctx(&hunk("src/api.rs", '-', ROUTE), &["src/api.rs"]),
        )
        .unwrap();
    assert!(
        removed.is_idempotent,
        "deleting a mutating endpoint was reported as declaring one: {:?}",
        removed.findings
    );
}

#[test]
fn a_key_already_in_the_file_still_excuses_an_added_route() {
    // The reason `all` exists. Judging the key over added lines alone would
    // flag a file that already handles it, because context is not an addition
    // -- trading one false verdict for another.
    let diff = format!(
        "diff --git a/src/api.rs b/src/api.rs\n--- a/src/api.rs\n+++ b/src/api.rs\n\
         @@ -1,2 +1,3 @@\n let key = headers.get(\"Idempotency-Key\");\n+{ROUTE}\n"
    );
    let report = IdempotencyGuard::new()
        .evaluate_idempotency(Path::new("."), &ctx(&diff, &["src/api.rs"]))
        .unwrap();
    assert!(
        report.is_idempotent,
        "the key is present in the file as context: {:?}",
        report.findings
    );
}

#[test]
fn no_gate_files_a_finding_against_a_path_it_invented() {
    let invented =
        "+ let handler = registry.rs_lookup(\"a.rs\");\n+ ".to_string() + UNBOUNDED + "\n";
    let c = ctx(&invented, &["src/x.rs"]);
    assert!(
        ConstantWorkGuard::new()
            .evaluate_constant_work(Path::new("."), &c)
            .unwrap()
            .is_bounded
    );
    assert!(
        IdempotencyGuard::new()
            .evaluate_idempotency(Path::new("."), &c)
            .unwrap()
            .is_idempotent
    );
    assert!(
        FinOpsUnitCostRatchet::new()
            .evaluate_unit_cost(Path::new("."), &c)
            .unwrap()
            .findings
            .is_empty()
    );
}

#[test]
fn both_header_spellings_state_the_path_and_neither_guesses_it() {
    // `diff --git` is a header too, and taking the path from it is not the
    // defect that was fixed. The defect was reading the last whitespace token
    // of whatever line happened to be first, header or not.
    let from_git_line = diffs_by_path("diff --git a/src/a.rs b/src/a.rs\n+x\n");
    assert_eq!(from_git_line[0].path, "src/a.rs");

    let from_plus = diffs_by_path("+++ b/src/a.rs\n+x\n");
    assert_eq!(from_plus[0].path, "src/a.rs");

    // Split on ` b/`, not on whitespace, so a path with a space survives.
    let spaced = diffs_by_path("diff --git a/src/my file.rs b/src/my file.rs\n+x\n");
    assert_eq!(spaced[0].path, "src/my file.rs");
}
