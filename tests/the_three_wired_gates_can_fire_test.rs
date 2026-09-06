//! Each of the three newly-wired gates, shown to flag and shown to spare.
//!
//! `cloud_native_guard` and `stack_whitelist_guard`
//! were written, tested and invoked by nothing: no `GateStatus` field, no row in
//! `GATE_LABELS`, no call in the certify pipeline. `STAGES_WITHOUT_A_CALLER`
//! counted them, and nothing else in the tree did.
//!
//! A gate entering the corpus arrives with its proof. Both halves are required:
//! a gate with only the flagging half cannot be shown to discriminate, and one
//! with only the sparing half has never been seen to work. These six fixtures
//! are what `gate_proof::GATE_PROOFS` cites for the three.

use anvil::git_manager::{PrDiffContext, SubjectRoot, Uncloned};

fn ctx(dir: &std::path::Path, diff: &str, changed: &[&str]) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        previous_head_sha: None,
        repo_working_dir: SubjectRoot::asserted(dir.to_path_buf(), Uncloned::TestFixture),
        diff_content: diff.to_string(),
        changed_files: changed.iter().map(|s| s.to_string()).collect(),
        is_incremental: false,
    }
}

fn tree() -> tempfile::TempDir {
    tempfile::tempdir().expect("a private temporary directory")
}

// ---------------------------------------------------------------------------
// cloud_native_status
// ---------------------------------------------------------------------------

/// A core layer reaching a proprietary cloud SDK. The rule is about the layer,
/// so the same import outside `core/` is the sparing twin below.
#[test]
fn cloud_native_flags_a_proprietary_sdk_in_a_core_layer() {
    let dir = tree();
    let diff = "diff --git a/src/core/store.rs b/src/core/store.rs\n\
                --- a/src/core/store.rs\n\
                +++ b/src/core/store.rs\n\
                @@ -1,2 +1,3 @@\n\
                +use aws_sdk_s3::Client;\n";
    let report = anvil::cloud_native_guard::CloudNativeGuard::new()
        .evaluate_cloud_native(dir.path(), &ctx(dir.path(), diff, &["src/core/store.rs"]))
        .expect("the guard reads this diff");
    assert!(
        !report.is_compliant,
        "a core layer that imports aws_sdk_s3 is the defect this gate exists to \
         catch: {report:?}"
    );
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.category == "PROPRIETARY_CLOUD_SDK_IN_CORE"),
        "and it must be named as that, not as something else: {:?}",
        report.violations
    );
}

/// The same import in an adapter is the point of having layers.
#[test]
fn cloud_native_spares_the_same_sdk_in_an_adapter() {
    let dir = tree();
    let diff = "diff --git a/src/adapters/s3.rs b/src/adapters/s3.rs\n\
                --- a/src/adapters/s3.rs\n\
                +++ b/src/adapters/s3.rs\n\
                @@ -1,2 +1,3 @@\n\
                +use aws_sdk_s3::Client;\n";
    let report = anvil::cloud_native_guard::CloudNativeGuard::new()
        .evaluate_cloud_native(dir.path(), &ctx(dir.path(), diff, &["src/adapters/s3.rs"]))
        .expect("the guard reads this diff");
    assert!(
        report.is_compliant,
        "an adapter is where a vendor SDK belongs; flagging it here would make \
         the gate unsatisfiable: {:?}",
        report.violations
    );
}

// ---------------------------------------------------------------------------
// stack_whitelist_status
// ---------------------------------------------------------------------------

#[test]
fn stack_whitelist_flags_a_technology_the_approved_list_does_not_name() {
    let dir = tree();
    let diff = "diff --git a/src/db.rs b/src/db.rs\n\
                --- a/src/db.rs\n\
                +++ b/src/db.rs\n\
                @@ -1,2 +1,3 @@\n\
                +use mongodb::Client;\n";
    let report = anvil::stack_whitelist_guard::StackWhitelistGuard::new()
        .evaluate_stack_whitelist(dir.path(), &ctx(dir.path(), diff, &["src/db.rs"]), false)
        .expect("the guard reads this diff");
    assert!(
        !report.is_compliant,
        "ADR-0709 mandates PostgreSQL; adding MongoDB is the defect: {report:?}"
    );
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.category == "UNAPPROVED_STACK_TECHNOLOGY"),
        "{:?}",
        report.violations
    );
}

/// The approved stack itself must pass, or the gate refuses every change.
#[test]
fn stack_whitelist_spares_the_approved_stack() {
    let dir = tree();
    let diff = "diff --git a/src/db.rs b/src/db.rs\n\
                --- a/src/db.rs\n\
                +++ b/src/db.rs\n\
                @@ -1,2 +1,3 @@\n\
                +use sqlx::postgres::PgPool;\n";
    let report = anvil::stack_whitelist_guard::StackWhitelistGuard::new()
        .evaluate_stack_whitelist(dir.path(), &ctx(dir.path(), diff, &["src/db.rs"]), false)
        .expect("the guard reads this diff");
    assert!(
        report.is_compliant,
        "PostgreSQL through sqlx is the mandated stack: {:?}",
        report.violations
    );
}

// ---------------------------------------------------------------------------
