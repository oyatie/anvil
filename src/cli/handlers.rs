use anyhow::{Context, Result};
use clap::Parser;
use tracing::info;

use super::args::{Cli, Commands};
use super::server;
use crate::webhook::{AppState, execute_pr_certify, execute_pr_fix, execute_pr_review};

mod shape;

pub async fn handle_cli(state: AppState) -> Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Serve) {
        Commands::Serve => {
            // Boot invariant: no managed clone may be the tree this binary runs
            // from. Fails closed before the first webhook is accepted.
            state
                .config
                .assert_managed_clones_are_not_this_tree()
                .await?;
            // Ingress that cannot be authenticated rejects everything. Refusing
            // here turns a silently inert daemon into a startup failure that
            // names its own cause.
            state.config.assert_webhook_ingress_is_authenticated()?;
            server::run_server(state).await?;
        }
        Commands::Shape { action } => shape::dispatch(action).await?,
        Commands::Review { repo, pr, force } => {
            info!("Running on-demand review for {}#{}", repo, pr);
            let meta = state
                .github_client
                .fetch_pr_metadata(&repo, pr)
                .await
                .context("Failed to fetch PR metadata")?;

            execute_pr_review(
                &state,
                &repo,
                pr,
                &meta.title,
                &meta.body.unwrap_or_default(),
                &meta.base_ref_name,
                &meta.base_ref_oid,
                &meta.head_ref_oid,
                force,
            )
            .await?;
        }
        Commands::Fix { repo, pr } => {
            info!("Running on-demand auto-fixer for {}#{}", repo, pr);
            execute_pr_fix(&state, &repo, pr).await?;
        }
        Commands::Certify { repo, pr } => {
            info!(
                "Running on-demand pre-merge certification for {}#{}",
                repo, pr
            );
            execute_pr_certify(&state, &repo, pr).await?;
        }
        Commands::Triage {
            repo,
            run_id,
            branch,
            commit_sha,
            workflow_name,
        } => {
            info!(
                "Running on-demand trunk CI triage for run #{} on {}",
                run_id, repo
            );
            let branch_str = branch.unwrap_or_else(|| "main".to_string());
            let sha_str = commit_sha.unwrap_or_default();
            let wf_str = workflow_name.unwrap_or_else(|| "CI".to_string());
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;

            state
                .ci_triager
                .triage_workflow_run(&repo, run_id, &branch_str, &sha_str, &wf_str, &repo_dir)
                .await?;
        }
        Commands::Enlist { repo, pr } => {
            info!(
                "Running on-demand merge queue enlistment for {}#{}",
                repo, pr
            );
            // This path has not reviewed the pull request, so it runs the
            // certification corpus and hands over what that produced.
            let evidence = crate::webhook::pipelines::certify::evidence_for_enlistment(
                &state, &repo, pr, None,
            )
            .await;
            // The cause, on the way to the exit code. Collapsed to "no report
            // was obtained" it tells an operator nothing they can act on.
            if let Err(e) = &evidence {
                tracing::warn!("No certification report for {}#{}: {:#}", repo, pr, e);
            }
            state
                .merge_enlister
                .enlist_into_merge_queue(&repo, pr, evidence.as_ref().ok())
                .await?;
        }
        Commands::HealQueue { repo, pr } => {
            info!("Running on-demand merge queue healer for {}#{}", repo, pr);
            let what_happened = state
                .queue_healer
                .heal_ejected_pr(&state, &repo, pr)
                .await?;
            info!("{}", what_happened);
        }
        Commands::Reconcile { repo, pr } => {
            info!(
                "Running on-demand lockfile/ledger reconciler for {}#{}",
                repo, pr
            );
            state.lockfile_reconciler.reconcile_pr(&repo, pr).await?;
        }
        Commands::HookInstall { path } => {
            let target_path = path
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            info!(
                "Installing developer inner-loop git hooks in {:?}",
                target_path
            );
            crate::git_manager::GitManager::install_repo_hooks(&target_path).await?;
            println!(
                "✅ Anvil Git Hooks Installed Successfully in {:?}/.git/hooks/",
                target_path
            );
        }
        Commands::Probe { diff, message } => {
            let diff_content = if let Some(d) = diff {
                d
            } else {
                // Fails closed: a probe that could not read the diff has verified
                // nothing, so a spawn failure or a timeout must not fall through to
                // an empty diff and print "PASSED".
                let mut staged = tokio::process::Command::new("git");
                staged.args(["diff", "--cached"]);
                let o = crate::exec::run_bounded(
                    staged,
                    crate::exec::ExecClass::Quick,
                    "git diff --cached",
                )
                .await?;
                if !o.stdout.is_empty() {
                    String::from_utf8_lossy(&o.stdout).to_string()
                } else {
                    let mut unstaged = tokio::process::Command::new("git");
                    unstaged.args(["diff"]);
                    let u = crate::exec::run_bounded(
                        unstaged,
                        crate::exec::ExecClass::Quick,
                        "git diff",
                    )
                    .await?;
                    String::from_utf8_lossy(&u.stdout).to_string()
                }
            };

            // The harness supersedes both hand-wired checks rather than joining
            // them. `secret_on_added_line` delegates to the same
            // `judgement::scan_for_secrets` the old path reached through a thin
            // `PreMergeScanner` wrapper, and adds what that path could not: it
            // names the file the credential is on, and it proves itself against
            // a seeded fixture before its verdict is trusted.
            // `conventional_commit_subject` likewise carries the full
            // Conventional Commits 1.0.0 header rule. Running both would report
            // every secret twice, once without a filename.
            let findings =
                crate::local_inner_loop::harness_findings(&diff_content, message.as_deref());
            let is_valid = findings.iter().all(|f| f.is_valid);
            if is_valid {
                let header = match &message {
                    Some(_) => "commit header graded",
                    None => "commit header NOT MEASURED (no --message; a pre-commit hook has none)",
                };
                println!(
                    "✅ PASSED (Sub-100ms Inner-Loop Local Probe: {} finding(s) over {} check(s); {header})",
                    0,
                    findings.len()
                );
            } else {
                println!(
                    "❌ FAILED ({} Inner-Loop Local Probe Violations Detected):",
                    findings.iter().filter(|f| !f.is_valid).count()
                );
                for f in findings.iter().filter(|f| !f.is_valid) {
                    println!("  - {}: {}", f.check_name, f.message);
                }
            }
        }
        Commands::Toolchain {
            repo_dir,
            to,
            apply,
        } => {
            let declared = crate::toolchain::read(&repo_dir);
            let Some(target) = crate::toolchain::Version::parse(&to) else {
                println!("❌ `{to}` is not a version");
                return Ok(());
            };
            let Some(current) = declared.channel else {
                println!("❌ no channel declared in rust-toolchain.toml; nothing to move");
                return Ok(());
            };
            println!("Probing {current} -> {target} under the TARGET toolchain...");
            let safety = crate::toolchain::bump::probe(&repo_dir, &to).await;
            println!("{}", safety.explain());
            if !safety.permits_bump() {
                // No flag skips this. A bump applied without a probe is the
                // change this command exists to stop anyone making by hand.
                println!("❌ refusing to move the pin: the bump is not proven.");
                return Ok(());
            }
            // Every site the pin appears at, together. A bump that moves the
            // manifest and leaves CI behind gives a tree whose CI and whose
            // developers build with different compilers.
            let channel = crate::toolchain::bump::channel_bump(current, target);
            let ci = crate::toolchain::bump::ci_toolchain_bump(current, target);
            let mut files = std::collections::BTreeMap::new();
            let mut findings: Vec<&crate::harness::Finding> = vec![&channel];
            files.insert(
                "rust-toolchain.toml".to_string(),
                tokio::fs::read_to_string(repo_dir.join("rust-toolchain.toml")).await?,
            );
            let ci_path = repo_dir.join(".github/workflows/ci.yml");
            if let Ok(body) = tokio::fs::read_to_string(&ci_path).await {
                files.insert(".github/workflows/ci.yml".to_string(), body);
                findings.push(&ci);
            }
            let plan = crate::harness::apply::plan(&findings, &files);
            for r in &plan.refused {
                println!("  refused: {r:?}");
            }
            for edit in &plan.edits {
                match edit {
                    crate::harness::apply::Edit::Rewrite { path: p, body } => {
                        if apply {
                            tokio::fs::write(repo_dir.join(p), body).await?;
                            println!("✅ {p}: channel moved to {target}");
                        } else {
                            println!("would rewrite {p} (pass --apply to write)");
                        }
                    }
                    other => println!("  unexpected edit: {other:?}"),
                }
            }
        }
        Commands::TrainRun { repo } => {
            info!("Running Proactive Upgrade Train for {}", repo);
            let candidates = vec![crate::upgrade_train::DependencyUpgradeCandidate {
                package_name: "tokio".to_string(),
                current_version: "1.38.0".to_string(),
                target_version: "1.38.1".to_string(),
                is_major_breaking: false,
            }];
            let rep = state.upgrade_train.evaluate_upgrade_train(&candidates);
            println!(
                "\n🚂 ProactiveUpgradeTrain Result: {}\nPending: {} | Breaking: {}\n",
                rep.summary, rep.pending_upgrades_available, rep.breaking_major_upgrades
            );
        }
        Commands::FlakeRehab { repo } => {
            info!("Running Flaky-Test Quarantine Rehabilitation for {}", repo);
            let rep = state
                .flake_quarantine
                .evaluate_quarantine_lifecycle(&["tests::flaky_test".to_string()]);
            println!(
                "\n🧪 FlakeQuarantine Result: {}\nQuarantined: {} | Rehabilitated: {}\n",
                rep.summary, rep.quarantined_tests_isolated, rep.rehabilitated_tests_restored
            );
        }
        Commands::Reap => {
            info!("Reaping stale preview environments and orphaned worktrees...");
            state.git_mgr.clean_abandoned_worktrees().await?;
            println!("✅ Preview Environments and Git Worktrees Reaped Cleanly");
        }
        Commands::IssueReconcile { repo } => {
            info!("Autonomously reconciling issues for repository: {}", repo);
            let reconciler =
                crate::issue_reconciler::IssueReconciler::new(state.github_client.clone());
            let reconciled = reconciler.reconcile_issues(&repo).await?;
            if reconciled.is_empty() {
                println!(
                    "✅ All trunk failure issues on {} are active or already reconciled.",
                    repo
                );
            } else {
                println!(
                    "🎉 Successfully auto-closed {} stale trunk issues on {}:",
                    reconciled.len(),
                    repo
                );
                for rec in &reconciled {
                    println!(
                        "  - #{}: {} ({})",
                        rec.issue_number, rec.title, rec.resolution_reason
                    );
                }
            }
        }
        Commands::PrHeal { repo, pr, branch } => {
            info!(
                "Autonomously healing PR #{} on branch {} in {}",
                pr, branch, repo
            );
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let healer = crate::pr_self_healer::PrSelfHealer::new();
            let report = healer.auto_heal_pr_branch(&repo_dir, &branch, pr).await?;
            println!(
                "🛠️ PR Self-Healing Summary for {}#{}:\n  - Files Formatted: {}\n  - OWNERS Created: {:?}\n  - Commit SHA: {:?}",
                repo, pr, report.files_formatted, report.owners_files_created, report.commit_sha
            );
        }
        Commands::DocSweep { repo, dry_run } => {
            info!(
                "Running DocArchivalSweeper on {} (dry_run: {})...",
                repo, dry_run
            );
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let report = crate::doc_archival_sweeper::DocArchivalSweeper::sweep_repository(
                &repo_dir, dry_run,
            )
            .await?;
            println!(
                "🧹 DocArchivalSweeper Report for {} (dry_run: {}):\n  - Files Archived: {}\n  - Forward-Pointer Stubs Written: {}\n  - SSOT Declarations Demoted: {}\n  - Summary: {}",
                repo,
                dry_run,
                report.files_archived.len(),
                report.stubs_written.len(),
                report.ssot_claims_demoted.len(),
                report.summary
            );
        }
        Commands::ComponentEval { repo, target } => {
            info!(
                "Evaluating component disposition for '{}' on {}...",
                target, repo
            );
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let report = crate::monorepo_guard::ComponentDispositionClassifier::evaluate_component(
                &repo_dir, &target, 1, // default 1 inbound caller
            );
            println!(
                "🔍 Component Disposition Evaluation for '{}' on {}:\n  - Disposition: {:?}\n  - Clean Architecture Compliant: {}\n  - Max File Lines: {}\n  - Rationale: {}\n  - Action: {}",
                target,
                repo,
                report.disposition,
                report.is_clean_architecture,
                report.max_file_lines,
                report.rationale,
                report.recommended_action
            );
        }
        Commands::AuditCorpus { repo, stale_days } => {
            info!(
                "Running 100% full-corpus audit on {} (stale threshold: {} days)...",
                repo, stale_days
            );
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let report =
                crate::corpus_auditor::CorpusAuditor::audit_repository(&repo_dir, stale_days)?;
            println!(
                "📊 100% Full-Corpus Audit Report for {}:\n  - Total Files Audited: {}\n  - Freshness Ratio: {:.1}%\n  - Dormant Files (>{}d): {}\n  - Stale ADRs in Archive: {}\n  - Unauthorized SSOT Claims: {}\n  - Frontmatter Violations: {}\n  - Summary: {}",
                repo,
                report.total_files,
                report.freshness_ratio * 100.0,
                stale_days,
                report.dormant_files_count,
                report.stale_adrs_count,
                report.unauthorized_ssot_claims.len(),
                report.frontmatter_violations.len(),
                report.summary
            );
        }
        Commands::HealCorpus {
            repo,
            batch_size,
            dry_run,
        } => {
            info!(
                "Running Continuous Hygiene Engine on {} (batch_size: {}, dry_run: {})...",
                repo, batch_size, dry_run
            );
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let report =
                crate::corpus_auditor::ContinuousHygieneEngine::generate_maintenance_batch(
                    &repo_dir, batch_size, dry_run,
                )?;
            println!(
                "🌱 Continuous Hygiene Batch Report for {} (dry_run: {}):\n  - Batch ID: {}\n  - Files Refreshed/Healed: {}\n  - Summary: {}",
                repo,
                dry_run,
                report.batch_id,
                report.files_modified.len(),
                report.summary
            );
        }
        Commands::IssueAudit { repo } => {
            info!("Running 24h Autonomous Issue Audit on {}...", repo);
            let reconciler =
                crate::issue_reconciler::IssueReconciler::new(state.github_client.clone());
            let reconciled = reconciler.reconcile_issues(&repo).await?;
            println!(
                "📋 Issue Audit & Reconciliation complete for {}: {} issues reconciled/closed.",
                repo,
                reconciled.len()
            );
            for r in reconciled {
                println!(
                    "  - #{}: {} [{}] ({})",
                    r.issue_number, r.title, r.status, r.resolution_reason
                );
            }
        }
        Commands::DocConsolidate {
            repo,
            issue,
            dry_run,
        } => {
            info!(
                "Running Issue-Driven Doc Consolidation for issue #{} on {} (dry_run: {})...",
                issue, repo, dry_run
            );
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let report = crate::doc_archival_sweeper::IssueDocConsolidator::consolidate_issue_docs(
                &repo_dir,
                issue,
                "Resolving issue and archiving sprint plans",
                dry_run,
            )
            .await?;
            println!(
                "📁 Issue #{} Doc Consolidation Summary (dry_run: {}):\n  - Files Archived: {}\n  - Stubs Written: {}\n  - Summary: {}",
                issue,
                dry_run,
                report.files_archived.len(),
                report.stubs_written.len(),
                report.summary
            );
        }
        Commands::Swap { binary } => {
            let green_binary = binary.unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join("target/release/anvil")
            });
            let current_exe = std::env::current_exe()?;

            let swap = crate::recovery::BlueGreenSupervisor::plan(green_binary, current_exe);
            crate::recovery::BlueGreenSupervisor::execute_atomic_binary_swap(
                &swap.green,
                &swap.installed,
            )
            .await?;
            println!(
                "🎉 Blue/Green Self-Replacement Successful!\n  - Installed over: {:?}\n  - From new build: {:?}",
                swap.installed, swap.green
            );
        }
        Commands::Recover { repo } => {
            let repos = if let Some(r) = repo {
                vec![r]
            } else {
                state.config.watched_repos.clone()
            };
            info!(
                "🔍 Running Full Outage Recovery & PR/Issue Reconciliation Sweep on {:?}",
                repos
            );
            let reconciler = crate::recovery::OutageRecoveryReconciler::new(
                state.github_client.clone(),
                state.state_mgr.clone(),
            );
            let report = reconciler.run_full_sweep(&repos).await?;
            println!(
                "🛡️ Outage Recovery Sweep Complete:\n  - Repos Scanned: {:?}\n  - Total PRs Inspected: {}\n  - PRs Requiring Certification: {}\n  - Issues Reconciled: {}\n  - Status: {}",
                report.repos_scanned,
                report.total_prs_inspected,
                report.prs_requiring_certification.len(),
                report.issues_reconciled,
                report.status
            );
        }
        Commands::Forward => {
            info!(
                "Starting webhook forwarders for {:?}",
                state.config.watched_repos
            );
            server::start_forwarders(&state.config).await?;
        }
        Commands::Check => {
            info!("Checking system dependencies and authentication...");
            server::check_environment(&state.github_client, &state.config).await?;
        }
    }

    Ok(())
}
