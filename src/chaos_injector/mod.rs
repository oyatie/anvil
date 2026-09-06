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
//! A hit is [`GateStatus::Warning`]. It is measured evidence read off the change
//! itself, but the scan cannot tell a test from production code, clippy files
//! the same property under the opt-in `restriction` group rather than a default
//! deny, and the first version of this gate that blocked was red on ten hits in
//! its own diff with no true positive among them. Debt, surfaced; not a refused
//! merge. The feature-flag gate in this same change reasons its way to exactly
//! this conclusion for exactly this reason.
//!
//! No hit is [`GateStatus::NotMeasured`], not `Passed`: nothing was made to
//! fail, so nothing survived failing.

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
                unhandled_awaits,
                summary: NO_RUNNING_SYSTEM.to_string(),
            };
        }

        let summary = format!(
            "{} added line(s) unwrap the result of an awaited call, which panics when that \
             call fails. No fault was injected; this is a lint over the diff, the property \
             clippy::unwrap_used checks, and it cannot tell a test from production code: {}",
            unhandled_awaits.len(),
            unhandled_awaits
                .iter()
                .map(|u| format!("{}: {}", u.file_path, u.code_line))
                .collect::<Vec<_>>()
                .join("; ")
        );

        ChaosInjectorReport {
            status: GateStatus::Warning(summary.clone()),
            unhandled_awaits,
            summary,
        }
    }

    /// The property is the unwrap on the await, not the receiver's name: the
    /// scan this replaces matched exactly `.send().await.unwrap()` and
    /// `.query().await.unwrap()`, so a panic on any other awaited call was
    /// invisible to it.
    ///
    /// Text, not syntax, but only over [`code_only`]: an occurrence inside a
    /// comment or a string literal is prose ABOUT the property, not the
    /// property. Scanning the raw line made this gate red on its own
    /// implementation, on its own tests' fixture strings, and on the registry
    /// sentence describing it -- ten hits, none of them an unwrapped await. An
    /// unwrap split across lines is still invisible; the registry gap says so.
    fn unwraps_on_awaited_calls(diff_content: &str) -> Vec<UnhandledAwait> {
        let mut out = Vec::new();
        // `None` until the diff names a file; see the note in
        // `feature_flag_ratchet::scan_flag_references`, which had the identical
        // `String::new()` seed. Measured on the old code, a `+` line before any
        // header produced `chaos findings: [""]` -- an unhandled-await
        // accusation with no location on it.
        let mut current_file: Option<&str> = None;

        for line in diff_content.lines() {
            if let Some(stripped) = line.strip_prefix("+++ b/") {
                current_file = Some(stripped.trim());
                continue;
            }
            if !line.starts_with('+') || line.starts_with("+++") {
                continue;
            }
            let Some(current_file) = current_file else {
                continue;
            };
            let code_line = line[1..].trim();
            let squashed: String = code_only(code_line)
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            if squashed.contains(".await.unwrap()") {
                out.push(UnhandledAwait {
                    file_path: current_file.to_string(),
                    code_line: code_line.to_string(),
                });
            }
        }
        out
    }
}

/// One line of Rust with its commentary and its string-literal CONTENTS removed,
/// so a scan over it sees code and nothing else.
///
/// A `//` comment truncates the line. A string literal keeps its quotes and
/// loses its body, so `contains(".await.unwrap()")` no longer matches the source
/// line of a scan looking for that text, nor a test fixture quoting it. Escapes
/// are honoured so `"\""` does not leave the scanner inside a string forever.
///
/// One line of Rust as code: commentary gone, string-literal BODIES gone,
/// quotes kept.
///
/// Delegates to the shared scanner. This was one of nine spellings of the same
/// idea; keeping the name here keeps the call sites and the fidelity registry's
/// citations of it intact, while the behaviour has exactly one definition.
pub fn code_only(line: &str) -> String {
    crate::source_scan::code_only(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_an_unwrapped_await_is_reported() {
        let report = ChaosFaultInjector::new().scan_for_unhandled_await_without_a_running_system(
            "+++ b/src/net.rs\n+ let resp = client.send().await.unwrap();",
        );
        assert!(matches!(report.status, GateStatus::Warning(_)));
        assert_eq!(report.unhandled_awaits.len(), 1);
    }

    #[test]
    fn test_a_clean_diff_is_unmeasured_rather_than_resilient() {
        let report = ChaosFaultInjector::new()
            .scan_for_unhandled_await_without_a_running_system("+ let n = 1;");
        assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(report.unhandled_awaits.is_empty());
    }

    #[test]
    fn test_prose_about_the_property_is_not_the_property() {
        // Every line below is one this module's own diff adds. Scanning the raw
        // text made the gate red on all of them.
        let own_diff = concat!(
            "+++ b/src/chaos_injector/mod.rs\n",
            "+    /// `.send().await.unwrap()` and `.query().await.unwrap()`\n",
            "+        if squashed.contains(\".await.unwrap()\") {\n",
            "+++ b/tests/gates_that_cannot_fire_test.rs\n",
            "+        \"+++ b/src/net.rs\\n+ let resp = client.send().await.unwrap();\",\n",
            "+    let n = 1; // client.send().await.unwrap() used to live here\n",
        );
        let report =
            ChaosFaultInjector::new().scan_for_unhandled_await_without_a_running_system(own_diff);
        assert_eq!(
            report.unhandled_awaits,
            vec![],
            "comments and string literals are prose about the lint, not a hit"
        );
        assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
    }
}
