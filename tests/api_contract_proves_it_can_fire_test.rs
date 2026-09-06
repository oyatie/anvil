//! The OpenAPI contract gate demonstrates both halves.
//!
//! Both fixtures are committed git repositories. The guard reads
//! `git status --porcelain` and treats any dirty path containing "openapi" as
//! a reconciled schema, so an uncommitted fixture would hand the gate the very
//! evidence it is being asked to withhold, and the red case would silently
//! turn green.

use anvil::api_contract_guard::ApiContractGuard;
use anvil::git_manager::{PrDiffContext, SubjectRoot, Uncloned};
use std::path::{Path, PathBuf};
use std::process::Command;

fn ctx() -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: String::new(),
        changed_files: vec!["api/openapi.yaml".to_string()],
        repo_working_dir: SubjectRoot::asserted(PathBuf::from("."), Uncloned::TestFixture),
        is_incremental: false,
        previous_head_sha: None,
    }
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A committed repository holding `api/openapi.yaml`, plus whatever else the
/// case needs. Committed, so `git status --porcelain` is empty.
fn repo(tag: &str, extra: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("anvil-apic-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("api")).expect("scratch");
    std::fs::write(dir.join("api/openapi.yaml"), "openapi: 3.0.0\n").expect("schema");
    for (path, body) in extra {
        let full = dir.join(path);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("dir");
        std::fs::write(&full, body).expect("file");
    }
    git(&dir, &["init", "--quiet"]);
    git(&dir, &["config", "user.email", "fixture@example.invalid"]);
    git(&dir, &["config", "user.name", "Fixture"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "--quiet", "-m", "fixture"]);
    let porcelain = Command::new("git")
        .current_dir(&dir)
        .args(["status", "--porcelain"])
        .output()
        .expect("git status runs");
    assert!(
        porcelain.stdout.is_empty(),
        "the fixture tree is dirty, so the guard would read it as an \
         auto-reconciled schema and spare the change regardless of the checker: {}",
        String::from_utf8_lossy(&porcelain.stdout)
    );
    dir
}

#[tokio::test]
async fn api_contract_fires_on_a_schema_the_checker_refuses() {
    let dir = repo(
        "red",
        &[("scripts/check-openapi-refs.mjs", "process.exit(1);\n")],
    );
    let report = ApiContractGuard::new()
        .ensure_contract_integrity("oyatie/anvil", &dir, &ctx())
        .await
        .expect("the guard runs");

    assert!(
        !report.is_intact,
        "the repository's own contract checker refused the schema and nothing \
         was reconciled, yet the gate reported the contract intact: {}",
        report.summary
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn api_contract_spares_a_schema_change_nothing_flags() {
    let dir = repo("green", &[]);
    let report = ApiContractGuard::new()
        .ensure_contract_integrity("oyatie/anvil", &dir, &ctx())
        .await
        .expect("the guard runs");

    assert!(
        report.is_intact,
        "an API change with no checker to refuse it and no drift on disk is \
         intact. Refusing it would refuse every schema edit: {}",
        report.summary
    );
    let _ = std::fs::remove_dir_all(&dir);
}
