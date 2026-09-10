//! Parsing and validating the declared routing table.
//!
//! Split from the dispatcher: `chain.rs` decides which model serves a stage and
//! runs the turn; this decides whether the file is a table at all. Both
//! directions are checked -- a stage key no `Stage` names is a load error, and a
//! `Stage` with no chain is one too, because a chain nothing dispatches and a
//! stage with no providers are both absent evidence.

use super::{Stage, StagePlan, Tier};
use crate::ai_driver::provider::ModelProvider;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTier {
    model: String,
    provider: String,
    effort: String,
    timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMeta {
    /// A ceiling on every tier's effort for this stage, when the stage's job is
    /// cheap by definition.
    ///
    /// `issue_triage` carried this as a separate test over a hardcoded table.
    /// The table was deleted and the test with it, unmentioned, and the new
    /// declaration ran `high`/600s where the old one was `low`/<=90s. A ceiling
    /// that lives in the file it bounds cannot be deleted without deleting the
    /// thing it bounds.
    /// The stage this one judges, when it judges one.
    #[serde(default)]
    audits: Option<String>,
    /// Stages this one requires to have already run.
    ///
    /// `audits = X` already implies "after X" and the loader derives that edge,
    /// so naming it here as well is the duplicate-tier defect in another shape
    /// and is refused. This key is for order WITHOUT judgement.
    ///
    /// Neither this nor `audits` had a reader before: `runs_after` was declared
    /// on four stages and `authored_before` on one, serde dropped both on the
    /// floor, and the pull request that added them claimed "ordering is data in
    /// a validated file". It was data in a file nothing parsed. `authored_before`
    /// is gone -- one relation spelled two ways, in opposite directions, is how
    /// the two spellings came to disagree (`test_authoring` claimed it ran
    /// before `implementation`; `implementation` named only
    /// `test_authoring_review`, which claimed nothing).
    #[serde(default)]
    runs_after: Vec<String>,
    #[serde(default)]
    max_effort: Option<String>,
    #[serde(default)]
    max_timeout_secs: Option<u64>,
    /// Path prefixes a turn at this stage may stage for commit.
    ///
    /// Required, and an empty list is a real answer: it means the stage may
    /// commit nothing. Absent is a load error, because a stage with no declared
    /// scope is absent evidence rather than an unrestricted one.
    writes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(default)]
    stage: BTreeMap<String, Vec<RawTier>>,
    #[serde(default)]
    stage_meta: BTreeMap<String, RawMeta>,
}

/// The provider strings the file may use.
///
/// Exhaustive over `ModelProvider`, so adding a variant without giving it a
/// spelling here fails to compile rather than becoming a provider no chain can
/// name.
fn provider_named(s: &str) -> Option<ModelProvider> {
    let all = [
        ModelProvider::AnthropicClaudeCode,
        ModelProvider::OpenAiCodex,
        ModelProvider::CursorAgent,
        ModelProvider::XAiGrok,
        ModelProvider::Antigravity,
        ModelProvider::Muse,
        ModelProvider::SubscriptionEnsemble,
    ];
    all.into_iter().find(|p| {
        let name = match p {
            ModelProvider::AnthropicClaudeCode => "claude",
            ModelProvider::OpenAiCodex => "codex",
            ModelProvider::CursorAgent => "cursor",
            ModelProvider::XAiGrok => "grok",
            ModelProvider::Antigravity => "agy",
            ModelProvider::Muse => "muse",
            ModelProvider::SubscriptionEnsemble => "ensemble",
        };
        name == s
    })
}

/// Parse and validate the declared table.
pub(super) fn parse_table(text: &str) -> Result<BTreeMap<Stage, StagePlan>> {
    let raw: RawFile = toml::from_str(text).context("config/model-routing.toml does not parse")?;
    let mut out: BTreeMap<Stage, StagePlan> = BTreeMap::new();

    for (key, tiers) in &raw.stage {
        let Some(stage) = Stage::ALL.iter().copied().find(|s| s.key() == key) else {
            bail!(
                "config/model-routing.toml declares stage `{key}`, which no `Stage` variant \
                 names. A chain nothing dispatches is not a routing decision."
            );
        };
        if tiers.is_empty() {
            bail!("stage `{key}` declares no tiers; a stage with an empty chain cannot run");
        }
        let mut built = Vec::new();
        for t in tiers {
            let Some(provider) = provider_named(&t.provider) else {
                bail!(
                    "stage `{key}` names provider `{}`, which is not one of claude, codex, cursor, grok, agy, ensemble",
                    t.provider
                );
            };
            if t.model.trim().is_empty() {
                bail!("stage `{key}` has a tier with an empty model id");
            }
            built.push(Tier {
                provider,
                model: t.model.clone(),
                effort: t.effort.clone(),
                timeout: Duration::from_secs(t.timeout_secs),
            });
        }
        let Some(meta) = raw.stage_meta.get(key) else {
            bail!(
                "stage `{key}` declares no `[stage_meta.{key}] writes = [..]`. A stage with no \
                 declared write scope is absent evidence, not an unrestricted stage: the run-scope \
                 guardrail would have nothing to enforce and would silently pass."
            );
        };
        for prefix in &meta.writes {
            if let Err(why) = super::scope_grammar::scope_prefix_is_writable_by_the_hook(prefix) {
                bail!("stage `{key}` declares write prefix {prefix:?}: {why}");
            }
        }
        // A duplicate tier is never intentional: the second is unreachable
        // except as time spent failing the first again, and it doubles the
        // chain's worst-case latency where nothing is looking.

        // Effort is ordered, so a ceiling is a comparison rather than equality.
        const EFFORT_RANK: &[&str] = &["low", "medium", "high", "xhigh", "max", "ultra"];
        if let Some(ceiling) = &meta.max_effort {
            let cap = EFFORT_RANK
                .iter()
                .position(|e| e == ceiling)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "stage `{key}` declares max_effort {ceiling:?}, which is not an effort"
                    )
                })?;
            for t in &built {
                let got = EFFORT_RANK
                    .iter()
                    .position(|e| *e == t.effort)
                    .unwrap_or(usize::MAX);
                if got > cap {
                    bail!(
                        "stage `{key}` caps effort at {ceiling:?} and tier {} declares {:?}. \
                         This stage's work is cheap by definition; spending more per call is \
                         the cost the cap exists to bound.",
                        t.model,
                        t.effort
                    );
                }
            }
        }
        if let Some(cap) = meta.max_timeout_secs {
            for t in &built {
                if t.timeout.as_secs() > cap {
                    bail!(
                        "stage `{key}` caps a turn at {cap}s and tier {} declares {}s. \
                         A call allowed that long has stopped being cheap.",
                        t.model,
                        t.timeout.as_secs()
                    );
                }
            }
        }
        let mut seen_tier = std::collections::BTreeSet::new();
        for t in &built {
            if !seen_tier.insert((format!("{:?}", t.provider), t.model.clone())) {
                bail!(
                    "stage `{key}` lists {} on {:?} more than once; a repeated tier is \
                     unreachable except as the time spent failing the first one again",
                    t.model,
                    t.provider
                );
            }
        }
        out.insert(
            stage,
            StagePlan {
                tiers: built,
                writes: meta.writes.clone(),
                audits: meta.audits.clone(),
                runs_after: meta.runs_after.clone(),
            },
        );
    }

    // An auditor may not write what it judges, or the finding and the fix come
    // from one act. It may still WRITE: judging by reading and judging by
    // building are both auditing, so the rule bounds the scope, not the act.
    for (stage, plan) in &out {
        let Some(audited_key) = plan.audits.clone() else {
            continue;
        };
        let Some(audited) = Stage::ALL.iter().copied().find(|s| s.key() == audited_key) else {
            bail!(
                "stage `{}` declares it audits `{audited_key}`, which no `Stage` names",
                stage.key()
            );
        };
        let Some(audited_plan) = out.get(&audited) else {
            continue;
        };
        for w in &plan.writes {
            for a in &audited_plan.writes {
                let (w, a) = (w.trim_end_matches('/'), a.trim_end_matches('/'));
                if w == a || w.starts_with(&format!("{a}/")) || a.starts_with(&format!("{w}/")) {
                    bail!(
                        "stage `{}` audits `{audited_key}` and both may write {w:?} / {a:?}. \
                         An auditor that can edit what it judges is not an auditor: the finding \
                         and the fix would come from one act.",
                        stage.key()
                    );
                }
            }
        }
    }

    super::order::validate(&out)?;

    for stage in Stage::ALL {
        if !out.contains_key(stage) {
            bail!(
                "`Stage::{stage:?}` has no chain in config/model-routing.toml. A stage with no \
                 declared chain has no providers to try, and would fail on its first turn."
            );
        }
    }
    Ok(out)
}
