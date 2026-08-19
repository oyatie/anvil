pub mod compatibility_checker;

use compatibility_checker::CompatibilityChecker;

#[derive(Clone, Debug)]
pub struct SchemaEvolutionReport {
    pub passed: bool,
    pub breaking_field_changes: usize,
    pub tag_renumbering_detected: bool,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct SchemaEvolutionRatchet {
    checker: CompatibilityChecker,
}

impl Default for SchemaEvolutionRatchet {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaEvolutionRatchet {
    pub fn new() -> Self {
        Self {
            checker: CompatibilityChecker::new(),
        }
    }

    pub fn evaluate_schema_evolution(&self, diff_content: &str) -> SchemaEvolutionReport {
        let violations = self.checker.check_schema_diff(diff_content);
        let breaking_count = violations.len();
        let tag_renumbering = violations.iter().any(|v| v.contains("tag renumbered"));

        let passed = breaking_count == 0;
        let summary = if passed {
            "All Protobuf, OpenAPI and wire schemas maintain strict forward/backward compatibility."
                .to_string()
        } else {
            format!(
                "Detected {} breaking wire schema changes or tag renumberings.",
                breaking_count
            )
        };

        SchemaEvolutionReport {
            passed,
            breaking_field_changes: breaking_count,
            tag_renumbering_detected: tag_renumbering,
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_evolution_nominal() {
        let ratchet = SchemaEvolutionRatchet::new();
        let diff = "+ optional string new_field = 4;";
        let report = ratchet.evaluate_schema_evolution(diff);
        assert!(report.passed);
    }
}
