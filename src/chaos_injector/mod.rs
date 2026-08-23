//! Pre-merge chaos injection — the gate that ran three experiments and one test.
//!
//! # What was here
//!
//! `inject_synthetic_chaos` declared three faults — a 5% packet drop, 250ms of
//! DNS latency, a database leader failover — and handed each to
//! `simulate_chaos_fault`, which never read its `fault` parameter. The same
//! two-substring scan therefore ran three times over the same diff and produced
//! three identical verdicts, each carrying `recovery_time_ms: 45`: a recovery
//! time for an experiment that did not run, since nothing here starts a system,
//! drops a packet or fails anything over.
//!
//! When it did fire it blocked the merge with "provoked unhandled panic/outage
//! in preview sandbox", naming a sandbox that is not deployed, spawned or
//! configured anywhere in this repository — so an author holding a blocked pull
//! request was sent to look for a thing that does not exist.
//!
//! Every chaos tool the gate is named after acts on a **running** system:
//! Chaos Monkey terminates live EC2 and Titus instances through Spinnaker, AWS
//! FIS and Gremlin inject faults into live resources, LitmusChaos into live
//! Kubernetes workloads, and the steady-state hypothesis at
//! principlesofchaos.org presupposes a system in a steady state to disturb. No
//! established chaos tool decides resilience by pattern-matching a diff.
//!
//! # What is here now
//!
//! No fault is declared and no recovery is timed. What survives is the one
//! honest computation the module had: a scan of the added lines for an
//! `.unwrap()` on an awaited call, which is the panic a network fault would
//! provoke. That is a **lint** — the same property as `clippy::unwrap_used`,
//! which upstream files under the opt-in `restriction` group — and it is
//! published as one.
//!
//! A hit is [`GateStatus::Failed`], because it is measured evidence read off the
//! change itself. No hit is [`GateStatus::NotMeasured`], not `Passed`: nothing
//! was made to fail, so nothing survived failing.

use serde::{Deserialize, Serialize};

use crate::pre_merge_guard::report::GateStatus;

/// Matches the `PreMergeCertificationReport` field name, so `unmeasured_gates`
/// names a gate a reader can look up in the fidelity registry.
const GATE_ID: &str = "chaos_injection_status";

const NO_RUNNING_SYSTEM: &str = "no fault was injected: nothing here starts a running deployment and no fault \
     injector (Chaos Monkey, AWS FIS, Gremlin, LitmusChaos) is driven, so no steady \
     state was disturbed and none was observed returning. What ran is a lint over the \
     added lines, and it found no unwrap on an awaited call";

/// One added line that unwraps the result of an awaited call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnhandledAwait {
    pub file_path: String,
    pub code_line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosInjectorReport {
    pub status: GateStatus,
    /// Whether a fault was injected AND the system handled it. False always,
    /// today: no fault is injected. A clean lint is not a survived experiment.
    pub passed: bool,
    pub unhandled_awaits: Vec<UnhandledAwait>,
    pub summary: String,
}

#[derive(Debug, Clone, Default)]
pub struct ChaosFaultInjector;

impl ChaosFaultInjector {
    pub fn new() -> Self {
        Self
    }

    /// Lints the added lines for an `.unwrap()` on an awaited call, and reports
    /// that no fault was injected into anything.
    ///
    /// Only added lines are read: a line the diff carries past as context, or
    /// removes, is not something this change did.
    pub fn scan_for_unhandled_await_without_a_running_system(
        &self,
        diff_content: &str,
    ) -> ChaosInjectorReport {
        let unhandled_awaits = Self::unwraps_on_awaited_calls(diff_content);

        if unhandled_awaits.is_empty() {
            return ChaosInjectorReport {
                status: GateStatus::NotMeasured {
                    gate_id: GATE_ID.to_string(),
                    reason: NO_RUNNING_SYSTEM.to_string(),
                },
                passed: false,
                unhandled_awaits,
                summary: NO_RUNNING_SYSTEM.to_string(),
            };
        }

        let summary = format!(
            "{} added line(s) unwrap the result of an awaited call, which panics when that \
             call fails. No fault was injected; this is a lint over the diff, the property \
             clippy::unwrap_used checks: {}",
            unhandled_awaits.len(),
            unhandled_awaits
                .iter()
                .map(|u| format!("{}: {}", u.file_path, u.code_line))
                .collect::<Vec<_>>()
                .join("; ")
        );

        ChaosInjectorReport {
            status: GateStatus::Failed(summary.clone()),
            passed: false,
            unhandled_awaits,
            summary,
        }
    }

    /// The property is the unwrap on the await, not the receiver's name: the
    /// scan this replaces matched exactly `.send().await.unwrap()` and
    /// `.query().await.unwrap()`, so a panic on any other awaited call was
    /// invisible to it.
    ///
    /// Text, not syntax: `.await` inside a string literal or a comment on an
    /// added line counts, and an unwrap split across lines does not. The
    /// registry gap says so.
    fn unwraps_on_awaited_calls(diff_content: &str) -> Vec<UnhandledAwait> {
        let mut out = Vec::new();
        let mut current_file = String::new();

        for line in diff_content.lines() {
            if let Some(stripped) = line.strip_prefix("+++ b/") {
                current_file = stripped.trim().to_string();
                continue;
            }
            if !line.starts_with('+') || line.starts_with("+++") {
                continue;
            }
            let code_line = line[1..].trim();
            let squashed: String = code_line.chars().filter(|c| !c.is_whitespace()).collect();
            if squashed.contains(".await.unwrap()") {
                out.push(UnhandledAwait {
                    file_path: current_file.clone(),
                    code_line: code_line.to_string(),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_an_unwrapped_await_is_reported() {
        let report = ChaosFaultInjector::new().scan_for_unhandled_await_without_a_running_system(
            "+++ b/src/net.rs\n+ let resp = client.send().await.unwrap();",
        );
        assert!(matches!(report.status, GateStatus::Failed(_)));
        assert_eq!(report.unhandled_awaits.len(), 1);
    }

    #[test]
    fn test_a_clean_diff_is_unmeasured_rather_than_resilient() {
        let report = ChaosFaultInjector::new()
            .scan_for_unhandled_await_without_a_running_system("+ let n = 1;");
        assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(!report.passed);
    }
}
