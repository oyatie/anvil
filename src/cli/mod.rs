use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::info;

pub mod server;

use crate::webhook::{execute_pr_certify, execute_pr_fix, execute_pr_review, AppState};

#[derive(Parser, Debug)]
#[command(name = "pr-watch")]
#[command(about = "Oyatie Autonomous Engineering Pipeline: Reviewer, Auto-Fixer, CI Triager, Queue Healer & Domain Guards")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the real-time webhook server and forwarders (default)
    Serve,
    /// Trigger an immediate manual review for a specific PR
    Review {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,

        #[arg(short, long, help = "Force re-review even if SHA was already reviewed")]
        force: bool,
    },
    /// Trigger resolution and auto-fixing for review comments on a PR
    Fix {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Pre-Merge Quality Certification and Domain Gates on a PR (21 gates)
    Certify {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Rust 2024 Edition Quality Guard (rust-skills 380 rules) on a PR
    RustSkillsCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Clean Architecture (Core/Ports/Adapters/Facade) Guard check on a PR
    ArchCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Hyperscaler Monorepo & Hermeticity Guard check on a PR
    MonorepoCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Deprecation & Reorg Drain Ratchet check on a PR
    DebtCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Code Modularization (100-300 lines max) check on a PR
    ModularCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Differential Test Coverage (>=85%) Guard check on a PR
    CoverageCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Ghost DB Migration & Zero-Lock Validator on a PR
    GhostMigrationCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run AST Chaos Mutation Test Adequacy Guard on a PR
    ChaosMutationCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Feature Flag Lifecycle & Dead Code Ratchet on a PR
    FeatureFlagCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Micro-Benchmark & Latency Regression Ratchet on a PR
    BenchCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Cedar IAM Policy Guard check on a PR
    CedarCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Dynamic Regulatory Compliance Guard check on a PR
    ComplianceCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run OpenAPI & Wire Contract Integrity Guard check on a PR
    ApiCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Cell Boundary & Tenant Isolation Guard check on a PR
    CellCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Supply Chain & CVE Audit Guard check on a PR
    SupplyChainCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Generate and stamp a cryptographic lane receipt on a PR
    Attest {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Triage a failed CI workflow run on main/dev
    Triage {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Workflow Run ID")]
        run_id: u64,

        #[arg(short, long, help = "Branch name (default: main)")]
        branch: Option<String>,

        #[arg(short, long, help = "Commit SHA")]
        commit_sha: Option<String>,

        #[arg(short, long, help = "Workflow Name")]
        workflow_name: Option<String>,
    },
    /// Enlist a certified PR into the GitHub Merge Queue
    Enlist {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Heal a failed/ejected PR from the merge train
    HealQueue {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Auto-reconcile lockfiles and documentation ledgers on a PR branch
    Reconcile {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run GitHub CLI webhook forwarding manually
    Forward,
    /// Verify GitHub CLI authentication and environment readiness
    Check,
}

pub async fn handle_cli(state: AppState) -> Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Serve) {
        Commands::Serve => {
            server::run_server(state).await?;
        }
        Commands::Review { repo, pr, force } => {
            info!("Running on-demand review for {}#{}", repo, pr);
            let meta = state.github_client
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
            info!("Running on-demand pre-merge certification for {}#{}", repo, pr);
            execute_pr_certify(&state, &repo, pr).await?;
        }
        Commands::RustSkillsCheck { repo, pr } => {
            info!("Running Rust Skills Guard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state.git_mgr.prepare_pr_diff(&repo, pr, &meta.base_ref_name, &meta.base_ref_oid, &meta.head_ref_oid, None).await?;
            let res = state.rust_skills_guard.evaluate_rust_quality(&repo_dir, &diff_ctx)?;
            println!("\n🦀 RustSkillsGuard Result: {}\nFindings: {}\n", res.summary, res.findings.len());
        }
        Commands::ArchCheck { repo, pr } => {
            info!("Running Clean Architecture Guard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let diff_ctx = state.git_mgr.prepare_pr_diff(&repo, pr, &meta.base_ref_name, &meta.base_ref_oid, &meta.head_ref_oid, None).await?;
            let res = state.clean_arch_guard.evaluate_architecture(&diff_ctx)?;
            println!("\n🏛️ CleanArchitectureGuard Result: {}\nViolations: {}\n", res.summary, res.violations.len());
        }
        Commands::MonorepoCheck { repo, pr } => {
            info!("Running Monorepo Guard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state.git_mgr.prepare_pr_diff(&repo, pr, &meta.base_ref_name, &meta.base_ref_oid, &meta.head_ref_oid, None).await?;
            let res = state.monorepo_guard.evaluate_monorepo_hygiene(&repo_dir, &diff_ctx).await?;
            println!("\n🏢 MonorepoGuard Result: {}\nViolations: {}\n", res.summary, res.violations.len());
        }
        Commands::DebtCheck { repo, pr } => {
            info!("Running Debt Shrink Guard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state.git_mgr.prepare_pr_diff(&repo, pr, &meta.base_ref_name, &meta.base_ref_oid, &meta.head_ref_oid, None).await?;
            let res = state.debt_shrink_guard.evaluate_debt_shrink(&repo_dir, &diff_ctx)?;
            println!("\n📉 DebtShrinkGuard Result: {}\nViolations: {}\n", res.summary, res.violations.len());
        }
        Commands::ModularCheck { repo, pr } => {
            info!("Running Modularization Guard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let diff_ctx = state.git_mgr.prepare_pr_diff(&repo, pr, &meta.base_ref_name, &meta.base_ref_oid, &meta.head_ref_oid, None).await?;
            let res = state.modularization_guard.evaluate_modularization(&diff_ctx)?;
            println!("\n🧩 ModularizationGuard Result: {}\nOversized: {}\n", res.summary, res.oversized_files.len());
        }
        Commands::CoverageCheck { repo, pr } => {
            info!("Running CoverageGuard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state.git_mgr.prepare_pr_diff(&repo, pr, &meta.base_ref_name, &meta.base_ref_oid, &meta.head_ref_oid, None).await?;
            let res = state.coverage_guard.evaluate_diff_coverage(&repo_dir, &diff_ctx)?;
            println!("\n🎯 CoverageGuard Result: {}\nFindings: {}\n", res.summary, res.findings.len());
        }
        Commands::GhostMigrationCheck { repo, pr } => {
            info!("Running GhostMigrationHarness on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state.git_mgr.prepare_pr_diff(&repo, pr, &meta.base_ref_name, &meta.base_ref_oid, &meta.head_ref_oid, None).await?;
            let res = state.ghost_migration_harness.evaluate_migrations(&repo_dir, &diff_ctx)?;
            println!("\n🐘 GhostMigrationHarness Result: {}\nViolations: {}\n", res.summary, res.violations.len());
        }
        Commands::ChaosMutationCheck { repo, pr } => {
            info!("Running ChaosMutationGuard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let diff_ctx = state.git_mgr.prepare_pr_diff(&repo, pr, &meta.base_ref_name, &meta.base_ref_oid, &meta.head_ref_oid, None).await?;
            let res = state.chaos_mutation_guard.evaluate_mutation_adequacy(&diff_ctx)?;
            println!("\n💥 ChaosMutationGuard Result: {}\nFindings: {}\n", res.summary, res.surviving_findings.len());
        }
        Commands::FeatureFlagCheck { repo, pr } => {
            info!("Running FeatureFlagRatchet on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state.git_mgr.prepare_pr_diff(&repo, pr, &meta.base_ref_name, &meta.base_ref_oid, &meta.head_ref_oid, None).await?;
            let res = state.feature_flag_ratchet.evaluate_feature_flags(&repo_dir, &diff_ctx)?;
            println!("\n🚩 FeatureFlagRatchet Result: {}\nViolations: {}\n", res.summary, res.violations.len());
        }
        Commands::BenchCheck { repo, pr } => {
            info!("Running CriterionBenchRatchet on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state.git_mgr.prepare_pr_diff(&repo, pr, &meta.base_ref_name, &meta.base_ref_oid, &meta.head_ref_oid, None).await?;
            let res = state.criterion_bench_ratchet.evaluate_benchmarks(&repo_dir, &diff_ctx)?;
            println!("\n⚡ CriterionBenchRatchet Result: {}\nViolations: {}\n", res.summary, res.violations.len());
        }
        Commands::CedarCheck { repo, pr } => {
            info!("Running Cedar IAM Policy Guard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state.git_mgr.prepare_pr_diff(&repo, pr, &meta.base_ref_name, &meta.base_ref_oid, &meta.head_ref_oid, None).await?;
            let res = state.cedar_guard.evaluate_cedar_policies(&repo, &repo_dir, &diff_ctx, &meta.title).await?;
            println!("\n🛡️ CedarGuard Result: {}\nFiles: {:?}\n", res.summary, res.files_created_or_updated);
        }
        Commands::ComplianceCheck { repo, pr } => {
            info!("Running Dynamic Compliance Guard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let diff_ctx = state.git_mgr.prepare_pr_diff(&repo, pr, &meta.base_ref_name, &meta.base_ref_oid, &meta.head_ref_oid, None).await?;
            let res = state.compliance_guard.evaluate_compliance(&diff_ctx)?;
            println!("\n🏛️ ComplianceGuard Result: {}\nViolations: {}\n", res.summary, res.violations.len());
        }
        Commands::ApiCheck { repo, pr } => {
            info!("Running ApiContractGuard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state.git_mgr.prepare_pr_diff(&repo, pr, &meta.base_ref_name, &meta.base_ref_oid, &meta.head_ref_oid, None).await?;
            let res = state.api_contract_guard.ensure_contract_integrity(&repo, &repo_dir, &diff_ctx).await?;
            println!("\n📐 ApiContractGuard Result: {}\nSynced Files: {:?}\n", res.summary, res.auto_synced_files);
        }
        Commands::CellCheck { repo, pr } => {
            info!("Running CellIsolationGuard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let diff_ctx = state.git_mgr.prepare_pr_diff(&repo, pr, &meta.base_ref_name, &meta.base_ref_oid, &meta.head_ref_oid, None).await?;
            let res = state.cell_isolation_guard.evaluate_cell_isolation(&diff_ctx)?;
            println!("\n🌐 CellIsolationGuard Result: {}\nViolations: {}\n", res.summary, res.violations.len());
        }
        Commands::SupplyChainCheck { repo, pr } => {
            info!("Running SupplyChainGuard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let diff_ctx = state.git_mgr.prepare_pr_diff(&repo, pr, &meta.base_ref_name, &meta.base_ref_oid, &meta.head_ref_oid, None).await?;
            let res = state.supply_chain_guard.audit_supply_chain(&repo_dir, &diff_ctx)?;
            println!("\n📦 SupplyChainGuard Result: {}\nAudited: {}\n", res.summary, res.audited_packages);
        }
        Commands::Attest { repo, pr } => {
            info!("Running AttestationGuard on {}#{}", repo, pr);
            let meta = state.github_client.fetch_pr_metadata(&repo, pr).await?;
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;
            let res = state.attestation_guard.stamp_lane_receipt(&repo_dir, &repo, pr, &meta.head_ref_oid).await?;
            println!("\n🔏 AttestationGuard Result: {}\nPath: {:?}\n", res.summary, res.stamped_receipt_path);
        }
        Commands::Triage {
            repo,
            run_id,
            branch,
            commit_sha,
            workflow_name,
        } => {
            info!("Running on-demand trunk CI triage for run #{} on {}", run_id, repo);
            let branch_str = branch.unwrap_or_else(|| "main".to_string());
            let sha_str = commit_sha.unwrap_or_default();
            let wf_str = workflow_name.unwrap_or_else(|| "CI".to_string());
            let repo_dir = state.git_mgr.ensure_repo_cloned(&repo).await?;

            state.ci_triager
                .triage_workflow_run(&repo, run_id, &branch_str, &sha_str, &wf_str, &repo_dir)
                .await?;
        }
        Commands::Enlist { repo, pr } => {
            info!("Running on-demand merge queue enlistment for {}#{}", repo, pr);
            state.merge_enlister.enlist_into_merge_queue(&repo, pr).await?;
        }
        Commands::HealQueue { repo, pr } => {
            info!("Running on-demand merge queue healer for {}#{}", repo, pr);
            state.queue_healer.heal_ejected_pr(&repo, pr).await?;
        }
        Commands::Reconcile { repo, pr } => {
            info!("Running on-demand lockfile/ledger reconciler for {}#{}", repo, pr);
            state.lockfile_reconciler.reconcile_pr(&repo, pr).await?;
        }
        Commands::Forward => {
            info!("Starting webhook forwarders for {:?}", state.config.watched_repos);
            server::start_forwarders(&state.config).await?;
        }
        Commands::Check => {
            info!("Checking system dependencies and authentication...");
            server::check_environment(&state.github_client, &state.config).await?;
        }
    }

    Ok(())
}
