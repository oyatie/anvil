pub mod autonomous_fix_engine;
pub mod source_doc_verifier;
pub mod task_dag_sequencer;

pub use autonomous_fix_engine::{AutonomousFixEngine, TaskExecutionReport};
pub use source_doc_verifier::{ScopedTaskDefinition, SourceDocVerifier, VerificationFinding};
pub use task_dag_sequencer::{SequencedTaskBatch, TaskDagSequencer};

use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use tracing::info;

pub struct AutonomousTaskOrchestrator {
    pub verifier: Arc<SourceDocVerifier>,
    pub sequencer: Arc<TaskDagSequencer>,
    pub fix_engine: Arc<AutonomousFixEngine>,
}

impl AutonomousTaskOrchestrator {
    pub fn new(
        verifier: Arc<SourceDocVerifier>,
        sequencer: Arc<TaskDagSequencer>,
        fix_engine: Arc<AutonomousFixEngine>,
    ) -> Self {
        Self {
            verifier,
            sequencer,
            fix_engine,
        }
    }

    /// Discovers scoped ADR work, validates against SSOT truth, sequences topologically, and executes through multi-model loop
    pub async fn sweep_and_execute_adrs(
        &self,
        repo: &str,
        repo_dir: &Path,
    ) -> Result<Vec<TaskExecutionReport>> {
        info!(
            "🚀 [Task Orchestrator] Ingesting and sequencing scoped work for '{}'...",
            repo
        );

        // 1. Scan ADRs
        let raw_tasks = self.verifier.scan_adrs_for_work(repo_dir)?;
        let mut verified_tasks = Vec::new();

        // 2. Verify truth and anti-staleness
        for task in raw_tasks {
            let finding = self.verifier.verify_scoped_task(&task, repo_dir)?;
            if finding.is_valid {
                verified_tasks.push(task);
            }
        }

        // 3. Topological sequencing
        let stages = self.sequencer.sequence_tasks(verified_tasks)?;
        let mut reports = Vec::new();

        let base_branch = if repo.contains("oyatie") { "dev" } else { "main" };

        // 4. Staged autonomous execution
        for stage in stages {
            info!(
                "⚡ [Task Orchestrator] Executing Stage {} with {} tasks...",
                stage.stage_index,
                stage.tasks.len()
            );
            for task in stage.tasks {
                let rep = self.fix_engine.execute_task(repo, &task, base_branch).await?;
                reports.push(rep);
            }
        }

        Ok(reports)
    }
}
