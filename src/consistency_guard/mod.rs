pub mod conflict_detector;

use conflict_detector::ConflictDetector;

#[derive(Clone, Debug)]
pub struct ConsistencyReport {
    pub passed: bool,
    pub split_brain_risks: usize,
    pub unversioned_mutations: usize,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct ActiveActiveConsistencyGuard {
    detector: ConflictDetector,
}

impl Default for ActiveActiveConsistencyGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl ActiveActiveConsistencyGuard {
    pub fn new() -> Self {
        Self {
            detector: ConflictDetector::new(),
        }
    }

    pub fn evaluate_active_active_invariants(&self, diff_content: &str) -> ConsistencyReport {
        let findings = self.detector.detect_consistency_invariants(diff_content);
        let split_brain_count = findings.iter().filter(|f| f.is_split_brain_risk).count();
        let unversioned_count = findings.iter().filter(|f| !f.has_version_vector).count();

        let passed = split_brain_count == 0;
        let summary = if passed {
            "Active-active multi-region operations enforce vector clock ordering & CRDT conflict resolution.".to_string()
        } else {
            format!(
                "Detected {} multi-region split-brain or unversioned concurrent mutation risks.",
                split_brain_count
            )
        };

        ConsistencyReport {
            passed,
            split_brain_risks: split_brain_count,
            unversioned_mutations: unversioned_count,
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consistency_guard_nominal() {
        let guard = ActiveActiveConsistencyGuard::new();
        let diff = "+ let update = crdt_merge(local, remote, lamport_clock);";
        let report = guard.evaluate_active_active_invariants(diff);
        assert!(report.passed);
    }
}
