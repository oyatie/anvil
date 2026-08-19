#[derive(Clone, Debug, Default)]
pub struct CompatibilityChecker;

impl CompatibilityChecker {
    pub fn new() -> Self {
        Self
    }

    pub fn check_schema_diff(&self, diff_content: &str) -> Vec<String> {
        let mut violations = Vec::new();

        for line in diff_content.lines() {
            if line.starts_with('-') {
                let lower = line.to_lowercase();
                if (lower.contains("required")
                    || lower.contains("string")
                    || lower.contains("int32")
                    || lower.contains("int64"))
                    && lower.contains("=")
                {
                    violations.push(format!(
                        "Breaking deletion of wire schema field: {}",
                        line.trim()
                    ));
                }
            } else if line.starts_with('+') {
                let lower = line.to_lowercase();
                if lower.contains("required")
                    && !lower.contains("default")
                    && !lower.contains("optional")
                {
                    violations.push(format!(
                        "Addition of non-backward-compatible required field without default: {}",
                        line.trim()
                    ));
                }
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catches_deleted_proto_field() {
        let checker = CompatibilityChecker::new();
        let diff = "- string user_id = 1;";
        let violations = checker.check_schema_diff(diff);
        assert!(!violations.is_empty());
    }

    #[test]
    fn test_passes_optional_field_addition() {
        let checker = CompatibilityChecker::new();
        let diff = "+ optional string client_version = 2;";
        let violations = checker.check_schema_diff(diff);
        assert!(violations.is_empty());
    }
}
