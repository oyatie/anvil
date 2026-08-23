//! Four gates scoped their scan to a hardcoded set of path fragments, then
//! published `findings.is_empty()` as a pass.
//!
//! # The defect
//!
//! `debt_shrink_status`, `gitops_drift_status`, `migration_orch_status` and
//! `ghost_migration_status` each decide what to look at by testing a changed
//! path against a fixed list of substrings -- `deprecated`/`legacy`/`/old/`,
//! `applicationset`/`application.yaml`, `.sql`, `migration`/`.sql`. None of
//! those lists matches a single tracked file in this repository:
//!
//! ```text
//! $ git ls-files | grep -icE 'deprecated|legacy|/old/'            0
//! $ git ls-files | grep -icE 'applicationset|application\.yaml'   0
//! $ git ls-files | grep -icE '\.sql$'                             0
//! ```
//!
//! So nothing was ever in scope, `is_empty()` was vacuously true, and each
//! gate published a green: "zero prohibited expansions on deprecating
//! targets", "all GitOps ApplicationSets maintain declarative integrity", "all
//! database schema transitions adhere to Expand-Contract lifecycle
//! invariants", "zero exclusive locks or table rewrites". Every one of those
//! sentences is about a corpus the gate never had.
//!
//! # Why this is `NotMeasured` and coverage's empty diff is not
//!
//! `src/trace_context_guard/mod.rs` states the repository's rule: coverage
//! maps `NothingToMeasure` -- there was nothing to look at -- to `Passed`, and
//! a thing that was there and could not be judged to `NotMeasured`.
//!
//! Coverage's scope is "the coverable lines this diff adds", and that set is
//! read exhaustively off the diff the gate is holding. When it is empty, the
//! gate has *observed* that there was nothing to look at.
//!
//! These four observe something weaker and publish something stronger. What
//! they measure is "no changed path contained this substring"; what they
//! publish is "the property holds". Those are different propositions, and the
//! substring is a guess about spelling: deprecation is a decision recorded in
//! a drain ledger (`governance/REORG-DRAIN.md`, absent from this repository),
//! an ArgoCD `Application` need not be filed under `application.yaml`, and a
//! schema migration arrives as `db/migrate/*.rb` or `schema.rb` at least as
//! often as `*.sql`. An empty result from a predicate that is not exhaustive
//! over the diff is not "nothing to look at"; it is "did not look", which is
//! invariant I1's absent evidence.
//!
//! # Why each gate needs three tests, not one
//!
//! A gate rewritten to report `NotMeasured` unconditionally would satisfy the
//! empty-scope test while measuring exactly as little as before. Each gate
//! below is therefore pinned from both ends: an empty scope must not pass, and
//! an in-scope file must still be scanned and judged -- clean to `Passed`,
//! dirty to a verdict that is neither `Passed` nor `NotMeasured`.

use anvil::debt_shrink_guard::DebtShrinkGuard;
use anvil::ghost_migration_harness::GhostMigrationHarness;
use anvil::git_manager::PrDiffContext;
use anvil::gitops_drift_reconciler::GitOpsDriftReconciler;
use anvil::migration_orchestrator::MigrationLifecycleOrchestrator;
use anvil::pre_merge_guard::GateStatus;
use std::path::{Path, PathBuf};

fn ctx(diff: &str, changed: &[&str]) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "main".to_string(),
        base_sha: "a".to_string(),
        head_sha: "b".to_string(),
        diff_content: diff.to_string(),
        changed_files: changed.iter().map(|s| s.to_string()).collect(),
        repo_working_dir: PathBuf::from("."),
        is_incremental: false,
        previous_head_sha: None,
    }
}

/// A `NotMeasured` whose reason is blank names no scope, so a reader cannot
/// tell which corpus was missing. Every empty-scope verdict below is checked
/// for a reason that says what was not scanned.
fn assert_unmeasured(status: &GateStatus, gate_id: &str, must_mention: &[&str]) {
    let GateStatus::NotMeasured {
        gate_id: id,
        reason,
    } = status
    else {
        panic!("{gate_id}: an empty scope must not be a pass, got {status:?}");
    };
    assert_eq!(
        id, gate_id,
        "the NotMeasured verdict must name its own gate"
    );
    for needle in must_mention {
        assert!(
            reason.contains(needle),
            "{gate_id}: the reason must say what was not scanned; `{needle}` missing from {reason:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// debt_shrink_status
// ---------------------------------------------------------------------------

#[test]
fn debt_shrink_does_not_pass_a_diff_with_no_deprecating_target() {
    let rep = DebtShrinkGuard::new()
        .evaluate_debt_shrink(
            Path::new("."),
            &ctx("+++ b/src/lib.rs\n+pub fn f() {}", &["src/lib.rs"]),
        )
        .expect("evaluates");

    assert_unmeasured(&rep.status, "debt_shrink_status", &["deprecat"]);
    assert!(
        !rep.is_acceptable,
        "the boolean must not disagree with the status it was derived from"
    );
}

#[test]
fn debt_shrink_still_passes_a_shrink_on_a_deprecating_target() {
    let rep = DebtShrinkGuard::new()
        .evaluate_debt_shrink(
            Path::new("."),
            &ctx(
                "+++ b/src/legacy/router.ts\n-const dead = 1;\n-const gone = 2;",
                &["src/legacy/router.ts"],
            ),
        )
        .expect("evaluates");

    assert_eq!(
        rep.status,
        GateStatus::Passed,
        "a deprecating target that only shrank was scanned and is clean"
    );
    assert!(rep.is_acceptable);
    assert_eq!(rep.total_debt_shrunk, 2);
}

#[test]
fn debt_shrink_still_fails_growth_on_a_deprecating_target() {
    let rep = DebtShrinkGuard::new()
        .evaluate_debt_shrink(
            Path::new("."),
            &ctx(
                "+++ b/src/legacy/router.ts\n+const feature = true;\n+const more = 42;",
                &["src/legacy/router.ts"],
            ),
        )
        .expect("evaluates");

    assert!(
        matches!(rep.status, GateStatus::Failed(_)),
        "growth on a deprecating target must still block, got {:?}",
        rep.status
    );
    assert!(!rep.is_acceptable);
    assert_eq!(rep.violations.len(), 1);
}

/// A deprecating target that neither grew nor shrank was still read, and the
/// ratchet has nothing to complain about. This is the third arm of the summary
/// -- in scope, clean, and zero lines drained -- and it is the arm closest to
/// the vacuous pass, so it is the one worth pinning apart from it.
#[test]
fn debt_shrink_passes_a_deprecating_target_that_neither_grew_nor_shrank() {
    let rep = DebtShrinkGuard::new()
        .evaluate_debt_shrink(
            Path::new("."),
            &ctx(
                "+++ b/src/legacy/router.ts\n+const renamed = 1;\n-const old = 1;",
                &["src/legacy/router.ts"],
            ),
        )
        .expect("evaluates");

    assert_eq!(rep.status, GateStatus::Passed);
    assert_eq!(rep.total_debt_shrunk, 0);
    assert!(
        rep.summary.contains("zero prohibited expansions"),
        "a net-zero edit on a deprecating target is a scan, not an empty scope: {:?}",
        rep.summary
    );
}

// ---------------------------------------------------------------------------
// gitops_drift_status
// ---------------------------------------------------------------------------

#[test]
fn gitops_drift_does_not_pass_a_diff_with_no_argocd_manifest() {
    let rep = GitOpsDriftReconciler::new()
        .evaluate_gitops_drift(
            Path::new("."),
            &ctx("+replicaCount: 3", &["infra/gitops/values.yaml"]),
        )
        .expect("evaluates");

    assert_unmeasured(&rep.status, "gitops_drift_status", &["applicationset"]);
    assert!(!rep.is_safe);
}

#[test]
fn gitops_drift_still_passes_a_manifest_deleted_behind_a_finalizer() {
    let rep = GitOpsDriftReconciler::new()
        .evaluate_gitops_drift(
            Path::new("."),
            &ctx(
                "deleted file mode 100644\n-  finalizers:\n-  - resources-finalizer.argocd.argoproj.io",
                &["iac/apps/team-applicationset.yaml"],
            ),
        )
        .expect("evaluates");

    assert_eq!(
        rep.status,
        GateStatus::Passed,
        "an ApplicationSet was in scope, was read, and carries cascade protection"
    );
    assert!(rep.is_safe);
}

#[test]
fn gitops_drift_still_flags_an_unprotected_applicationset_deletion() {
    let rep = GitOpsDriftReconciler::new()
        .evaluate_gitops_drift(
            Path::new("."),
            &ctx(
                "deleted file mode 100644\n--- a/iac/apps/orphan-applicationset.yaml\n+++ /dev/null",
                &["iac/apps/orphan-applicationset.yaml"],
            ),
        )
        .expect("evaluates");

    assert!(
        matches!(rep.status, GateStatus::Warning(_)),
        "an unprotected ApplicationSet deletion must still be reported, got {:?}",
        rep.status
    );
    assert_eq!(rep.orphan_findings.len(), 1);
}

// ---------------------------------------------------------------------------
// migration_orch_status
// ---------------------------------------------------------------------------

/// The scope test was `file_diff.contains(".sql")` against the *chunk text*,
/// not the path, so a Rust file whose body mentions `.sql` entered the SQL
/// scope while a chunk with no derivable path fell back to a default of
/// `migration.sql` and entered it too. The scope is now the path parsed out of
/// the `diff --git` header, and a chunk with no such header is not in scope.
#[test]
fn migration_orch_does_not_pass_a_diff_with_no_sql_file() {
    let rep = MigrationLifecycleOrchestrator::new()
        .evaluate_migration_lifecycle(
            Path::new("."),
            &ctx(
                "diff --git a/src/db.rs b/src/db.rs\n+// loads schema.sql at boot",
                &["src/db.rs"],
            ),
        )
        .expect("evaluates");

    assert_unmeasured(&rep.status, "migration_orch_status", &[".sql"]);
    assert!(!rep.is_ordered);
}

#[test]
fn migration_orch_still_passes_an_annotated_contract_drop() {
    let rep = MigrationLifecycleOrchestrator::new()
        .evaluate_migration_lifecycle(
            Path::new("."),
            &ctx(
                "diff --git a/migrations/0002_drop.sql b/migrations/0002_drop.sql\n+-- PHASE: CONTRACT\n+ALTER TABLE users DROP COLUMN legacy_token;",
                &["migrations/0002_drop.sql"],
            ),
        )
        .expect("evaluates");

    assert_eq!(
        rep.status,
        GateStatus::Passed,
        "a SQL migration was in scope, was parsed, and declares its phase"
    );
    assert!(rep.is_ordered);
}

#[test]
fn migration_orch_still_fails_an_unannotated_drop() {
    let rep = MigrationLifecycleOrchestrator::new()
        .evaluate_migration_lifecycle(
            Path::new("."),
            &ctx(
                "diff --git a/migrations/0002_drop.sql b/migrations/0002_drop.sql\n+ALTER TABLE users DROP COLUMN legacy_token;",
                &["migrations/0002_drop.sql"],
            ),
        )
        .expect("evaluates");

    assert!(
        matches!(rep.status, GateStatus::Failed(_)),
        "an unannotated destructive drop must still block, got {:?}",
        rep.status
    );
    assert_eq!(rep.findings.len(), 1);
}

// ---------------------------------------------------------------------------
// ghost_migration_status
// ---------------------------------------------------------------------------

#[test]
fn ghost_migration_does_not_pass_a_diff_with_no_migration_file() {
    let rep = GhostMigrationHarness::new()
        .evaluate_migrations(
            Path::new("."),
            &ctx("+++ b/src/lib.rs\n+pub fn f() {}", &["src/lib.rs"]),
        )
        .expect("evaluates");

    assert_unmeasured(&rep.status, "ghost_migration_status", &["migration"]);
    assert!(!rep.is_safe);
    assert_eq!(rep.migrations_evaluated, 0);
}

/// An empty `changed_files` is not a diff that touched no migration -- it is a
/// diff whose file list the gate never received. It scopes against that list,
/// so it must not read a missing list as a clean one.
#[test]
fn ghost_migration_does_not_pass_a_diff_with_no_file_list_at_all() {
    let rep = GhostMigrationHarness::new()
        .evaluate_migrations(
            Path::new("."),
            &ctx("+++ b/src/lib.rs\n+pub fn f() {}", &[]),
        )
        .expect("evaluates");

    assert_unmeasured(&rep.status, "ghost_migration_status", &["migration"]);
    assert!(!rep.is_safe);
}

#[test]
fn ghost_migration_still_passes_a_concurrent_index() {
    let rep = GhostMigrationHarness::new()
        .evaluate_migrations(
            Path::new("."),
            &ctx(
                "+++ b/migrations/001_idx.sql\n+CREATE INDEX CONCURRENTLY idx_users ON users(tenant_id);",
                &["migrations/001_idx.sql"],
            ),
        )
        .expect("evaluates");

    assert_eq!(
        rep.status,
        GateStatus::Passed,
        "a migration was in scope, was read, and takes no exclusive lock"
    );
    assert!(rep.is_safe);
    assert_eq!(rep.migrations_evaluated, 1);
}

#[test]
fn ghost_migration_still_fails_a_non_concurrent_index() {
    let rep = GhostMigrationHarness::new()
        .evaluate_migrations(
            Path::new("."),
            &ctx(
                "+++ b/migrations/002_idx.sql\n+CREATE INDEX idx_users_email ON users(email);",
                &["migrations/002_idx.sql"],
            ),
        )
        .expect("evaluates");

    assert!(
        matches!(rep.status, GateStatus::Failed(_)),
        "a non-concurrent index still takes an exclusive lock, got {:?}",
        rep.status
    );
    assert_eq!(rep.violations[0].violation_type, "EXCLUSIVE_INDEX_LOCK");
}

// ---------------------------------------------------------------------------
// The four together
// ---------------------------------------------------------------------------

/// Rewriting all four to report `NotMeasured` unconditionally would satisfy
/// every empty-scope test above while measuring nothing at all. This pins the
/// other direction once more, in one place: on a diff that puts a file in
/// every one of the four scopes, not one of them may say it was unmeasured.
#[test]
fn a_diff_in_every_scope_leaves_no_gate_unmeasured() {
    let d = ctx(
        "diff --git a/migrations/003_x.sql b/migrations/003_x.sql\n\
         +++ b/migrations/003_x.sql\n\
         +-- PHASE: CONTRACT\n\
         +ALTER TABLE users DROP COLUMN old_token;\n\
         diff --git a/src/legacy/router.ts b/src/legacy/router.ts\n\
         +++ b/src/legacy/router.ts\n\
         -const dead = 1;\n",
        &[
            "migrations/003_x.sql",
            "src/legacy/router.ts",
            "iac/apps/team-applicationset.yaml",
        ],
    );

    let statuses = [
        (
            "debt_shrink_status",
            DebtShrinkGuard::new()
                .evaluate_debt_shrink(Path::new("."), &d)
                .expect("evaluates")
                .status,
        ),
        (
            "gitops_drift_status",
            GitOpsDriftReconciler::new()
                .evaluate_gitops_drift(Path::new("."), &d)
                .expect("evaluates")
                .status,
        ),
        (
            "migration_orch_status",
            MigrationLifecycleOrchestrator::new()
                .evaluate_migration_lifecycle(Path::new("."), &d)
                .expect("evaluates")
                .status,
        ),
        (
            "ghost_migration_status",
            GhostMigrationHarness::new()
                .evaluate_migrations(Path::new("."), &d)
                .expect("evaluates")
                .status,
        ),
    ];

    for (gate, status) in &statuses {
        assert!(
            status.is_measured(),
            "{gate} had a file in its scope and still reported no measurement: {status:?}"
        );
    }
}
