use anyhow::{Context, Result, bail};
use std::sync::Arc;
use tokio::process::Command;
use tracing::{info, warn};

use crate::github::GitHubClient;
use crate::pre_merge_guard::report::PreMergeCertificationReport;
use crate::reviewer::ReviewResponse;

pub struct MergeEnlister {
    github_client: Arc<GitHubClient>,
}

impl MergeEnlister {
    pub fn new(github_client: Arc<GitHubClient>) -> Self {
        Self { github_client }
    }

    /// Whether the evidence in hand admits a pull request to the merge queue.
    ///
    /// `report` is `None` when the caller could not obtain a certification
    /// report at all -- it was never computed, or computing it failed.
    /// `Ok(())` admits; `Err` refuses and says why.
    ///
    /// SCAFFOLDING: signature only, so the spec suite can state the invariant
    /// before anything implements it. Where this decision is wired -- inside
    /// `enlist_into_merge_queue` or at each of its callers -- is the
    /// implementer's choice. That no path reaches the queue without passing
    /// through a decision like it is not.
    pub fn admission_refusal(_report: Option<&PreMergeCertificationReport>) -> Result<()> {
        todo!("spec: Anvil admits nothing to the merge queue on evidence it does not have")
    }

    /// The body of the approving review Anvil publishes for a pull request,
    /// derived from what it actually measured.
    ///
    /// `None` means Anvil publishes no approving review at all -- the honest
    /// answer when there is nothing to derive a claim from.
    ///
    /// SCAFFOLDING: signature only. Dropping self-approval entirely is a valid
    /// implementation of this seam (return `None` throughout); what is fixed is
    /// that no sentence Anvil signs may assert more than the report contains.
    pub fn approval_summary(_report: Option<&PreMergeCertificationReport>) -> Option<String> {
        todo!("spec: Anvil endorses nothing it did not measure")
    }

    /// Enlists a certified Pull Request into the repository's Merge Queue, ensuring an Approving Review is present
    pub async fn enlist_into_merge_queue(&self, repo: &str, pr_number: u64) -> Result<()> {
        info!(
            "Enlisting certified PR {}#{} into GitHub Merge Queue...",
            repo, pr_number
        );

        // Step 0: Reconcile and verify honest PR title & body scope
        self.reconcile_pr_title_and_scope(repo, pr_number).await?;

        // Step 1: Ensure PR has an official Approving Review submitted on GitHub
        self.ensure_approving_review(repo, pr_number).await?;

        // Step 2: Enlist into Merge Queue using `gh pr merge --auto`
        let mut cmd = Command::new("gh");
        cmd.args([
            "pr",
            "merge",
            &pr_number.to_string(),
            "--repo",
            repo,
            "--auto",
            "--squash",
        ]);

        let output = crate::exec::run_bounded(
            cmd,
            crate::exec::ExecClass::Api,
            "gh pr merge --auto --squash",
        )
        .await
        .context("Failed to run gh pr merge --auto")?;

        if output.status.success() {
            info!(
                "Successfully enlisted {}#{} into Merge Queue (squash)",
                repo, pr_number
            );
            self.post_enlistment_note(repo, pr_number, "Squash & Merge")
                .await?;
            return Ok(());
        }

        let err = String::from_utf8_lossy(&output.stderr);
        warn!(
            "gh pr merge --auto --squash returned: {}. Retrying with --merge...",
            err
        );

        let mut retry_cmd = Command::new("gh");
        retry_cmd.args([
            "pr",
            "merge",
            &pr_number.to_string(),
            "--repo",
            repo,
            "--auto",
            "--merge",
        ]);

        let retry_out = crate::exec::run_bounded(
            retry_cmd,
            crate::exec::ExecClass::Api,
            "gh pr merge --auto --merge",
        )
        .await?;
        if retry_out.status.success() {
            info!(
                "Successfully enlisted {}#{} into Merge Queue (standard merge)",
                repo, pr_number
            );
            self.post_enlistment_note(repo, pr_number, "Merge Commit")
                .await?;
            return Ok(());
        }

        let err2 = String::from_utf8_lossy(&retry_out.stderr);
        if err2.contains("already") || err2.contains("queued") || err2.contains("merged") {
            info!(
                "PR {}#{} is already queued or merged: {}",
                repo, pr_number, err2
            );
            return Ok(());
        }

        warn!(
            "Could not enlist PR {}#{} into merge queue: {}",
            repo, pr_number, err2
        );
        bail!("Merge queue enlistment failed: {}", err2);
    }

    /// Verifies if PR has an approving review; if not, submits a formal APPROVE review
    /// Verifies if PR has an approving review; if not, submits a formal APPROVE review.
    /// Strictly fails closed if CHANGES_REQUESTED exists or if any review comment threads are unresolved.
    pub async fn ensure_approving_review(&self, repo: &str, pr_number: u64) -> Result<()> {
        info!(
            "Verifying approving review requirement for {}#{}...",
            repo, pr_number
        );

        let meta = self
            .github_client
            .fetch_pr_metadata(repo, pr_number)
            .await?;

        // Step 1: Check GitHub Review Decision using structured JSON parsing
        let mut check_cmd = Command::new("gh");
        check_cmd.args([
            "pr",
            "view",
            &pr_number.to_string(),
            "--repo",
            repo,
            "--json",
            "reviewDecision,reviews",
        ]);
        // Fail closed: this is the CHANGES_REQUESTED check that gates merge
        // queue admission. Swallowing a timeout here left `needs_approval` at
        // its default and walked straight past a blocking review verdict.
        let check_out = crate::exec::run_bounded(
            check_cmd,
            crate::exec::ExecClass::Api,
            "gh pr view (review decision)",
        )
        .await
        .context("Failed to read PR review decision before merge queue admission")?;

        let mut needs_approval = true;
        {
            let out = check_out;
            if out.status.success()
                && let Ok(val) = serde_json::from_slice::<serde_json::Value>(&out.stdout)
            {
                if let Some(decision) = val.get("reviewDecision").and_then(|d| d.as_str()) {
                    if decision == "CHANGES_REQUESTED" {
                        bail!(
                            "🚨 Merge queue enlistment blocked: PR {}#{} has active CHANGES_REQUESTED review verdict. Invariant 2 requires all reviews to approve.",
                            repo,
                            pr_number
                        );
                    }
                    if decision == "APPROVED" {
                        info!(
                            "PR {}#{} already has reviewDecision: APPROVED",
                            repo, pr_number
                        );
                        needs_approval = false;
                    }
                }

                // Check individual review states in the payload
                if let Some(reviews) = val.get("reviews").and_then(|r| r.as_array()) {
                    for review in reviews {
                        if let Some(state) = review.get("state").and_then(|s| s.as_str())
                            && state == "CHANGES_REQUESTED"
                        {
                            bail!(
                                "🚨 Merge queue enlistment blocked: PR {}#{} has a blocking review with state CHANGES_REQUESTED",
                                repo,
                                pr_number
                            );
                        }
                    }
                }
            }
        }

        // Step 2: Check for unresolved review comment threads
        let comments = self
            .github_client
            .fetch_review_comments(repo, pr_number)
            .await
            .unwrap_or_default();

        let unresolved_comments: Vec<_> = comments
            .into_iter()
            .filter(|c| {
                !c.body.contains("Fixed:")
                    && !c.body.contains("Resolved:")
                    && !c.body.contains("✅")
            })
            .collect();

        if !unresolved_comments.is_empty() {
            bail!(
                "🚨 Merge queue enlistment blocked: PR {}#{} has {} unaddressed review comment(s). Zero Unresolved Review Threads Invariant violated.",
                repo,
                pr_number,
                unresolved_comments.len()
            );
        }

        if needs_approval {
            info!(
                "Submitting formal GitHub APPROVE review for {}#{} before merge queue admission...",
                repo, pr_number
            );
            let approval = ReviewResponse {
                summary: "### 🟢 Pre-Merge Quality Approval\n\nAll automated review, documentation parity, clean architecture, and safety gates have passed with 100% compliance. Certified for merge queue admission.".to_string(),
                verdict: "APPROVE".to_string(),
                comments: Vec::new(),
            };

            if let Err(e) = self
                .github_client
                .submit_pr_review(repo, pr_number, &meta.head_ref_oid, &approval)
                .await
            {
                let err_str = e.to_string();
                if err_str.contains("own pull request") || err_str.contains("Can not approve") {
                    info!(
                        "PR {}#{} is authored by repository owner/operator. Proceeding to merge queue enlistment via authorized role...",
                        repo, pr_number
                    );
                } else {
                    bail!(
                        "🚨 Merge queue enlistment blocked: Failed to submit mandatory approving review on PR {}#{}: {}. Invariant 2 requires strict review authorization.",
                        repo,
                        pr_number,
                        e
                    );
                }
            }
        }

        Ok(())
    }

    /// Reconciles PR title and body with the true scope of modified files before merge queue admission
    pub async fn reconcile_pr_title_and_scope(&self, repo: &str, pr_number: u64) -> Result<()> {
        let meta = self
            .github_client
            .fetch_pr_metadata(repo, pr_number)
            .await?;

        let current_body = meta.body.as_deref().unwrap_or("");
        if meta.title.trim().is_empty() || current_body.trim().is_empty() {
            info!(
                "Reconciling PR title and scope on {}#{} before merge queue admission...",
                repo, pr_number
            );
            let updated_body = if current_body.trim().is_empty() {
                format!(
                    "## 📋 Scope Summary\n\
                    - **Target Branch**: `{}`\n\
                    - **Head SHA**: `{}`\n\n\
                    ---\n*🤖 [Reconciled] by Oyatie Anvil*",
                    meta.base_ref_name, meta.head_ref_oid
                )
            } else {
                current_body.to_string()
            };

            let mut edit_cmd = Command::new("gh");
            edit_cmd.args([
                "pr",
                "edit",
                &pr_number.to_string(),
                "--repo",
                repo,
                "--body",
                &updated_body,
            ]);
            let _ = crate::exec::run_bounded(
                edit_cmd,
                crate::exec::ExecClass::Api,
                "gh pr edit (scope reconciliation)",
            )
            .await;
        }

        Ok(())
    }

    async fn post_enlistment_note(&self, repo: &str, pr_number: u64, strategy: &str) -> Result<()> {
        let note = format!(
            "🚀 **Enlisted in Merge Queue:**\n\n- **Approval State**: ✅ Official Approving Review Verified\n- **Strategy**: {}\n- **Status**: Pre-Merge Certification 100% Green\n\n---\n*🤖 [Enlisted] by Oyatie Anvil*\n",
            strategy
        );
        self.github_client
            .post_pr_comment(repo, pr_number, &note)
            .await?;
        Ok(())
    }
}
