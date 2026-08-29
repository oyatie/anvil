//! The fleet sweep task: measure in `shape`, plan in `change_delivery`.
//!
//! The composition root for those two units. Neither imports the other —
//! doing the planning inside the sweep made `shape` depend on
//! `change_delivery` while `change_delivery` depends on `shape`, closing a
//! cycle whose every edge is individually legal and which therefore no
//! per-edge rule can see.
//!
//! It also raises what the audits find into `intake::Queue`, because a finding
//! printed and not queued is a finding that will be found again. Same reason
//! the planning lives here: `intake` is a leaf that must not import its
//! producers, so only a composition root may know them.
//!
//! Report-only (I25): blocks nobody, mutates nothing in any repository.

use tracing::info;

/// How long a corpus file may go untouched before the audit calls it dormant.
///
/// The sweep's own number rather than the CLI's default, because the sweep is
/// unattended: a threshold tuned for a human asking once is not the threshold
/// for something that asks every hour.
const CORPUS_STALE_DAYS: u64 = 90;

/// Start the hourly sweep over every watched repository.
pub fn spawn(state: &crate::webhook::AppState) {
    let deps = crate::shape::facade::sweep::SweepDeps {
        git_mgr: state.git_mgr.clone(),
        telemetry: state.telemetry_store.clone(),
        data_dir: state.config.data_dir.clone(),
    };
    let repos = state.config.watched_repos.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            for repo in &repos {
                // Measure in `shape`, plan in `change_delivery`, compose
                // here. The sweep used to do both, which made shape import
                // change_delivery while change_delivery imports shape --
                // a cycle no per-edge rule can see. The composition root
                // is the one place allowed to know both units.
                match crate::shape::facade::sweep::sweep_repo(&deps, repo).await {
                    Ok(crate::shape::facade::sweep::Swept::Skipped(why)) => {
                        info!("[Shape Sweep] {why}")
                    }
                    Ok(crate::shape::facade::sweep::Swept::Measured { report, summary }) => {
                        match crate::change_delivery::facade::plan::write_move_plan(
                            &deps.data_dir,
                            repo,
                            &report,
                        )
                        .await
                        {
                            Ok(path) => info!("[Shape Sweep] {summary} -> {}", path.display()),
                            Err(e) => {
                                tracing::warn!("[Shape Sweep] {summary}; plan not written: {e}")
                            }
                        }
                    }
                    Err(e) => tracing::warn!("[Shape Sweep] {repo} noticed: {e}"),
                }

                // The backlog, raised from the audits that already ran.
                //
                // The corpus audit needs a checkout. `ensure_repo_cloned` is
                // idempotent and the shape sweep above has already paid for it,
                // so this is a lookup rather than a second clone. When it
                // cannot be had, the corpus producer is ABSENT from the record
                // rather than reported as having found nothing.
                let corpus = match deps.git_mgr.ensure_repo_cloned(repo).await {
                    Ok(dir) => match crate::corpus_auditor::CorpusAuditor::audit_repository(
                        &dir,
                        CORPUS_STALE_DAYS,
                    ) {
                        Ok(report) => Some(report),
                        Err(e) => {
                            tracing::warn!("[Intake] {repo}: corpus audit did not run: {e}");
                            None
                        }
                    },
                    Err(e) => {
                        tracing::warn!("[Intake] {repo}: no checkout to audit: {e}");
                        None
                    }
                };

                let raised = crate::cli::intake_sweep::raise_for_repo(repo, corpus.as_ref());
                let triage = crate::intake::triage::triage(&raised.queue);
                let recurring: std::collections::BTreeMap<String, usize> = triage
                    .recurring_classes()
                    .into_iter()
                    .map(|(c, n)| (c.to_string(), n))
                    .collect();
                info!(
                    "[Intake] {repo}: {} outstanding ({:.0}% unclassified) from {:?}",
                    raised.queue.len(),
                    triage.unclassified_share() * 100.0,
                    raised.by_producer
                );
                deps.telemetry
                    .record_work_queue(crate::telemetry_store::WorkQueueRecord {
                        repo: repo.clone(),
                        depth: raised.queue.len(),
                        unclassified_share: triage.unclassified_share(),
                        recurring,
                        by_source: raised.by_source(),
                        recorded_at: chrono::Utc::now(),
                    })
                    .await;
            }
        }
    });
}
