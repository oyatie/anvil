//! A finding must be attributed to the file that actually carries it.
//!
//! The guard iterated `changed_files`, and for every path containing `/core/`
//! it scanned the WHOLE diff. The path decided whether to look; the diff
//! decided what was found; nothing connected the two. So an `aws_sdk_` import
//! added in an adapter -- where this guard's own remedy text says it belongs --
//! was reported as a violation *of the core file*, by name, once per core file
//! in the change.
//!
//! That is the worse half of a false positive: it accuses conformant code, and
//! sends the author to a file that is clean. `diffs_by_path` already existed
//! and already attributes hunks to paths.

use anvil::cloud_native_guard::CloudNativeGuard;
use anvil::git_manager::PrDiffContext;
use std::path::Path;

/// An SDK import added in an adapter, alongside an untouched-by-SDK core file.
///
/// This is the arrangement the guard's own advice produces: "Use an abstract
/// Port trait in core and isolate SDK in adapters." A change that follows the
/// advice must not be flagged for following it.
fn conformant_change() -> PrDiffContext {
    let diff = "\
diff --git a/billing/core/src/invoice.rs b/billing/core/src/invoice.rs
--- a/billing/core/src/invoice.rs
+++ b/billing/core/src/invoice.rs
@@ -1,0 +1,2 @@
+pub trait InvoiceStore { fn put(&self, id: u64); }
+
diff --git a/billing/adapters/src/s3.rs b/billing/adapters/src/s3.rs
--- a/billing/adapters/src/s3.rs
+++ b/billing/adapters/src/s3.rs
@@ -1,0 +1,2 @@
+use aws_sdk_s3::Client;
+pub struct S3Store { client: Client }
";
    PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 42,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: diff.to_string(),
        changed_files: vec![
            "billing/core/src/invoice.rs".to_string(),
            "billing/adapters/src/s3.rs".to_string(),
        ],
        repo_working_dir: std::path::PathBuf::from("."),
        is_incremental: false,
        previous_head_sha: None,
    }
}

#[test]
fn an_sdk_isolated_in_an_adapter_is_not_a_core_violation() {
    let report = CloudNativeGuard::new()
        .evaluate_cloud_native(Path::new("."), &conformant_change())
        .expect("guard runs");
    let core_findings: Vec<_> = report
        .violations
        .iter()
        .filter(|v| v.category == "PROPRIETARY_CLOUD_SDK_IN_CORE")
        .collect();
    assert!(
        core_findings.is_empty(),
        "the SDK was added in billing/adapters/src/s3.rs, which is where this \
         guard's own remedy says to put it. Accusing the core file of a line \
         it does not contain sends the author to a clean file. Got: {core_findings:#?}"
    );
}

#[test]
fn a_real_core_violation_is_still_found_and_named_correctly() {
    let diff = "\
diff --git a/billing/core/src/invoice.rs b/billing/core/src/invoice.rs
--- a/billing/core/src/invoice.rs
+++ b/billing/core/src/invoice.rs
@@ -1,0 +1,1 @@
+use aws_sdk_s3::Client;
";
    let mut ctx = conformant_change();
    ctx.diff_content = diff.to_string();
    ctx.changed_files = vec!["billing/core/src/invoice.rs".to_string()];

    let report = CloudNativeGuard::new()
        .evaluate_cloud_native(Path::new("."), &ctx)
        .expect("guard runs");
    let core: Vec<_> = report
        .violations
        .iter()
        .filter(|v| v.category == "PROPRIETARY_CLOUD_SDK_IN_CORE")
        .collect();
    assert_eq!(
        core.len(),
        1,
        "the guard must still catch the thing it exists to catch. {core:#?}"
    );
    assert!(core[0].description.contains("billing/core/src/invoice.rs"));
}

#[test]
fn one_offending_line_produces_one_finding_not_one_per_core_file() {
    let diff = "\
diff --git a/a/core/src/one.rs b/a/core/src/one.rs
--- a/a/core/src/one.rs
+++ b/a/core/src/one.rs
@@ -1,0 +1,1 @@
+use aws_sdk_s3::Client;
diff --git a/b/core/src/two.rs b/b/core/src/two.rs
--- a/b/core/src/two.rs
+++ b/b/core/src/two.rs
@@ -1,0 +1,1 @@
+pub fn clean() {}
";
    let mut ctx = conformant_change();
    ctx.diff_content = diff.to_string();
    ctx.changed_files = vec![
        "a/core/src/one.rs".to_string(),
        "b/core/src/two.rs".to_string(),
    ];

    let report = CloudNativeGuard::new()
        .evaluate_cloud_native(Path::new("."), &ctx)
        .expect("guard runs");
    let core: Vec<_> = report
        .violations
        .iter()
        .filter(|v| v.category == "PROPRIETARY_CLOUD_SDK_IN_CORE")
        .collect();
    assert_eq!(
        core.len(),
        1,
        "scanning the whole diff once per core file reported the same line \
         once per core file. {core:#?}"
    );
}
