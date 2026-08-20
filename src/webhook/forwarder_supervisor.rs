//! Keeps the webhook transport alive across upstream disconnects.
//!
//! `gh webhook forward` holds a WebSocket to webhook-forwarder.github.com and
//! loses it routinely:
//!
//! ```text
//! Error: error receiving json event: websocket: close 1006 (abnormal closure): unexpected EOF
//! ```
//!
//! The original spawn site checked only `if let Err(e) = cmd.status().await`.
//! `status()` yields `Ok(status)` when the child ran and exited; `Err` fires
//! only when it could not be *spawned*. A forwarder killed by a 1006 drop
//! therefore returned `Ok(non-zero)`, logged nothing, and was never restarted --
//! webhook delivery for that repository stopped silently until the whole daemon
//! was restarted. That is the failure behind an observed 1h38m of uptime whose
//! only output was telemetry polling.
//!
//! Exit code deliberately does not decide whether to restart. `gh` may exit 0
//! on a dropped socket, and a transport that is not running is an outage
//! regardless of how politely it stopped.

use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};

/// Bounds on how fast a dead forwarder is respawned.
#[derive(Debug, Clone)]
pub struct RestartPolicy {
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        RestartPolicy {
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
        }
    }
}

/// Monotonic counter mixed into the jitter so two calls in the same nanosecond
/// still differ. Without it, three forwarders dropped by one upstream blip
/// would recompute identical delays and reconnect in lockstep.
static JITTER_SEQ: AtomicU64 = AtomicU64::new(0);

fn entropy() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    let seq = JITTER_SEQ.fetch_add(1, Ordering::Relaxed);
    // Cheap mix; this seeds a backoff, it is not a security primitive.
    nanos
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(seq.wrapping_mul(1_442_695_040_888_963_407))
}

/// Equal-jitter backoff: half the window is fixed, half is random.
///
/// Full jitter (`random(0, ceiling)`) is the more common form but can return a
/// near-zero delay at any attempt, which defeats the purpose of backing off at
/// all when upstream is degraded. Equal jitter guarantees the delay still grows
/// with consecutive failures while keeping enough spread to break a herd.
pub fn next_restart_delay(attempt: u32, policy: &RestartPolicy) -> Duration {
    let base_ms = policy.base_delay.as_millis().max(1) as u64;
    let max_ms = policy.max_delay.as_millis().max(1) as u64;

    // Saturating throughout: attempt is unbounded in a long outage and
    // `base * 2^attempt` overflows well before anyone notices.
    let ceiling = base_ms
        .saturating_mul(1u64.checked_shl(attempt.min(32)).unwrap_or(u64::MAX))
        .min(max_ms);

    let half = ceiling / 2;
    let spread = if half == 0 { 0 } else { entropy() % (half + 1) };
    Duration::from_millis((half + spread).min(max_ms))
}

/// Restarts `spawn` forever, backing off between consecutive failures.
///
/// `spawn` yields the child's exit code. The loop does not inspect it: see the
/// module note on why exit status must not gate the restart.
pub async fn supervise<F, Fut>(name: &str, policy: &RestartPolicy, spawn: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::io::Result<i32>>,
{
    supervise_bounded(name, policy, usize::MAX, spawn).await
}

/// [`supervise`] with a cap on total starts, so tests can observe the loop.
pub async fn supervise_bounded<F, Fut>(
    name: &str,
    policy: &RestartPolicy,
    max_starts: usize,
    mut spawn: F,
) where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::io::Result<i32>>,
{
    let mut consecutive_failures: u32 = 0;

    for start in 1..=max_starts {
        match spawn().await {
            Ok(code) => {
                // Any exit is an outage for this repository.
                warn!(
                    "Webhook forwarder for {} exited with code {} (start #{}); \
                     deliveries for this repository stopped until it is respawned",
                    name, code, start
                );
            }
            Err(e) => {
                error!("Webhook forwarder for {} could not be spawned: {}", name, e);
            }
        }

        if start >= max_starts {
            break;
        }

        consecutive_failures = consecutive_failures.saturating_add(1);
        let delay = next_restart_delay(consecutive_failures, policy);
        info!(
            "Respawning webhook forwarder for {} in {:?} (consecutive failures: {})",
            name, delay, consecutive_failures
        );
        tokio::time::sleep(delay).await;
    }
}
