//! The reference is the baseline as committed at the merge-base — not the
//! working copy and not the head commit. A change that rewrites its own
//! baseline is judged against the one it branched from.

use anvil::ratchet::adapters::GitMergeBase;
use anvil::ratchet::facade::{Reference, load_reference};
use anvil::ratchet::ports::FrozenReferenceSource;
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@x")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@x")
        .args(args)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

const BASELINE: &str = ".anvil/baselines/shape.baseline.json";

fn baseline_json(keys: &[&str]) -> String {
    let ks: Vec<String> = keys.iter().map(|k| format!("\"{k}\"")).collect();
    format!(
        r#"{{"schema":"anvil/ratchet-baseline/v1","measured_at":"{}","rules":{{"file_misplaced":{{"mode":"baseline-block-on-new","keys":[{}]}}}}}}"#,
        "a".repeat(40),
        ks.join(",")
    )
}

#[tokio::test]
async fn the_reference_is_read_at_the_merge_base_not_head_or_worktree() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    git(d, &["init", "-q", "-b", "main"]);
    git(d, &["config", "commit.gpgsign", "false"]);
    std::fs::create_dir_all(d.join(".anvil/baselines")).unwrap();
    std::fs::write(d.join(BASELINE), baseline_json(&["a"])).unwrap();
    git(d, &["add", "-A"]);
    git(d, &["commit", "-q", "-m", "baseline a"]);
    let base_sha = git(d, &["rev-parse", "HEAD"]);

    // Branch: the change rewrites the baseline to launder key "b" in.
    git(d, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(d.join(BASELINE), baseline_json(&["a", "b"])).unwrap();
    git(d, &["commit", "-q", "-am", "grow the baseline"]);
    let head = git(d, &["rev-parse", "HEAD"]);
    // Working copy differs again.
    std::fs::write(d.join(BASELINE), baseline_json(&["a", "b", "c"])).unwrap();

    let source = GitMergeBase::resolve(d, "main", &head)
        .await
        .expect("merge-base");
    assert_eq!(source.reference_rev(), base_sha);
    source
        .preload(&[BASELINE, ".anvil/baselines/shape.signoff.json"])
        .await
        .unwrap();
    match load_reference(&source, BASELINE, ".anvil/baselines/shape.signoff.json").unwrap() {
        Reference::Frozen { baseline, .. } => {
            let keys = &baseline.rules["file_misplaced"].keys;
            assert!(keys.contains("a") && !keys.contains("b") && !keys.contains("c"));
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn no_baseline_at_the_merge_base_is_bootstrap_not_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    git(d, &["init", "-q", "-b", "main"]);
    git(d, &["config", "commit.gpgsign", "false"]);
    std::fs::write(d.join("README.md"), "x").unwrap();
    git(d, &["add", "-A"]);
    git(d, &["commit", "-q", "-m", "init"]);
    git(d, &["checkout", "-q", "-b", "feature"]);
    std::fs::create_dir_all(d.join(".anvil/baselines")).unwrap();
    std::fs::write(d.join(BASELINE), baseline_json(&["a"])).unwrap();
    git(d, &["add", "-A"]);
    git(d, &["commit", "-q", "-m", "introduce baseline"]);
    let head = git(d, &["rev-parse", "HEAD"]);

    let source = GitMergeBase::resolve(d, "main", &head).await.unwrap();
    source
        .preload(&[BASELINE, ".anvil/baselines/shape.signoff.json"])
        .await
        .unwrap();
    assert!(matches!(
        load_reference(&source, BASELINE, ".anvil/baselines/shape.signoff.json").unwrap(),
        Reference::Bootstrap { .. }
    ));
}
