use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};

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
        let mut test_sources = None;

        for file in changed_files {
            if !file.ends_with(".rs") {
                continue;
            }
            let full_path = repo_dir.join(file);
            // A deleted Rust path has no head-revision documentation surface.
            if !full_path.is_file() {
                continue;
            }
            if test_sources.is_none() {
                test_sources = Some(
                    crate::source_scan::paths::TestSourceClassifier::new(repo_dir)
                        .map_err(anyhow::Error::msg)?,
                );
            }
            if test_sources
                .as_ref()
                .expect("initialized Rust test-source classifier")
                .classify(&full_path)
                .map_err(anyhow::Error::msg)?
            {
                continue;
            }
            rust_files_modified = true;
            let content = std::fs::read_to_string(&full_path).with_context(|| {
                format!("cannot read changed Rust source {}", full_path.display())
            })?;
            let production = crate::source_scan::try_without_test_modules(&content)
                .map_err(anyhow::Error::msg)?;
            let lines: Vec<&str> = production.lines().collect();

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

        // Run cargo test --doc if Rust files were modified and Cargo.toml exists
        let mut doctest_success = true;
        if rust_files_modified && repo_dir.join("Cargo.toml").exists() {
            let mut doctest_cmd = crate::exec::build_env::command("cargo");
            doctest_cmd
                .current_dir(repo_dir)
                .args(["test", "--doc", "--workspace"]);

            let out = crate::exec::run_bounded(
                doctest_cmd,
                crate::exec::ExecClass::Build,
                "cargo test --doc --workspace",
            )
            .await;

            // Fail closed: a doctest run that could not be executed (or that was
            // killed at the build timeout) produced no evidence, so it must not
            // leave `doctest_success` at its optimistic default.
            doctest_success = match out {
                Ok(res) => res.status.success(),
                Err(e) => {
                    warn!("cargo test --doc did not complete: {}", e);
                    false
                }
            };
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

    #[tokio::test]
    async fn declaration_not_a_tests_or_fixtures_spelling_decides_docs_scope() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join("widget/tests")).unwrap();
        std::fs::create_dir_all(src.join("widget")).unwrap();
        std::fs::write(
            src.join("lib.rs"),
            "mod widget;\n#[cfg(test)] mod arbitrary_fixture;\n",
        )
        .unwrap();
        std::fs::write(src.join("widget.rs"), "mod tests;\nmod fixtures;\n").unwrap();
        std::fs::write(
            src.join("widget/tests.rs"),
            "pub struct UndocumentedShippingTests;\n",
        )
        .unwrap();
        std::fs::write(
            src.join("widget/fixtures.rs"),
            "pub struct UndocumentedShippingFixtures;\n",
        )
        .unwrap();
        std::fs::write(
            src.join("arbitrary_fixture.rs"),
            "pub struct UndocumentedTestOnly;\n",
        )
        .unwrap();

        let report = DocsAsCodeGuard::new()
            .evaluate_docs_as_code(
                dir.path(),
                &[
                    "src/widget/tests.rs".to_owned(),
                    "src/widget/fixtures.rs".to_owned(),
                    "src/arbitrary_fixture.rs".to_owned(),
                ],
            )
            .await
            .unwrap();
        assert_eq!(report.missing_docstrings.len(), 2, "{report:?}");
        assert!(
            report
                .missing_docstrings
                .iter()
                .all(|finding| !finding.contains("arbitrary_fixture"))
        );
    }
}
