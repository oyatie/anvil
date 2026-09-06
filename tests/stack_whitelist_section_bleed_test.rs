//! A dependency section belongs to the file that declares it.
//!
//! The dependency-expansion rule iterated `changed_files`, and for every path
//! ending in `Cargo.toml` it walked the WHOLE diff carrying an
//! `in_deps_section` flag. The flag is raised by a `[dependencies]` header and
//! lowered only by a line beginning `[`. Nothing in a `diff --git` header
//! begins with `[`, so once raised the flag stayed raised across every
//! following file in the diff.
//!
//! A change that adds one dependency and also edits Rust source therefore
//! reported every added source line as an unauthorised dependency -- and named
//! the Cargo.toml as the file each was added to.
//!
//! Adding a dependency and touching code in the same pull request is the
//! ordinary case, so this fired on ordinary work.

use anvil::git_manager::PrDiffContext;
use anvil::stack_whitelist_guard::StackWhitelistGuard;
use std::path::Path;

fn dep_plus_source_change() -> PrDiffContext {
    let diff = "\
diff --git a/Cargo.toml b/Cargo.toml
--- a/Cargo.toml
+++ b/Cargo.toml
@@ -10,0 +11,1 @@
 [dependencies]
+serde_yaml = \"0.9\"
diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,0 +1,3 @@
+pub fn parse() {}
+pub fn render() {}
+pub fn emit() {}
";
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 7,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: diff.to_string(),
        changed_files: vec!["Cargo.toml".to_string(), "src/lib.rs".to_string()],
        repo_working_dir: anvil::git_manager::SubjectRoot::asserted(
            std::path::PathBuf::from("."),
            anvil::git_manager::Uncloned::TestFixture,
        ),
        is_incremental: false,
        previous_head_sha: None,
    }
}

fn dependency_findings(ctx: &PrDiffContext) -> Vec<String> {
    StackWhitelistGuard::new()
        .evaluate_stack_whitelist(Path::new("."), ctx, false)
        .expect("guard runs")
        .violations
        .into_iter()
        .filter(|v| v.category == "UNAUTHORIZED_DEPENDENCY_EXPANSION")
        .map(|v| v.item)
        .collect()
}

#[test]
fn added_source_lines_are_not_dependencies() {
    let found = dependency_findings(&dep_plus_source_change());
    assert!(
        !found.iter().any(|i| i.contains("pub fn")),
        "a function added to src/lib.rs was reported as a dependency, because \
         the section flag raised by Cargo.toml's [dependencies] header was \
         never lowered when the diff moved to the next file. Got: {found:?}"
    );
}

#[test]
fn the_one_real_dependency_is_still_caught_exactly_once() {
    let found = dependency_findings(&dep_plus_source_change());
    assert_eq!(
        found.len(),
        1,
        "exactly one dependency was added. The guard must neither miss it nor \
         multiply it. Got: {found:?}"
    );
    assert!(found[0].contains("serde_yaml"), "got: {found:?}");
}

#[test]
fn a_human_authored_change_is_not_subject_to_the_ratchet() {
    let ctx = dep_plus_source_change();
    let report = StackWhitelistGuard::new()
        .evaluate_stack_whitelist(Path::new("."), &ctx, true)
        .expect("guard runs");
    assert!(
        !report
            .violations
            .iter()
            .any(|v| v.category == "UNAUTHORIZED_DEPENDENCY_EXPANSION"),
        "the rule is an agent-authority ratchet, not a dependency ban"
    );
}
