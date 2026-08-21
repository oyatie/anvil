//! The fleet sweep measures a trunk, records the trend, and writes the move
//! plan — and a repository without a spec is reported as skipped, never as
//! a zero-distance success.

use anvil::shape::facade::sweep::{SweepDeps, sweep_repo};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

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
    "face_dependency_matrix": { "facade": ["ports", "adapters"], "adapters": ["ports", "core"], "ports": ["core"], "core": [] },
    "satellites": { "policy": { "dir": "policy/", "form": "**", "aliases": ["policies/"] } },
    "allowed_unit_root_files": ["manifest.json"] } },
  "root_files": { "mode": "allowlist", "rules": [ { "id": "any", "kind": "suffix", "value": ".md" } ] },
  "destination_stable_default": true,
  "rules": { "satellite_alias_used": { "mode": "baseline-block-on-new" } }
}"#;

/// A bare "origin" with one commit, cloned by ensure_repo_cloned's URL? The
/// sweep only needs a clone directory that git can read, so we lay the clone
/// where the GitManager expects it and give it an `origin` remote pointing at
/// a local bare repo with a `main` branch.
fn seed(repos_dir: &Path, name: &str, with_spec: bool) {
    let origin = repos_dir.join(format!("{name}-origin.git"));
    let work = repos_dir.join("seed");
    std::fs::create_dir_all(&work).unwrap();
    git(&work, &["init", "-q", "-b", "main"]);
    git(&work, &["config", "commit.gpgsign", "false"]);
    std::fs::create_dir_all(work.join("iam/policies")).unwrap();
    std::fs::write(work.join("iam/manifest.json"), "{}").unwrap();
    std::fs::write(work.join("iam/policies/rbac.json"), "{}").unwrap();
    if with_spec {
        std::fs::create_dir_all(work.join(".anvil")).unwrap();
        std::fs::write(work.join(".anvil/shape.json"), SPEC).unwrap();
    }
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-q", "-m", "seed"]);
    git(
        &work,
        &[
            "clone",
            "-q",
            "--bare",
            work.to_str().unwrap(),
            origin.to_str().unwrap(),
        ],
    );
    let clone = repos_dir.join(name);
    git(
        repos_dir,
        &[
            "clone",
            "-q",
            origin.to_str().unwrap(),
            clone.to_str().unwrap(),
        ],
    );
    std::fs::remove_dir_all(&work).unwrap();
}

#[tokio::test]
async fn a_trunk_is_measured_recorded_and_planned() {
    let tmp = tempfile::tempdir().unwrap();
    let repos_dir = tmp.path().join("repos");
    std::fs::create_dir_all(&repos_dir).unwrap();
    seed(&repos_dir, "shaped", true);
    let deps = SweepDeps {
        git_mgr: Arc::new(anvil::git_manager::GitManager::new(repos_dir.clone())),
        telemetry: Arc::new(
            anvil::telemetry_store::TelemetryStore::new(tmp.path().join("data/telemetry")).await,
        ),
        data_dir: tmp.path().join("data"),
    };
    let summary = sweep_repo(&deps, "fixture/shaped").await.expect("sweeps");
    assert!(summary.contains("distance 1"), "{summary}");
    let latest = deps.telemetry.latest_shape_measurements().await;
    assert_eq!(latest["fixture/shaped"].findings_total, 1);
    let plan_path = tmp.path().join("data/shape/fixture-shaped.moveplan.json");
    let raw = std::fs::read(&plan_path).expect("move plan written");
    let plan = anvil::change_delivery::ports::ShapeMovePlan::parse(&raw).expect("parses");
    assert_eq!(plan.moves.len(), 1);
    assert_eq!(plan.moves[0].to, "iam/policy/rbac.json");
}

#[tokio::test]
async fn a_repo_without_a_spec_is_skipped_visibly_not_zeroed() {
    let tmp = tempfile::tempdir().unwrap();
    let repos_dir = tmp.path().join("repos");
    std::fs::create_dir_all(&repos_dir).unwrap();
    seed(&repos_dir, "bare", false);
    let deps = SweepDeps {
        git_mgr: Arc::new(anvil::git_manager::GitManager::new(repos_dir.clone())),
        telemetry: Arc::new(
            anvil::telemetry_store::TelemetryStore::new(tmp.path().join("data/telemetry")).await,
        ),
        data_dir: tmp.path().join("data"),
    };
    let summary = sweep_repo(&deps, "fixture/bare")
        .await
        .expect("skips cleanly");
    assert!(summary.contains("no shape spec adopted"), "{summary}");
    assert!(
        deps.telemetry.latest_shape_measurements().await.is_empty(),
        "no fabricated zero"
    );
}
