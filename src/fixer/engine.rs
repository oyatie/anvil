use crate::model_prompt::{HarnessText, ModelPrompt};
use crate::reviewer::untrusted::{Untrusted, UntrustedLabel};
use anyhow::{Context, Result, bail};
use std::path::Path;
use tokio::process::Command;
use tracing::{error, info, warn};

use super::evaluator::{ItemEvaluation, ReviewFeedbackItem};

/// Builds the write-capable fix turn from classified review fields. Returning
/// only [`ModelPrompt`] lets behavioral tests exercise the real sink without
/// exposing a raw prompt constructor.
pub fn build_apply_prompt(
    repo: &str,
    valid_items: &[(ReviewFeedbackItem, ItemEvaluation)],
) -> Result<ModelPrompt> {
    let mut prompt = ModelPrompt::builder();
    prompt.push_harness(HarnessText::FixApplyPreambleAndRepository);
    prompt.push_repository(repo)?;
    prompt.push_harness(HarnessText::FixApplyRepositoryEnd);

    for (index, (item, eval)) in valid_items.iter().enumerate() {
        prompt
            .push_harness(HarnessText::FixApplyItemStart)
            .push_usize(index)
            .push_harness(HarnessText::FixApplyItemHeaderEnd);
        if let Some(path) = item.file_path.as_deref() {
            prompt.push_untrusted(Untrusted::new(UntrustedLabel::FilePath, path));
        } else {
            prompt.push_harness(HarnessText::FixApplyMissingPath);
        }
        prompt.push_untrusted(Untrusted::new(UntrustedLabel::ReviewComment, &item.body));
        if let Some(proposed) = eval.proposed_fix.as_deref() {
            prompt.push_untrusted(Untrusted::new(UntrustedLabel::ProposedFix, proposed));
        } else {
            prompt.push_harness(HarnessText::FixApplyMissingProposal);
        }
        prompt.push_harness(HarnessText::FixApplyItemEnd);
    }

    prompt.push_harness(HarnessText::FixApplyTask);
    prompt.finish()
}

/// Builds the self-correction turn while retaining both path-order extremes of
/// an oversized working diff and restoring trusted instructions at the tail.
pub fn build_self_correction_prompt(diff: &str) -> Result<ModelPrompt> {
    let mut prompt = ModelPrompt::builder();
    prompt
        .push_harness(HarnessText::FixSelfCorrectionPreamble)
        .push_untrusted(Untrusted::new(UntrustedLabel::WorkingDiff, diff))
        .push_harness(HarnessText::FixSelfCorrectionTask);
    prompt.finish()
}

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
        let prompt = build_apply_prompt(repo, valid_items)?;

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

        // The diff is the contributor's, and this prompt drives a turn with
        // write access to the tree. See `reviewer::untrusted`.
        let prompt = build_self_correction_prompt(&diff_str)?;

        let _ = self.run_agy_prompt(&prompt, repo_dir).await?;
        Ok(())
    }

    async fn run_agy_prompt(&self, prompt: &ModelPrompt, working_dir: &Path) -> Result<String> {
        let budget = crate::exec::ExecClass::Model.timeout();
        let cmd = crate::exec::agy_agent(
            &crate::exec::Posture::in_workspace(working_dir),
            &self.agy_effort,
            budget,
            None,
        )?;

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
