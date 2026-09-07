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
struct RawTier {
    model: String,
    provider: String,
    effort: String,
    timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
struct RawMeta {
    /// Path prefixes a turn at this stage may stage for commit.
    ///
    /// Required, and an empty list is a real answer: it means the stage may
    /// commit nothing. Absent is a load error, because a stage with no declared
    /// scope is absent evidence rather than an unrestricted one.
    writes: Vec<String>,
}

#[derive(Debug, Deserialize)]
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
            if prefix.starts_with('/') || prefix.contains("..") || prefix.trim().is_empty() {
                bail!(
                    "stage `{key}` declares write prefix {prefix:?}, which is not a relative path inside the repository"
                );
            }
        }
        // A stage may not list the same (provider, model) twice.
        //
        // `recon` and `planning` shipped with ten tiers where five were unique:
        // a regeneration script ran twice and nothing objected. Worst-case
        // fallback latency doubled, 39 minutes to 78, and the tests could not
        // see it -- one asserted only that a chain is non-empty, and the other
        // read `text.split("[[stage.recon]]").nth(1)`, which is the FIRST copy.
        // A duplicate tier is never intentional: the second is unreachable
        // except as time spent failing the first again.
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
            },
        );
    }

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
