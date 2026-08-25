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
        info!(
            "Running lockfile and ledger reconciliation for {}#{}...",
            repo, pr_number
        );

        let meta = self
            .github_client
            .fetch_pr_metadata(repo, pr_number)
            .await?;
        let repo_dir = self.git_mgr.ensure_repo_cloned(repo).await?;

        // Checkout PR branch
        let mut fetch_cmd = Command::new("git");
        fetch_cmd.current_dir(&repo_dir).args([
            "fetch",
            "origin",
            &format!("pull/{}/head", pr_number),
            "--force",
        ]);
        let _ = crate::exec::run_bounded(
            fetch_cmd,
            crate::exec::ExecClass::Vcs,
            "git fetch pull head (lockfile reconciler)",
        )
        .await;

        let branch_name = format!("pr-{}", pr_number);
        let mut checkout_cmd = Command::new("git");
        checkout_cmd
            .current_dir(&repo_dir)
            .args(["checkout", "-B", &branch_name, "FETCH_HEAD"]);
        let _ = crate::exec::run_bounded(
            checkout_cmd,
            crate::exec::ExecClass::Vcs,
            "git checkout -B (lockfile reconciler)",
        )
        .await;

        // 1. Rust Cargo lockfile reconciliation
        if repo_dir.join("Cargo.toml").exists() {
            info!("Reconciling Cargo.lock in {:?}", repo_dir);
            let mut cargo_cmd = Command::new("cargo");
            cargo_cmd.current_dir(&repo_dir).args(["check", "--quiet"]);
            let _ = crate::exec::run_bounded(
                cargo_cmd,
                crate::exec::ExecClass::Build,
                "cargo check --quiet (lockfile reconciler)",
            )
            .await;
        }

        // 2. Node.js package-lock reconciliation
        if repo_dir.join("package.json").exists() {
            info!("Reconciling package-lock.json in {:?}", repo_dir);
            let mut npm_cmd = Command::new("npm");
            npm_cmd.current_dir(&repo_dir).args([
                "install",
                "--package-lock-only",
                "--ignore-scripts",
            ]);
            let _ = crate::exec::run_bounded(
                npm_cmd,
                crate::exec::ExecClass::Build,
                "npm install --package-lock-only",
            )
            .await;
        }

        // 3. Truth Ledgers & Documentation Manifests
        let doc_manifest_script =
            repo_dir.join("scripts/console/generate-documentation-manifest.mjs");
        if doc_manifest_script.exists() {
            info!("Reconciling documentation manifest in {:?}", repo_dir);
            let mut node_cmd = Command::new("node");
            node_cmd
                .current_dir(&repo_dir)
                .arg("scripts/console/generate-documentation-manifest.mjs");
            let _ = crate::exec::run_bounded(
                node_cmd,
                crate::exec::ExecClass::Build,
                "node generate-documentation-manifest.mjs",
            )
            .await;
        }

        let adr_index_script = repo_dir.join("scripts/console/generate-adr-index.mjs");
        if adr_index_script.exists() {
            info!("Reconciling ADR index in {:?}", repo_dir);
            let mut node_cmd = Command::new("node");
            node_cmd
                .current_dir(&repo_dir)
                .arg("scripts/console/generate-adr-index.mjs");
            let _ = crate::exec::run_bounded(
                node_cmd,
                crate::exec::ExecClass::Build,
                "node generate-adr-index.mjs",
            )
            .await;
        }

        // 4. Check for modified files
        let mut status_cmd = Command::new("git");
        status_cmd
            .current_dir(&repo_dir)
            .args(["status", "--porcelain"]);
        let status_out = crate::exec::run_bounded(
            status_cmd,
            crate::exec::ExecClass::Quick,
            "git status --porcelain (lockfile reconciler)",
        )
        .await
        .context("Failed to check git status")?;

        let modified_lines = String::from_utf8_lossy(&status_out.stdout);
        let reconciled_files: Vec<String> = modified_lines
            .lines()
            .map(|l| l[3..].trim().to_string())
            .filter(|f| f.contains("lock") || f.contains("manifest") || f.contains("index"))
            .collect();

        if reconciled_files.is_empty() {
            info!(
                "No lockfile or ledger drift found for {}#{}",
                repo, pr_number
            );
            return Ok(false);
        }

        info!(
            "Lockfiles/ledgers reconciled: {:?}. Committing & pushing...",
            reconciled_files
        );

        for file in &reconciled_files {
            let mut add_cmd = Command::new("git");
            add_cmd.current_dir(&repo_dir).args(["add", file]);
            let _ = crate::exec::run_bounded(
                add_cmd,
                crate::exec::ExecClass::Quick,
                "git add (lockfile reconciler)",
            )
            .await;
        }

        let commit_msg = format!(
            "chore(deps): auto-reconcile lockfiles and documentation ledgers on PR #{}",
            pr_number
        );
        let mut commit_cmd = Command::new("git");
        commit_cmd
            .current_dir(&repo_dir)
            .args(["commit", "-m", &commit_msg]);
        let _ = crate::exec::run_bounded(
            commit_cmd,
            crate::exec::ExecClass::Quick,
            "git commit (lockfile reconciler)",
        )
        .await;

        // Never push to a branch that belongs to a fork; see github::fork_guard.
        crate::github::fork_guard::ensure_push_allowed(repo, pr_number, meta.is_cross_repository)?;
        let push_target = format!("HEAD:{}", meta.head_ref_name);
        let mut push_cmd = Command::new("git");
        push_cmd
            .current_dir(&repo_dir)
            .args(["push", "origin", &push_target]);
        let push_out = crate::exec::run_bounded(
            push_cmd,
            crate::exec::ExecClass::Vcs,
            "git push (lockfile reconciler)",
        )
        .await?;

        if push_out.status.success() {
            info!(
                "Pushed reconciled lockfiles to origin/{}",
                meta.head_ref_name
            );
            Ok(true)
        } else {
            let err = String::from_utf8_lossy(&push_out.stderr);
            warn!("Failed to push reconciled lockfiles: {}", err);
            Ok(false)
        }
    }
}
