//! What a change did to a file, not what the file already was.
//!
//! Both checks here read the file from disk and judged its total state, so a
//! pull request that touched a large file inherited its size, and one that
//! fixed a typo in a core file inherited every I/O import already there. Anvil
//! has 57 files over the 300-line budget; charging every toucher made those
//! files unmergeable, including by the decomposition the gate was asking for.
//!
//! A gate on a pre-existing condition is a ratchet, not a threshold: a file
//! already over budget may not grow, and a line the change did not add is
//! not the change's fault.

use super::MonorepoViolation;
use std::path::Path;

/// What this change did to one file, as the diff reports it.
pub struct FileChange<'a> {
    /// Only the lines this change ADDS, without their `+`.
    pub added: &'a str,
    /// Lines added minus lines removed. Negative means the file shrank.
    pub net_lines: i64,
}

use crate::source_scan::paths::is_test_source as is_test_path;

impl FileChange<'_> {
    fn grew(&self) -> bool {
        self.net_lines > 0
    }

    fn adds(&self, line: &str) -> bool {
        let needle = line.trim();
        !needle.is_empty() && self.added.lines().any(|a| a.trim() == needle)
    }
}

pub struct WholeFileExpansion;

impl WholeFileExpansion {
    pub const MAX_WHOLE_FILE_LINES: usize = 300;

    /// Judges what `change` did to `file_path`.
    ///
    /// The file on disk is still read, because "is this file over budget" and
    /// "does this line import I/O" are properties of the file. What changed is
    /// who is charged: only a file this change GREW, and only a line this
    /// change ADDED.
    pub fn evaluate_whole_file(
        repo_dir: &Path,
        file_path: &str,
        change: &FileChange<'_>,
    ) -> Vec<MonorepoViolation> {
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
        // Over budget AND made worse here. A change that shrinks an oversized
        // file is the remedy this gate asks for and must not be refused for
        // arriving mid-way.
        //
        // Test code is exempt by Cargo's layout, not by whether the word
        // "test" appears in the path. The substring spelling exempted
        // `attestation_guard.rs` and `predictive_test_selector/workspace_dag.rs`
        // -- both production, both already past this ceiling -- because
        // "attestation" and "predictive_test_selector" contain it.
        if line_count > Self::MAX_WHOLE_FILE_LINES
            && change.grew()
            && is_rust
            && !is_test_path(file_path)
        {
            violations.push(MonorepoViolation {
                category: "OVERSIZED_WHOLE_FILE".to_string(),
                description: format!(
                    "File '{}' is {} lines and this change grew it by {}, past the module-size ceiling of {}. Split it into more modules inside the same crate.",
                    file_path, line_count, change.net_lines, Self::MAX_WHOLE_FILE_LINES
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
                // A line that was already here is not this change's finding.
                if !change.adds(line) {
                    continue;
                }
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

        // 3. Raw `unwrap` in production code this change added.
        //
        // Three strippers, none of which this rule had. `without_test_modules`
        // removes `#[cfg(test)]` blocks, which is where `tempdir().unwrap()`
        // lives and why a production file was charged for its own fixtures.
        // `code_only` removes comments and string literals, without which the
        // rule read its own error message and its own comparison line as
        // findings -- it reported itself, three times, on this pull request.
        // And a path substring test is not a test check: it skipped
        // `src/latest_state.rs` for containing "test" while scanning every
        // `#[cfg(test)]` block in every other file.
        if is_rust && !is_test_path(file_path) {
            let production =
                crate::source_scan::code_only(&crate::source_scan::without_test_modules(&content));
            for (idx, line) in production.lines().enumerate() {
                if line.contains(".unwrap()") && change.adds(line) {
                    violations.push(MonorepoViolation {
                        category: "PRODUCTION_UNWRAP_DETECTED".to_string(),
                        description: format!(
                            "Production file '{}' gains a raw unwrap at line {}. Use `?`, `unwrap_or_default()`, or explicit error handling.",
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

        // A change that CREATES the file: every line is added, and the net
        // growth is the whole file. Both findings are this change's.
        let change = FileChange {
            added: &code,
            net_lines: code.lines().count() as i64,
        };
        let violations = WholeFileExpansion::evaluate_whole_file(
            dir.path(),
            "billing/core/src/order.rs",
            &change,
        );
        assert!(
            violations
                .iter()
                .any(|v| v.category == "OVERSIZED_WHOLE_FILE")
        );
        assert!(
            violations
                .iter()
                .any(|v| v.category == "CLEAN_ARCHITECTURE_CORE_IO_VIOLATION")
        );
    }
}
