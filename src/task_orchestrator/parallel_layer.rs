//! One DAG layer: fail closed on path overlap, then run all tasks concurrently.
//!
//! Occupancy is git paths. Launchers bind one fresh agent per hop. This
//! module does not fold N ready tasks onto one worker.

use anyhow::Result;
use futures::future::join_all;
use std::future::Future;

use super::autonomous_fix_engine::TaskExecutionReport;
use super::path_occupancy::assert_layer_paths_disjoint;
use super::source_doc_verifier::ScopedTaskDefinition;

pub async fn run_layer_parallel<F, Fut>(
    tasks: Vec<ScopedTaskDefinition>,
    run: F,
) -> Result<Vec<TaskExecutionReport>>
where
    F: Fn(ScopedTaskDefinition) -> Fut,
    Fut: Future<Output = Result<TaskExecutionReport>>,
{
    assert_layer_paths_disjoint(&tasks)?;
    let futs: Vec<_> = tasks.into_iter().map(run).collect();
    let mut reports = Vec::new();
    for item in join_all(futs).await {
        reports.push(item?);
    }
    Ok(reports)
}
