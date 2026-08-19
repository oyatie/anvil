use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::stage_router::AgenticStage;
use super::task_classifier::{ProgrammingLanguage, TaskCategory, TaskComplexity};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionTelemetry {
    pub task_id: String,
    pub stage: AgenticStage,
    pub language: ProgrammingLanguage,
    pub category: TaskCategory,
    pub complexity: TaskComplexity,
    pub model_used: String,
    pub reasoning_effort: String,
    pub duration_ms: u64,
    pub tokens_consumed: u64,
    pub estimated_cost_usd: f64,
    pub first_pass_success: bool,
    pub quality_score: f64, // 0.0 to 1.0
    pub author_disputed: bool,
    pub timestamp_epoch_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPerformanceStats {
    pub total_trials: usize,
    pub successful_trials: usize,
    pub total_duration_ms: u64,
    pub total_tokens: u64,
    pub total_cost_usd: f64,
    pub avg_quality_score: f64,
    pub composite_score: f64, // Pareto reward score
}

#[derive(Debug, Clone, Default)]
pub struct AdaptiveRoutingBandit {
    ledger: Arc<RwLock<Vec<TaskExecutionTelemetry>>>,
    stats_table: Arc<RwLock<HashMap<String, ModelPerformanceStats>>>,
}

impl AdaptiveRoutingBandit {
    pub fn new() -> Self {
        Self {
            ledger: Arc::new(RwLock::new(Vec::new())),
            stats_table: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Records task execution telemetry and updates reinforcement learning reward weights
    pub fn record_telemetry(&self, record: TaskExecutionTelemetry) {
        let key = format!(
            "{:?}:{:?}:{}",
            record.language, record.category, record.model_used
        );

        if let Ok(mut stats_map) = self.stats_table.write() {
            let stats = stats_map.entry(key).or_insert(ModelPerformanceStats {
                total_trials: 0,
                successful_trials: 0,
                total_duration_ms: 0,
                total_tokens: 0,
                total_cost_usd: 0.0,
                avg_quality_score: 0.8,
                composite_score: 1.0,
            });

            stats.total_trials += 1;
            if record.first_pass_success && !record.author_disputed {
                stats.successful_trials += 1;
            }
            stats.total_duration_ms += record.duration_ms;
            stats.total_tokens += record.tokens_consumed;
            stats.total_cost_usd += record.estimated_cost_usd;

            let success_rate = stats.successful_trials as f64 / stats.total_trials as f64;
            stats.avg_quality_score =
                (stats.avg_quality_score * 0.8) + (record.quality_score * 0.2);

            // Pareto Reward Formula:
            // R = (SuccessRate * Quality) / (1.0 + (CostUSD * 0.5) + (DurationSec / 600.0))
            let avg_cost = stats.total_cost_usd / stats.total_trials as f64;
            let avg_duration_sec =
                (stats.total_duration_ms as f64 / stats.total_trials as f64) / 1000.0;
            stats.composite_score = (success_rate * stats.avg_quality_score)
                / (1.0 + (avg_cost * 0.5) + (avg_duration_sec / 600.0));
        }

        if let Ok(mut ledger_lock) = self.ledger.write() {
            ledger_lock.push(record);
        }
    }

    /// Returns recommended reasoning effort tuned to task complexity and economics
    pub fn recommend_effort(complexity: TaskComplexity, category: TaskCategory) -> &'static str {
        if complexity == TaskComplexity::Critical || category == TaskCategory::SecurityAudit {
            "xhigh"
        } else if complexity == TaskComplexity::High
            || category == TaskCategory::ArchitectureRefactor
        {
            "high"
        } else if complexity == TaskComplexity::Medium
            || category == TaskCategory::ContractMigration
        {
            "medium"
        } else {
            "low"
        }
    }

    /// Fetches the empirical win rate and reward score for a model in a specific domain
    pub fn get_model_score(
        &self,
        lang: ProgrammingLanguage,
        category: TaskCategory,
        model_name: &str,
    ) -> Option<ModelPerformanceStats> {
        let key = format!("{:?}:{:?}:{}", lang, category, model_name);
        if let Ok(stats_map) = self.stats_table.read() {
            stats_map.get(&key).cloned()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_ledger_updates_pareto_rewards() {
        let bandit = AdaptiveRoutingBandit::new();

        let tele1 = TaskExecutionTelemetry {
            task_id: "task-001".to_string(),
            stage: AgenticStage::Implementation,
            language: ProgrammingLanguage::Rust,
            category: TaskCategory::NewFeatureSynthesis,
            complexity: TaskComplexity::Medium,
            model_used: "grok-4.6".to_string(),
            reasoning_effort: "xhigh".to_string(),
            duration_ms: 12000,
            tokens_consumed: 4500,
            estimated_cost_usd: 0.15,
            first_pass_success: true,
            quality_score: 0.95,
            author_disputed: false,
            timestamp_epoch_secs: 1770000000,
        };

        bandit.record_telemetry(tele1);

        let score = bandit.get_model_score(
            ProgrammingLanguage::Rust,
            TaskCategory::NewFeatureSynthesis,
            "grok-4.6",
        );

        assert!(score.is_some());
        let s = score.unwrap();
        assert_eq!(s.total_trials, 1);
        assert_eq!(s.successful_trials, 1);
        assert!(s.composite_score > 0.5);
    }
}
