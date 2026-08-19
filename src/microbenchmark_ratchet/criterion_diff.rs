use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrobenchmarkSample {
    pub benchmark_name: String,
    pub base_ns_per_op: f64,
    pub head_ns_per_op: f64,
    pub p99_cpu_cycles_base: u64,
    pub p99_cpu_cycles_head: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BenchmarkRegressionVerdict {
    Optimal,
    Regression {
        benchmark: String,
        ns_increase_pct: f64,
        explanation: String,
    },
}

#[derive(Debug, Clone, Default)]
pub struct CriterionDiffAnalyzer;

impl CriterionDiffAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates microsecond/nanosecond benchmark regressions on critical execution hotpaths
    pub fn evaluate_benchmark_diff(
        &self,
        sample: &MicrobenchmarkSample,
    ) -> BenchmarkRegressionVerdict {
        if sample.base_ns_per_op > 0.0 {
            let pct_increase =
                ((sample.head_ns_per_op - sample.base_ns_per_op) / sample.base_ns_per_op) * 100.0;

            // Flag hotpath microbenchmark regressions > 5%
            if pct_increase > 5.0 {
                return BenchmarkRegressionVerdict::Regression {
                    benchmark: sample.benchmark_name.clone(),
                    ns_increase_pct: pct_increase,
                    explanation: format!(
                        "Microbenchmark '{}' regressed by {:.2}% (Base: {:.2}ns/op, Head: {:.2}ns/op)",
                        sample.benchmark_name, pct_increase, sample.base_ns_per_op, sample.head_ns_per_op
                    ),
                };
            }
        }

        BenchmarkRegressionVerdict::Optimal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catches_microbenchmark_regression() {
        let analyzer = CriterionDiffAnalyzer::new();
        let sample = MicrobenchmarkSample {
            benchmark_name: "jwt_signature_verification".to_string(),
            base_ns_per_op: 120.0,
            head_ns_per_op: 140.0,
            p99_cpu_cycles_base: 300,
            p99_cpu_cycles_head: 360,
        };

        let verdict = analyzer.evaluate_benchmark_diff(&sample);
        match verdict {
            BenchmarkRegressionVerdict::Regression {
                ns_increase_pct, ..
            } => {
                assert!(ns_increase_pct > 15.0);
            }
            _ => panic!("Expected regression verdict"),
        }
    }

    #[test]
    fn test_passes_optimal_benchmark() {
        let analyzer = CriterionDiffAnalyzer::new();
        let sample = MicrobenchmarkSample {
            benchmark_name: "jwt_signature_verification".to_string(),
            base_ns_per_op: 120.0,
            head_ns_per_op: 118.0,
            p99_cpu_cycles_base: 300,
            p99_cpu_cycles_head: 295,
        };
        assert_eq!(
            analyzer.evaluate_benchmark_diff(&sample),
            BenchmarkRegressionVerdict::Optimal
        );
    }
}
