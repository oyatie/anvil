#![allow(
    dead_code,
    unused_imports,
    clippy::too_many_arguments,
    clippy::new_without_default,
    clippy::collapsible_if,
    clippy::type_complexity,
    clippy::large_enum_variant,
    clippy::manual_strip,
    clippy::useless_format,
    clippy::useless_borrows_in_formatting,
    clippy::double_ended_iterator_last,
    clippy::single_match,
    clippy::redundant_closure,
    clippy::ptr_arg,
    clippy::derivable_impls
)]

use anyhow::Result;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod adr_drift_ratchet;
mod ai_driver;
mod api_contract_guard;
mod attestation_guard;
mod auto_rollback;
mod automated_canary;
mod canary_rollout;
mod carbon_aware;
mod cedar_guard;
mod cell_isolation_guard;
mod chaos_injector;
mod chaos_mutation_guard;
mod ci_runner_economics;
mod ci_triager;
mod ci_wallclock_ratchet;
mod clean_architecture_guard;
mod cli;
mod cluster_state_auditor;
mod compile_time_profiler;
mod compliance_guard;
mod config;
mod consistency_guard;
mod constant_work_guard;
mod cosign_signer;
mod coverage_guard;
mod criterion_bench_ratchet;
mod cross_service_impact;
mod deadlock_analyzer;
mod debt_shrink_guard;
mod doc_guard;
mod early_exit_cascade;
mod ephemeral_sandbox;
mod ephemeral_secrets;
mod feature_flag_ratchet;
mod finops_ratchet;
mod fixer;
mod flake_bisector;
mod flake_cost_dampener;
mod flake_quarantine;
mod formal_verification;
mod ghost_migration_harness;
mod git_manager;
mod github;
mod gitops_drift_reconciler;
mod gitops_promotion;
mod hermetic_build;
mod idempotency_guard;
mod incident_healer;
mod incident_sentry;
mod jittered_backoff;
mod kani_guard;
mod local_inner_loop;
mod lockfile_reconciler;
mod mainline_ci_healer;
mod merge_enlister;
mod microbenchmark_ratchet;
mod migration_orchestrator;
mod modularization_guard;
mod monorepo_guard;
mod pre_merge_guard;
mod predictive_test_selector;
mod preview_env_reaper;
mod progressive_rollout;
mod psa_admission_guard;
mod queue_healer;
mod remote_cache_optimizer;
mod replay_harness;
mod review_memory;
mod reviewer;
mod rust_skills_guard;
mod schema_evolution;
mod semantic_abi_ratchet;
mod shadow_traffic_harness;
mod shuffle_shard_simulator;
mod slo_canary_guard;
mod stacked_diffs;
mod state;
mod supply_chain_guard;
mod trace_context_guard;
mod unresolved_review_guard;
mod upgrade_train;
mod vex_scanner;
mod wasm_sandbox;
mod webhook;
mod zero_day_patcher;
mod zero_trust_workload;

use adr_drift_ratchet::AdrDriftRatchet;
use api_contract_guard::ApiContractGuard;
use attestation_guard::AttestationGuard;
use auto_rollback::AutoRollbackPostmortemEngine;
use automated_canary::AutomatedCanaryAnalysis;
use canary_rollout::CanaryRolloutGuard;
use carbon_aware::CarbonAwareComputeRatchet;
use cedar_guard::CedarGuard;
use cell_isolation_guard::CellIsolationGuard;
use chaos_injector::ChaosFaultInjector;
use chaos_mutation_guard::ChaosMutationGuard;
use ci_runner_economics::CiRunnerEconomicsOptimizer;
use ci_triager::CiTriager;
use ci_wallclock_ratchet::CiWallclockEconomicsRatchet;
use clean_architecture_guard::CleanArchitectureGuard;
use cli::handle_cli;
use cluster_state_auditor::ClusterStateAuditor;
use compile_time_profiler::CompileTimeProfiler;
use compliance_guard::ComplianceGuard;
use config::Config;
use consistency_guard::ActiveActiveConsistencyGuard;
use constant_work_guard::ConstantWorkGuard;
use cosign_signer::CosignProvenanceSigner;
use coverage_guard::CoverageGuard;
use criterion_bench_ratchet::CriterionBenchRatchet;
use cross_service_impact::CrossServiceImpactEngine;
use deadlock_analyzer::DeadlockStaticAnalyzer;
use debt_shrink_guard::DebtShrinkGuard;
use doc_guard::DocGuard;
use early_exit_cascade::EarlyExitCascadeGuard;
use ephemeral_sandbox::EphemeralSandboxManager;
use ephemeral_secrets::EphemeralSecretInjector;
use feature_flag_ratchet::FeatureFlagRatchet;
use finops_ratchet::FinOpsUnitCostRatchet;
use fixer::Fixer;
use flake_bisector::FlakeBisectorEngine;
use flake_cost_dampener::FlakeCostDampener;
use flake_quarantine::FlakeQuarantineLifecycle;
use formal_verification::FormalVerificationGuard;
use ghost_migration_harness::GhostMigrationHarness;
use git_manager::GitManager;
use github::GitHubClient;
use gitops_drift_reconciler::GitOpsDriftReconciler;
use gitops_promotion::GitOpsPromotionEngine;
use hermetic_build::HermeticBuildValidator;
use idempotency_guard::IdempotencyGuard;
use incident_healer::IncidentHealer;
use incident_sentry::IncidentSentryCircuitBreaker;
use jittered_backoff::JitteredBackoffGuard;
use kani_guard::KaniGuard;
use local_inner_loop::LocalInnerLoopProbe;
use lockfile_reconciler::LockfileReconciler;
use mainline_ci_healer::MainlineCiHealer;
use merge_enlister::MergeEnlister;
use microbenchmark_ratchet::MicroBenchmarkRatchet;
use migration_orchestrator::MigrationLifecycleOrchestrator;
use modularization_guard::ModularizationGuard;
use monorepo_guard::MonorepoGuard;
use pre_merge_guard::PreMergeGuard;
use predictive_test_selector::PredictiveTestSelector;
use preview_env_reaper::PreviewEnvReaper;
use progressive_rollout::ProgressiveRingOrchestrator;
use psa_admission_guard::PsaAdmissionGuard;
use queue_healer::QueueHealer;
use remote_cache_optimizer::RemoteCacheOptimizer;
use replay_harness::DeterministicReplayHarness;
use review_memory::ReviewMemoryEngine;
use reviewer::Reviewer;
use rust_skills_guard::RustSkillsGuard;
use schema_evolution::SchemaEvolutionRatchet;
use semantic_abi_ratchet::SemanticAbiRatchet;
use shadow_traffic_harness::ShadowTrafficHarness;
use shuffle_shard_simulator::ShuffleShardSimulator;
use slo_canary_guard::SloCanaryGuard;
use stacked_diffs::StackedDiffsOrchestrator;
use state::StateManager;
use supply_chain_guard::SupplyChainGuard;
use trace_context_guard::TraceContextGuard;
use unresolved_review_guard::UnresolvedReviewGuard;
use upgrade_train::ProactiveUpgradeTrain;
use vex_scanner::OpenVexReachabilityScanner;
use wasm_sandbox::WasmPolicySandbox;
use webhook::AppState;
use zero_day_patcher::ZeroDayAutoPatcher;
use zero_trust_workload::ZeroTrustWorkloadGate;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "anvil=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Arc::new(Config::from_env());
    let git_mgr = Arc::new(GitManager::new(config.repos_dir.clone()));
    let reviewer = Arc::new(Reviewer::new(
        config.to_model_config(),
        config.rules_path.clone(),
    ));
    let github_client = Arc::new(GitHubClient::new());
    let state_mgr = Arc::new(StateManager::load(&config.data_dir).await?);
    let fixer = Arc::new(Fixer::new(
        git_mgr.clone(),
        github_client.clone(),
        config.agy_effort.clone(),
    ));
    let doc_guard = Arc::new(DocGuard::new(config.agy_effort.clone()));
    let cedar_guard = Arc::new(CedarGuard::new(config.agy_effort.clone()));
    let compliance_guard = Arc::new(ComplianceGuard::new());
    let api_contract_guard = Arc::new(ApiContractGuard::new());
    let cell_isolation_guard = Arc::new(CellIsolationGuard::new());
    let supply_chain_guard = Arc::new(SupplyChainGuard::new());
    let clean_arch_guard = Arc::new(CleanArchitectureGuard::new());
    let monorepo_guard = Arc::new(MonorepoGuard::new());
    let debt_shrink_guard = Arc::new(DebtShrinkGuard::new());
    let modularization_guard = Arc::new(ModularizationGuard::new());
    let coverage_guard = Arc::new(CoverageGuard::new());
    let rust_skills_guard = Arc::new(RustSkillsGuard::new(&config.data_dir));
    let kani_guard = Arc::new(KaniGuard::new());
    let slo_canary_guard = Arc::new(SloCanaryGuard::new());
    let adr_drift_ratchet = Arc::new(AdrDriftRatchet::new());
    let shuffle_shard_simulator = Arc::new(ShuffleShardSimulator::new());
    let trace_context_guard = Arc::new(TraceContextGuard::new());
    let constant_work_guard = Arc::new(ConstantWorkGuard::new());
    let idempotency_guard = Arc::new(IdempotencyGuard::new());
    let finops_ratchet = Arc::new(FinOpsUnitCostRatchet::new());
    let ghost_migration_harness = Arc::new(GhostMigrationHarness::new());
    let gitops_promotion_engine = Arc::new(GitOpsPromotionEngine::new());
    let gitops_drift_reconciler = Arc::new(GitOpsDriftReconciler::new());
    let canary_rollout_guard = Arc::new(CanaryRolloutGuard::new());
    let cluster_state_auditor = Arc::new(ClusterStateAuditor::new());
    let migration_orchestrator = Arc::new(MigrationLifecycleOrchestrator::new());
    let incident_healer = Arc::new(IncidentHealer::new(config.agy_effort.clone()));
    let ci_wallclock_ratchet = Arc::new(CiWallclockEconomicsRatchet::new());
    let predictive_test_selector = Arc::new(PredictiveTestSelector::new());
    let compile_time_profiler = Arc::new(CompileTimeProfiler::new());
    let remote_cache_optimizer = Arc::new(RemoteCacheOptimizer::new());
    let ci_runner_economics = Arc::new(CiRunnerEconomicsOptimizer::new());
    let early_exit_cascade = Arc::new(EarlyExitCascadeGuard::new());
    let flake_cost_dampener = Arc::new(FlakeCostDampener::new());
    let ephemeral_sandbox = Arc::new(EphemeralSandboxManager::new());
    let cross_service_impact = Arc::new(CrossServiceImpactEngine::new());
    let ephemeral_secrets = Arc::new(EphemeralSecretInjector::new());
    let psa_admission_guard = Arc::new(PsaAdmissionGuard::new());
    let shadow_traffic_harness = Arc::new(ShadowTrafficHarness::new());
    let flake_bisector = Arc::new(FlakeBisectorEngine::new());
    let unresolved_review_guard = Arc::new(UnresolvedReviewGuard::new(github_client.clone()));
    let local_inner_loop = Arc::new(LocalInnerLoopProbe::new());
    let semantic_abi_ratchet = Arc::new(SemanticAbiRatchet::new());
    let incident_sentry = Arc::new(IncidentSentryCircuitBreaker::new());
    let preview_env_reaper = Arc::new(PreviewEnvReaper::new());
    let review_memory = Arc::new(ReviewMemoryEngine::new());
    let zero_day_patcher = Arc::new(ZeroDayAutoPatcher::new());
    let mainline_ci_healer = Arc::new(MainlineCiHealer::new(github_client.clone()));
    let formal_verification = Arc::new(FormalVerificationGuard::new());
    let deadlock_analyzer = Arc::new(DeadlockStaticAnalyzer::new());
    let automated_canary = Arc::new(AutomatedCanaryAnalysis::new());
    let progressive_rollout = Arc::new(ProgressiveRingOrchestrator::new());
    let hermetic_build = Arc::new(HermeticBuildValidator::new());
    let vex_scanner = Arc::new(OpenVexReachabilityScanner::new());
    let cosign_signer = Arc::new(CosignProvenanceSigner::new());
    let chaos_injector = Arc::new(ChaosFaultInjector::new());
    let stacked_diffs = Arc::new(StackedDiffsOrchestrator::new());
    let microbenchmark_ratchet = Arc::new(MicroBenchmarkRatchet::new());
    let jittered_backoff = Arc::new(JitteredBackoffGuard::new());
    let schema_evolution = Arc::new(SchemaEvolutionRatchet::new());
    let auto_rollback = Arc::new(AutoRollbackPostmortemEngine::new());
    let wasm_sandbox = Arc::new(WasmPolicySandbox::new());
    let consistency_guard = Arc::new(ActiveActiveConsistencyGuard::new());
    let flake_quarantine = Arc::new(FlakeQuarantineLifecycle::new());
    let zero_trust_workload = Arc::new(ZeroTrustWorkloadGate::new());
    let carbon_aware = Arc::new(CarbonAwareComputeRatchet::new());
    let replay_harness = Arc::new(DeterministicReplayHarness::new());
    let upgrade_train = Arc::new(ProactiveUpgradeTrain::new());
    let chaos_mutation_guard = Arc::new(ChaosMutationGuard::new());
    let feature_flag_ratchet = Arc::new(FeatureFlagRatchet::new());
    let criterion_bench_ratchet = Arc::new(CriterionBenchRatchet::new());
    let attestation_guard = Arc::new(AttestationGuard::new());
    let pre_merge_guard = Arc::new(PreMergeGuard::new());
    let merge_enlister = Arc::new(MergeEnlister::new(github_client.clone()));
    let queue_healer = Arc::new(QueueHealer::new(
        git_mgr.clone(),
        github_client.clone(),
        merge_enlister.clone(),
        config.agy_effort.clone(),
    ));
    let lockfile_reconciler = Arc::new(LockfileReconciler::new(
        git_mgr.clone(),
        github_client.clone(),
    ));
    let ci_triager = Arc::new(CiTriager::new(
        github_client.clone(),
        config.agy_effort.clone(),
    ));

    let app_state = AppState {
        config: config.clone(),
        git_mgr: git_mgr.clone(),
        reviewer: reviewer.clone(),
        fixer: fixer.clone(),
        doc_guard: doc_guard.clone(),
        cedar_guard: cedar_guard.clone(),
        compliance_guard: compliance_guard.clone(),
        api_contract_guard: api_contract_guard.clone(),
        cell_isolation_guard: cell_isolation_guard.clone(),
        supply_chain_guard: supply_chain_guard.clone(),
        clean_arch_guard: clean_arch_guard.clone(),
        monorepo_guard: monorepo_guard.clone(),
        debt_shrink_guard: debt_shrink_guard.clone(),
        modularization_guard: modularization_guard.clone(),
        coverage_guard: coverage_guard.clone(),
        rust_skills_guard: rust_skills_guard.clone(),
        kani_guard: kani_guard.clone(),
        slo_canary_guard: slo_canary_guard.clone(),
        adr_drift_ratchet: adr_drift_ratchet.clone(),
        shuffle_shard_simulator: shuffle_shard_simulator.clone(),
        trace_context_guard: trace_context_guard.clone(),
        constant_work_guard: constant_work_guard.clone(),
        idempotency_guard: idempotency_guard.clone(),
        finops_ratchet: finops_ratchet.clone(),
        ghost_migration_harness: ghost_migration_harness.clone(),
        gitops_promotion_engine: gitops_promotion_engine.clone(),
        gitops_drift_reconciler: gitops_drift_reconciler.clone(),
        canary_rollout_guard: canary_rollout_guard.clone(),
        cluster_state_auditor: cluster_state_auditor.clone(),
        migration_orchestrator: migration_orchestrator.clone(),
        incident_healer: incident_healer.clone(),
        ci_wallclock_ratchet: ci_wallclock_ratchet.clone(),
        predictive_test_selector: predictive_test_selector.clone(),
        compile_time_profiler: compile_time_profiler.clone(),
        remote_cache_optimizer: remote_cache_optimizer.clone(),
        ci_runner_economics: ci_runner_economics.clone(),
        early_exit_cascade: early_exit_cascade.clone(),
        flake_cost_dampener: flake_cost_dampener.clone(),
        ephemeral_sandbox: ephemeral_sandbox.clone(),
        cross_service_impact: cross_service_impact.clone(),
        ephemeral_secrets: ephemeral_secrets.clone(),
        psa_admission_guard: psa_admission_guard.clone(),
        shadow_traffic_harness: shadow_traffic_harness.clone(),
        flake_bisector: flake_bisector.clone(),
        unresolved_review_guard: unresolved_review_guard.clone(),
        local_inner_loop: local_inner_loop.clone(),
        semantic_abi_ratchet: semantic_abi_ratchet.clone(),
        incident_sentry: incident_sentry.clone(),
        preview_env_reaper: preview_env_reaper.clone(),
        review_memory: review_memory.clone(),
        zero_day_patcher: zero_day_patcher.clone(),
        mainline_ci_healer: mainline_ci_healer.clone(),
        formal_verification: formal_verification.clone(),
        deadlock_analyzer: deadlock_analyzer.clone(),
        automated_canary: automated_canary.clone(),
        progressive_rollout: progressive_rollout.clone(),
        hermetic_build: hermetic_build.clone(),
        vex_scanner: vex_scanner.clone(),
        cosign_signer: cosign_signer.clone(),
        chaos_injector: chaos_injector.clone(),
        stacked_diffs: stacked_diffs.clone(),
        microbenchmark_ratchet: microbenchmark_ratchet.clone(),
        jittered_backoff: jittered_backoff.clone(),
        schema_evolution: schema_evolution.clone(),
        auto_rollback: auto_rollback.clone(),
        wasm_sandbox: wasm_sandbox.clone(),
        consistency_guard: consistency_guard.clone(),
        flake_quarantine: flake_quarantine.clone(),
        zero_trust_workload: zero_trust_workload.clone(),
        carbon_aware: carbon_aware.clone(),
        replay_harness: replay_harness.clone(),
        upgrade_train: upgrade_train.clone(),
        chaos_mutation_guard: chaos_mutation_guard.clone(),
        feature_flag_ratchet: feature_flag_ratchet.clone(),
        criterion_bench_ratchet: criterion_bench_ratchet.clone(),
        attestation_guard: attestation_guard.clone(),
        pre_merge_guard: pre_merge_guard.clone(),
        merge_enlister: merge_enlister.clone(),
        queue_healer: queue_healer.clone(),
        lockfile_reconciler: lockfile_reconciler.clone(),
        ci_triager: ci_triager.clone(),
        github_client: github_client.clone(),
        state_mgr: state_mgr.clone(),
    };

    let res = handle_cli(app_state).await;
    match res {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            tracing::error!("Command failed: {:?}", e);
            std::process::exit(1);
        }
    }
}
