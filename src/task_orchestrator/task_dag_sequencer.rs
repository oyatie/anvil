use anyhow::{Result, bail};
use std::collections::{HashMap, VecDeque};
use tracing::info;

use super::source_doc_verifier::ScopedTaskDefinition;

#[derive(Debug, Clone)]
pub struct SequencedTaskBatch {
    pub stage_index: usize,
    pub tasks: Vec<ScopedTaskDefinition>,
}

#[derive(Clone, Default)]
pub struct TaskDagSequencer;

impl TaskDagSequencer {
    pub fn new() -> Self {
        Self
    }

    /// Takes a list of scoped tasks and performs topological sorting to return properly sequenced execution stages
    pub fn sequence_tasks(
        &self,
        tasks: Vec<ScopedTaskDefinition>,
    ) -> Result<Vec<SequencedTaskBatch>> {
        info!(
            "🧭 [Task DAG Sequencer] Topologically ordering {} candidate tasks...",
            tasks.len()
        );

        let mut task_map: HashMap<String, ScopedTaskDefinition> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();

        for task in &tasks {
            task_map.insert(task.task_id.clone(), task.clone());
            in_degree.insert(task.task_id.clone(), 0);
            adj.insert(task.task_id.clone(), Vec::new());
        }

        // Build edges: parent -> child
        for task in &tasks {
            for dep in &task.dependencies {
                if task_map.contains_key(dep) {
                    adj.get_mut(dep).unwrap().push(task.task_id.clone());
                    *in_degree.get_mut(&task.task_id).unwrap() += 1;
                }
            }
        }

        // Kahn's algorithm with stage grouping
        let mut ready_queue: VecDeque<String> = VecDeque::new();
        for (id, deg) in &in_degree {
            if *deg == 0 {
                ready_queue.push_back(id.clone());
            }
        }

        let mut stages: Vec<SequencedTaskBatch> = Vec::new();
        let mut processed_count = 0;
        let mut stage_idx = 0;

        while !ready_queue.is_empty() {
            let mut current_stage_ids = Vec::new();
            let count_in_this_layer = ready_queue.len();

            for _ in 0..count_in_this_layer {
                if let Some(id) = ready_queue.pop_front() {
                    current_stage_ids.push(id);
                }
            }

            // Sort current stage by priority (P0 first, then P1, then P2)
            let mut current_stage_tasks: Vec<ScopedTaskDefinition> = current_stage_ids
                .iter()
                .filter_map(|id| task_map.get(id).cloned())
                .collect();
            current_stage_tasks.sort_by_key(|t| t.priority);

            processed_count += current_stage_tasks.len();

            // Decrease in-degrees for downstream dependents
            for id in &current_stage_ids {
                if let Some(children) = adj.get(id) {
                    for child in children {
                        if let Some(deg) = in_degree.get_mut(child) {
                            *deg -= 1;
                            if *deg == 0 {
                                ready_queue.push_back(child.clone());
                            }
                        }
                    }
                }
            }

            stages.push(SequencedTaskBatch {
                stage_index: stage_idx,
                tasks: current_stage_tasks,
            });
            stage_idx += 1;
        }

        if processed_count < tasks.len() {
            bail!(
                "Circular dependency cycle detected in task DAG! Processed {}/{} tasks.",
                processed_count,
                tasks.len()
            );
        }

        info!(
            "✅ [Task DAG Sequencer] Successfully partitioned into {} sequential execution stages.",
            stages.len()
        );

        Ok(stages)
    }
}
