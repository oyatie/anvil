//! The fleet sweep: measure every watched repository's trunk on a cadence and
//! record the trend. Report-only (I25): mutates nothing in any repository and
//! blocks nobody.
//!
//! Measurement only. Turning a report into a move plan is delivery's job, and
//! doing it here made `shape` import `change_delivery` while
//! `change_delivery` imports `shape` -- a two-member dependency cycle whose
//! every edge is individually legal, which is why no per-edge rule can see it.
//! The composition root holds both halves.

use super::measure::{MeasureRequest, measure_repo};
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

/// What one sweep of one repository produced.
///
/// `Skipped` is not a failure and not a measurement: a repository with no
/// adopted spec has nothing to measure against, and reporting that as a clean
/// sweep would be an absence read as a pass.
pub enum Swept {
    Measured {
        report: crate::shape::ports::ShapeReport,
        summary: String,
    },
    Skipped(String),
}

pub struct SweepDeps {
    pub git_mgr: Arc<GitManager>,
    pub telemetry: Arc<TelemetryStore>,
    pub data_dir: PathBuf,
}

/// Measures one repository's trunk. Returns the one-line summary that is
/// logged; a repository without an adopted spec is reported as skipped —
/// visibly, never as a zero.
pub async fn sweep_repo(deps: &SweepDeps, repo: &str) -> Result<Swept, String> {
    let repo_dir = deps
        .git_mgr
        .ensure_repo_cloned(repo)
        .await
        .map_err(|e| e.to_string())?;
    let rev = trunk_rev(repo_dir.as_path()).await?;
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
            return Ok(Swept::Skipped(format!(
                "{repo} @ {}: no shape spec adopted; not measured",
                &rev[..12]
            )));
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

    Ok(Swept::Measured {
        summary: format!(
            "{repo} @ {}: distance {} ({}/{} units conformant)",
            &report.rev[..12],
            d.findings_total,
            d.units_conformant,
            d.units_total
        ),
        report,
    })
}
