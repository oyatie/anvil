use anyhow::Result;
use std::future::Future;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
pub struct WatchdogConfig {
    pub heartbeat_interval_secs: u64,
    pub stall_threshold_secs: u64,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_secs: 10,
            stall_threshold_secs: 60,
        }
    }
}

pub struct PipelineWatchdog;

impl PipelineWatchdog {
    /// Wraps any asynchronous pipeline operation with a live heartbeat and deterministic stall auto-remediation
    pub async fn run_with_watchdog<F, Fut, T, FallbackFn>(
        stage_name: &'static str,
        target_entity: &str,
        timeout_duration: Duration,
        operation: F,
        fallback: FallbackFn,
    ) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>> + Send + 'static,
        T: Send + 'static,
        FallbackFn: FnOnce(String) -> Result<T>,
    {
        let start = Instant::now();
        let entity = target_entity.to_string();
        let entity_clone = entity.clone();

        // Background Heartbeat Task
        let heartbeat_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                let elapsed = start.elapsed();
                info!(
                    "⏳ [Heartbeat] {} is active on {} (elapsed: {:.1}s)...",
                    stage_name,
                    entity_clone,
                    elapsed.as_secs_f64()
                );
            }
        });

        let op_future = operation();

        // Race operation against hard bounded timeout
        let result = match tokio::time::timeout(timeout_duration, op_future).await {
            Ok(Ok(val)) => {
                heartbeat_handle.abort();
                let elapsed = start.elapsed();
                info!(
                    "✅ [Watchdog] {} completed successfully on {} in {:.2}s",
                    stage_name,
                    entity,
                    elapsed.as_secs_f64()
                );
                Ok(val)
            }
            Ok(Err(e)) => {
                heartbeat_handle.abort();
                warn!(
                    "⚠️ [Watchdog] {} returned error on {}: {}. Triggering deterministic remediation...",
                    stage_name, entity, e
                );
                fallback(format!("Execution failed: {}", e))
            }
            Err(_) => {
                heartbeat_handle.abort();
                let elapsed = start.elapsed();
                error!(
                    "🚨 [WATCHDOG STALL DETECTED] {} STALLED on {} (exceeded SLA timeout of {:.0}s, elapsed: {:.1}s). Initiating Auto-Remediation & Fallback...",
                    stage_name,
                    entity,
                    timeout_duration.as_secs_f64(),
                    elapsed.as_secs_f64()
                );
                fallback(format!(
                    "Operation stalled and was aborted after exceeding timeout of {}s",
                    timeout_duration.as_secs()
                ))
            }
        };

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_watchdog_detects_stall_and_remediates_with_fallback() {
        let result = PipelineWatchdog::run_with_watchdog(
            "StalledDocCheck",
            "oyatie/anvil#99",
            Duration::from_millis(50),
            || async {
                // Simulate stuck process
                tokio::time::sleep(Duration::from_millis(500)).await;
                Ok("Should never return".to_string())
            },
            |err_reason| {
                // Remediation fallback
                Ok(format!("REMEDIATED: {}", err_reason))
            },
        )
        .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert!(val.starts_with("REMEDIATED: Operation stalled"));
    }

    #[tokio::test]
    async fn test_watchdog_passes_fast_operation() {
        let result = PipelineWatchdog::run_with_watchdog(
            "FastCheck",
            "oyatie/anvil#1",
            Duration::from_secs(5),
            || async { Ok(42) },
            |_| Ok(0),
        )
        .await;

        assert_eq!(result.unwrap(), 42);
    }
}
