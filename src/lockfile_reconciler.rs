use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::process::Command;
use tracing::{info, warn};

use crate::git_manager::GitManager;
use crate::github::GitHubClient;

pub struct LockfileReconciler {
    git_mgr: Arc<GitManager>,
    github_client: Arc<GitHubClient>,
}

impl LockfileReconciler {
    pub fn new(git_mgr: Arc<GitManager>, github_client: Arc<GitHubClient>) -> Self {
        Self {
            git_mgr,
            github_client,
        }
    }

    /// Reconciles lockfiles and truth ledgers for a Pull Request branch
    pub async fn reconcile_pr(&self, repo: &str, pr_number: u64) -> Result<bool> {
        info!("Running lockfile and ledger reconciliation for {}#{}...", repo, pr_number);

        let meta = self.github_client.fetch_pr_metadata(repo, pr_number).await?;
        let repo_dir = self.git_mgr.ensure_repo_cloned(repo).await?;

        // Checkout PR branch
        let _ = Command::new("git")
            .current_dir(&repo_dir)
            .args(["fetch", "origin", &format!("pull/{}/head", pr_number), "--force"])
            .output()
            .await;

        let branch_name = format!("pr-{}", pr_number);
        let _ = Command::new("git")
            .current_dir(&repo_dir)
            .args(["checkout", "-B", &branch_name, "FETCH_HEAD"])
            .output()
            .await;

        // 1. Rust Cargo lockfile reconciliation
        if repo_dir.join("Cargo.toml").exists() {
            info!("Reconciling Cargo.lock in {:?}", repo_dir);
            let _ = Command::new("cargo")
                .current_dir(&repo_dir)
                .args(["check", "--quiet"])
                .output()
                .await;
        }

        // 2. Node.js package-lock reconciliation
        if repo_dir.join("package.json").exists() {
            info!("Reconciling package-lock.json in {:?}", repo_dir);
            let _ = Command::new("npm")
                .current_dir(&repo_dir)
                .args(["install", "--package-lock-only", "--ignore-scripts"])
                .output()
                .await;
        }

        // 3. Truth Ledgers & Documentation Manifests
        let doc_manifest_script = repo_dir.join("scripts/console/generate-documentation-manifest.mjs");
        if doc_manifest_script.exists() {
            info!("Reconciling documentation manifest in {:?}", repo_dir);
            let _ = Command::new("node")
                .current_dir(&repo_dir)
                .arg("scripts/console/generate-documentation-manifest.mjs")
                .output()
                .await;
        }

        let adr_index_script = repo_dir.join("scripts/console/generate-adr-index.mjs");
        if adr_index_script.exists() {
            info!("Reconciling ADR index in {:?}", repo_dir);
            let _ = Command::new("node")
                .current_dir(&repo_dir)
                .arg("scripts/console/generate-adr-index.mjs")
                .output()
                .await;
        }

        // 4. Check for modified files
        let status_out = Command::new("git")
            .current_dir(&repo_dir)
            .args(["status", "--porcelain"])
            .output()
            .await
            .context("Failed to check git status")?;

        let modified_lines = String::from_utf8_lossy(&status_out.stdout);
        let reconciled_files: Vec<String> = modified_lines
            .lines()
            .map(|l| l[3..].trim().to_string())
            .filter(|f| f.contains("lock") || f.contains("manifest") || f.contains("index"))
            .collect();

        if reconciled_files.is_empty() {
            info!("No lockfile or ledger drift found for {}#{}", repo, pr_number);
            return Ok(false);
        }

        info!("Lockfiles/ledgers reconciled: {:?}. Committing & pushing...", reconciled_files);

        for file in &reconciled_files {
            let _ = Command::new("git")
                .current_dir(&repo_dir)
                .args(["add", file])
                .output()
                .await;
        }

        let commit_msg = format!("chore(deps): auto-reconcile lockfiles and documentation ledgers on PR #{}", pr_number);
        let _ = Command::new("git")
            .current_dir(&repo_dir)
            .args(["commit", "-m", &commit_msg])
            .output()
            .await;

        let push_target = format!("HEAD:{}", meta.head_ref_name);
        let push_out = Command::new("git")
            .current_dir(&repo_dir)
            .args(["push", "origin", &push_target])
            .output()
            .await?;

        if push_out.status.success() {
            info!("Pushed reconciled lockfiles to origin/{}", meta.head_ref_name);
            Ok(true)
        } else {
            let err = String::from_utf8_lossy(&push_out.stderr);
            warn!("Failed to push reconciled lockfiles: {}", err);
            Ok(false)
        }
    }
}
