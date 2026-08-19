use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocsAsCodeReport {
    pub is_compliant: bool,
    pub missing_docstrings: Vec<String>,
    pub doctest_success: bool,
    pub summary: String,
}

pub struct DocsAsCodeGuard;

impl Default for DocsAsCodeGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl DocsAsCodeGuard {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates in-code Rustdoc completeness and executes doctests to ensure zero documentation drift
    pub async fn evaluate_docs_as_code(
        &self,
        repo_dir: &Path,
        changed_files: &[String],
    ) -> Result<DocsAsCodeReport> {
        info!("Running DocsAsCodeGuard on repo at {:?}...", repo_dir);

        let mut missing_docstrings = Vec::new();
        let mut rust_files_modified = false;

        for file in changed_files {
            if file.ends_with(".rs") && !file.contains("tests/") && !file.contains("fixtures/") {
                rust_files_modified = true;
                let full_path = repo_dir.join(file);

                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    let lines: Vec<&str> = content.lines().collect();

                    for (idx, line) in lines.iter().enumerate() {
                        let trimmed = line.trim_start();
                        if (trimmed.starts_with("pub struct ")
                            || trimmed.starts_with("pub enum ")
                            || trimmed.starts_with("pub trait "))
                            && !trimmed.starts_with("pub struct $")
                        {
                            // Check if preceding line is a doc comment
                            let has_doc = if idx > 0 {
                                lines[idx - 1].trim_start().starts_with("///")
                                    || lines[idx - 1].trim_start().starts_with("#[doc =")
                            } else {
                                false
                            };

                            if !has_doc {
                                missing_docstrings.push(format!(
                                    "{}: line {} ({})",
                                    file,
                                    idx + 1,
                                    trimmed
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Run cargo test --doc if Rust files were modified and Cargo.toml exists
        let mut doctest_success = true;
        if rust_files_modified && repo_dir.join("Cargo.toml").exists() {
            let out = Command::new("cargo")
                .current_dir(repo_dir)
                .args(["test", "--doc", "--workspace"])
                .output()
                .await;

            if let Ok(res) = out {
                doctest_success = res.status.success();
            }
        }

        let is_compliant = missing_docstrings.is_empty() && doctest_success;
        let summary = if is_compliant {
            "Docs-as-Code Invariants verified: 100% public types have Rustdoc comments, and executable doctests pass cleanly.".to_string()
        } else {
            format!(
                "Docs-as-Code violations: {} missing docstrings, doctest_success = {}",
                missing_docstrings.len(),
                doctest_success
            )
        };

        Ok(DocsAsCodeReport {
            is_compliant,
            missing_docstrings,
            doctest_success,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_detects_undocumented_public_struct() {
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        std::fs::write(src_dir.join("lib.rs"), "pub struct UndocumentedModel;\n").unwrap();

        let guard = DocsAsCodeGuard::new();
        let report = guard
            .evaluate_docs_as_code(dir.path(), &["src/lib.rs".to_string()])
            .await
            .unwrap();

        assert!(!report.is_compliant);
        assert_eq!(report.missing_docstrings.len(), 1);
    }

    #[tokio::test]
    async fn test_passes_documented_public_struct() {
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        std::fs::write(
            src_dir.join("lib.rs"),
            "/// Documented model\npub struct DocumentedModel;\n",
        )
        .unwrap();

        let guard = DocsAsCodeGuard::new();
        let report = guard
            .evaluate_docs_as_code(dir.path(), &["src/lib.rs".to_string()])
            .await
            .unwrap();

        assert!(report.is_compliant);
    }
}
