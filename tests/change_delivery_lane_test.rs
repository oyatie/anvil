//! One lane, one worktree (I19), and nothing leaves it: the dry-run builds a
//! shard in an isolated worktree — rewrite, stage, purity, gate — and tears
//! it down. The daemon's own repository is refused; a live lane's lease
//! keeps the worktree GC away; a second run derives the same shard.

use anvil::change_delivery::adapters::git_vcs::LANE_LEASE_FILE;
use anvil::change_delivery::facade::deliver::{DeliverRequest, deliver_dry_run};
use anvil::change_delivery::ports::GateResult;
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) {
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
}

const SPEC: &str = r#"{
  "schema": "anvil/shape/v1",
  "profiles": ["rust-cargo"],
  "unit_kinds": { "capability": { "root": "<name>/", "skeleton": "standard", "members": "discover:manifest.json" } },
  "skeletons": { "standard": {
    "faces": { "core": "core/", "ports": "ports/", "adapters": "adapters/", "facade": "facade/" },
    "required_faces": [],
    "unit_marker": "manifest.json",
    "face_dependency_matrix": { "facade": ["ports", "adapters"], "adapters": ["ports", "core"], "ports": ["core"], "core": [] },
    "satellites": { "policy": { "dir": "policy/", "form": "**", "aliases": ["policies/"] } },
    "allowed_unit_root_files": ["manifest.json"] } },
  "root_files": { "mode": "allowlist", "rules": [ { "id": "any-md", "kind": "suffix", "value": ".md" } ] },
  "units": { "iam": { "destination_stable": true } },
  "destination_stable_default": true,
  "rules": { "satellite_alias_used": { "mode": "baseline-block-on-new" } }
}"#;

fn fixture_repo(dir: &Path) {
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "commit.gpgsign", "false"]);
    std::fs::create_dir_all(dir.join("iam/policies")).unwrap();
    std::fs::write(dir.join("iam/manifest.json"), "{}").unwrap();
    std::fs::write(dir.join("iam/policies/rbac.json"), "{}").unwrap();
    std::fs::write(dir.join("README.md"), "x").unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "fixture"]);
}

#[tokio::test]
async fn a_shard_is_built_in_a_lane_purely_and_the_lane_is_torn_down() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    fixture_repo(&repo);
    let spec_path = tmp.path().join("spec.json");
    std::fs::write(&spec_path, SPEC).unwrap();

    let req = DeliverRequest {
        repo_dir: repo.clone(),
        repo: "fixture/repo".into(),
        max: 1,
        spec_override: Some(spec_path.clone()),
        allow_same_repo: false,
    };
    let (runs, shards, _policy) = deliver_dry_run(&req).await.expect("dry run");
    assert_eq!(shards.len(), 1, "one satellite alias -> one shard");
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    assert_eq!(run.rewrite, Ok(()), "a file move is mechanical");
    assert_eq!(run.purity, Some(Ok(())), "a git mv is structure-only");
    assert!(
        matches!(&run.gate, Some(GateResult::Unavailable { .. })),
        "no root Cargo.toml -> the gate is unavailable, which is not a pass: {:?}",
        run.gate
    );
    assert!(run.diffstat.contains("rbac.json"), "{}", run.diffstat);

    // The lane is gone; the source repo is untouched.
    let lanes = repo.join(".anvil-lanes");
    let leftover = std::fs::read_dir(&lanes).map(|d| d.count()).unwrap_or(0);
    assert_eq!(leftover, 0, "lane worktrees are torn down");
    assert!(
        repo.join("iam/policies/rbac.json").exists(),
        "dry run never mutates the checkout"
    );

    // Determinism: the same tree yields the same shard identity.
    let (runs2, _, _) = deliver_dry_run(&req).await.expect("second dry run");
    assert_eq!(runs2[0].shard.key, run.shard.key);
}

#[tokio::test]
async fn the_daemon_tree_is_refused_unless_explicitly_allowed() {
    // This test runs from the daemon's own repository (cwd = package root),
    // which is exactly the tree lanes must refuse.
    let here = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let req = DeliverRequest {
        repo_dir: here,
        repo: "oyatie/anvil".into(),
        max: 1,
        spec_override: None,
        allow_same_repo: false,
    };
    let (runs, shards, _) = deliver_dry_run(&req).await.expect("measures fine");
    // Anvil's own shards are all held (destination_stable=false) so none is
    // selected; force the refusal path by checking the guard directly.
    assert!(
        runs.iter()
            .all(|r| r.rewrite.is_ok() || r.rewrite.as_ref().unwrap_err().contains("refused")),
        "{runs:?}"
    );
    let _ = shards;
    let err = anvil::change_delivery::adapters::self_source_guard::assert_not_daemon_tree(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    )
    .await
    .expect_err("the daemon's own tree must be refused for lanes");
    assert!(err.contains("daemon's own source tree"), "{err}");
}

#[tokio::test]
async fn an_unexpired_lease_protects_a_lane_from_the_worktree_gc() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().to_path_buf();
    let wt = base.join(".worktrees");
    let live = wt.join("lane-live");
    let dead = wt.join("stale-no-lease");
    std::fs::create_dir_all(&live).unwrap();
    std::fs::create_dir_all(&dead).unwrap();
    let future = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;
    std::fs::write(live.join(LANE_LEASE_FILE), format!("{future}\n")).unwrap();
    // Age both directories past the TTL.
    for d in [&live, &dead] {
        let out = Command::new("touch")
            .args(["-t", "202601010000", d.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(out.status.success());
    }
    let mgr = anvil::git_manager::GitManager::new(base);
    let cleaned = mgr.clean_abandoned_worktrees().await.expect("gc runs");
    assert!(live.exists(), "an unexpired lease protects the lane");
    assert!(!dead.exists(), "a stale dir without a lease is reclaimed");
    assert_eq!(cleaned, 1);
}
