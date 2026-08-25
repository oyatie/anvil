//! A gate may not file a finding against a file it was handed rather than found.
//!
//! `rust_language_policy` and `compliance_guard` both seeded their file cursor
//! with `diff_ctx.changed_files.first()`. Every line before the diff's first
//! `+++ b/` header was therefore attributed to whichever file happened to be
//! first in the pull request's file list.
//!
//! This is the most credible of the misattributions found today. The other
//! gates invented paths -- `unknown.rs`, `hotpath`, `manifest.yaml` -- which at
//! least might look odd to a reviewer. This one names a REAL file that is
//! genuinely part of the change. Measured against the old code, a diff whose
//! only line was `+let v = maybe.unwrap();` produced:
//!
//! ```text
//! rust_policy idiomatic=false findings=["src/innocent.rs"]
//! ```
//!
//! A reviewer opens `src/innocent.rs`, finds no `.unwrap()`, and has nothing to
//! tell them the gate picked that name off a list.

use anvil::compliance_guard::ComplianceGuard;
use anvil::git_manager::diff_context::PrDiffContext;
use anvil::rust_language_policy::RustLanguagePolicy;
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
        repo_working_dir: PathBuf::from("."),
    }
}

const UNWRAP: &str = "let v = maybe.unwrap();";

#[test]
fn a_line_before_any_header_is_not_pinned_on_the_first_changed_file() {
    let report = RustLanguagePolicy::new()
        .evaluate_rust_quality(
            Path::new("."),
            &ctx(&format!("+{UNWRAP}\n"), &["src/innocent.rs"]),
        )
        .expect("evaluates");

    assert!(
        report.findings.is_empty(),
        "a finding was filed against a file the diff never named: {:?}",
        report
            .findings
            .iter()
            .map(|f| f.file_path.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn the_rule_still_fires_once_the_diff_names_the_file() {
    // The red half. The fix must not have been bought by making the rule inert.
    let diff = format!("--- a/src/api.rs\n+++ b/src/api.rs\n+{UNWRAP}\n");
    let report = RustLanguagePolicy::new()
        .evaluate_rust_quality(Path::new("."), &ctx(&diff, &["src/innocent.rs"]))
        .expect("evaluates");

    assert!(!report.is_idiomatic, "the unwrap must still be found");
    assert!(
        report.findings.iter().all(|f| f.file_path == "src/api.rs"),
        "every finding must name the file the diff named, not the first changed file: {:?}",
        report
            .findings
            .iter()
            .map(|f| f.file_path.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn compliance_guard_carried_the_identical_seed() {
    // Same defect, same shape, one module over. It is a test rather than a
    // comment because the two were fixed together and either could regress
    // alone.
    //
    // The payload matches KR_PIPA_RRN_STRICT_BAN, whose `trigger_extensions`
    // include `rs`. That matters: the OLD code derived `current_ext` from the
    // seeded path too, so `src/innocent.rs` supplied both the file the finding
    // was pinned on AND the extension that let the rule fire at all.
    let report = ComplianceGuard::new()
        .evaluate_compliance(&ctx(
            "+let rrn = \"900101-1234567\";\n",
            &["src/innocent.rs"],
        ))
        .expect("evaluates");

    assert!(
        report
            .violations
            .iter()
            .all(|v| v.file_path != "src/innocent.rs"),
        "a violation was pinned on the first changed file: {:?}",
        report
            .violations
            .iter()
            .map(|v| v.file_path.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn compliance_guard_still_fires_when_the_diff_names_the_file() {
    let report = ComplianceGuard::new()
        .evaluate_compliance(&ctx(
            "--- a/src/user.rs\n+++ b/src/user.rs\n+let rrn = \"900101-1234567\";\n",
            &["src/innocent.rs"],
        ))
        .expect("evaluates");

    let hit = report
        .violations
        .iter()
        .find(|v| v.file_path == "src/user.rs")
        .unwrap_or_else(|| {
            panic!(
                "the rule must still fire, against the named file: {:?}",
                report
                    .violations
                    .iter()
                    .map(|v| v.file_path.clone())
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(hit.file_path, "src/user.rs");
}
