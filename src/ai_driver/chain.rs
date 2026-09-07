//! The declared routing chain, and the dispatch that walks it.
//!
//! # Why this exists
//!
//! `config/model-routing.toml` is the authority on which model serves which
//! stage, and this module is the only way a stage gets a turn. What the file
//! declares is what dispatches, or the load fails loudly -- there is no second
//! table, and no site picks a provider for itself.
//!
//! A stage names the WORK; the file names the model. A site that chooses a
//! provider has one provider and no tier beneath it.
//!
//! No provider-level circuit breaker yet: a tier that is down is retried next
//! turn and pays its timeout again. Per-account cooldown in `AiRouter` is the
//! only memory of a failure.

use crate::ai_driver::provider::ModelProvider;
use crate::exec::Posture;
use crate::model_prompt::ModelPrompt;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

/// The routing table, compiled in.
///
/// `include_str!` rather than a runtime path: the binary and the tests read one
/// string, and a deployment cannot end up with a config the build never saw.
const DECLARED: &str = include_str!("../../config/model-routing.toml");

/// A stage of the delivery pipeline.
///
/// The enum is the authority on which stages exist. A chain in the file naming
/// a stage absent here is a load error, and a stage here with no chain in the
/// file is too -- a typo in either direction is absent evidence, not a stage
/// that silently never dispatches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    Recon,
    Planning,
    PlanReview,
    ArchitectSpec,
    SpecReview,
    Implementation,
    Falsification,
    CodeReviewAudit,
    SecurityAudit,
    TestHardening,
    TestAudit,
    Remediation,
    Orchestration,
    IssueTriage,
}

impl Stage {
    /// Every stage, so the loader can check the file covers all of them.
    pub const ALL: &'static [Stage] = &[
        Stage::Recon,
        Stage::Planning,
        Stage::PlanReview,
        Stage::ArchitectSpec,
        Stage::SpecReview,
        Stage::Implementation,
        Stage::Falsification,
        Stage::CodeReviewAudit,
        Stage::SecurityAudit,
        Stage::TestHardening,
        Stage::TestAudit,
        Stage::Remediation,
        Stage::Orchestration,
        Stage::IssueTriage,
    ];

    /// The key this stage carries in the file.
    pub fn key(self) -> &'static str {
        match self {
            Stage::Recon => "recon",
            Stage::Planning => "planning",
            Stage::PlanReview => "plan_review",
            Stage::ArchitectSpec => "architect_spec",
            Stage::SpecReview => "spec_review",
            Stage::Implementation => "implementation",
            Stage::Falsification => "falsification",
            Stage::CodeReviewAudit => "code_review_audit",
            Stage::SecurityAudit => "security_audit",
            Stage::TestHardening => "test_hardening",
            Stage::TestAudit => "test_audit",
            Stage::Remediation => "remediation",
            Stage::Orchestration => "orchestration",
            Stage::IssueTriage => "issue_triage",
        }
    }
}

/// One tier of a stage's chain, exactly as the file declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tier {
    pub provider: ModelProvider,
    pub model: String,
    pub effort: String,
    pub timeout: Duration,
}

#[derive(Debug, Deserialize)]
struct RawTier {
    model: String,
    provider: String,
    effort: String,
    timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
struct RawFile {
    #[serde(default)]
    stage: BTreeMap<String, Vec<RawTier>>,
    /// Prose about each stage. Accepted and ignored, rather than left to
    /// `deny_unknown_fields` to reject as an unknown key.
    #[serde(default)]
    #[allow(dead_code)]
    stage_meta: Option<toml::Value>,
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
pub fn parse(text: &str) -> Result<BTreeMap<Stage, Vec<Tier>>> {
    let raw: RawFile = toml::from_str(text).context("config/model-routing.toml does not parse")?;
    let mut out: BTreeMap<Stage, Vec<Tier>> = BTreeMap::new();

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
        out.insert(stage, built);
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

fn table() -> &'static BTreeMap<Stage, Vec<Tier>> {
    static TABLE: OnceLock<BTreeMap<Stage, Vec<Tier>>> = OnceLock::new();
    TABLE.get_or_init(|| {
        parse(DECLARED).unwrap_or_else(|e| {
            panic!("the compiled-in routing table is invalid, so no stage can dispatch: {e}")
        })
    })
}

/// The declared chain for `stage`, primary first.
pub fn chain(stage: Stage) -> &'static [Tier] {
    table()
        .get(&stage)
        .map(Vec::as_slice)
        .expect("every Stage has a chain; the loader refuses a table where one does not")
}

/// Build the tier's command. The typed constructors are the only spawn seam.
///
/// Public so a test can put every declared tier through real argv validation
/// without spawning anything.
pub fn command_for(
    tier: &Tier,
    posture: &Posture,
    budget: Duration,
) -> Result<crate::exec::AgentCommand> {
    let model = tier.model.as_str();
    match tier.provider {
        ModelProvider::AnthropicClaudeCode | ModelProvider::SubscriptionEnsemble => {
            crate::exec::claude_agent(posture, model)
        }
        ModelProvider::OpenAiCodex => crate::exec::codex_agent(posture, model),
        ModelProvider::CursorAgent => crate::exec::cursor_agent(posture, Some(model)),
        ModelProvider::XAiGrok => crate::exec::grok_agent(posture, model),
        ModelProvider::Antigravity => {
            crate::exec::agy_agent(posture, &tier.effort, budget, Some(model))
        }
        ModelProvider::Muse => crate::exec::muse_agent(posture, model, &tier.effort),
    }
}

/// Run `stage` against the declared chain, taking the first tier that answers.
///
/// Every production site goes through this, so a stage names *what it is doing*
/// and the file decides which model does it. An exhausted chain is an error
/// naming every tier tried -- never an empty string, which reads downstream as
/// a model that answered and had nothing to say.
pub async fn run_stage(
    stage: Stage,
    prompt: &ModelPrompt,
    working_dir: &Path,
    what: &str,
) -> Result<String> {
    run_stage_within(stage, prompt, working_dir, what, None).await
}

/// [`run_stage`], with each tier's timeout capped at `budget`.
///
/// A caller already under a supervisor keeps its own bound: a tier declaring
/// 600s must not outlive a watchdog that gives the whole probe less.
pub async fn run_stage_within(
    stage: Stage,
    prompt: &ModelPrompt,
    working_dir: &Path,
    what: &str,
    budget: Option<Duration>,
) -> Result<String> {
    let posture = Posture::in_workspace(working_dir);
    let mut refusals = Vec::new();

    for (i, tier) in chain(stage).iter().enumerate() {
        let label = format!("{what} [{}/{} {}]", i + 1, chain(stage).len(), tier.model);
        let timeout = budget.map_or(tier.timeout, |b| b.min(tier.timeout));
        let cmd = match command_for(tier, &posture, timeout) {
            Ok(c) => c,
            Err(e) => {
                refusals.push(format!("{}: could not be built ({e})", tier.model));
                continue;
            }
        };
        match crate::exec::turn::run(cmd, prompt, timeout, &label).await {
            Ok(turn) if turn.status.success() => match turn.into_result() {
                Ok(text) if !text.trim().is_empty() => return Ok(text),
                Ok(_) => refusals.push(format!("{}: answered with nothing", tier.model)),
                Err(e) => refusals.push(format!("{}: {e}", tier.model)),
            },
            Ok(turn) => refusals.push(format!("{}: exited {}", tier.model, turn.status)),
            Err(e) => refusals.push(format!("{}: {e}", tier.model)),
        }
    }

    bail!(
        "stage `{}` exhausted every declared tier, so nothing answered:\n  {}",
        stage.key(),
        refusals.join("\n  ")
    )
}
