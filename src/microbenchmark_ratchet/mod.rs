use serde::{Deserialize, Serialize};

pub mod criterion_diff;
pub use criterion_diff::{BenchmarkRegressionVerdict, CriterionDiffAnalyzer, MicrobenchmarkSample};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrobenchmarkReport {
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

    pub fn evaluate_benchmark_regression(
        &self,
        sample: &MicrobenchmarkSample,
    ) -> MicrobenchmarkReport {
        let verdict = self.analyzer.evaluate_benchmark_diff(sample);
        let passed = matches!(verdict, BenchmarkRegressionVerdict::Optimal);

        MicrobenchmarkReport { passed, verdict }
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
    }
}
