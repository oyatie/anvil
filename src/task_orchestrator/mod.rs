pub mod autonomous_fix_engine;
pub mod default_layout;
pub mod delivery_board;
pub mod delivery_role;
pub mod intake;
pub mod parallel_layer;
pub mod path_occupancy;
pub mod role_graph;
pub mod source_doc_verifier;
pub mod task_dag_sequencer;

pub use autonomous_fix_engine::{AutonomousFixEngine, TaskExecutionReport};
pub use default_layout::{
    cap_child_ok, enforce_on_repo, layout_violations, ALLOWED_ROOT_DIRS, CAP_CHILDREN, FACES,
    FORBIDDEN_NAMES,
};
pub use delivery_board::{ClaimedHop, DeliveryBoard, ReadyHop, ReadySnapshot, SliceState};
pub use delivery_role::{DeliveryRole, HandoffAgent};
pub use intake::{interview, ArtifactPackage, IntakeVerdict, InterviewDraft};
pub use parallel_layer::run_layer_parallel;
pub use path_occupancy::{assert_layer_paths_disjoint, occupy_move, path_sets_disjoint};
pub use role_graph::{fan_out_after_implement, is_unblocked, transitive_deps};
pub use source_doc_verifier::{ScopedTaskDefinition, SourceDocVerifier, VerificationFinding};
pub use task_dag_sequencer::{SequencedTaskBatch, TaskDagSequencer};

use anyhow::Result;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use tracing::info;

/// Publishes ready hops. Does not spawn implementers.
pub struct AutonomousTaskOrchestrator {
    pub verifier: Arc<SourceDocVerifier>,
}

impl AutonomousTaskOrchestrator {
    pub fn new(verifier: Arc<SourceDocVerifier>) -> Self {
        Self { verifier }
    }

    pub fn ingest_interview(&self, draft: &InterviewDraft, repo_dir: &Path) -> IntakeVerdict {
        interview(draft, repo_dir)
    }

    pub fn admit_package(
        &self,
        board: &mut DeliveryBoard,
        package: &ArtifactPackage,
        repo: &str,
    ) -> Result<()> {
        crate::task_orchestrator::intake::package_must_not_land_in_product_dump(package)?;
        board.admit_slice(
            package.package_id.clone(),
            package.target_paths.clone(),
            package.handoff,
            repo,
            BTreeSet::new(),
        )
    }

    /// ADR-derived slices that already passed SSOT verification.
    pub fn sweep_and_publish(&self, repo: &str, repo_dir: &Path) -> Result<ReadySnapshot> {
        info!("publishing ready hops for '{repo}' (no agent spawn)");
        let raw_tasks = self.verifier.scan_adrs_for_work(repo_dir)?;
        let mut board = DeliveryBoard::new();
        for task in raw_tasks {
            let finding = self.verifier.verify_scoped_task(&task, repo_dir)?;
            if !finding.is_valid {
                continue;
            }
            let _ = board.admit_slice(
                task.task_id,
                task.target_files,
                HandoffAgent::Program,
                repo,
                BTreeSet::new(),
            );
        }
        Ok(board.snapshot(repo))
    }
}
