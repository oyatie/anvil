//! Inert complete diffs through the actual cloud report/status consumer.

use std::path::{Path, PathBuf};

use anvil::cloud_native_guard::CloudNativeGuard;
use anvil::git_manager::{PrDiffContext, SubjectRoot, Uncloned};
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
fn deleting_a_python_script_is_not_adding_prohibited_tooling() {
    let diff = "diff --git a/scripts/old.py b/scripts/old.py\n\
        deleted file mode 100644\n--- a/scripts/old.py\n+++ /dev/null\n\
        @@ -1 +0,0 @@\n-print('old')\n";
    let report = CloudNativeGuard::new()
        .evaluate_cloud_native(Path::new("."), &context(diff, &["scripts/old.py"]))
        .expect("complete deletion is observable");
    assert!(report.violations.is_empty(), "deletion is not new tooling");
    assert!(report.is_compliant);
    assert!(matches!(report.gate_status(), GateStatus::Passed));
}

#[test]
fn new_tooling_policy_preserves_actual_change_kinds() {
    let cases = [
        (
            "scripts/old.py",
            "diff --git a/scripts/old.py b/scripts/old.py\n--- a/scripts/old.py\n+++ b/scripts/old.py\n@@ -1 +1 @@\n-old\n+new\ndiff --git a/scripts/old.py b/tools/new.py\ncopy from scripts/old.py\ncopy to tools/new.py\n",
            true,
        ),
        (
            "scripts/new.py",
            "diff --git a/scripts/new.py b/scripts/new.py\nnew file mode 100644\n",
            true,
        ),
        (
            "scripts/old.py",
            "diff --git a/scripts/old.py b/scripts/old.py\n--- a/scripts/old.py\n+++ b/scripts/old.py\n@@ -1 +1 @@\n-old\n+new\n",
            false,
        ),
        (
            "tools/new.py",
            "diff --git a/source/old.py b/tools/new.py\nrename from source/old.py\nrename to tools/new.py\n",
            true,
        ),
        (
            "tools/new.py",
            "diff --git a/scripts/old.py b/tools/new.py\nrename from scripts/old.py\nrename to tools/new.py\n",
            false,
        ),
        (
            "tools/new.py",
            "diff --git a/scripts/old.py b/tools/new.py\ncopy from scripts/old.py\ncopy to tools/new.py\n",
            true,
        ),
        (
            "tools/new.py",
            "diff --git a/source/old.py b/tools/new.py\ncopy from source/old.py\ncopy to tools/new.py\n",
            true,
        ),
        (
            "scripts/old.py",
            "diff --git a/scripts/old.py b/source/old.py\nrename from scripts/old.py\nrename to source/old.py\n",
            false,
        ),
    ];
    for (path, diff, introduced) in cases {
        let report = CloudNativeGuard::new()
            .evaluate_cloud_native(Path::new("."), &context(diff, &[path]))
            .unwrap();
        assert_eq!(report.violations.len(), usize::from(introduced), "{diff}");
        assert_eq!(
            matches!(report.gate_status(), GateStatus::Failed(_)),
            introduced
        );
        if introduced {
            assert_eq!(report.violations[0].category, "NON_RUST_SCRIPT_TOOLING");
        }
    }
}

#[test]
fn changed_names_cannot_replace_missing_or_contradictory_observation() {
    for diff in [
        "",
        "+++ b/scripts/old.py\n+text\n",
        "diff --git a/scripts/old.py b/scripts/old.py\nnew file mode 100644\ndeleted file mode 100644\n",
        "diff --git a/scripts/old.py b/scripts/old.py\nnew file mode 100644\ndiff --git a/scripts/old.py b/scripts/old.py\ndeleted file mode 100644\n",
        "diff --git \"a/scripts/old.py\" \"b/scripts/old.py\"\nnew file mode 100644\n",
    ] {
        assert!(
            CloudNativeGuard::new()
                .evaluate_cloud_native(Path::new("."), &context(diff, &["scripts/old.py"]))
                .is_err(),
            "{diff}"
        );
    }
}

#[test]
fn a_real_addition_is_not_hidden_by_an_unrelated_deletion_or_name_list() {
    let diff = "diff --git a/scripts/old.py b/scripts/old.py\ndeleted file mode 100644\n\
        diff --git a/tools/new.py b/tools/new.py\nnew file mode 100644\n";
    let report = CloudNativeGuard::new()
        .evaluate_cloud_native(Path::new("."), &context(diff, &["scripts/old.py"]))
        .unwrap();
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].snippet, "tools/new.py");
    assert!(matches!(report.gate_status(), GateStatus::Failed(_)));
}
