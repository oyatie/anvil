//! The boundary between `ai_driver`'s novel half and its executor half.
//!
//! `ai_driver` carries two fates. The migration ledger (`src/migration/registry.rs`,
//! entry `"ai_driver"`) records `Rewired`: the executor half — `router.rs`, which
//! spawns vendor CLI subprocesses, and the model routing around it — has an oyatie
//! counterpart and is superseded, while the task classifier, the routing bandit and
//! the cross-model validator have none and must survive absorption.
//!
//! Nothing in this module spawns, leases an account, or names a vendor. It is the
//! only thing the novel half is allowed to know about execution, so the novel half
//! compiles and runs against a test double with no subscription, no network and no
//! CLI on PATH — and today's CLI-spawning adapter can be swapped for an
//! oyatie-backed one without touching a single caller.
//!
//! Two ports rather than one, because the two consumers genuinely need different
//! things: the novel half needs "run this prompt against the model called `x`" and
//! has no business knowing which vendor CLI that resolves to, whereas the stage
//! fallback chain hands down a fully resolved [`ModelExecutionConfig`] — provider,
//! reasoning effort and timeout included — and collapsing that to a bare model name
//! would silently drop the effort and timeout budgets it just computed.

use anyhow::Result;
use std::path::Path;

use super::provider::ModelExecutionConfig;

/// Runs a prompt against an already-resolved execution configuration.
///
/// Used by the per-stage fallback chain, which has already decided the provider,
/// the specific model, the reasoning effort and the timeout for each tier and needs
/// all four to reach the adapter intact.
// `async fn` in traits is stable, but not dyn-compatible. `stage_router` holds
// this as `Arc<dyn ConfiguredPromptExecutor>` so the fallback chain can carry a
// swappable adapter, which is the whole point of the port. The macro stays here
// and only here: `PromptExecutor` is used generically and needs no erasure.
#[async_trait::async_trait]
pub trait ConfiguredPromptExecutor: Send + Sync {
    /// Runs `prompt` under `config`, with `working_dir` as the working directory,
    /// and returns the model's response text.
    async fn execute_configured(
        &self,
        prompt: &str,
        working_dir: &Path,
        config: &ModelExecutionConfig,
    ) -> Result<String>;
}
