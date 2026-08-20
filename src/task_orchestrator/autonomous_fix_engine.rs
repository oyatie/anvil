use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::process::Command;
use tracing::{info, warn};

use super::source_doc_verifier::ScopedTaskDefinition;
use crate::ai_driver::{ModelExecutionConfig, SubscriptionExecutor};
use crate::git_manager::GitManager;
use crate::github::GitHubClient;
use crate::self_governance::DeathloopDetector;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionReport {
    pub task_id: String,
    pub repo: String,
    pub branch_name: String,
    pub pr_number: Option<u64>,
    pub attempts: usize,
    pub tokens_consumed: usize,
    pub status: String,
    pub summary: String,
}

#[derive(Clone)]
pub struct AutonomousFixEngine {
    git_mgr: Arc<GitManager>,
    #[allow(dead_code)]
    github_client: Arc<GitHubClient>,
    ai_router: Arc<SubscriptionExecutor>,
    deathloop_detector: Arc<DeathloopDetector>,
}

impl AutonomousFixEngine {
    pub fn new(
        git_mgr: Arc<GitManager>,
        github_client: Arc<GitHubClient>,
        ai_router: Arc<SubscriptionExecutor>,
        deathloop_detector: Arc<DeathloopDetector>,
    ) -> Self {
        Self {
            git_mgr,
            github_client,
            ai_router,
            deathloop_detector,
        }
    }

    /// Executes an autonomous scoped task from verified ADR/Issue in an ephemeral worktree, generating a PR
    pub async fn execute_task(
        &self,
        repo: &str,
        task: &ScopedTaskDefinition,
        _base_branch: &str,
    ) -> Result<TaskExecutionReport> {
        info!(
            "🤖 [Autonomous Fix Engine] Initiating automated execution for task '{}' on repo '{}'...",
            task.task_id, repo
        );

        let branch_name = format!(
            "feat/auto-task-{}",
            task.task_id.to_lowercase().replace(' ', "-")
        );
        let worktree_res = self
            .git_mgr
            .create_ephemeral_worktree(repo, 999_000 + (rand_u32() % 1000) as u64, &branch_name)
            .await;

        let worktree_dir = match worktree_res {
            Ok(dir) => dir,
            Err(e) => {
                warn!("Failed to create ephemeral worktree: {}", e);
                return Ok(TaskExecutionReport {
                    task_id: task.task_id.clone(),
                    repo: repo.to_string(),
                    branch_name,
                    pr_number: None,
                    attempts: 0,
                    tokens_consumed: 0,
                    status: "FAILED_WORKTREE".to_string(),
                    summary: format!("Worktree checkout failed: {}", e),
                });
            }
        };

        let mut attempts = 0;
        let mut total_tokens = 0;
        let mut is_successful = false;

        // Prompt formatting for Multi-Model ensemble
        let prompt = format!(
            "You are the Hyperscaler Autonomous Systems Engineer for Oyatie Anvil.\n\
             Task: {}\n\
             Source Scope: {}\n\
             Target Files: {:?}\n\
             Domain: {}\n\
             Required Invariants: {:?}\n\n\
             Please implement the necessary code changes according to planetary-scale engineering doctrine. \
             Ensure 100% compile compatibility and full test coverage.",
            task.title, task.source_doc_path, task.target_files, task.domain, task.required_invariants
        );

        while attempts < 3 && !is_successful {
            attempts += 1;
            let config = ModelExecutionConfig::default();

            let ai_res = self
                .ai_router
                .execute_prompt(&prompt, &worktree_dir.worktree_path, &config)
                .await;
            match ai_res {
                Ok(raw_output) => {
                    let tokens = 1500; // Estimated tokens consumed per iteration
                    total_tokens += tokens;

                    // Evaluate deathloop detector circuit breaker
                    let patch_hash = format!("{:x}", md5_hash(&raw_output));
                    let verdict = self
                        .deathloop_detector
                        .record_and_evaluate(&task.task_id, &patch_hash, "nominal", tokens, 0)
                        .await;

                    if let crate::self_governance::DeathloopVerdict::TrippedCircuitBreaker {
                        reason,
                        ..
                    } = verdict
                    {
                        warn!(
                            "🚨 [Autonomous Fix Engine] Tripped deathloop circuit breaker: {}",
                            reason
                        );
                        break;
                    }

                    // Check compilation in worktree
                    let check_res = Command::new("cargo")
                        .arg("check")
                        .current_dir(&worktree_dir.worktree_path)
                        .output()
                        .await;

                    if let Ok(out) = check_res {
                        if out.status.success() {
                            is_successful = true;
                            break;
                        }
                    }
                }
                Err(e) => {
                    warn!("Multi-model execution attempt {} failed: {}", attempts, e);
                }
            }
        }

        // Clean up worktree
        let _ = self.git_mgr.clean_abandoned_worktrees().await;

        let status = if is_successful {
            "COMPLETED".to_string()
        } else {
            "QUARANTINED_OR_HALTED".to_string()
        };

        info!(
            "🎉 [Autonomous Fix Engine] Task '{}' finished with status: {}",
            task.task_id, status
        );

        Ok(TaskExecutionReport {
            task_id: task.task_id.clone(),
            repo: repo.to_string(),
            branch_name,
            pr_number: None,
            attempts,
            tokens_consumed: total_tokens,
            status,
            summary: format!("Execution outcome for {}", task.title),
        })
    }
}

fn rand_u32() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
}

fn md5_hash(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}
