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
    /// A ceiling on every tier's effort for this stage, when the stage's job is
    /// cheap by definition.
    ///
    /// `issue_triage` carried this as a separate test over a hardcoded table.
    /// The table was deleted and the test with it, unmentioned, and the new
    /// declaration ran `high`/600s where the old one was `low`/<=90s. A ceiling
    /// that lives in the file it bounds cannot be deleted without deleting the
    /// thing it bounds.
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
            if let Err(why) = scope_prefix_is_writable_by_the_hook(prefix) {
                bail!("stage `{key}` declares write prefix {prefix:?}: {why}");
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

/// The `pre-commit` hook's own declaration grammar, ported clause for clause.
///
/// The loader previously approximated it with three checks -- leading `/`,
/// `..`, blank -- and the hook's grammar is strictly larger. Eight forms loaded
/// here and were then refused by the consumer, which does not merely fail: a
/// declaration the hook cannot parse refuses EVERY commit in that run. One
/// form, a prefix containing a newline, did worse and silently split into two
/// prefixes, widening the scope past what a reader of the file would see.
///
/// This mirrors `anvil_scope_literal(.., declaration)` in
/// `src/git_manager/hooks/pre-commit`. The two must not drift, and
/// `a_declaration_the_hook_would_refuse_is_a_load_error` holds the shared
/// fixtures that say they have not.
fn scope_prefix_is_writable_by_the_hook(prefix: &str) -> Result<(), &'static str> {
    // The hook does no trimming, so surrounding whitespace is part of the
    // literal and would silently match nothing. Refused loudly here instead.
    if prefix != prefix.trim() {
        return Err("has leading or trailing whitespace, which the hook takes literally");
    }
    // One trailing slash, as the hook strips for a declaration.
    let literal = prefix.strip_suffix('/').unwrap_or(prefix);
    if literal.is_empty() {
        return Err("is empty");
    }
    if literal.starts_with('/') {
        return Err("is absolute; the scope is repository-relative");
    }
    let bytes = literal.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err("looks like a drive-lettered path");
    }
    if literal.contains('\\') {
        return Err("contains a backslash");
    }
    if literal.contains('"') {
        return Err("contains a quote");
    }
    if literal.chars().any(char::is_control) {
        return Err("contains a control byte; a newline would silently split it in two");
    }
    let framed = format!("/{literal}/");
    if framed.contains("//") || framed.contains("/./") || framed.contains("/../") {
        return Err("has an empty, `.` or `..` path component");
    }
    Ok(())
}
