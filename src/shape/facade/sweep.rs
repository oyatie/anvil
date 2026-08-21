//! The fleet sweep: measure every watched repository's trunk on a cadence,
//! record the trend, and write the ranked move plan to disk for the
//! delivery step to consume. Report-only (I25): the sweep mutates nothing
//! in any repository and blocks nobody.

use super::measure::{MeasureRequest, measure_repo};
use crate::change_delivery::facade::plan::plan_from_report;
use crate::exec::{ExecClass, run_bounded};
use crate::git_manager::GitManager;
use crate::shape::adapters::GitTreeAtRev;
use crate::telemetry_store::{ShapeMeasurementRecord, TelemetryStore};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Command;

/// origin's default branch head in `repo_dir`, as a full sha: the symbolic
/// origin/HEAD when the clone carries one, else the first of origin/main,
/// origin/dev, origin/master that exists.
pub async fn trunk_rev(repo_dir: &Path) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_dir)
        .args(["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]);
    if let Ok(out) = run_bounded(cmd, ExecClass::Quick, "git symbolic-ref (sweep)").await
        && out.status.success()
    {
        let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !name.is_empty() {
            return GitTreeAtRev::resolve(repo_dir, &name)
                .await
                .map_err(|e| e.to_string());
        }
    }
    for cand in ["origin/main", "origin/dev", "origin/master"] {
        if let Ok(sha) = GitTreeAtRev::resolve(repo_dir, cand).await {
            return Ok(sha);
        }
    }
    Err(format!(
        "no origin default branch in {}",
        repo_dir.display()
    ))
}

pub struct SweepDeps {
    pub git_mgr: Arc<GitManager>,
    pub telemetry: Arc<TelemetryStore>,
    pub data_dir: PathBuf,
}

/// Measures one repository's trunk. Returns the one-line summary that is
/// logged; a repository without an adopted spec is reported as skipped —
/// visibly, never as a zero.
pub async fn sweep_repo(deps: &SweepDeps, repo: &str) -> Result<String, String> {
    let repo_dir = deps
        .git_mgr
        .ensure_repo_cloned(repo)
        .await
        .map_err(|e| e.to_string())?;
    let rev = trunk_rev(&repo_dir).await?;
    let report = match measure_repo(&MeasureRequest {
        repo_dir: repo_dir.clone(),
        rev: rev.clone(),
        repo: repo.to_string(),
        spec_override: None,
        registry_override: None,
    })
    .await
    {
        Ok(r) => r,
        Err(e) if e.to_string().contains("pass --spec-override") => {
            return Ok(format!(
                "{repo} @ {}: no shape spec adopted; not measured",
                &rev[..12]
            ));
        }
        Err(e) => return Err(e.to_string()),
    };
    let d = report.distance();
    let mut per_rule = std::collections::BTreeMap::new();
    for f in &report.findings {
        *per_rule.entry(f.rule.0.clone()).or_insert(0usize) += 1;
    }
    deps.telemetry
        .record_shape_measurement(ShapeMeasurementRecord {
            repo: repo.to_string(),
            rev: report.rev.clone(),
            spec_source: "adopted".to_string(),
            findings_total: d.findings_total,
            units_total: d.units_total,
            units_conformant: d.units_conformant,
            per_rule,
            blocking_regressions: 0,
            advisory_regressions: 0,
            recorded_at: chrono::Utc::now(),
        })
        .await;

    let plan = plan_from_report(&report, "adopted");
    let dir = deps.data_dir.join("shape");
    let _ = tokio::fs::create_dir_all(&dir).await;
    let path = dir.join(format!("{}.moveplan.json", repo.replace('/', "-")));
    tokio::fs::write(&path, format!("{}\n", plan.to_json()))
        .await
        .map_err(|e| e.to_string())?;

    Ok(format!(
        "{repo} @ {}: distance {} ({}/{} units conformant, {} move(s) planned -> {})",
        &report.rev[..12],
        d.findings_total,
        d.units_conformant,
        d.units_total,
        plan.moves.len(),
        path.display()
    ))
}
