//! Inert complete diffs through the actual GitOps report/status consumer.

use std::path::{Path, PathBuf};

use anvil::git_manager::{PrDiffContext, SubjectRoot, Uncloned};
use anvil::gitops_drift_reconciler::GitOpsDriftReconciler;
use anvil::pre_merge_guard::GateStatus;

fn context(diff: &str, paths: &[&str]) -> PrDiffContext {
    PrDiffContext {
        repo: "fixture/change-kind".into(),
        pr_number: 1,
        base_branch: "dev".into(),
        base_sha: "base".into(),
        head_sha: "head".into(),
        is_incremental: false,
        previous_head_sha: None,
        diff_content: diff.into(),
        changed_files: paths.iter().map(|path| (*path).into()).collect(),
        repo_working_dir: SubjectRoot::asserted(PathBuf::from("."), Uncloned::TestFixture),
    }
}

#[test]
fn a_normal_header_deleted_manifest_is_an_actual_warning() {
    let path = "gitops/team-applicationset.yaml";
    let diff = format!(
        "diff --git a/{path} b/{path}\n\
        deleted file mode 100644\n--- a/{path}\n+++ /dev/null\n\
        @@ -1 +0,0 @@\n-kind: ApplicationSet\n"
    );
    let report = GitOpsDriftReconciler::new()
        .evaluate_gitops_drift(Path::new("."), &context(&diff, &[path]))
        .expect("complete deletion is observable");
    assert_eq!(
        report.orphan_findings.len(),
        1,
        "normal headers do not prove survival"
    );
    assert_eq!(report.orphan_findings[0].file_path, path);
    assert!(!report.is_safe);
    assert!(matches!(report.status, GateStatus::Warning(_)));
}

#[test]
fn additions_renames_and_modified_manifests_are_not_deletions() {
    let path = "gitops/team-applicationset.yaml";
    for diff in [
        format!(
            "diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1 +1 @@\n-old\n+new\ndiff --git a/{path} b/gitops/copy-applicationset.yaml\ncopy from {path}\ncopy to gitops/copy-applicationset.yaml\n"
        ),
        format!("diff --git a/{path} b/{path}\nnew file mode 100644\n"),
        format!(
            "diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1 +1 @@\n-old\n+new\n"
        ),
        format!(
            "diff --git a/{path} b/gitops/moved.yaml\nrename from {path}\nrename to gitops/moved.yaml\n"
        ),
        format!("diff --git a/old.yaml b/{path}\ncopy from old.yaml\ncopy to {path}\n"),
    ] {
        let report = GitOpsDriftReconciler::new()
            .evaluate_gitops_drift(Path::new("."), &context(&diff, &[path]))
            .unwrap();
        assert!(report.orphan_findings.is_empty(), "{diff}");
        assert!(report.is_safe);
        assert!(matches!(report.status, GateStatus::Passed));
    }
}

#[test]
fn only_the_definitely_deleted_scoped_manifest_is_accused() {
    let removed = "gitops/removed-applicationset.yaml";
    let kept = "gitops/kept-applicationset.yaml";
    let diff = format!(
        "diff --git a/old.txt b/old.txt\ndeleted file mode 100644\n\
        diff --git a/{removed} b/{removed}\ndeleted file mode 100644\n\
        diff --git a/{kept} b/{kept}\n--- a/{kept}\n+++ b/{kept}\n@@ -1 +1 @@\n-old\n+new\n"
    );
    let report = GitOpsDriftReconciler::new()
        .evaluate_gitops_drift(Path::new("."), &context(&diff, &["old.txt", removed, kept]))
        .unwrap();
    assert_eq!(report.orphan_findings.len(), 1);
    assert_eq!(report.orphan_findings[0].file_path, removed);
    assert!(matches!(report.status, GateStatus::Warning(_)));
}

#[test]
fn unrelated_deletion_does_not_accuse_an_edited_manifest() {
    let path = "gitops/team-applicationset.yaml";
    let diff = format!(
        "diff --git a/old.txt b/old.txt\ndeleted file mode 100644\n\
        diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1 +1 @@\n-old\n+new\n"
    );
    let report = GitOpsDriftReconciler::new()
        .evaluate_gitops_drift(Path::new("."), &context(&diff, &["old.txt", path]))
        .unwrap();
    assert!(report.orphan_findings.is_empty());
    assert!(matches!(report.status, GateStatus::Passed));
}

#[test]
fn relevant_unknown_is_error_but_absent_scope_remains_not_measured() {
    for diff in [
        "",
        "+++ b/gitops/application.yaml\n+text\n",
        "diff --git a/gitops/application.yaml b/gitops/application.yaml\nnew file mode 100644\ndeleted file mode 100644\n",
        "diff --git a/gitops/application.yaml b/gitops/application.yaml\nnew file mode 100644\ndiff --git a/gitops/application.yaml b/gitops/application.yaml\ndeleted file mode 100644\n",
    ] {
        assert!(
            GitOpsDriftReconciler::new()
                .evaluate_gitops_drift(Path::new("."), &context(diff, &["gitops/application.yaml"]))
                .is_err()
        );
    }
    let report = GitOpsDriftReconciler::new()
        .evaluate_gitops_drift(Path::new("."), &context("", &["ordinary.txt"]))
        .unwrap();
    assert!(report.orphan_findings.is_empty());
    assert!(!report.is_safe);
    assert!(matches!(report.status, GateStatus::NotMeasured { .. }));
}
