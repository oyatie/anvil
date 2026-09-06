use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::ai_driver::task_classifier::{TaskCategory, TaskComplexity};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicSlaProfile {
    pub max_duration: Duration,
    /// How long an operation may be silent AFTER it has spoken once.
    pub max_inactivity_window: Duration,
    /// How long an operation may take to produce its FIRST vital sign.
    ///
    /// Startup is not liveness, and one window cannot measure both.
    pub startup_grace: Duration,
    pub heartbeat_interval: Duration,
}

impl DynamicSlaProfile {
    /// Computes dynamic SLA envelope based on AST size, task category, and reasoning effort
    pub fn compute_envelope(
        complexity: TaskComplexity,
        category: TaskCategory,
        files_count: usize,
        lines_changed: usize,
        reasoning_effort: &str,
    ) -> Self {
        let (base_secs, idle_secs) =
            if complexity == TaskComplexity::Critical || category == TaskCategory::SecurityAudit {
                (600, 45)
            } else if complexity == TaskComplexity::High
                || category == TaskCategory::ArchitectureRefactor
            {
                (420, 35)
            } else if complexity == TaskComplexity::Medium
                || category == TaskCategory::ContractMigration
            {
                (240, 25)
            } else if complexity == TaskComplexity::Low || category == TaskCategory::DocSweeping {
                (60, 15)
            } else {
                (180, 20)
            };

        // Scale by reasoning effort
        let effort_multiplier = match reasoning_effort {
            "xhigh" => 2.0,
            "high" => 1.5,
            "medium" => 1.0,
            _ => 0.8,
        };

        // Scale by AST line/file volume
        let volume_secs = ((lines_changed / 200) * 15 + files_count * 5) as u64;
        let total_max_secs =
            ((base_secs as f64 * effort_multiplier) as u64 + volume_secs).clamp(30, 1800);

        // The inactivity window is scaled by effort too.
        //
        // Only `max_duration` was, which is inconsistent on its face: a run
        // told to reason harder thinks longer before it speaks as well as in
        // total. The floor keeps a low-effort run from being given less than
        // the base window.
        let idle_scaled = ((idle_secs as f64 * effort_multiplier) as u64).max(idle_secs);

        Self {
            max_duration: Duration::from_secs(total_max_secs),
            max_inactivity_window: Duration::from_secs(idle_scaled),
            // Startup is not liveness.
            //
            // The inactivity window also governed the wait for the FIRST vital
            // sign, and a model emits nothing at all while it is thinking. A
            // doc-parity probe is classified DocSweeping/Low, so it was given
            // fifteen seconds to produce its first byte and was killed at
            // fifteen. The call was healthy; it had simply not started
            // speaking. The gate then published `Errored`, which blocks
            // merge-queue admission -- so pull requests were being refused by a
            // watchdog rather than by a finding.
            //
            // Kubernetes separates `startupProbe` from `livenessProbe` for
            // exactly this reason: a slow start is not a hang.
            startup_grace: Duration::from_secs((idle_scaled * 4).clamp(60, 300)),
            heartbeat_interval: Duration::from_secs(10),
        }
    }
}

/// Progress event emitted by active operations to signal ongoing vital signs
#[derive(Debug, Clone)]
pub struct ProgressSignal {
    pub step_description: String,
    pub bytes_or_tokens_processed: usize,
}

#[derive(Clone)]
pub struct ActivityHandle {
    base_anchor: Instant,
    last_activity_elapsed_ms: Arc<AtomicU64>,
    /// Whether any vital sign has EVER been reported.
    ///
    /// `last_activity_elapsed` cannot tell "silent since it started" from
    /// "went silent after speaking", and those are different failures that
    /// deserve different windows.
    has_spoken: Arc<AtomicBool>,
    progress_tx: mpsc::UnboundedSender<ProgressSignal>,
}

impl ActivityHandle {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<ProgressSignal>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let handle = Self {
            base_anchor: Instant::now(),
            last_activity_elapsed_ms: Arc::new(AtomicU64::new(0)),
            has_spoken: Arc::new(AtomicBool::new(false)),
            progress_tx: tx,
        };
        (handle, rx)
    }

    /// Emits a vital sign / progress heartbeat resetting the inactivity timer
    pub fn report_progress(&self, step: &str, bytes_or_tokens: usize) {
        let elapsed = self.base_anchor.elapsed().as_millis() as u64;
        self.last_activity_elapsed_ms
            .store(elapsed, Ordering::Relaxed);
        self.has_spoken.store(true, Ordering::Relaxed);
        let _ = self.progress_tx.send(ProgressSignal {
            step_description: step.to_string(),
            bytes_or_tokens_processed: bytes_or_tokens,
        });
    }

    /// Whether this operation has produced any vital sign yet.
    pub fn has_spoken(&self) -> bool {
        self.has_spoken.load(Ordering::Relaxed)
    }

    pub fn last_activity_elapsed(&self) -> Duration {
        let last_elapsed_ms = self.last_activity_elapsed_ms.load(Ordering::Relaxed);
        let current_elapsed_ms = self.base_anchor.elapsed().as_millis() as u64;
        if current_elapsed_ms >= last_elapsed_ms {
            Duration::from_millis(current_elapsed_ms - last_elapsed_ms)
        } else {
            Duration::ZERO
        }
    }
}

pub struct PipelineWatchdog;

impl PipelineWatchdog {
    /// Distinguishes between genuinely long tasks and deadlocked stalls via sliding inactivity windows
    pub async fn run_with_adaptive_watchdog<F, Fut, T, FallbackFn>(
        stage_name: &'static str,
        target_entity: &str,
        sla_profile: DynamicSlaProfile,
        operation: F,
        fallback: FallbackFn,
    ) -> Result<T>
    where
        F: FnOnce(ActivityHandle) -> Fut,
        Fut: Future<Output = Result<T>> + Send + 'static,
        T: Send + 'static,
        FallbackFn: FnOnce(String) -> Result<T>,
    {
        let start = Instant::now();
        let entity = target_entity.to_string();
        let entity_clone = entity.clone();
        let (activity_handle, mut progress_rx) = ActivityHandle::new();
        let activity_checker = activity_handle.clone();

        // Background Vital Signs & Heartbeat Monitor Task
        let max_idle = sla_profile.max_inactivity_window;
        let startup_grace = sla_profile.startup_grace;
        let tick_duration = (max_idle / 3).clamp(Duration::from_millis(20), Duration::from_secs(5));
        let mut stall_detected_rx: tokio::task::JoinHandle<Result<(), String>> = tokio::spawn(
            async move {
                let mut interval = tokio::time::interval(tick_duration);
                loop {
                    interval.tick().await;

                    // Drain any pending progress reports
                    while let Ok(signal) = progress_rx.try_recv() {
                        info!(
                            "⚡ [Vital Sign] {} on {}: {} (processed: {} units)",
                            stage_name,
                            entity_clone,
                            signal.step_description,
                            signal.bytes_or_tokens_processed
                        );
                    }

                    let idle_time = activity_checker.last_activity_elapsed();
                    let total_elapsed = start.elapsed();

                    // Before the first vital sign the startup window applies.
                    // A model that has not begun emitting has not hung, and
                    // killing it there reports a stall that never happened.
                    let (window, phase) = if activity_checker.has_spoken() {
                        (max_idle, "idle")
                    } else {
                        (startup_grace, "startup")
                    };

                    if idle_time > window {
                        warn!(
                            "⚠️ [{} Threshold Breached] {} on {} emitted 0 vital signs for {:.1}s (max allowed {}: {:.0}s)",
                            phase,
                            stage_name,
                            entity_clone,
                            idle_time.as_secs_f64(),
                            phase,
                            window.as_secs_f64()
                        );
                        return Err(format!(
                            "Inactivity stall: 0 bytes/tokens/syscalls emitted in {:.1}s ({phase} SLA: {:.0}s)",
                            idle_time.as_secs_f64(),
                            window.as_secs_f64()
                        ));
                    }

                    info!(
                        "⏳ [Heartbeat] {} is active on {} (total elapsed: {:.1}s, last active: {:.1}s ago)...",
                        stage_name,
                        entity_clone,
                        total_elapsed.as_secs_f64(),
                        idle_time.as_secs_f64()
                    );
                }
            },
        );

        let op_future = operation(activity_handle);

        // Race operation against:
        // 1. Hard Global Dynamic Deadline (max_duration)
        // 2. Sliding Inactivity Stall Detector (stall_detected_rx)
        tokio::select! {
            res = op_future => {
                stall_detected_rx.abort();
                match res {
                    Ok(val) => {
                        let elapsed = start.elapsed();
                        info!(
                            "✅ [Adaptive Watchdog] {} completed successfully on {} in {:.2}s",
                            stage_name, entity, elapsed.as_secs_f64()
                        );
                        Ok(val)
                    }
                    Err(e) => {
                        warn!(
                            "⚠️ [Adaptive Watchdog] {} failed on {}: {}. Triggering deterministic remediation...",
                            stage_name, entity, e
                        );
                        fallback(format!("Execution failed: {}", e))
                    }
                }
            }
            stall_res = &mut stall_detected_rx => {
                let reason = match stall_res {
                    Ok(Err(r)) => r,
                    _ => "Inactivity stall detected".to_string(),
                };
                error!(
                    "🚨 [DEADLOCK / INGRESS STALL DETECTED] {} STALLED on {}: {}. Initiating Auto-Remediation...",
                    stage_name, entity, reason
                );
                fallback(format!("Remediated from stall: {}", reason))
            }
            _ = tokio::time::sleep(sla_profile.max_duration) => {
                stall_detected_rx.abort();
                error!(
                    "🚨 [GLOBAL SLA EXCEEDED] {} exceeded maximum global duration of {:.0}s on {}. Aborting...",
                    stage_name, sla_profile.max_duration.as_secs_f64(), entity
                );
                fallback(format!("Exceeded global SLA limit of {}s", sla_profile.max_duration.as_secs()))
            }
        }
    }

    /// Legacy wrapper for backwards-compatibility
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
        let profile = DynamicSlaProfile {
            max_duration: timeout_duration,
            max_inactivity_window: Duration::from_secs((timeout_duration.as_secs() / 2).max(5)),
            startup_grace: Duration::from_secs((timeout_duration.as_secs() / 2).max(30)),
            heartbeat_interval: Duration::from_secs(5),
        };

        Self::run_with_adaptive_watchdog(
            stage_name,
            target_entity,
            profile,
            |_| operation(),
            fallback,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_genuinely_long_task_succeeds_with_vital_signs() {
        let profile = DynamicSlaProfile {
            max_duration: Duration::from_secs(5),
            max_inactivity_window: Duration::from_millis(200), // Short idle threshold
            startup_grace: Duration::from_millis(400),
            heartbeat_interval: Duration::from_millis(50),
        };

        // Genuinely long task running 600ms, but emitting vital signs every 100ms
        let result = PipelineWatchdog::run_with_adaptive_watchdog(
            "LongCompilerTask",
            "oyatie/anvil#1",
            profile,
            |activity| async move {
                for i in 1..=5 {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    activity.report_progress(&format!("compiling crate shard {}", i), i * 1000);
                }
                Ok("Compilation Succeeded".to_string())
            },
            |_| Ok("Fallback".to_string()),
        )
        .await;

        assert_eq!(result.unwrap(), "Compilation Succeeded");
    }

    #[tokio::test]
    async fn test_deadlocked_task_aborts_on_inactivity() {
        let profile = DynamicSlaProfile {
            max_duration: Duration::from_secs(5),
            max_inactivity_window: Duration::from_millis(150), // Short idle threshold
            // This task never speaks at all, so it breaches the STARTUP window,
            // not the idle one. Both abort; they are different facts.
            startup_grace: Duration::from_millis(150),
            heartbeat_interval: Duration::from_millis(50),
        };

        // Task deadlocks (sleeps 500ms without emitting any progress)
        let result = PipelineWatchdog::run_with_adaptive_watchdog(
            "DeadlockedTask",
            "oyatie/anvil#2",
            profile,
            |_activity| async move {
                tokio::time::sleep(Duration::from_millis(500)).await;
                Ok("Should not reach here".to_string())
            },
            |err| Ok(format!("REMEDIATED: {}", err)),
        )
        .await;

        assert!(
            result
                .unwrap()
                .starts_with("REMEDIATED: Remediated from stall: Inactivity stall")
        );
    }

    /// A slow FIRST token is not a hang.
    ///
    /// The defect this split exists for. A doc-parity probe is classified
    /// DocSweeping/Low, which gave it a fifteen-second inactivity window -- and
    /// that window also governed the wait for its first byte. A model emits
    /// nothing while it is thinking, so a healthy call was killed at fifteen
    /// seconds, the gate published `Errored`, and `Errored` blocks merge-queue
    /// admission. Pull requests were being refused by a watchdog rather than by
    /// a finding.
    ///
    /// Verified red before green: with the phase split removed, this test fails
    /// and the other three still pass.
    #[tokio::test]
    async fn a_slow_first_token_is_not_a_stall() {
        let profile = DynamicSlaProfile {
            max_duration: Duration::from_secs(5),
            // Tighter than the startup silence below: under one window this
            // task dies, which is exactly what used to happen.
            max_inactivity_window: Duration::from_millis(100),
            startup_grace: Duration::from_millis(600),
            heartbeat_interval: Duration::from_millis(50),
        };

        let result = PipelineWatchdog::run_with_adaptive_watchdog(
            "ModelInference",
            "oyatie/anvil#1",
            profile,
            |activity| async move {
                // Thinking: 300ms before the first token, three times the idle
                // window and well inside the startup grace.
                tokio::time::sleep(Duration::from_millis(300)).await;
                activity.report_progress("first token", 1);
                for i in 2..=4 {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    activity.report_progress("token", i);
                }
                Ok("judgement".to_string())
            },
            |_| Ok("Fallback".to_string()),
        )
        .await;

        assert_eq!(
            result.unwrap(),
            "judgement",
            "a call that had not begun speaking was killed as though it had hung"
        );
    }

    /// The other half: silence AFTER speaking is still a stall.
    ///
    /// The startup grace must not become a blanket extension, or the watchdog
    /// stops detecting the hang it exists for.
    #[tokio::test]
    async fn going_silent_after_speaking_still_aborts() {
        let profile = DynamicSlaProfile {
            max_duration: Duration::from_secs(5),
            max_inactivity_window: Duration::from_millis(120),
            // Generous, and deliberately irrelevant once the task has spoken.
            startup_grace: Duration::from_secs(4),
            heartbeat_interval: Duration::from_millis(50),
        };

        let result = PipelineWatchdog::run_with_adaptive_watchdog(
            "ModelInference",
            "oyatie/anvil#1",
            profile,
            |activity| async move {
                activity.report_progress("first token", 1);
                tokio::time::sleep(Duration::from_millis(800)).await;
                Ok("should never be reached".to_string())
            },
            |reason| Ok(format!("FALLBACK: {reason}")),
        )
        .await
        .expect("the fallback answers");

        assert!(
            result.starts_with("FALLBACK:"),
            "a task that spoke once and then hung must still be caught: {result}"
        );
        assert!(
            result.contains("idle SLA"),
            "the message must say WHICH window was breached, or an operator \
             cannot tell a hang from a slow start: {result}"
        );
    }
}
