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
use anyhow::{Result, bail};
pub use budget::{MIN_TIER_ALLOTMENT, StageBudget};
pub use order::runs_after_transitively;
pub use run_scope::RunScope;

/// Parse an arbitrary table, so a test can exercise the loader's refusals
/// without the compiled-in one.
pub fn parse_table_for_test(text: &str) -> Result<std::collections::BTreeMap<Stage, StagePlan>> {
    table::parse_table(text)
}
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;
use table::parse_table;

pub mod budget;
mod order;
mod run_scope;
mod scope_grammar;
mod table;

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
    /// Tests written from the spec, BEFORE the implementation exists.
    TestAuthoring,
    /// Adversarial review of those tests, before any code can satisfy them.
    TestAuthoringReview,
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
        Stage::TestAuthoring,
        Stage::TestAuthoringReview,
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
            Stage::TestAuthoring => "test_authoring",
            Stage::TestAuthoringReview => "test_authoring_review",
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

/// What a stage may write, and with which models.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagePlan {
    pub tiers: Vec<Tier>,
    /// The stage this one judges, if any.
    pub audits: Option<String>,
    /// Path prefixes this stage may stage for commit; empty means none.
    pub writes: Vec<String>,
    /// Stages this one requires to have already run, as declared.
    ///
    /// Order WITHOUT judgement; `audits` carries the rest. See
    /// [`runs_after_transitively`] for the closure, which is what a caller
    /// asking "is this stage downstream of that one" actually wants.
    pub runs_after: Vec<String>,
}

/// One tier of a stage's chain, exactly as the file declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tier {
    pub provider: ModelProvider,
    pub model: String,
    pub effort: String,
    pub timeout: Duration,
}

fn table() -> &'static BTreeMap<Stage, StagePlan> {
    static TABLE: OnceLock<BTreeMap<Stage, StagePlan>> = OnceLock::new();
    TABLE.get_or_init(|| {
        parse_table(DECLARED).unwrap_or_else(|e| {
            panic!("the compiled-in routing table is invalid, so no stage can dispatch: {e}")
        })
    })
}

/// The declared chain for `stage`, primary first.
pub fn chain(stage: Stage) -> &'static [Tier] {
    &plan(stage).tiers
}

/// The declared plan for `stage`: its chain and what it may write.
pub fn plan(stage: Stage) -> &'static StagePlan {
    table()
        .get(&stage)
        .expect("every Stage has a plan; the loader refuses a table where one does not")
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

/// [`run_stage`], bounded so the WHOLE stage fits inside `budget`.
///
/// A caller already under a supervisor keeps its own bound, and the bound is on
/// the stage rather than on each attempt: five tiers declaring 600s apiece
/// under a 600s watchdog get 600s between them, not 3000s. [`StageBudget`]
/// carries the measurement behind that.
///
/// # No run scope is declared here
///
/// A scope held for the length of a turn is released when the turn ends, and no
/// production turn commits -- every prompt says "Do NOT commit; leave your
/// changes in the working tree." `queue_healer` stages and commits 125 lines
/// AFTER its turn returns, so a scope held here is gone by the time the hook
/// runs and the guardrail cannot fire.
///
/// The scope belongs to the OPERATION that commits: its caller declares it and
/// holds it across dispatch, staging and commit. See [`RunScope::declare`].
pub async fn run_stage_within(
    stage: Stage,
    prompt: &ModelPrompt,
    working_dir: &Path,
    what: &str,
    budget: Option<Duration>,
) -> Result<String> {
    let posture = Posture::in_workspace(working_dir);
    let mut refusals = Vec::new();
    let mut left = StageBudget::of(budget);
    let mut stopped_by_budget = false;

    for (i, tier) in chain(stage).iter().enumerate() {
        let label = format!("{what} [{}/{} {}]", i + 1, chain(stage).len(), tier.model);
        // `None` means the rest of the chain was never reached, which is a
        // different fact from a tier that ran and refused. I1 forbids
        // collapsing the two.
        let Some(timeout) = left.allot(tier.timeout) else {
            for untried in &chain(stage)[i..] {
                refusals.push(format!(
                    "{}: not tried, the stage budget was spent first",
                    untried.model
                ));
            }
            stopped_by_budget = true;
            break;
        };
        let cmd = match command_for(tier, &posture, timeout) {
            Ok(c) => c,
            Err(e) => {
                refusals.push(format!("{}: could not be built ({e})", tier.model));
                continue;
            }
        };
        let started = std::time::Instant::now();
        let outcome = crate::exec::turn::run(cmd, prompt, timeout, &label).await;
        left.spend(started.elapsed());
        match outcome {
            Ok(turn) if turn.status.success() => match turn.into_result() {
                Ok(text) if !text.trim().is_empty() => return Ok(text),
                Ok(_) => refusals.push(format!("{}: answered with nothing", tier.model)),
                Err(e) => refusals.push(format!("{}: {e}", tier.model)),
            },
            Ok(turn) => refusals.push(format!("{}: exited {}", tier.model, turn.status)),
            Err(e) => refusals.push(format!("{}: {e}", tier.model)),
        }
    }

    // "Exhausted" and "ran out of time" are different facts, and a caller that
    // reads the first when the second happened will retry the same chain under
    // the same bound. Untried tiers are not exhausted tiers.
    if stopped_by_budget {
        bail!(
            "stage `{}` ran out of the {:?} its caller allowed before a tier \
             answered, so the rest of the chain was never reached:\n  {}",
            stage.key(),
            budget.unwrap_or_default(),
            refusals.join("\n  ")
        )
    }
    bail!(
        "stage `{}` exhausted every declared tier, so nothing answered:\n  {}",
        stage.key(),
        refusals.join("\n  ")
    )
}

/// Declare a stage's scope the way `run_stage_within` does.
///
/// Exposed so a test can exercise the PRODUCER rather than fabricate its
/// output: every existing run-scope test writes the file itself, which is how
/// the guardrail merged inert.
pub fn declare_run_scope_for_test(working_dir: &Path, stage: Stage) -> Result<impl Sized> {
    RunScope::declare(working_dir, &plan(stage).writes)
}
