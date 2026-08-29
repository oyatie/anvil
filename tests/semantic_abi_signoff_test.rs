//! A signed-off signature change admits; an unsigned one still refuses.
//!
//! Before this the gate had no path for a deliberate API change: no allowlist,
//! no waiver, no baseline. The only ways past a considered change were to
//! revert it or leave the gate red, so the gate's own correctness pushed
//! authors toward the first.
//!
//! The signoff records the decision instead of hiding it, and it is the same
//! vocabulary the shape ratchet already uses -- `ratchet::core::signoff` --
//! rather than a second one invented here.

use anvil::git_manager::PrDiffContext;
use anvil::pre_merge_guard::report::GateStatus;
use anvil::semantic_abi_ratchet::{ABI_SIGNOFF_PATH, SemanticAbiRatchet};
use std::path::Path;

/// A diff that changes one public signature: `sweep_repo` gains a return type.
fn breaking_diff() -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "dev".to_string(),
        base_sha: String::new(),
        head_sha: String::new(),
        is_incremental: false,
        previous_head_sha: None,
        diff_content: "diff --git a/src/shape/facade/sweep.rs b/src/shape/facade/sweep.rs\n\
             --- a/src/shape/facade/sweep.rs\n\
             +++ b/src/shape/facade/sweep.rs\n\
             @@ -1 +1 @@\n\
             -pub fn sweep_repo(dir: &str) -> ShapeReport {}\n\
             +pub fn sweep_repo(dir: &str, rev: &str) -> Swept {}\n"
            .to_string(),
        changed_files: vec!["src/shape/facade/sweep.rs".to_string()],
        repo_working_dir: std::path::PathBuf::new(),
    }
}

fn signoff_dir(keys: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temp dir");
    let path = dir.path().join(ABI_SIGNOFF_PATH);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
    std::fs::write(
        &path,
        format!(
            r#"{{"schema":"anvil/ratchet-signoff/v1",
                 "_sign_off_additions":{{"semantic_abi_status":[{keys}]}},
                 "signings":[{{"by":"t","date":"2026-08-28","note":"n"}}]}}"#
        ),
    )
    .expect("write signoff");
    dir
}

#[test]
fn an_unsigned_signature_change_still_fails() {
    let dir = signoff_dir("");
    let report = SemanticAbiRatchet::new()
        .evaluate_abi_stability(dir.path(), &breaking_diff())
        .expect("the scan runs");
    assert!(
        matches!(report.status, GateStatus::Failed(_)),
        "an unsigned public signature change must refuse; got {:?}",
        report.status
    );
}

#[test]
fn the_signed_off_change_admits() {
    let dir = signoff_dir(r#""sweep_repo@src/shape/facade/sweep.rs""#);
    let report = SemanticAbiRatchet::new()
        .evaluate_abi_stability(dir.path(), &breaking_diff())
        .expect("the scan runs");
    assert!(
        !matches!(report.status, GateStatus::Failed(_)),
        "a signed-off change must not refuse; got {:?}",
        report.status
    );
}

/// A signoff covers the key it names and nothing else.
#[test]
fn a_signoff_for_another_symbol_does_not_cover_this_one() {
    let dir = signoff_dir(r#""something_else@src/other.rs""#);
    let report = SemanticAbiRatchet::new()
        .evaluate_abi_stability(dir.path(), &breaking_diff())
        .expect("the scan runs");
    assert!(
        matches!(report.status, GateStatus::Failed(_)),
        "a signoff naming a different symbol must not admit this one; got {:?}",
        report.status
    );
}

/// The repository's own signoff parses and names both changes this PR makes.
#[test]
fn the_committed_signoff_is_valid_and_names_both_changes() {
    let body = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(ABI_SIGNOFF_PATH))
        .expect("the committed signoff exists");
    let signoff = anvil::ratchet::facade::Signoff::parse(&body).expect("it parses");
    for key in [
        "body@src/publish/mod.rs",
        "sweep_repo@src/shape/facade/sweep.rs",
    ] {
        assert!(
            signoff.covers("semantic_abi_status", key),
            "the committed signoff must name {key}"
        );
    }
}
