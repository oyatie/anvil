use serde::{Deserialize, Serialize};

pub mod lock_graph;
pub use lock_graph::{LockOrderFinding, LockOrderGraph};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadlockReport {
    pub passed: bool,
    pub findings: Vec<LockOrderFinding>,
}

#[derive(Debug, Clone, Default)]
pub struct DeadlockStaticAnalyzer {
    analyzer: LockOrderGraph,
}

impl DeadlockStaticAnalyzer {
    pub fn new() -> Self {
        Self {
            analyzer: LockOrderGraph::new(),
        }
    }

    pub fn evaluate_deadlock_invariants(
        &self,
        file_path: &str,
        diff_content: &str,
    ) -> DeadlockReport {
        let findings = self
            .analyzer
            .find_lock_order_cycles(file_path, diff_content);
        DeadlockReport {
            passed: findings.is_empty(),
            findings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deadlock_analyzer_nominal() {
        let analyzer = DeadlockStaticAnalyzer::new();
        let report = analyzer.evaluate_deadlock_invariants("src/main.rs", "let x = 1;");
        assert!(report.passed);
        assert!(report.findings.is_empty());
    }
}
