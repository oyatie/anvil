//! The compile-level half of the `ai_driver` boundary.
//!
//! DEFECT UNDER TEST
//! -----------------
//! `CrossModelDualValidator` stores a concrete `SubscriptionExecutor`
//! (`src/ai_driver/cross_model_validator.rs:21`) and builds one in its own
//! constructor (`:27`). Everything that makes the type novel -- the agreement
//! arithmetic, the discrepancy record, the fail-closed branch when one peer model
//! dies -- is therefore reachable only by spawning `claude` and `codex` as child
//! processes with a leased subscription account.
//!
//! The visible consequence, in this tree today: `verify_cross_model_consensus`
//! has no test and no caller. `grep -rn verify_cross_model_consensus src/ tests/`
//! returns one hit, its own definition. The file's single unit test constructs a
//! `CrossModelConsensusReport` literal by hand and asserts that the literal holds
//! the values it was just given. 190 lines of novel logic, zero lines covered --
//! because covering them would mean running two vendor CLIs.
//!
//! WHY THIS FILE IS THE GUARANTEE AND THE SOURCE SCAN IS NOT
//! --------------------------------------------------------
//! `tests/ai_driver_novel_boundary_test.rs` greps for the forbidden identifiers.
//! That is a check someone can satisfy and then quietly regress, and it can only
//! ever say "the name is absent" -- never "the dependency is impossible".
//!
//! This file says the second thing. It constructs the validator over
//! `ScriptedExecutor`, a test double defined here, and drives the consensus logic
//! to a verdict. For that to compile, the validator must be parameterised by a
//! port; and a type parameter bounded by a port has no syntax in which to name
//! `SubscriptionExecutor`, reach the account pool, or spawn a process. The
//! dependency is not forbidden by a rule, it is unwriteable. Nothing here can be
//! satisfied by editing an import line.
//!
//! THE PORT THESE TESTS REQUIRE
//! ----------------------------
//! ```ignore
//! // src/ai_driver/executor_port.rs -- re-exported from src/ai_driver/mod.rs
//! #[async_trait::async_trait]
//! pub trait PromptExecutor: Send + Sync {
//!     /// Runs `prompt` against the model named `model` in `working_dir`.
//!     async fn execute(&self, model: &str, prompt: &str, working_dir: &Path)
//!         -> anyhow::Result<String>;
//! }
//! ```
//! The model is an opaque name the *adapter* resolves, not a `ModelProvider`
//! variant: `AnthropicClaudeCode` and `OpenAiCodex` are the vocabulary of which
//! vendor CLI to spawn, and the novel side has no business knowing it. Which two
//! models to compare becomes a constructor argument, so the validator stops
//! hardcoding a duel between two specific subscriptions.
//!
//! `SubscriptionExecutor` implements this trait -- that is the `Rewired` shape the
//! migration ledger records for `ai_driver`: the port survives absorption, and
//! today's CLI-spawning adapter is swapped for an oyatie-backed one behind it.
//! Nothing about the executor is deleted here; it stays live and running.
//!
//! WHY PROMPTING WOULD NOT PREVENT THIS
//! ------------------------------------
//! The coupling conceals its own cost. Because the validator cannot be
//! constructed without a real executor, no one writes the test that would expose
//! it -- and an untested file with no failing test reads as finished, not as
//! untestable. "Make it injectable" is also the exact instruction that gets
//! deferred under delivery pressure, because the code already works when a human
//! runs it by hand. A compile-level bound is the only form of the instruction
//! that survives the prompt leaving the context window.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anvil::ai_driver::{CrossModelDualValidator, PromptExecutor};

/// Shared record of which models the validator actually asked for.
type CallLog = Arc<Mutex<Vec<String>>>;

/// A `PromptExecutor` that returns canned text and records what it was asked.
///
/// It spawns nothing. If this type can stand in for the executor, the novel half
/// is testable without a subscription, a network, or a vendor CLI on PATH.
struct ScriptedExecutor {
    /// model name -> Ok(text) or Err(message)
    script: HashMap<String, Result<String, String>>,
    calls: CallLog,
}

impl ScriptedExecutor {
    fn new(script: &[(&str, Result<&str, &str>)]) -> Self {
        Self::with_log(script, CallLog::default())
    }

    fn with_log(script: &[(&str, Result<&str, &str>)], calls: CallLog) -> Self {
        Self {
            script: script
                .iter()
                .map(|(m, r)| {
                    (
                        (*m).to_string(),
                        match r {
                            Ok(t) => Ok((*t).to_string()),
                            Err(e) => Err((*e).to_string()),
                        },
                    )
                })
                .collect(),
            calls,
        }
    }
}

#[async_trait::async_trait]
impl PromptExecutor for ScriptedExecutor {
    async fn execute(
        &self,
        model: &str,
        _prompt: &str,
        _working_dir: &Path,
    ) -> anyhow::Result<String> {
        self.calls
            .lock()
            .expect("call log is not poisoned")
            .push(model.to_string());
        match self.script.get(model) {
            Some(Ok(text)) => Ok(text.clone()),
            Some(Err(msg)) => anyhow::bail!("{msg}"),
            None => anyhow::bail!("ScriptedExecutor has no script for model `{model}`"),
        }
    }
}

const MODEL_A: &str = "model-a";
const MODEL_B: &str = "model-b";

fn validator(script: &[(&str, Result<&str, &str>)]) -> CrossModelDualValidator<ScriptedExecutor> {
    CrossModelDualValidator::new(ScriptedExecutor::new(script), MODEL_A, MODEL_B)
}

/// The bound that makes the dependency unwriteable.
///
/// If `CrossModelDualValidator`'s parameter were ever widened back to a concrete
/// type, or the port dropped, this stops compiling -- which is the point: the
/// failure is a build error, not a red assertion someone can delete.
fn requires_port<E: PromptExecutor>() {}

#[test]
fn the_validator_is_parameterised_by_a_port_not_by_a_cli_spawner() {
    requires_port::<ScriptedExecutor>();
}

/// Both peers approve -> consensus.
///
/// The scripted double never touches a process, so this test passing is itself
/// the evidence that the consensus arithmetic runs without a subscription.
/// (That the port is the thing actually consulted -- rather than an executor the
/// validator quietly builds for itself -- is pinned by
/// `the_compared_models_are_the_ones_the_caller_named` below.)
#[tokio::test]
async fn agreeing_peers_reach_consensus_without_spawning_anything() {
    let v = validator(&[
        (MODEL_A, Ok("LGTM, invariants hold.")),
        (MODEL_B, Ok("Approved, no safety concerns.")),
    ]);

    let report = v
        .verify_cross_model_consensus("review this diff", Path::new("."))
        .await
        .expect("both peers answered, so a report is produced");

    assert!(report.is_consensus_reached, "{report:?}");
    assert_eq!(report.agreement_score, 1.0, "{report:?}");
    assert!(report.identified_discrepancies.is_empty(), "{report:?}");
}

/// Conflicting verdicts must block consensus and be recorded, not averaged away.
///
/// This is the branch that justifies the component existing at all, and it is the
/// branch that has never executed.
#[tokio::test]
async fn conflicting_verdicts_block_consensus_and_record_the_divergence() {
    let v = validator(&[
        (MODEL_A, Ok("Approved.")),
        (
            MODEL_B,
            Ok("REQUEST_CHANGES: unchecked unwrap on the auth path."),
        ),
    ]);

    let report = v
        .verify_cross_model_consensus("review this diff", Path::new("."))
        .await
        .expect("both peers answered, so a report is produced");

    assert!(!report.is_consensus_reached, "{report:?}");
    assert!(report.agreement_score < 1.0, "{report:?}");
    assert!(
        !report.identified_discrepancies.is_empty(),
        "a divergence that leaves no discrepancy behind is indistinguishable from agreement \
         to every downstream reader: {report:?}"
    );
}

/// One peer dies, the survivor rejects -> fail closed.
///
/// A dual-verification gate whose safety depends on both peers answering must not
/// approve when it has half the evidence and that half says no.
#[tokio::test]
async fn a_dead_peer_and_a_rejecting_survivor_fails_closed() {
    let v = validator(&[
        (MODEL_A, Ok("VIOLATION: secret written to logs.")),
        (MODEL_B, Err("subscription CLI exited 1")),
    ]);

    let report = v
        .verify_cross_model_consensus("review this diff", Path::new("."))
        .await
        .expect("one peer answered, so a fail-closed report is produced");

    assert!(
        !report.is_consensus_reached,
        "the only model that answered rejected the change; approving on that evidence is the \
         failure this gate exists to prevent: {report:?}"
    );
    assert!(!report.identified_discrepancies.is_empty(), "{report:?}");
}

/// Both peers die -> an error, never a manufactured approval.
#[tokio::test]
async fn two_dead_peers_produce_an_error_not_a_verdict() {
    let v = validator(&[
        (MODEL_A, Err("subscription CLI not found")),
        (MODEL_B, Err("subscription CLI not found")),
    ]);

    let result = v
        .verify_cross_model_consensus("review this diff", Path::new("."))
        .await;

    assert!(
        result.is_err(),
        "with no evidence at all the gate must refuse to produce a verdict, not synthesise \
         one: {result:?}"
    );
}

/// The validator asks the port for exactly the two models it was given.
///
/// Pins that which models are compared is now the caller's decision rather than a
/// hardcoded duel between two named subscriptions.
#[tokio::test]
async fn the_compared_models_are_the_ones_the_caller_named() {
    let log = CallLog::default();
    let executor =
        ScriptedExecutor::with_log(&[(MODEL_A, Ok("ok")), (MODEL_B, Ok("ok"))], log.clone());
    let v = CrossModelDualValidator::new(executor, MODEL_A, MODEL_B);

    let _ = v
        .verify_cross_model_consensus("review this diff", Path::new("."))
        .await;

    let mut asked = log.lock().expect("call log is not poisoned").clone();
    asked.sort();
    assert_eq!(asked, vec![MODEL_A.to_string(), MODEL_B.to_string()]);
}
