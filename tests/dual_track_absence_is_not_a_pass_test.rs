//! A repository with no second build track has not passed anything.
//!
//! The guard returned `is_synchronized: true` for a repository with no `BUCK`
//! and no `reindeer.toml`, because the drift check that could have lowered the
//! flag only ran when a Buck2 track already existed. The pass was never
//! decided; it was the initial value of a `bool` nobody reassigned.
//!
//! Anvil itself is such a repository, so the gate that exists to notice a
//! missing hermetic build track was green on the tree that has none. The one
//! test the guard shipped with wrote a `BUCK` file into its fixture first, so
//! the vacuous arm was never executed by anything.

use anvil::dual_track_build_guard::{DualTrackBuildGuard, DualTrackVerdict};
use anvil::git_manager::PrDiffContext;
use std::path::Path;
use tempfile::tempdir;

fn ctx(dir: &Path, changed: &[&str]) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: String::new(),
        changed_files: changed.iter().map(|s| s.to_string()).collect(),
        repo_working_dir: dir.to_path_buf(),
        is_incremental: false,
        previous_head_sha: None,
    }
}

fn verdict_for(
    files: &[(&str, &str)],
    changed: &[&str],
) -> anvil::dual_track_build_guard::DualTrackBuildReport {
    let dir = tempdir().expect("tempdir");
    for (name, body) in files {
        std::fs::write(dir.path().join(name), body).expect("write fixture");
    }
    DualTrackBuildGuard::new()
        .evaluate_dual_track_build(dir.path(), &ctx(dir.path(), changed))
        .expect("guard runs")
}

#[test]
fn a_repository_with_no_buck2_track_is_not_synchronized() {
    let r = verdict_for(&[("Cargo.toml", "[workspace]\n")], &["Cargo.toml"]);
    assert_eq!(r.verdict, DualTrackVerdict::NoBuck2Track);
    assert!(
        !r.is_synchronized,
        "nothing was compared, so nothing agreed. This assertion is the whole \
         defect: `is_synchronized` was true here."
    );
    assert!(!r.verdict.is_pass());
    assert!(!r.verdict.measured());
}

#[test]
fn the_summary_for_an_absent_track_does_not_say_passed() {
    let r = verdict_for(&[("Cargo.toml", "[workspace]\n")], &[]);
    assert!(
        !r.summary.contains("PASSED"),
        "the previous summary carried the word PASSED beside `ready = false`, \
         so a reader skimming verdict lines could not tell an absence from a \
         measurement. Got: {}",
        r.summary
    );
    assert!(r.summary.contains("NOT MEASURED"), "got: {}", r.summary);
}

#[test]
fn the_absent_capability_is_named_so_it_can_be_provisioned() {
    let r = verdict_for(&[("Cargo.toml", "[workspace]\n")], &[]);
    let cap = r
        .verdict
        .missing_capability()
        .expect("names what is missing");
    assert!(
        cap.contains("buck2"),
        "an absence that does not name what is absent cannot be acted on. Got: {cap}"
    );
}

#[test]
fn no_cargo_workspace_is_its_own_answer_not_a_buck2_finding() {
    let r = verdict_for(&[("BUCK", "# root\n")], &[]);
    assert_eq!(r.verdict, DualTrackVerdict::NoCargoTrack);
}

#[test]
fn both_tracks_present_and_moved_together_is_a_real_pass() {
    let r = verdict_for(
        &[("Cargo.toml", "[workspace]\n"), ("BUCK", "# root\n")],
        &["Cargo.toml", "BUCK"],
    );
    assert_eq!(r.verdict, DualTrackVerdict::Synchronized);
    assert!(r.verdict.is_pass() && r.verdict.measured());
}

#[test]
fn both_tracks_present_and_only_cargo_moved_is_drift() {
    let r = verdict_for(
        &[("Cargo.toml", "[workspace]\n"), ("BUCK", "# root\n")],
        &["Cargo.toml"],
    );
    assert!(matches!(r.verdict, DualTrackVerdict::Drifted { .. }));
    assert!(!r.verdict.is_pass());
    assert!(
        r.verdict.measured(),
        "drift IS a measurement -- it must not be confused with an absence"
    );
}

#[test]
fn anvils_own_tree_reports_the_missing_track_rather_than_a_pass() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let r = DualTrackBuildGuard::new()
        .evaluate_dual_track_build(repo, &ctx(repo, &["Cargo.toml"]))
        .expect("guard runs on anvil");
    assert_eq!(
        r.verdict,
        DualTrackVerdict::NoBuck2Track,
        "anvil has no BUCK and no reindeer.toml. When it gains one this test \
         must be updated by the change that provisions it -- which is the \
         point: the absence is now visible instead of green."
    );
}
