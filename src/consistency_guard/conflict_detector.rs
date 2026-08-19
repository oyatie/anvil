#[derive(Clone, Debug)]
pub struct ConsistencyFinding {
    pub is_split_brain_risk: bool,
    pub has_version_vector: bool,
    pub description: String,
}

#[derive(Clone, Debug, Default)]
pub struct ConflictDetector;

impl ConflictDetector {
    pub fn new() -> Self {
        Self
    }

    pub fn detect_consistency_invariants(&self, diff_content: &str) -> Vec<ConsistencyFinding> {
        let mut findings = Vec::new();

        for line in diff_content.lines() {
            if line.starts_with('+') {
                let lower = line.to_lowercase();
                if lower.contains("global_table")
                    || lower.contains("multi_region")
                    || lower.contains("cross_region")
                {
                    let has_vector = lower.contains("vector_clock")
                        || lower.contains("lamport")
                        || lower.contains("crdt")
                        || lower.contains("version");

                    let split_brain = !has_vector
                        && (lower.contains("raw_write")
                            || lower.contains("blind_overwrite")
                            || lower.contains("put_item")
                            || lower.contains("insert")
                            || lower.contains("update_item"));

                    findings.push(ConsistencyFinding {
                        is_split_brain_risk: split_brain,
                        has_version_vector: has_vector,
                        description: line.trim().to_string(),
                    });
                }
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_blind_cross_region_overwrite() {
        let detector = ConflictDetector::new();
        let diff = "+ multi_region_db.raw_write(key, val);";
        let findings = detector.detect_consistency_invariants(diff);
        assert!(!findings.is_empty());
        assert!(findings[0].is_split_brain_risk);
    }

    #[test]
    fn test_passes_vector_clock_update() {
        let detector = ConflictDetector::new();
        let diff = "+ multi_region_db.write_with_vector_clock(key, val, vector_clock);";
        let findings = detector.detect_consistency_invariants(diff);
        assert!(!findings.is_empty());
        assert!(!findings[0].is_split_brain_risk);
    }
}
