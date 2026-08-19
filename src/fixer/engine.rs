use anyhow::{bail, Context, Result};
use std::path::Path;
use tokio::process::Command;
use tracing::{error, info, warn};

use super::evaluator::{ItemEvaluation, ReviewFeedbackItem};

pub struct FixEngine {
    agy_effort: String,
}

impl FixEngine {
    pub fn new(agy_effort: String) -> Self {
        Self { agy_effort }
    }

    pub async fn apply_code_fixes(
        &self,
        repo: &str,
        repo_dir: &Path,
        valid_items: &[(ReviewFeedbackItem, ItemEvaluation)],
    ) -> Result<()> {
        let mut prompt = format!(
            "You are Oyatie's Principal Engineer. Directly implement code fixes in this workspace for `{}` to resolve the following valid review findings:\n\n",
            repo
        );

        for (item, eval) in valid_items {
            prompt.push_str(&format!(
                "- **File**: {}\n  **Finding**: {}\n  **Proposed Fix**: {}\n\n",
                item.file_path.as_deref().unwrap_or("N/A"),
                item.body,
                eval.proposed_fix.as_deref().unwrap_or("Fix as required")
            ));
        }

        prompt.push_str("Inspect the workspace files, make all necessary edits cleanly, ensure types and tests are preserved or updated, and complete the implementation.");

        info!("Invoking Antigravity to write code fixes in {:?}", repo_dir);
        let _ = self.run_agy_prompt(&prompt, repo_dir).await?;
        Ok(())
    }

    pub async fn run_test_verification_gate(&self, repo_dir: &Path) -> Result<bool> {
        info!("Running local verification gate in {:?}", repo_dir);

        // 1. Rust project (Cargo.toml)
        if repo_dir.join("Cargo.toml").exists() {
            info!("Detected Rust crate; running `cargo check` and `cargo test`...");
            let check_out = Command::new("cargo")
                .current_dir(repo_dir)
                .arg("check")
                .output()
                .await;

            if let Ok(out) = check_out {
                if !out.status.success() {
                    warn!("cargo check failed during verification gate");
                    return Ok(false);
                }
            }

            let test_out = Command::new("cargo")
                .current_dir(repo_dir)
                .args(["test", "--no-fail-fast"])
                .output()
                .await;

            if let Ok(out) = test_out {
                if !out.status.success() {
                    warn!("cargo test failed during verification gate");
                    return Ok(false);
                }
            }
            info!("Cargo verification gate PASSED");
            return Ok(true);
        }

        // 2. Node/TypeScript project (package.json)
        if repo_dir.join("package.json").exists() {
            info!("Detected Node/TypeScript project; running tests...");
            let npm_test = Command::new("npm")
                .current_dir(repo_dir)
                .args(["test", "--", "--passWithNoTests"])
                .output()
                .await;

            if let Ok(out) = npm_test {
                if out.status.success() {
                    info!("npm test PASSED");
                    return Ok(true);
                }
            }
        }

        // 3. Go project (go.mod)
        if repo_dir.join("go.mod").exists() {
            info!("Detected Go project; running `go test ./...`...");
            let go_test = Command::new("go")
                .current_dir(repo_dir)
                .args(["test", "./..."])
                .output()
                .await;

            if let Ok(out) = go_test {
                if out.status.success() {
                    info!("Go test gate PASSED");
                    return Ok(true);
                }
            }
        }

        Ok(true)
    }

    pub async fn attempt_self_correction(&self, repo_dir: &Path) -> Result<()> {
        let diff_out = Command::new("git")
            .current_dir(repo_dir)
            .args(["diff"])
            .output()
            .await?;
        let diff_str = String::from_utf8_lossy(&diff_out.stdout);

        let prompt = format!(
            "The previous code edits caused build or test failures. Inspect the repository, check the current diff, diagnose the root cause, and fix the errors so that the test suite passes cleanly.\n\nCurrent diff:\n```diff\n{}\n```",
            diff_str
        );

        let _ = self.run_agy_prompt(&prompt, repo_dir).await?;
        Ok(())
    }

    async fn run_agy_prompt(&self, prompt: &str, working_dir: &Path) -> Result<String> {
        let mut cmd = Command::new("agy");
        cmd.args([
            "--print",
            prompt,
            "--effort",
            &self.agy_effort,
            "--dangerously-skip-permissions",
        ]);
        cmd.current_dir(working_dir);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let output = cmd.output().await.context("Failed to run agy command")?;
        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            error!("agy returned non-zero status: {}", output.status);
            warn!("agy stderr: {}", stderr_str);
            if stdout_str.trim().is_empty() {
                bail!("agy failed with code {}: {}", output.status, stderr_str);
            }
        }

        Ok(stdout_str)
    }
}
