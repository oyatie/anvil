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
    /// report at all -- it was never computed, or computing it failed. That is
    /// the one shape of absent evidence the report itself cannot answer for,
    /// so it is answered here; everything the report *can* answer for is
    /// answered by the report, in one place, so the door and the two
    /// publishers cannot drift apart.
    pub fn admission_refusal(report: Option<&PreMergeCertificationReport>) -> Result<()> {
        match report {
            None => bail!(
                "merge queue admission withheld: no pre-merge certification report was \
                 obtained for this pull request. Absent evidence is not permission."
            ),
            Some(report) => report.admission_refusal(),
        }
    }

    /// Whether the report in hand is about *this* pull request at *this*
    /// commit.
    ///
    /// `admission_refusal` proves that some certification run produced an
    /// all-passing report. It cannot prove the run measured the commit that is
    /// about to be queued, and two live paths move the head between the run
    /// and the enlistment: a contributor pushing while the corpus is running,
    /// and the queue healer, which pushes a healed commit and then certifies
    /// whatever GitHub reports as head. A report about commit X is not evidence
    /// about commit Y.
    ///
    /// `head` is the head re-read at the entry point, immediately before the
    /// merge is requested. `gh pr merge --match-head-commit` carries the same
    /// SHA to GitHub, so the queue rejects the merge if it moves again between
    /// this check and the request.
    /// Exposed rather than private because the only caller is the door, and the
    /// door talks to GitHub. Left private, this check was pinned by nothing:
    /// `if false && subject.head_sha != head` kept clippy clean and all 68 test
    /// targets green while admitting a report measured against another commit.
    pub fn subject_refusal(
        report: Option<&PreMergeCertificationReport>,
        repo: &str,
        pr_number: u64,
        head: &str,
    ) -> Result<()> {
        let Some(subject) = report.and_then(|r| r.subject()) else {
            bail!(
                "merge queue admission withheld: the certification report for {}#{} names no \
                 pull request and no commit, so nothing establishes that it was measured \
                 against {}. A report about an unnamed commit is not evidence about this one.",
                repo,
                pr_number,
                head
            );
        };
        if !subject.repo.eq_ignore_ascii_case(repo) || subject.pr_number != pr_number {
            bail!(
                "merge queue admission withheld: the certification report is for {}#{}, and \
                 this is {}#{}.",
                subject.repo,
                subject.pr_number,
                repo,
                pr_number
            );
        }
        if subject.head_sha != head {
            bail!(
                "merge queue admission withheld: the certification report for {}#{} was \
                 measured against {}, and the pull request head is now {}. The corpus never \
                 saw the commit that would be queued.",
                repo,
                pr_number,
                subject.head_sha,
                head
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
    ///
    /// `AutoUpdated` is counted apart from `Passed`, and said out loud. A guard
    /// reports `AutoUpdated` when it found a deficiency and wrote files to fix
    /// it, and those files are staged and committed only in Anvil's own shared
    /// clone -- nothing in the crate pushes them to the pull request's branch.
    /// The enlistment then pins the merge to `meta.head_ref_oid`, the head
    /// *without* the fix. Folded into the passed count, the approving review
    /// published "72 of 72 gates passed" about a tree that will never be merged,
    /// while the commit that does merge is the one whose doc-parity, cedar and
    /// api-contract gates did not pass. A compliance claim has to be about the
    /// commit being endorsed.
    fn measured_lines(report: &PreMergeCertificationReport) -> String {
        let named = report.named_statuses();
        let is_clean = |status: &GateStatus| matches!(status, GateStatus::Passed);
        let is_auto = |status: &GateStatus| matches!(status, GateStatus::AutoUpdated);
        let passed = named.iter().filter(|(_, s)| is_clean(s)).count();
        let auto_updated: Vec<&str> = named
            .iter()
            .filter(|(_, s)| is_auto(s))
            .map(|(gate, _)| *gate)
            .collect();

        let mut lines = vec![format!("- {} of {} gates passed", passed, named.len())];
        if !auto_updated.is_empty() {
            lines.push(format!(
                "- {} gate(s) were auto-corrected and are NOT in the commit being merged ({}): \
                 the fix exists only in Anvil's local clone and was never pushed to this branch",
                auto_updated.len(),
                auto_updated.join(", ")
            ));
        }
        lines.extend(
            named
                .iter()
                .filter(|(_, status)| !is_clean(status) && !is_auto(status))
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

        // The pull request as it is now, read once and used for every decision
        // below. The head this returns is the commit the queue would take.
        let meta = self
            .github_client
            .fetch_pr_metadata(repo, pr_number)
            .await?;

        // Invariant I1, on identity: the report has to be about this commit.
        Self::subject_refusal(report, repo, pr_number, &meta.head_ref_oid)?;

        // Step 0: Reconcile and verify honest PR title & body scope
        self.reconcile_pr_title_and_scope(repo, pr_number, &meta)
            .await?;

        // Step 1: Ensure PR has an official Approving Review submitted on GitHub
        self.ensure_approving_review(repo, pr_number, &meta, report)
            .await?;

        // Step 2: Enlist into Merge Queue using `gh pr merge --auto`.
        //
        // `--match-head-commit` carries the certified SHA to GitHub, so a push
        // landing between the check above and this request is rejected rather
        // than armed on a report that never saw it.
        //
        // What it does NOT do, stated rather than implied: GitHub validates
        // `--match-head-commit` at the moment auto-merge is enabled, and the
        // merge itself happens later, whenever the required checks go green. A
        // contributor with write access who pushes after that point moves the
        // head, and GitHub does not disable auto-merge for it -- so the commit
        // that eventually merges can be one no report ever measured. Nothing in
        // this crate calls `gh pr merge --disable-auto`, and the review
        // pipeline's response to a later, inadmissible head is a `warn!`.
        //
        // That window is accepted here, knowingly, and not closed by this flag.
        // Closing it needs a disarm path -- re-running the corpus for the new
        // head and calling `--disable-auto` when it does not certify -- which
        // depends on GitHub's documented auto-merge-disable conditions for the
        // token's permission level, and neither was verified for this change.
        // The flag narrows the certify-to-enable race; it does not make a moved
        // head unmergeable.
        let mut cmd = Command::new("gh");
        cmd.args([
            "pr",
            "merge",
            &pr_number.to_string(),
            "--repo",
            repo,
            "--auto",
            "--squash",
            "--match-head-commit",
            &meta.head_ref_oid,
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
            // Warned, not propagated. The merge queue arming has already
            // happened and no failure to comment undoes it, so a rate-limited
            // or refused `post_pr_comment` must not turn a completed
            // enlistment into `Err` -- `/api/enlist` would answer `409 …
            // refused` about a pull request that is armed and will merge, and
            // `record_certification` would record `enlisted = false` for one
            // that was enlisted. That is the same fabricated negative the 504
            // was removed for, one step later. A note that must be able to
            // block would have to be posted before the merge is requested.
            if let Err(e) = self
                .post_enlistment_note(repo, pr_number, "Squash & Merge", report)
                .await
            {
                warn!(
                    "{}#{} is enlisted; its enlistment note could not be posted: {:#}",
                    repo, pr_number, e
                );
            }
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
            "--match-head-commit",
            &meta.head_ref_oid,
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
            // Warned, not propagated; see the squash arm above.
            if let Err(e) = self
                .post_enlistment_note(repo, pr_number, "Merge Commit", report)
                .await
            {
                warn!(
                    "{}#{} is enlisted; its enlistment note could not be posted: {:#}",
                    repo, pr_number, e
                );
            }
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

    /// Verifies if PR has an approving review; if not, submits a formal APPROVE
    /// review derived from `report`.
    ///
    /// Fails closed on CHANGES_REQUESTED, on unresolved review comment threads,
    /// and on *either read failing* -- an unreadable review state is not an
    /// absent one, and this function may not sign an APPROVE on evidence it did
    /// not obtain.
    ///
    /// The `report` argument is defence in depth and nothing more. Its `None`
    /// arm below is unreachable in the shipped code: `enlist_into_merge_queue`
    /// runs `Self::admission_refusal(report)?` at its entry point, so `report`
    /// is already `Some` and already admissible by the time this is called, and
    /// that entry-point guard is this call's only caller. The arm exists so
    /// that a future second caller cannot walk from "no report" into
    /// `gh pr merge --auto`; it is not what protects the merge today. Do not
    /// remove the entry-point guard in the belief that this one stands in for
    /// it.
    pub async fn ensure_approving_review(
        &self,
        repo: &str,
        pr_number: u64,
        meta: &crate::github::PrMetadata,
        report: Option<&PreMergeCertificationReport>,
    ) -> Result<()> {
        info!(
            "Verifying approving review requirement for {}#{}...",
            repo, pr_number
        );

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
        // Fail closed on every way this read can fail, not just on the one that
        // errors. `run_bounded` returns `Ok(output)` for a non-zero exit -- it
        // errors only on a spawn failure or a timeout -- and `gh pr view` exits
        // non-zero on auth failure, on rate limiting and on transient API
        // errors. Read only through `?`, all three of those skipped the whole
        // block below with `needs_approval` still at its `true` default, and
        // this function went on to publish a formal APPROVE without ever having
        // learned whether a CHANGES_REQUESTED verdict exists. An unreadable
        // review state is not an absent one.
        let check_out = crate::exec::run_bounded(
            check_cmd,
            crate::exec::ExecClass::Api,
            "gh pr view (review decision)",
        )
        .await
        .context("Failed to read PR review decision before merge queue admission")?;
        if !check_out.status.success() {
            bail!(
                "🚨 Merge queue enlistment blocked: the review state of {}#{} could not be read, \
                 so nothing establishes that no CHANGES_REQUESTED verdict exists: {}",
                repo,
                pr_number,
                String::from_utf8_lossy(&check_out.stderr).trim()
            );
        }
        let val: serde_json::Value =
            serde_json::from_slice(&check_out.stdout).with_context(|| {
                format!(
                    "🚨 Merge queue enlistment blocked: the review state of {}#{} did not parse, \
                     so nothing establishes that no CHANGES_REQUESTED verdict exists",
                    repo, pr_number
                )
            })?;

        let mut needs_approval = true;
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

        // Step 2: Check for unresolved review comment threads. Propagated, not
        // `unwrap_or_default()`: an API failure turned into an empty list reads
        // as "zero unresolved threads" and satisfies the check below, which is
        // the same fail-open shape as the one above.
        let comments = self
            .github_client
            .fetch_review_comments(repo, pr_number)
            .await
            .with_context(|| {
                format!(
                    "🚨 Merge queue enlistment blocked: the review threads on {}#{} could not be \
                     read, so nothing establishes that none are unresolved",
                    repo, pr_number
                )
            })?;

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
            // Absent evidence is not a reason to report success from the one
            // function whose job is to guarantee an approving review exists.
            // This used to `return Ok(())`, which is the wrong answer to give
            // whoever asked. It is unreachable behind the entry-point
            // `admission_refusal` -- see this function's doc comment -- so what
            // changed here is the answer a second caller would get, not a live
            // path into `gh pr merge --auto`.
            let Some(summary) = Self::approval_summary(report) else {
                bail!(
                    "🚨 Merge queue enlistment blocked: approving review not submitted on PR {}#{}: \
                     there is no measured report to derive one from, and Anvil does not sign for \
                     what it did not measure.",
                    repo,
                    pr_number
                );
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
    pub async fn reconcile_pr_title_and_scope(
        &self,
        repo: &str,
        pr_number: u64,
        meta: &crate::github::PrMetadata,
    ) -> Result<()> {
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

    /// Posts the enlistment note, or nothing when there is no report to derive
    /// one from.
    ///
    /// Like the `None` arm in `ensure_approving_review`, the "post nothing" arm
    /// is unreachable behind the entry-point `admission_refusal`: this is only
    /// called after `gh pr merge` succeeded, on a path that already proved
    /// `report` admissible.
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

#[cfg(test)]
mod subject_refusal_tests {
    use super::*;
    use crate::pre_merge_guard::report::CertifiedSubject;

    /// A report measured against one commit is not evidence about another.
    ///
    /// This is the door's headline defence against the healer race and the
    /// push-during-corpus race, and it was pinned by nothing: changing the
    /// comparison to `if false && subject.head_sha != head` left clippy clean
    /// and all 68 test targets green, while admitting — and formally
    /// approving — a pull request whose head no gate had seen. Deleting the
    /// call outright was caught only by a dead-code lint.
    ///
    /// It sits in-crate because `subject` is set by the evaluator alone; from
    /// outside `pre_merge_guard` no test can build a report that carries one,
    /// which is exactly why the invariant went uncovered.
    fn measured_at(head: &str) -> PreMergeCertificationReport {
        let mut report = PreMergeCertificationReport::unmeasured("fixture");
        report.subject = Some(CertifiedSubject {
            repo: "oyatie/anvil".to_string(),
            pr_number: 7,
            head_sha: head.to_string(),
        });
        report
    }

    #[test]
    fn the_door_refuses_a_report_measured_against_another_commit() {
        let measured = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let now_at = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let report = measured_at(measured);

        let err = MergeEnlister::subject_refusal(Some(&report), "oyatie/anvil", 7, now_at)
            .expect_err(
                "the report was measured against one commit and the head is now another; \
                 admitting it queues a commit the corpus never saw",
            );
        let msg = err.to_string();
        assert!(
            msg.contains(measured) && msg.contains(now_at),
            "the refusal must name both commits so an author can see what moved: {msg}"
        );
    }

    #[test]
    fn the_door_admits_the_commit_the_report_was_measured_on() {
        let measured = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let report = measured_at(measured);
        assert!(
            MergeEnlister::subject_refusal(Some(&report), "oyatie/anvil", 7, measured).is_ok(),
            "a check that refuses its own subject refuses everything, which passes the \
             test above for the wrong reason"
        );
    }

    #[test]
    fn a_report_about_another_pull_request_is_not_evidence_about_this_one() {
        let measured = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let report = measured_at(measured);
        assert!(
            MergeEnlister::subject_refusal(Some(&report), "oyatie/console", 7, measured).is_err(),
            "another repository"
        );
        assert!(
            MergeEnlister::subject_refusal(Some(&report), "oyatie/anvil", 8, measured).is_err(),
            "another pull request"
        );
        assert!(
            MergeEnlister::subject_refusal(None, "oyatie/anvil", 7, measured).is_err(),
            "no report at all is not evidence"
        );
    }
}
