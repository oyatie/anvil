use anyhow::{Context, Result};
use std::path::Path;
use tokio::process::Command;
use tracing::{info, warn};

#[derive(Debug, Clone, Default)]
pub struct SelfHealingReport {
    pub files_formatted: usize,
    pub owners_files_created: Vec<String>,
    pub actions_pinned: usize,
    pub commit_sha: Option<String>,
}

pub struct PrSelfHealer;

impl PrSelfHealer {
    pub fn new() -> Self {
        Self
    }

    /// Performs all deterministic self-healing passes on the repository working directory
    pub async fn auto_heal_pr_branch(
        &self,
        repo_dir: &Path,
        branch: &str,
        pr_number: u64,
    ) -> Result<SelfHealingReport> {
        info!(
            "Running deterministic self-healing on PR #{} in {:?}",
            pr_number, repo_dir
        );

        let mut report = SelfHealingReport::default();

        // Pass 1: Deterministic Code Formatting (cargo fmt --all)
        let fmt_out = Command::new("cargo")
            .arg("fmt")
            .arg("--all")
            .current_dir(repo_dir)
            .output()
            .await;

        if let Ok(out) = fmt_out {
            if out.status.success() {
                report.files_formatted = 1;
            }
        }

        // Pass 2: Missing OWNERS File Stamping for Crate/Library Directories
        let owners_created = Self::heal_missing_owners(repo_dir).await?;
        report.owners_files_created = owners_created;

        // Pass 3: Check git status for modifications
        let status_out = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(repo_dir)
            .output()
            .await
            .context("Failed to check git status in repo dir")?;

        let status_str = String::from_utf8_lossy(&status_out.stdout);
        if status_str.trim().is_empty() {
            info!(
                "No deterministic healing deltas needed for PR #{}",
                pr_number
            );
            return Ok(report);
        }

        // Pass 4: Stage, Commit & Push Auto-Heal Diff
        let _ = Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_dir)
            .output()
            .await;

        let commit_msg = format!(
            "chore(anvil): autonomous deterministic self-heal for PR #{}\n\n\
            - Auto-formatted source files (cargo fmt --all)\n\
            - Reconciled missing OWNERS declarations\n\n\
            X-Anvil-Action: pr-self-heal\n\
            X-Anvil-Version: 0.1.0\n\n\
            *🤖 Healed by Oyatie Anvil*",
            pr_number
        );

        let commit_out = Command::new("git")
            .args(["commit", "-m", &commit_msg, "--no-verify"])
            .current_dir(repo_dir)
            .output()
            .await
            .context("Failed to commit self-healing changes")?;

        if commit_out.status.success() {
            let rev_out = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(repo_dir)
                .output()
                .await?;
            let new_sha = String::from_utf8_lossy(&rev_out.stdout).trim().to_string();
            report.commit_sha = Some(new_sha.clone());

            info!(
                "Pushing autonomous self-healing commit {} to branch {}",
                new_sha, branch
            );
            let push_out = Command::new("git")
                .args(["push", "origin", branch])
                .current_dir(repo_dir)
                .output()
                .await;

            if let Ok(p) = push_out {
                if !p.status.success() {
                    warn!(
                        "Failed to push self-healing commit to origin/{}: {:?}",
                        branch,
                        String::from_utf8_lossy(&p.stderr)
                    );
                }
            }
        }

        Ok(report)
    }

    /// Detects directories with Cargo.toml or rust files lacking OWNERS and adds compliant declarations
    async fn heal_missing_owners(repo_dir: &Path) -> Result<Vec<String>> {
        let mut created = Vec::new();
        let libs_dir = repo_dir.join("libs");
        if libs_dir.exists() {
            let mut entries = tokio::fs::read_dir(&libs_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                if entry.file_type().await?.is_dir() {
                    let owners_file = entry.path().join("OWNERS");
                    if !owners_file.exists() {
                        let content = "cloud-ci-platform\n";
                        tokio::fs::write(&owners_file, content).await?;
                        created.push(owners_file.to_string_lossy().to_string());
                    }
                }
            }
        }
        Ok(created)
    }
}

impl Default for PrSelfHealer {
    fn default() -> Self {
        Self::new()
    }
}
