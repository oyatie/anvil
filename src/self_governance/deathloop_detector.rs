use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeathloopVerdict {
    Nominal,
    Warning(String),
    TrippedCircuitBreaker {
        reason: String,
        attempts: usize,
        tokens_drained: usize,
        quarantine_action: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptRecord {
    pub timestamp: DateTime<Utc>,
    pub patch_hash: String,
    pub failure_signature: String,
    pub tokens_consumed: usize,
    pub gates_failed_count: usize,
}

#[derive(Debug, Clone)]
pub struct TaskExecutionHistory {
    pub task_id: String,
    pub attempts: VecDeque<AttemptRecord>,
    pub total_tokens_drained: usize,
    pub is_quarantined: bool,
}

#[derive(Debug, Clone)]
pub struct DeathloopDetector {
    max_identical_patch_attempts: usize,
    max_consecutive_zero_progress_attempts: usize,
    max_task_token_budget: usize,
    task_histories: Arc<RwLock<HashMap<String, TaskExecutionHistory>>>,
}

impl Default for DeathloopDetector {
    fn default() -> Self {
        Self::new(3, 3, 500_000)
    }
}

impl DeathloopDetector {
    pub fn new(
        max_identical_patch_attempts: usize,
        max_consecutive_zero_progress_attempts: usize,
        max_task_token_budget: usize,
    ) -> Self {
        Self {
            max_identical_patch_attempts,
            max_consecutive_zero_progress_attempts,
            max_task_token_budget,
            task_histories: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Records an execution attempt and evaluates whether a deathloop circuit breaker should trip
    pub async fn record_and_evaluate(
        &self,
        task_id: &str,
        patch_hash: &str,
        failure_signature: &str,
        tokens_consumed: usize,
        gates_failed_count: usize,
    ) -> DeathloopVerdict {
        let mut guard = self.task_histories.write().await;
        let history = guard
            .entry(task_id.to_string())
            .or_insert_with(|| TaskExecutionHistory {
                task_id: task_id.to_string(),
                attempts: VecDeque::new(),
                total_tokens_drained: 0,
                is_quarantined: false,
            });

        if history.is_quarantined {
            return DeathloopVerdict::TrippedCircuitBreaker {
                reason: format!("Task '{}' is already quarantined in halted state", task_id),
                attempts: history.attempts.len(),
                tokens_drained: history.total_tokens_drained,
                quarantine_action: "HALT_EXECUTION_AUTO_TRIAGE".to_string(),
            };
        }

        history.total_tokens_drained += tokens_consumed;
        let attempt = AttemptRecord {
            timestamp: Utc::now(),
            patch_hash: patch_hash.to_string(),
            failure_signature: failure_signature.to_string(),
            tokens_consumed,
            gates_failed_count,
        };
        history.attempts.push_back(attempt);

        // Keep last 10 attempts
        if history.attempts.len() > 10 {
            history.attempts.pop_front();
        }

        let attempts_count = history.attempts.len();

        // 1. Invariant: Excessive Token Drain Budget Ceiling
        if history.total_tokens_drained >= self.max_task_token_budget {
            history.is_quarantined = true;
            error!(
                "🚨 [Deathloop Circuit Breaker] Task '{}' breached token budget ceiling ({} > {} tokens). Tripping circuit breaker!",
                task_id, history.total_tokens_drained, self.max_task_token_budget
            );
            return DeathloopVerdict::TrippedCircuitBreaker {
                reason: format!(
                    "Token budget ceiling breached: drained {} tokens without achieving green gates",
                    history.total_tokens_drained
                ),
                attempts: attempts_count,
                tokens_drained: history.total_tokens_drained,
                quarantine_action: "QUARANTINE_PR_HALT_REPAIRS".to_string(),
            };
        }

        // 2. Invariant: Repetitive Identical Patch Flapping
        if attempts_count >= self.max_identical_patch_attempts {
            let recent_hashes: Vec<&str> = history
                .attempts
                .iter()
                .rev()
                .take(self.max_identical_patch_attempts)
                .map(|a| a.patch_hash.as_str())
                .collect();

            if recent_hashes.iter().all(|h| *h == patch_hash) {
                history.is_quarantined = true;
                error!(
                    "🚨 [Deathloop Circuit Breaker] Task '{}' emitted {} identical patches with hash '{}'. Tripping circuit breaker to stop token drain.",
                    task_id, self.max_identical_patch_attempts, patch_hash
                );
                return DeathloopVerdict::TrippedCircuitBreaker {
                    reason: format!(
                        "Repetitive patch deathloop: generated {} consecutive identical patches with hash '{}'",
                        self.max_identical_patch_attempts, patch_hash
                    ),
                    attempts: attempts_count,
                    tokens_drained: history.total_tokens_drained,
                    quarantine_action: "QUARANTINE_PR_HALT_REPAIRS".to_string(),
                };
            }
        }

        // 3. Invariant: Consecutive Zero-Progress on Identical Failure Signature
        if attempts_count >= self.max_consecutive_zero_progress_attempts {
            let recent_failures: Vec<&str> = history
                .attempts
                .iter()
                .rev()
                .take(self.max_consecutive_zero_progress_attempts)
                .map(|a| a.failure_signature.as_str())
                .collect();

            let all_same_failure = recent_failures.iter().all(|f| *f == failure_signature);
            let no_reduction_in_failures = history
                .attempts
                .iter()
                .rev()
                .take(self.max_consecutive_zero_progress_attempts)
                .all(|a| a.gates_failed_count >= gates_failed_count);

            if all_same_failure && no_reduction_in_failures && gates_failed_count > 0 {
                history.is_quarantined = true;
                error!(
                    "🚨 [Deathloop Circuit Breaker] Task '{}' made zero progress across {} attempts on error '{}'. Quarantining PR.",
                    task_id, self.max_consecutive_zero_progress_attempts, failure_signature
                );
                return DeathloopVerdict::TrippedCircuitBreaker {
                    reason: format!(
                        "Zero-progress repair loop: {} consecutive attempts failed on identical signature '{}'",
                        self.max_consecutive_zero_progress_attempts, failure_signature
                    ),
                    attempts: attempts_count,
                    tokens_drained: history.total_tokens_drained,
                    quarantine_action: "QUARANTINE_PR_HALT_REPAIRS".to_string(),
                };
            }
        }

        if attempts_count >= 2 {
            DeathloopVerdict::Warning(format!(
                "Task '{}' has failed {} attempts. Monitoring for deathloop signature.",
                task_id, attempts_count
            ))
        } else {
            DeathloopVerdict::Nominal
        }
    }

    /// Resets the history for a given task ID upon verified resolution
    pub async fn reset_task(&self, task_id: &str) {
        let mut guard = self.task_histories.write().await;
        guard.remove(task_id);
        info!("Reset deathloop tracking for task '{}'", task_id);
    }
}
