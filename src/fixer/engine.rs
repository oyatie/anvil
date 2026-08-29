use anyhow::{Context, Result, bail};
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
            let mut check_cmd = Command::new("cargo");
            check_cmd.current_dir(repo_dir).arg("check");
            let check_out = crate::exec::run_bounded(
                check_cmd,
                crate::exec::ExecClass::Build,
                "cargo check verification gate",
            )
            .await;

            // A spawn failure previously fell through this `if let` and reached
            // the `Ok(true)` at the end of the branch, so a missing cargo
            // reported "verification gate PASSED". Invariant I1: a gate that
            // could not run must never pass.
            match check_out {
                Ok(out) if out.status.success() => {}
                Ok(_) => {
                    warn!("cargo check failed during verification gate");
                    return Ok(false);
                }
                Err(e) => {
                    bail!("verification gate could not run `cargo check`: {}", e);
                }
            }

            let mut test_cmd = Command::new("cargo");
            test_cmd
                .current_dir(repo_dir)
                .args(["test", "--no-fail-fast"]);
            let test_out = crate::exec::run_bounded(
                test_cmd,
                crate::exec::ExecClass::Build,
                "cargo test verification gate",
            )
            .await;

            match test_out {
                Ok(out) if out.status.success() => {}
                Ok(_) => {
                    warn!("cargo test failed during verification gate");
                    return Ok(false);
                }
                Err(e) => {
                    bail!("verification gate could not run `cargo test`: {}", e);
                }
            }
            info!("Cargo verification gate PASSED");
            return Ok(true);
        }

        // 2. Node/TypeScript project (package.json)
        if repo_dir.join("package.json").exists() {
            info!("Detected Node/TypeScript project; running tests...");
            let mut npm_cmd = Command::new("npm");
            npm_cmd
                .current_dir(repo_dir)
                .args(["test", "--", "--passWithNoTests"]);
            let npm_test = crate::exec::run_bounded(
                npm_cmd,
                crate::exec::ExecClass::Build,
                "npm test verification gate",
            )
            .await;

            match npm_test {
                Ok(out) => {
                    if out.status.success() {
                        info!("npm test PASSED");
                        return Ok(true);
                    }
                }
                // A gate that could not run (spawn failure or timeout) must not
                // fall through to the `Ok(true)` at the end of this function.
                Err(e) => {
                    bail!("verification gate could not run `npm test`: {}", e);
                }
            }
        }

        // 3. Go project (go.mod)
        if repo_dir.join("go.mod").exists() {
            info!("Detected Go project; running `go test ./...`...");
            let mut go_cmd = Command::new("go");
            go_cmd.current_dir(repo_dir).args(["test", "./..."]);
            let go_test = crate::exec::run_bounded(
                go_cmd,
                crate::exec::ExecClass::Build,
                "go test verification gate",
            )
            .await;

            match go_test {
                Ok(out) => {
                    if out.status.success() {
                        info!("Go test gate PASSED");
                        return Ok(true);
                    }
                }
                // Same rule as above: an unrunnable gate is not a passing gate.
                Err(e) => {
                    bail!("verification gate could not run `go test`: {}", e);
                }
            }
        }

        Ok(true)
    }

    pub async fn attempt_self_correction(&self, repo_dir: &Path) -> Result<()> {
        let mut diff_cmd = Command::new("git");
        diff_cmd.current_dir(repo_dir).args(["diff"]);
        let diff_out = crate::exec::run_bounded(
            diff_cmd,
            crate::exec::ExecClass::Quick,
            "git diff for self-correction",
        )
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
        let budget = crate::exec::ExecClass::Model.timeout();
        let mut cmd = crate::exec::agent("agy", &crate::exec::Posture::in_workspace(working_dir));
        crate::exec::turn::agy_turn(&mut cmd, &self.agy_effort, budget);

        let turn = crate::exec::turn::run(cmd, prompt, budget, "agy fix prompt")
            .await
            .context("Failed to run agy command")?;

        if !turn.status.success() {
            error!("agy returned non-zero status: {}", turn.status);
            warn!("agy stderr: {}", turn.stderr);
        }

        // Same rule as the queue healer, and for the same reason: this agent
        // edits the workspace directly, so a run that died mid-edit has left
        // the tree in a state nobody chose. Partial output is not partial
        // success.
        turn.into_result()
    }
}
