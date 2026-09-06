//! The monorepo hygiene gate demonstrates both halves.
//!
//! Its first rule quarantines AI agent scratch directories: a `.gemini/` or
//! `.antigravity/` path in a commit is the harness leaking into the repository
//! it was pointed at. Anvil is itself an agent harness, so this is the gate
//! most likely to be needed and least likely to be exercised by ordinary work.

use anvil::git_manager::{PrDiffContext, SubjectRoot, Uncloned};

fn ctx(files: Vec<&str>) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: String::new(),
        changed_files: files.into_iter().map(str::to_string).collect(),
        repo_working_dir: SubjectRoot::asserted(
            std::path::PathBuf::from("."),
            Uncloned::TestFixture,
        ),
        is_incremental: false,
        previous_head_sha: None,
    }
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("anvil-mono-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(d.join("src")).expect("scratch");
    d
}

#[tokio::test]
async fn monorepo_fires_on_an_agent_scratch_directory_in_a_commit() {
    let dir = scratch("red");
    let report = anvil::monorepo_guard::MonorepoGuard::new()
        .evaluate_monorepo_hygiene(&dir, &ctx(vec!["src/lib.rs", ".gemini/session.json"]))
        .await
        .expect("the guard runs");
    assert!(
        !report.is_compliant,
        "an agent's scratch directory entered the commit and the gate did not \
         see it. Anvil is itself a harness; this is the leak it must catch."
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn monorepo_spares_a_change_that_leaks_no_harness() {
    let dir = scratch("green");
    let report = anvil::monorepo_guard::MonorepoGuard::new()
        .evaluate_monorepo_hygiene(&dir, &ctx(vec!["src/lib.rs", "tests/unit.rs"]))
        .await
        .expect("the guard runs");
    assert!(
        report.is_compliant,
        "ordinary source and test paths carry no harness; flagging them would \
         refuse every change: {}",
        report.summary
    );
    let _ = std::fs::remove_dir_all(&dir);
}
