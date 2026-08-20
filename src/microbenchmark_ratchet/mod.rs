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
//! `CriterionDiffAnalyzer` is retained and still exported. It is honest
//! arithmetic over a caller-supplied sample and is the seam a real criterion
//! baseline plugs into; its verdict is still computed and published on the
//! report, but it is not what the scorecard reads. Until a benchmark actually
//! runs, a verdict derived from caller-supplied numbers is not a measurement of
//! this pull request, and `status` says so.

use serde::{Deserialize, Serialize};

use crate::pre_merge_guard::GateStatus;

pub mod criterion_diff;
pub use criterion_diff::{BenchmarkRegressionVerdict, CriterionDiffAnalyzer, MicrobenchmarkSample};

/// Matches the `PreMergeCertificationReport` field name.
const GATE_ID: &str = "microbench_status";

/// What must exist before a hotpath figure can be ratcheted.
const MISSING_CRITERION_BASELINE: &str =
    "no criterion benchmark harness or published baseline exists for this repository, \
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
pub struct MicroBenchmarkRatchet {
    analyzer: CriterionDiffAnalyzer,
}

impl MicroBenchmarkRatchet {
    pub fn new() -> Self {
        Self {
            analyzer: CriterionDiffAnalyzer::new(),
        }
    }

    /// Runs the analyzer over a caller-supplied sample and publishes the
    /// verdict, while reporting the gate itself as unmeasured: no benchmark was
    /// executed to produce that sample. See the module docs.
    pub fn evaluate_benchmark_regression(
        &self,
        sample: &MicrobenchmarkSample,
    ) -> MicrobenchmarkReport {
        let verdict = self.analyzer.evaluate_benchmark_diff(sample);
        let passed = matches!(verdict, BenchmarkRegressionVerdict::Optimal);

        MicrobenchmarkReport {
            status: Self::not_measured(),
            passed,
            verdict,
        }
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
    fn test_microbenchmark_ratchet_nominal() {
        let ratchet = MicroBenchmarkRatchet::new();
        let sample = MicrobenchmarkSample {
            benchmark_name: "sha256_hashing".to_string(),
            base_ns_per_op: 50.0,
            head_ns_per_op: 50.0,
            p99_cpu_cycles_base: 100,
            p99_cpu_cycles_head: 100,
        };
        let rep = ratchet.evaluate_benchmark_regression(&sample);
        assert!(rep.passed);
        // ...and the gate still reports that it measured nothing: the sample
        // above was written by the caller, not read off a benchmark run.
        assert_eq!(rep.status.unmeasured_gate_id(), Some(GATE_ID));
    }

    #[test]
    fn no_criterion_baseline_means_no_hotpath_claim() {
        let rep = MicroBenchmarkRatchet::new().evaluate_without_criterion_baseline();
        assert_eq!(rep.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(!rep.passed, "absent evidence must not certify");
    }
}
