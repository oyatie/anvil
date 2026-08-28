//! Shape x ratchet: seed a baseline from a measurement, and judge a head
//! commit against the baseline frozen at its merge-base.

use super::measure::{MeasureRequest, measure_repo};
use crate::ratchet::adapters::GitMergeBase;
use crate::ratchet::facade::{Reference, load_reference};
use crate::ratchet::ports::{Baseline, Mode, RatchetVerdict, compare};
use crate::shape::ports::{RuleMode, ShapeReport, ShapeSpec};
use anyhow::{Result, anyhow, bail};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Anvil's own config paths inside a tenant repository (not tenant layout).
pub const BASELINE_PATH: &str = ".anvil/baselines/shape.baseline.json";
pub const SIGNOFF_PATH: &str = ".anvil/baselines/shape.signoff.json";

fn mode_of(m: RuleMode) -> Mode {
    match m {
        RuleMode::AdvisoryUntilInfra => Mode::Advisory,
        RuleMode::BaselineBlockOnNew => Mode::BlockOnNew,
    }
}

pub fn keys_by_rule(report: &ShapeReport) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for f in &report.findings {
        out.entry(f.rule.0.clone())
            .or_default()
            .insert(f.key.clone());
    }
    out
}

fn modes_of(spec: &ShapeSpec) -> BTreeMap<String, (Mode, bool)> {
    spec.rules
        .iter()
        .map(|(r, c)| (r.clone(), (mode_of(c.mode), c.frozen_empty)))
        .collect()
}

/// A baseline for `report`, measured at `report.rev`. Rules the report could
/// not measure are seeded with no keys and stay in their declared mode: a
/// rule that becomes measurable later reports its whole set as regressions
/// until a human signs them off — the honest direction.
pub fn seed_baseline(report: &ShapeReport, spec: &ShapeSpec) -> Baseline {
    Baseline::seed(&report.rev, &keys_by_rule(report), &modes_of(spec))
}

/// Seeds from a named full sha (G1: never the working directory).
pub async fn seed_from_commit(
    repo_dir: &Path,
    rev: &str,
    spec_override: Option<&Path>,
) -> Result<(Baseline, ShapeReport)> {
    if rev.len() != 40 || !rev.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!(
            "--rev must be a full 40-character commit sha, got {rev:?}; a baseline is reproducible only from a named commit"
        );
    }
    let req = MeasureRequest {
        repo_dir: repo_dir.to_path_buf(),
        rev: rev.to_string(),
        repo: repo_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        spec_override: spec_override.map(Path::to_path_buf),
        registry_override: None,
    };
    let (report, spec) = measure_with_spec(&req).await?;
    Ok((seed_baseline(&report, &spec), report))
}

async fn measure_with_spec(req: &MeasureRequest) -> Result<(ShapeReport, ShapeSpec)> {
    let report = measure_repo(req).await?;
    let spec = match &req.spec_override {
        Some(p) => ShapeSpec::parse(&std::fs::read_to_string(p)?).map_err(|e| anyhow!("{e}"))?,
        None => {
            let probe = crate::shape::adapters::GitTreeAtRev::load(&req.repo_dir, &req.rev, |p| {
                p == super::cli::SPEC_PATH
            })
            .await
            .map_err(|e| anyhow!("{e}"))?;
            let bytes = crate::shape::ports::TreeSource::read(&probe, super::cli::SPEC_PATH)
                .map_err(|e| anyhow!("{e}"))?
                .ok_or_else(|| anyhow!("no spec at {}", req.rev))?;
            ShapeSpec::parse(std::str::from_utf8(bytes)?).map_err(|e| anyhow!("{e}"))?
        }
    };
    Ok((report, spec))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Judgement {
    /// No baseline at the merge-base: this change introduces it.
    Bootstrap {
        merge_base: String,
        report: ShapeReport,
    },
    Judged {
        merge_base: String,
        report: ShapeReport,
        verdict: RatchetVerdict,
    },
}

/// Measures `head` and compares it with the baseline frozen at
/// `merge-base(base_ref, head)`. `base_ref` must come from the PR context.
pub async fn judge(
    repo_dir: &Path,
    base_ref: &str,
    head: &str,
    spec_override: Option<&Path>,
) -> Result<Judgement> {
    let req = MeasureRequest {
        repo_dir: repo_dir.to_path_buf(),
        rev: head.to_string(),
        repo: repo_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        spec_override: spec_override.map(Path::to_path_buf),
        registry_override: None,
    };
    let (report, spec) = measure_with_spec(&req).await?;
    let source = GitMergeBase::resolve(repo_dir, base_ref, head)
        .await
        .map_err(|e| anyhow!("{e}"))?;
    source
        .preload(&[BASELINE_PATH, SIGNOFF_PATH])
        .await
        .map_err(|e| anyhow!("{e}"))?;
    match load_reference(&source, BASELINE_PATH, SIGNOFF_PATH).map_err(|e| anyhow!("{e}"))? {
        Reference::Bootstrap { rev } => Ok(Judgement::Bootstrap {
            merge_base: rev,
            report,
        }),
        Reference::Frozen {
            rev,
            baseline,
            signoff,
        } => {
            let modes = modes_of(&spec);
            // The rule set the CHANGE declares. `compare` needs it to tell a
            // rule that ran and found nothing from one the change stopped
            // declaring; the key map alone cannot, and the difference decides
            // whether withdrawing a rule launders its baselined keys.
            let declared_now: std::collections::BTreeSet<String> =
                spec.rules.keys().cloned().collect();
            let verdict = compare(
                &baseline,
                &keys_by_rule(&report),
                &signoff,
                |r| modes.get(r).copied(),
                &declared_now,
            );
            Ok(Judgement::Judged {
                merge_base: rev,
                report,
                verdict,
            })
        }
    }
}

pub fn render_judgement(j: &Judgement) -> String {
    match j {
        Judgement::Bootstrap { merge_base, report } => format!(
            "ratchet: no baseline at merge-base {} — this change bootstraps it ({} finding(s) measured at {})\n",
            &merge_base[..12],
            report.findings.len(),
            &report.rev[..12]
        ),
        Judgement::Judged {
            merge_base,
            report,
            verdict,
        } => {
            let mut out = format!(
                "ratchet: {} @ {} vs baseline at merge-base {} — {}\n",
                report.repo,
                &report.rev[..12],
                &merge_base[..12],
                if verdict.fails { "FAIL" } else { "ok" }
            );
            for (rule, v) in &verdict.per_rule {
                if v.regressions.is_empty() && v.fixed.is_empty() && v.signed_off.is_empty() {
                    continue;
                }
                out.push_str(&format!(
                    "  {rule:<32} {:?} +{} new  -{} fixed  {} signed-off  {} tolerated{}\n",
                    v.mode,
                    v.regressions.len(),
                    v.fixed.len(),
                    v.signed_off.len(),
                    v.tolerated.len(),
                    if v.fails { "  <- blocks" } else { "" }
                ));
                for k in v.regressions.iter().take(5) {
                    out.push_str(&format!("      + {k}\n"));
                }
            }
            for (rule, key) in &verdict.inert_signoff {
                out.push_str(&format!(
                    "  inert signoff: {rule} {key} (not present in the candidate)\n"
                ));
            }
            out
        }
    }
}
