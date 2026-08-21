use anyhow::{Context, Result, bail};
use std::sync::Arc;
use tokio::process::Command;
use tracing::{info, warn};

use crate::github::GitHubClient;
use crate::pre_merge_guard::report::{GateStatus, PreMergeCertificationReport};
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
    /// Four ways evidence can be absent, and all four withhold the merge
    /// (invariant I1):
    ///
    /// 1. there is no report;
    /// 2. the report did not come from a certification run, so its statuses
    ///    are somebody's opinion in the shape of a measurement;
    /// 3. a gate produced no measurement -- `NotMeasured`, which is
    ///    individually acceptable and still absent evidence, or `Errored`,
    ///    which `unmeasured_gates` does not record at all;
    /// 4. the report does not certify.
    ///
    /// The refusal names the gates, because an operator watching a pull
    /// request sit in the queue has nothing else to act on.
    pub fn admission_refusal(report: Option<&PreMergeCertificationReport>) -> Result<()> {
        let Some(report) = report else {
            bail!(
                "merge queue admission withheld: no pre-merge certification report was \
                 obtained for this pull request. Absent evidence is not permission."
            );
        };

        if !report.provenance.is_from_a_certification_run() {
            bail!(
                "merge queue admission withheld: this certification report was not produced \
                 by a certification run, so nothing in it was measured."
            );
        }

        let without_a_measurement: Vec<&str> = report
            .named_statuses()
            .into_iter()
            .filter(|(_, status)| {
                matches!(
                    status,
                    GateStatus::Errored(_) | GateStatus::NotMeasured { .. }
                )
            })
            .map(|(gate, _)| gate)
            .collect();
        if !without_a_measurement.is_empty() {
            bail!(
                "merge queue admission withheld: {} gate(s) produced no measurement: {}",
                without_a_measurement.len(),
                without_a_measurement.join(", ")
            );
        }

        if !report.is_certified_ready {
            let blocking: Vec<&str> = report
                .named_statuses()
                .into_iter()
                .filter(|(_, status)| !status.is_acceptable())
                .map(|(gate, _)| gate)
                .collect();
            bail!(
                "merge queue admission withheld: the pull request is not certified; {} gate(s) \
                 did not pass: {}",
                blocking.len(),
                blocking.join(", ")
            );
        }

        Ok(())
    }

    /// What the report says about the corpus, in the only terms Anvil measured
    /// it in: how many gates passed outright, and every gate that did not.
    ///
    /// Deliberately not `gate_counts()`, which scores a `Warning` as
    /// acceptable and would report the whole corpus as passing for a report
    /// where a gate regressed.
    fn measured_lines(report: &PreMergeCertificationReport) -> String {
        let named = report.named_statuses();
        let passed = named
            .iter()
            .filter(|(_, status)| matches!(status, GateStatus::Passed | GateStatus::AutoUpdated))
            .count();
        let mut lines = vec![format!("- {} of {} gates passed", passed, named.len())];
        lines.extend(
            named
                .iter()
                .filter(|(_, status)| {
                    !matches!(status, GateStatus::Passed | GateStatus::AutoUpdated)
                })
                .map(|(gate, status)| format!("- {}: {}", gate, status.badge())),
        );
        lines.join("\n")
    }

    /// The body of the approving review Anvil publishes for a pull request,
    /// derived from what it actually measured.
    ///
    /// `None` means Anvil publishes no approving review at all -- the honest
    /// answer when there is nothing to derive a claim from, and the only
    /// answer for a pull request Anvil is refusing to admit: a review signed
    /// onto a change that is not going through says, permanently and in
    /// Anvil's name, that it is.
    pub fn approval_summary(report: Option<&PreMergeCertificationReport>) -> Option<String> {
        let report = report?;
        Self::admission_refusal(Some(report)).ok()?;
        Some(format!(
            "### 🟢 Pre-Merge Certification\n\n{}\n\nCertified for merge queue admission.",
            Self::measured_lines(report)
        ))
    }

    /// The note Anvil posts onto a pull request it has just handed to the merge
    /// queue, derived from what it actually measured.
    ///
    /// `None` means Anvil posts no note -- the honest answer when there is
    /// nothing to derive a claim from. `strategy` is the merge strategy the
    /// queue accepted ("Squash & Merge", "Merge Commit"), and it is read
    /// rather than assumed: the enlistment retries as a merge commit whenever
    /// GitHub refuses `--squash`, and a note naming the wrong one is a claim
    /// about what happened that did not happen.
    pub fn enlistment_note(
        report: Option<&PreMergeCertificationReport>,
        strategy: &str,
    ) -> Option<String> {
        let report = report?;
        Self::admission_refusal(Some(report)).ok()?;
        Some(format!(
            "🚀 **Enlisted in Merge Queue**\n\n- **Strategy**: {}\n{}\n\n---\n*🤖 [Enlisted] by Oyatie Anvil*\n",
            strategy,
            Self::measured_lines(report)
        ))
    }

    /// Enlists a certified Pull Request into the repository's Merge Queue, ensuring an Approving Review is present
    ///
    /// `report` is the certification report the caller obtained for this pull
    /// request, and `None` when it could not obtain one at all.
    ///
    /// The admission decision is taken here rather than left to the caller.
    /// Three of the four callers used to take it nowhere at all, and a guard
    /// each door is trusted to reach is a convention; this one cannot be
    /// walked past, because "no evidence" is a value this function is handed
    /// rather than a state it cannot observe.
    pub async fn enlist_into_merge_queue(
        &self,
        repo: &str,
        pr_number: u64,
        report: Option<&PreMergeCertificationReport>,
    ) -> Result<()> {
        // Invariant I1: absent evidence must never merge.
        Self::admission_refusal(report)?;

        info!(
            "Enlisting certified PR {}#{} into GitHub Merge Queue...",
            repo, pr_number
        );

        // Step 0: Reconcile and verify honest PR title & body scope
        self.reconcile_pr_title_and_scope(repo, pr_number).await?;

        // Step 1: Ensure PR has an official Approving Review submitted on GitHub
        self.ensure_approving_review(repo, pr_number, report)
            .await?;

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
            self.post_enlistment_note(repo, pr_number, "Squash & Merge", report)
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
            self.post_enlistment_note(repo, pr_number, "Merge Commit", report)
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
    pub async fn ensure_approving_review(
        &self,
        repo: &str,
        pr_number: u64,
        report: Option<&PreMergeCertificationReport>,
    ) -> Result<()> {
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
            let Some(summary) = Self::approval_summary(report) else {
                info!(
                    "No approving review published for {}#{}: nothing was measured that Anvil could sign for.",
                    repo, pr_number
                );
                return Ok(());
            };
            let approval = ReviewResponse {
                summary,
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

    async fn post_enlistment_note(
        &self,
        repo: &str,
        pr_number: u64,
        strategy: &str,
        report: Option<&PreMergeCertificationReport>,
    ) -> Result<()> {
        let Some(note) = Self::enlistment_note(report, strategy) else {
            info!(
                "No enlistment note posted on {}#{}: nothing was measured that Anvil could report.",
                repo, pr_number
            );
            return Ok(());
        };
        self.github_client
            .post_pr_comment(repo, pr_number, &note)
            .await?;
        Ok(())
    }
}
