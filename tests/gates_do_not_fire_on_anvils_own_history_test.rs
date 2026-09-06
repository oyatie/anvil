//! A gate that accuses this repository's own history is not shippable.
//!
//! The three gates repaired alongside this file each published a claim their
//! mechanism did not support. Two of them were unfailable, so replaying history
//! through them proved nothing; the third, `adr_status`, was the opposite and
//! already fired on three of the last twenty commits:
//!
//! ```text
//! 8375ab2  ADR-0005-rust-1.97.1-edition-2024-dual-build.md  missing all five fields
//! c08ddff  docs/adr/0002-agentic-roster-and-delivery-fabric.md  missing achieves, overturn_when
//! 213aa3e  "AUTO-SCAFFOLDED docs/decisions/ADR-9015-pr-9015.md"  a file nothing wrote
//! ```
//!
//! The first two are the *whole-diff* regex firing, not the fix's file-scoped
//! check -- the fix would have found the same drift and more. That drift is
//! real: this repository runs two ADR conventions at once, a `## Schema` block
//! carrying the five fields in `docs/adr/0001` and `0003`, and MADR-style
//! Context/Decision records in `docs/decisions/ADR-0005` and `0006`. Which one
//! is the house rule is not a question a gate PR gets to settle, so the field
//! list is read from the repository (`docs/decisions/adr-schema.json`) and this
//! repository ships none. `adr_status` therefore reports `NotMeasured` here --
//! naming the file that would supply the schema -- rather than picking a side
//! and charging the loser.
//!
//! This test is a replay, not a fixture: it reads real commits with `git show`,
//! so it keeps working as history moves and cannot be satisfied by a hand-tuned
//! diff. What it forbids is an accusation -- `Failed` or `Warning`.
//! `NotMeasured` is the abstention this repository's I1 requires when a gate
//! did not look, and is not a firing.
//!
//! # Scope, and why it is not the whole tree
//!
//! The replay reads production paths and excludes `tests/`. The compliance
//! guard fired on `03058c3` -- this branch's own commit -- because a twenty
//! commit window reaches the RED fixtures this pull request adds to
//! `tests/gates_claim_only_what_they_check_test.rs`, which carry a card-shaped
//! and an identifier-shaped literal precisely so the guard has something to
//! catch. A gate accusing the corpus that proves it can fire has found nothing
//! about this repository.
//!
//! The replay also refuses to run on a shallow checkout, which is what
//! `actions/checkout` produces by default: a grafted root has no parent, so
//! `git show` emits the entire tree as one diff and `git log -20` returns one
//! commit. That is what made the `adr` and `cross_service` cases red on CI. It
//! fails rather than skips, and `.github/workflows/ci.yml` sets `fetch-depth: 0`
//! on the job that runs it -- #91 settled that shape for the same defect in
//! `tests/loose_blocking_patterns_test.rs`, and the two are kept the same.

use anvil::adr_drift_ratchet::AdrDriftRatchet;
use anvil::compliance_guard::ComplianceGuard;
use anvil::cross_service_impact::CrossServiceImpactEngine;
use anvil::git_manager::PrDiffContext;
use anvil::pre_merge_guard::report::GateStatus;
use std::path::PathBuf;
use std::process::Command;

/// Kept small enough that the replay stays under a few seconds and large enough
/// that it spans the corpus reorganisations, which are the changes most likely
/// to trip a path-shaped predicate.
const COMMITS: usize = 20;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("git is on PATH inside a git checkout");
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// Refuses to answer the claim on a checkout that does not hold the history.
///
/// `actions/checkout` clones at depth 1 unless told otherwise. A shallow
/// clone's root commit is grafted and has no parent, so `git show` on it emits
/// the entire repository as one added-lines diff, and `git log -20` returns one
/// commit rather than twenty. Replaying that is not a replay of history: it is
/// the working tree wearing a commit's name, and it is what made the `adr` and
/// `cross_service` cases red on CI.
///
/// Fail closed rather than skip, and the same shape as
/// `neither_gate_fires_on_this_repositorys_own_history` in
/// `tests/loose_blocking_patterns_test.rs`, which #91 fixed the same way:
/// `.github/workflows/ci.yml` sets `fetch-depth: 0` on the job that runs both,
/// and a test that silently passes when that line is dropped is a test that
/// stops being evidence without anyone noticing.
fn require_real_history() {
    assert_eq!(
        run(&["rev-parse", "--is-shallow-repository"]).trim(),
        "false",
        "shallow checkout: this repository's history is not present, so this \
         claim cannot be measured. Set `fetch-depth: 0` on the checkout step \
         in .github/workflows/ci.yml."
    );
}

/// The paths a replay of this repository's history may draw a conclusion from.
///
/// `tests/` is excluded, and the exclusion is the point rather than a
/// convenience. A gate's RED fixtures are the corpus that proves it can fire:
/// this pull request adds card-shaped and identifier-shaped literals to
/// `tests/gates_claim_only_what_they_check_test.rs` precisely so the compliance
/// guard has something to catch. A gate accusing its own seeded defects has
/// found nothing about this repository, and the twenty-commit window is wide
/// enough to reach the commit that adds them -- the guard fired on `03058c3`,
/// which is this branch's own commit.
///
/// What the replay is evidence about is the production tree: the code a pull
/// request touching this repository would actually be blocked over.
const PRODUCTION_PATHS: [&str; 3] = ["--", ".", ":(exclude)tests"];

fn recent_changes() -> Vec<(String, PrDiffContext)> {
    let shas: Vec<String> = run(&["log", "--format=%H", &format!("-{COMMITS}")])
        .lines()
        .map(str::to_string)
        .collect();
    assert!(
        shas.len() >= COMMITS,
        "expected {COMMITS} commits of history, found {}",
        shas.len()
    );
    shas.into_iter()
        .enumerate()
        .map(|(i, sha)| {
            let mut show: Vec<&str> = vec!["show", "--format=", "--unified=3", &sha];
            show.extend_from_slice(&PRODUCTION_PATHS);
            let diff_content = run(&show);

            let mut names: Vec<&str> = vec!["show", "--name-only", "--format=", &sha];
            names.extend_from_slice(&PRODUCTION_PATHS);
            let changed_files: Vec<String> = run(&names)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(str::to_string)
                .collect();
            let ctx = PrDiffContext {
                repo: "oyatie/anvil".to_string(),
                pr_number: 9000 + i as u64,
                base_branch: "main".to_string(),
                base_sha: format!("{sha}~1"),
                head_sha: sha.clone(),
                is_incremental: false,
                previous_head_sha: None,
                diff_content,
                changed_files,
                repo_working_dir: anvil::git_manager::SubjectRoot::asserted(
                    repo_root(),
                    anvil::git_manager::Uncloned::TestFixture,
                ),
            };
            (sha, ctx)
        })
        .collect()
}

/// An accusation, as opposed to a pass or an abstention.
fn accusation(status: &GateStatus) -> Option<String> {
    match status {
        GateStatus::Failed(m) | GateStatus::Warning(m) => Some(m.clone()),
        _ => None,
    }
}

#[test]
fn the_adr_ratchet_accuses_no_commit_in_anvils_own_history() {
    require_real_history();

    let root = repo_root();
    let ratchet = AdrDriftRatchet::new();
    let mut charged = Vec::new();
    let mut abstained = 0usize;

    for (sha, ctx) in recent_changes() {
        let report = ratchet
            .evaluate_adr_parity(&root, &ctx)
            .expect("the ratchet reads the diff");
        if let Some(msg) = accusation(&report.status) {
            charged.push(format!("{} :: {msg}", &sha[..8]));
        }
        if matches!(report.status, GateStatus::NotMeasured { .. }) {
            abstained += 1;
        }
        assert!(
            !format!("{report:?}").contains("scaffold"),
            "{} was told a file had been scaffolded for it",
            &sha[..8]
        );
    }

    assert!(
        charged.is_empty(),
        "the ADR ratchet fired on this repository's own history:\n{}",
        charged.join("\n")
    );
    assert_eq!(
        abstained, COMMITS,
        "this repository declares no ADR field schema, so every commit must be \
         an abstention rather than a silent pass"
    );
}

#[test]
fn the_compliance_guard_accuses_no_commit_in_anvils_own_history() {
    require_real_history();

    let guard = ComplianceGuard::new();
    let mut charged = Vec::new();

    for (sha, ctx) in recent_changes() {
        // A fixed date, so a rule taking effect tomorrow does not turn this
        // test red on a day nobody changed anything. The wall-clock path is
        // pinned separately in `gates_claim_only_what_they_check_test.rs`.
        let report = guard
            .evaluate_compliance_at(&ctx, "2026-08-23")
            .expect("the guard reads the diff");
        for v in &report.violations {
            charged.push(format!(
                "{} :: {} {} :: {}",
                &sha[..8],
                v.rule_id,
                v.file_path,
                v.line_snippet.chars().take(80).collect::<String>()
            ));
        }
    }

    assert!(
        charged.is_empty(),
        "the compliance guard fired on this repository's own history:\n{}",
        charged.join("\n")
    );
}

#[test]
fn the_cross_service_engine_accuses_no_commit_in_anvils_own_history() {
    require_real_history();

    let root = repo_root();
    let engine = CrossServiceImpactEngine::new();
    let mut charged = Vec::new();

    for (sha, ctx) in recent_changes() {
        let report = engine
            .evaluate_cross_service_impact(&root, &ctx)
            .expect("the engine reads the diff");
        for f in &report.breaking_findings {
            charged.push(format!(
                "{} :: {} lost required `{}`",
                &sha[..8],
                f.contract_file,
                f.removed_required_field
            ));
        }
    }

    assert!(
        charged.is_empty(),
        "the cross-service engine fired on this repository's own history:\n{}",
        charged.join("\n")
    );
}
