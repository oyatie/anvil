//! Microbenchmark hotpath ratchet — the gate that benchmarked nothing.
//!
//! # What was here
//!
//! `MicrobenchmarkReport` carried a single `passed: bool`, and the review
//! pipeline wrote the sample it was then asked to judge: a base and a head
//! figure that were the same literal, and two cycle counts that were the same
//! literal and are never read at all. A self-identical operand pair computes a
//! 0.00% change on every execution, so the gate reported `Optimal` for every
//! pull request forever — a statement about the caller's constants, not about
//! the code under review.
//!
//! # What is here now
//!
//! No sample is fabricated. This repository declares no criterion dependency
//! and carries no `benches/` directory, so there is no baseline to ratchet
//! against: the gate reports `GateStatus::NotMeasured` naming that missing
//! source. It does not report `Failed` — there is no evidence of a regression,
//! only an absence of timings.
//!
//! The analyzer that did that arithmetic over a caller-supplied sample is
//! deleted along with the sample: until a benchmark actually runs there is
//! nothing for it to read.

use serde::{Deserialize, Serialize};

use crate::pre_merge_guard::report::GateStatus;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BenchmarkRegressionVerdict {
    /// No benchmark was executed, so there is no base or head figure to
    /// compare. Deliberately distinct from `Optimal`: an unrun benchmark is not
    /// a fast one.
    NotMeasured,
    Optimal,
    Regression {
        benchmark: String,
        ns_increase_pct: f64,
        explanation: String,
    },
}

/// Matches the `PreMergeCertificationReport` field name.
const GATE_ID: &str = "microbench_status";

/// What must exist before a hotpath figure can be ratcheted.
const MISSING_CRITERION_BASELINE: &str = "no criterion benchmark harness or published baseline exists for this repository, \
     so neither the base nor the head ns/op figure was ever measured";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrobenchmarkReport {
    pub status: GateStatus,
    /// The analyzer's arithmetic over the caller-supplied sample. Not evidence
    /// about the pull request while `status` is `NotMeasured` — read `status`.
    pub passed: bool,
    pub verdict: BenchmarkRegressionVerdict,
}

#[derive(Debug, Clone, Default)]
pub struct MicroBenchmarkRatchet;

impl MicroBenchmarkRatchet {
    pub fn new() -> Self {
        Self
    }

    /// The review pipeline's entry point: no benchmark is run, so no sample is
    /// invented to feed the analyzer.
    pub fn evaluate_without_criterion_baseline(&self) -> MicrobenchmarkReport {
        MicrobenchmarkReport {
            status: Self::not_measured(),
            passed: false,
            verdict: BenchmarkRegressionVerdict::NotMeasured,
        }
    }

    fn not_measured() -> GateStatus {
        GateStatus::NotMeasured {
            gate_id: GATE_ID.to_string(),
            reason: MISSING_CRITERION_BASELINE.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_criterion_baseline_means_no_hotpath_claim() {
        let rep = MicroBenchmarkRatchet::new().evaluate_without_criterion_baseline();
        assert_eq!(rep.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(!rep.passed, "absent evidence must not certify");
    }
}
