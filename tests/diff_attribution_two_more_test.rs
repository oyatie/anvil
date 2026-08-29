//! Two more instances of the attribution defect, found by the ratchet.
//!
//! Neither was found by reading; both were found by the standing rule added
//! after the first two were fixed. That is the argument for the rule: the
//! class had four members and a careful manual census found two.

use anvil::git_manager::PrDiffContext;
use anvil::gitops_drift_reconciler::orphan_sweeper::OrphanSweeper;
use anvil::monorepo_guard::MonorepoGuard;
use std::path::Path;

fn ctx(diff: &str, files: &[&str]) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 9,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: diff.to_string(),
        changed_files: files.iter().map(|s| s.to_string()).collect(),
        repo_working_dir: std::path::PathBuf::from("."),
        is_incremental: false,
        previous_head_sha: None,
    }
}

/// One unrelated file deleted; one ApplicationSet manifest merely edited.
///
/// `scan_orphan_risk` asked whether the WHOLE diff contained "deleted file",
/// then attributed that answer to whichever manifest it was looping over. The
/// manifest here is not deleted at all.
#[test]
fn a_manifest_that_was_not_deleted_is_not_reported_as_deleted() {
    let diff = "\
diff --git a/scripts/old_helper.sh b/scripts/old_helper.sh
deleted file mode 100755
--- a/scripts/old_helper.sh
+++ /dev/null
@@ -1,1 +0,0 @@
-echo hi
diff --git a/gitops/apps/team-applicationset.yaml b/gitops/apps/team-applicationset.yaml
--- a/gitops/apps/team-applicationset.yaml
+++ b/gitops/apps/team-applicationset.yaml
@@ -3,0 +4,1 @@
+  description: nudge
";
    let findings = OrphanSweeper::new().scan_orphan_risk(
        &[
            "scripts/old_helper.sh".to_string(),
            "gitops/apps/team-applicationset.yaml".to_string(),
        ],
        diff,
    );
    // The path must actually satisfy `is_gitops_manifest`, which matches the
    // literal fragment `applicationset`. A first draft used `team-appset.yaml`
    // and passed without ever entering the loop body -- a vacuous green for
    // the defect it was written to catch, which is the class this whole day of
    // work is about.
    assert!(
        OrphanSweeper::is_gitops_manifest("gitops/apps/team-applicationset.yaml"),
        "fixture must exercise the loop body it is testing"
    );
    assert!(
        findings.is_empty(),
        "the ApplicationSet was edited, not deleted. The deletion is in an \
         unrelated shell script. Reporting it against the manifest sends the \
         author to add a finalizer to a file nothing is removing. Got: {findings:#?}"
    );
}

/// One file claims canonical authority; another, innocent, is also touched.
///
/// The callee's parameter is named `file_content`. The caller passed
/// `diff_ctx.diff_content` -- the whole diff -- so the claim in one file was
/// attributed to every non-canonical path in the change.
#[tokio::test]
async fn an_authority_claim_in_one_file_does_not_accuse_another() {
    let diff = "\
diff --git a/notes/design.md b/notes/design.md
--- a/notes/design.md
+++ b/notes/design.md
@@ -1,0 +1,1 @@
+canonical_authority: true
diff --git a/src/unrelated.rs b/src/unrelated.rs
--- a/src/unrelated.rs
+++ b/src/unrelated.rs
@@ -1,0 +1,1 @@
+pub fn helper() {}
";
    let report = MonorepoGuard::new()
        .evaluate_monorepo_hygiene(
            Path::new("."),
            &ctx(diff, &["notes/design.md", "src/unrelated.rs"]),
        )
        .await
        .expect("guard runs");
    let accused: Vec<&String> = report
        .violations
        .iter()
        .filter(|v| v.category == "UNAUTHORIZED_AUTHORITY_CLAIM")
        .map(|v| &v.snippet)
        .collect();
    assert!(
        !accused.iter().any(|s| s.contains("unrelated.rs")),
        "src/unrelated.rs adds a function and claims nothing. Got: {accused:?}"
    );
    assert_eq!(
        accused.len(),
        1,
        "exactly one file made the claim. Got: {accused:?}"
    );
}
