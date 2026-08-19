use super::MonorepoViolation;
use std::path::Path;

pub struct WholeFileExpansion;

impl WholeFileExpansion {
    pub const MAX_WHOLE_FILE_LINES: usize = 300;

    /// Evaluates the entire file content on disk for all touched files in a PR
    pub fn evaluate_whole_file(repo_dir: &Path, file_path: &str) -> Vec<MonorepoViolation> {
        let mut violations = Vec::new();
        let full_path = repo_dir.join(file_path);

        if !full_path.exists() || !full_path.is_file() {
            return violations;
        }

        // Only evaluate Rust source files and documentation
        let is_rust = file_path.ends_with(".rs");
        let is_doc = file_path.ends_with(".md") || file_path.ends_with(".yaml");

        if !is_rust && !is_doc {
            return violations;
        }

        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => return violations,
        };

        let lines: Vec<&str> = content.lines().collect();
        let line_count = lines.len();

        // 1. Whole-file line limit check
        if line_count > Self::MAX_WHOLE_FILE_LINES && is_rust && !file_path.contains("test") {
            violations.push(MonorepoViolation {
                category: "OVERSIZED_WHOLE_FILE".to_string(),
                description: format!(
                    "Modified file '{}' has {} total lines, exceeding the hyperscaler ceiling of {}. Decompose into cohesive submodules.",
                    file_path, line_count, Self::MAX_WHOLE_FILE_LINES
                ),
                snippet: format!("Total lines: {}", line_count),
            });
        }

        // 2. Clean Architecture Core I/O isolation check
        let is_core_layer = file_path.contains("/core/") || file_path.contains("-domain/");
        if is_core_layer && is_rust {
            let banned_io_keywords = [
                "sqlx::",
                "reqwest::",
                "tokio::net::",
                "std::net::",
                "redis::",
            ];
            for (idx, line) in lines.iter().enumerate() {
                for kw in &banned_io_keywords {
                    if line.contains(kw) {
                        violations.push(MonorepoViolation {
                            category: "CLEAN_ARCHITECTURE_CORE_IO_VIOLATION".to_string(),
                            description: format!(
                                "Domain Core file '{}' imports direct I/O library '{}' at line {}. Domain Core must be 100% pure business logic with zero I/O drivers.",
                                file_path, kw, idx + 1
                            ),
                            snippet: line.trim().to_string(),
                        });
                    }
                }
            }
        }

        // 3. Rust Safety check: raw unwrap in production
        if is_rust && !file_path.contains("test") && !file_path.starts_with("tests/") {
            for (idx, line) in lines.iter().enumerate() {
                if line.contains(".unwrap()") && !line.trim_start().starts_with("//") {
                    violations.push(MonorepoViolation {
                        category: "PRODUCTION_UNWRAP_DETECTED".to_string(),
                        description: format!(
                            "Production file '{}' contains raw .unwrap() at line {}. Use '?' operator, 'unwrap_or_default()', or explicit error handling.",
                            file_path, idx + 1
                        ),
                        snippet: line.trim().to_string(),
                    });
                }
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_catches_oversized_file_and_core_io() {
        let dir = tempdir().unwrap();
        let core_path = dir.path().join("billing/core/src");
        std::fs::create_dir_all(&core_path).unwrap();

        let mut code = "pub struct Order;\nuse sqlx::PgPool;\n".to_string();
        for i in 0..350 {
            code.push_str(&format!("fn helper_{}() {{}}\n", i));
        }
        std::fs::write(core_path.join("order.rs"), &code).unwrap();

        let violations =
            WholeFileExpansion::evaluate_whole_file(dir.path(), "billing/core/src/order.rs");
        assert!(violations
            .iter()
            .any(|v| v.category == "OVERSIZED_WHOLE_FILE"));
        assert!(violations
            .iter()
            .any(|v| v.category == "CLEAN_ARCHITECTURE_CORE_IO_VIOLATION"));
    }
}
