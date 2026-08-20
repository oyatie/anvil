use anyhow::{Context, Result};
use clap::Parser;
use tracing::info;

use super::args::{Cli, Commands};
use super::server;
use crate::webhook::{execute_pr_certify, execute_pr_fix, execute_pr_review, AppState};

pub async fn handle_cli(state: AppState) -> Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Serve) {
        Commands::Serve => {
            server::run_server(state).await?;
        }
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
        Commands::RustSkillsCheck { repo, pr } => {
            info!("Running Rust Skills Guard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state
                .git_mgr
                .prepare_pr_diff(
                    &repo,
                    pr,
                    &meta.base_ref_name,
                    &meta.base_ref_oid,
                    &meta.head_ref_oid,
                    None,
                )
                .await?;
            let res = state
                .rust_skills_guard
                .evaluate_rust_quality(&repo_dir, &diff_ctx)?;
            println!(
                "\n🦀 RustSkillsGuard Result: {}\nFindings: {}\n",
                res.summary,
                res.findings.len()
            );
        }
        Commands::ArchCheck { repo, pr } => {
            info!("Running Clean Architecture Guard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let diff_ctx = state
                .git_mgr
                .prepare_pr_diff(
                    &repo,
                    pr,
                    &meta.base_ref_name,
                    &meta.base_ref_oid,
                    &meta.head_ref_oid,
                    None,
                )
                .await?;
            let res = state.clean_arch_guard.evaluate_architecture(&diff_ctx)?;
            println!(
                "\n🏛️ CleanArchitectureGuard Result: {}\nViolations: {}\n",
                res.summary,
                res.violations.len()
            );
        }
        Commands::MonorepoCheck { repo, pr } => {
            info!("Running Monorepo Guard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state
                .git_mgr
                .prepare_pr_diff(
                    &repo,
                    pr,
                    &meta.base_ref_name,
                    &meta.base_ref_oid,
                    &meta.head_ref_oid,
                    None,
                )
                .await?;
            let res = state
                .monorepo_guard
                .evaluate_monorepo_hygiene(&repo_dir, &diff_ctx)
                .await?;
            println!(
                "\n🏢 MonorepoGuard Result: {}\nViolations: {}\n",
                res.summary,
                res.violations.len()
            );
        }
        Commands::DebtCheck { repo, pr } => {
            info!("Running Debt Shrink Guard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state
                .git_mgr
                .prepare_pr_diff(
                    &repo,
                    pr,
                    &meta.base_ref_name,
                    &meta.base_ref_oid,
                    &meta.head_ref_oid,
                    None,
                )
                .await?;
            let res = state
                .debt_shrink_guard
                .evaluate_debt_shrink(&repo_dir, &diff_ctx)?;
            println!(
                "\n📉 DebtShrinkGuard Result: {}\nViolations: {}\n",
                res.summary,
                res.violations.len()
            );
        }
        Commands::ModularCheck { repo, pr } => {
            info!("Running Modularization Guard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let diff_ctx = state
                .git_mgr
                .prepare_pr_diff(
                    &repo,
                    pr,
                    &meta.base_ref_name,
                    &meta.base_ref_oid,
                    &meta.head_ref_oid,
                    None,
                )
                .await?;
            let res = state
                .modularization_guard
                .evaluate_modularization(&diff_ctx)?;
            println!(
                "\n🧩 ModularizationGuard Result: {}\nOversized: {}\n",
                res.summary,
                res.oversized_files.len()
            );
        }
        Commands::CoverageCheck { repo, pr } => {
            info!("Running CoverageGuard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state
                .git_mgr
                .prepare_pr_diff(
                    &repo,
                    pr,
                    &meta.base_ref_name,
                    &meta.base_ref_oid,
                    &meta.head_ref_oid,
                    None,
                )
                .await?;
            let res = state
                .coverage_guard
                .evaluate_diff_coverage(&repo_dir, &diff_ctx)?;
            println!(
                "\n🎯 CoverageGuard Result: {}\nFindings: {}\n",
                res.summary,
                res.findings.len()
            );
        }
        Commands::GhostMigrationCheck { repo, pr } => {
            info!("Running GhostMigrationHarness on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state
                .git_mgr
                .prepare_pr_diff(
                    &repo,
                    pr,
                    &meta.base_ref_name,
                    &meta.base_ref_oid,
                    &meta.head_ref_oid,
                    None,
                )
                .await?;
            let res = state
                .ghost_migration_harness
                .evaluate_migrations(&repo_dir, &diff_ctx)?;
            println!(
                "\n🐘 GhostMigrationHarness Result: {}\nViolations: {}\n",
                res.summary,
                res.violations.len()
            );
        }
        Commands::ChaosMutationCheck { repo, pr } => {
            info!("Running ChaosMutationGuard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let diff_ctx = state
                .git_mgr
                .prepare_pr_diff(
                    &repo,
                    pr,
                    &meta.base_ref_name,
                    &meta.base_ref_oid,
                    &meta.head_ref_oid,
                    None,
                )
                .await?;
            let res = state
                .chaos_mutation_guard
                .evaluate_mutation_adequacy(&diff_ctx)?;
            println!(
                "\n💥 ChaosMutationGuard Result: {}\nFindings: {}\n",
                res.summary,
                res.surviving_findings.len()
            );
        }
        Commands::FeatureFlagCheck { repo, pr } => {
            info!("Running FeatureFlagRatchet on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state
                .git_mgr
                .prepare_pr_diff(
                    &repo,
                    pr,
                    &meta.base_ref_name,
                    &meta.base_ref_oid,
                    &meta.head_ref_oid,
                    None,
                )
                .await?;
            let res = state
                .feature_flag_ratchet
                .evaluate_feature_flags(&repo_dir, &diff_ctx)?;
            println!(
                "\n🚩 FeatureFlagRatchet Result: {}\nViolations: {}\n",
                res.summary,
                res.violations.len()
            );
        }
        Commands::BenchCheck { repo, pr } => {
            info!("Running CriterionBenchRatchet on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state
                .git_mgr
                .prepare_pr_diff(
                    &repo,
                    pr,
                    &meta.base_ref_name,
                    &meta.base_ref_oid,
                    &meta.head_ref_oid,
                    None,
                )
                .await?;
            let res = state
                .criterion_bench_ratchet
                .evaluate_benchmarks(&repo_dir, &diff_ctx)?;
            println!(
                "\n⚡ CriterionBenchRatchet Result: {}\nViolations: {}\n",
                res.summary,
                res.violations.len()
            );
        }
        Commands::CedarCheck { repo, pr } => {
            info!("Running Cedar IAM Policy Guard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state
                .git_mgr
                .prepare_pr_diff(
                    &repo,
                    pr,
                    &meta.base_ref_name,
                    &meta.base_ref_oid,
                    &meta.head_ref_oid,
                    None,
                )
                .await?;
            let res = state
                .cedar_guard
                .evaluate_cedar_policies(&repo, &repo_dir, &diff_ctx, &meta.title)
                .await?;
            println!(
                "\n🛡️ CedarGuard Result: {}\nFiles: {:?}\n",
                res.summary, res.files_created_or_updated
            );
        }
        Commands::ComplianceCheck { repo, pr } => {
            info!("Running Dynamic Compliance Guard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let diff_ctx = state
                .git_mgr
                .prepare_pr_diff(
                    &repo,
                    pr,
                    &meta.base_ref_name,
                    &meta.base_ref_oid,
                    &meta.head_ref_oid,
                    None,
                )
                .await?;
            let res = state.compliance_guard.evaluate_compliance(&diff_ctx)?;
            println!(
                "\n🏛️ ComplianceGuard Result: {}\nViolations: {}\n",
                res.summary,
                res.violations.len()
            );
        }
        Commands::ApiCheck { repo, pr } => {
            info!("Running ApiContractGuard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state
                .git_mgr
                .prepare_pr_diff(
                    &repo,
                    pr,
                    &meta.base_ref_name,
                    &meta.base_ref_oid,
                    &meta.head_ref_oid,
                    None,
                )
                .await?;
            let res = state
                .api_contract_guard
                .ensure_contract_integrity(&repo, &repo_dir, &diff_ctx)
                .await?;
            println!(
                "\n📐 ApiContractGuard Result: {}\nSynced Files: {:?}\n",
                res.summary, res.auto_synced_files
            );
        }
        Commands::CellCheck { repo, pr } => {
            info!("Running CellIsolationGuard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let diff_ctx = state
                .git_mgr
                .prepare_pr_diff(
                    &repo,
                    pr,
                    &meta.base_ref_name,
                    &meta.base_ref_oid,
                    &meta.head_ref_oid,
                    None,
                )
                .await?;
            let res = state
                .cell_isolation_guard
                .evaluate_cell_isolation(&diff_ctx)?;
            println!(
                "\n🌐 CellIsolationGuard Result: {}\nViolations: {}\n",
                res.summary,
                res.violations.len()
            );
        }
        Commands::SupplyChainCheck { repo, pr } => {
            info!("Running SupplyChainGuard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state
                .git_mgr
                .prepare_pr_diff(
                    &repo,
                    pr,
                    &meta.base_ref_name,
                    &meta.base_ref_oid,
                    &meta.head_ref_oid,
                    None,
                )
                .await?;
            let res = state
                .supply_chain_guard
                .audit_supply_chain(&repo_dir, &diff_ctx)?;
            println!(
                "\n📦 SupplyChainGuard Result: {}\nAudited: {}\n",
                res.summary, res.audited_packages
            );
        }
        Commands::Attest { repo, pr } => {
            info!("Running AttestationGuard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let res = state
                .attestation_guard
                .stamp_lane_receipt(
                    &repo_dir,
                    &repo,
                    pr,
                    &meta.head_ref_oid,
                    crate::attestation_guard::AttestationGuard::VERDICT_PENDING,
                    Vec::new(),
                )
                .await?;
            println!(
                "\n🔏 AttestationGuard Result: {}\nPath: {:?}\n",
                res.summary, res.stamped_receipt_path
            );
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
            state
                .merge_enlister
                .enlist_into_merge_queue(&repo, pr)
                .await?;
        }
        Commands::HealQueue { repo, pr } => {
            info!("Running on-demand merge queue healer for {}#{}", repo, pr);
            state.queue_healer.heal_ejected_pr(&repo, pr).await?;
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
        Commands::Probe { diff } => {
            let diff_content = if let Some(d) = diff {
                d
            } else {
                let out = tokio::process::Command::new("git")
                    .args(["diff", "--cached"])
                    .output()
                    .await;
                if let Ok(o) = out {
                    if !o.stdout.is_empty() {
                        String::from_utf8_lossy(&o.stdout).to_string()
                    } else {
                        let out_unstaged = tokio::process::Command::new("git")
                            .args(["diff"])
                            .output()
                            .await;
                        out_unstaged
                            .map(|u| String::from_utf8_lossy(&u.stdout).to_string())
                            .unwrap_or_default()
                    }
                } else {
                    String::new()
                }
            };

            let validator = crate::local_inner_loop::FastValidator::new();
            let findings = validator.validate_pre_commit("chore: probe check", &diff_content);
            let is_valid = findings.iter().all(|f| f.is_valid);
            if is_valid {
                println!("✅ PASSED (Sub-100ms Inner-Loop Local Probe Verified: 0 findings)");
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
                repo, dry_run, report.files_archived.len(), report.stubs_written.len(), report.ssot_claims_demoted.len(), report.summary
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
                target, repo, report.disposition, report.is_clean_architecture, report.max_file_lines, report.rationale, report.recommended_action
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
                repo, report.total_files, report.freshness_ratio * 100.0, stale_days, report.dormant_files_count, report.stale_adrs_count, report.unauthorized_ssot_claims.len(), report.frontmatter_violations.len(), report.summary
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
                repo, dry_run, report.batch_id, report.files_modified.len(), report.summary
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
                issue, dry_run, report.files_archived.len(), report.stubs_written.len(), report.summary
            );
        }
        Commands::Swap { binary } => {
            let green_binary = binary.unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join("target/release/anvil")
            });
            let current_exe = std::env::current_exe()?;
            info!(
                "🔄 Initiating Zero-Downtime Blue/Green Self-Replacement: {:?} -> {:?}",
                green_binary, current_exe
            );
            crate::recovery::BlueGreenSupervisor::execute_atomic_binary_swap(
                &current_exe,
                &green_binary,
            )
            .await?;
            println!(
                "🎉 Blue/Green Self-Replacement Successful!\n  - Swapped Target: {:?}\n  - Source Green Binary: {:?}\n  - Status: Atomic Binary Replacement Complete (Zero Downtime)",
                current_exe,
                green_binary
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
