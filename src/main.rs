//! Oyatie Anvil CLI & Autonomous Server Daemon
//!
//! Entrypoint for `anvil` CLI commands and background lifecycle daemons.

use anyhow::Result;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use anvil::adr_drift_ratchet::AdrDriftRatchet;
use anvil::api_contract_guard::ApiContractGuard;
use anvil::attestation_guard::AttestationGuard;
use anvil::auto_rollback::AutoRollbackPostmortemEngine;
use anvil::automated_canary::AutomatedCanaryAnalysis;
use anvil::canary_rollout::CanaryRolloutGuard;
use anvil::carbon_aware::CarbonAwareComputeRatchet;
use anvil::cedar_guard::CedarGuard;
use anvil::cell_isolation_guard::CellIsolationGuard;
use anvil::chaos_injector::ChaosFaultInjector;
use anvil::chaos_mutation_guard::ChaosMutationGuard;
use anvil::ci_runner_economics::CiRunnerEconomicsOptimizer;
use anvil::ci_triager::CiTriager;
use anvil::ci_wallclock_ratchet::CiWallclockEconomicsRatchet;
use anvil::clean_architecture_guard::CleanArchitectureGuard;
use anvil::cli::handle_cli;
use anvil::cluster_state_auditor::ClusterStateAuditor;
use anvil::compile_time_profiler::CompileTimeProfiler;
use anvil::compliance_guard::ComplianceGuard;
use anvil::config::Config;
use anvil::consistency_guard::ActiveActiveConsistencyGuard;
use anvil::constant_work_guard::ConstantWorkGuard;
use anvil::cosign_signer::CosignProvenanceSigner;
use anvil::coverage_guard::CoverageGuard;
use anvil::criterion_bench_ratchet::CriterionBenchRatchet;
use anvil::cross_service_impact::CrossServiceImpactEngine;
use anvil::deadlock_analyzer::DeadlockStaticAnalyzer;
use anvil::debt_shrink_guard::DebtShrinkGuard;
use anvil::doc_guard::DocGuard;
use anvil::early_exit_cascade::EarlyExitCascadeGuard;
use anvil::ephemeral_sandbox::EphemeralSandboxManager;
use anvil::ephemeral_secrets::EphemeralSecretInjector;
use anvil::feature_flag_ratchet::FeatureFlagRatchet;
use anvil::finops_ratchet::FinOpsUnitCostRatchet;
use anvil::fixer::Fixer;
use anvil::flake_bisector::FlakeBisectorEngine;
use anvil::flake_cost_dampener::FlakeCostDampener;
use anvil::flake_quarantine::FlakeQuarantineLifecycle;
use anvil::formal_verification::FormalVerificationGuard;
use anvil::ghost_migration_harness::GhostMigrationHarness;
use anvil::git_manager::GitManager;
use anvil::github::GitHubClient;
use anvil::gitops_drift_reconciler::GitOpsDriftReconciler;
use anvil::gitops_promotion::GitOpsPromotionEngine;
use anvil::hermetic_build::HermeticBuildValidator;
use anvil::idempotency_guard::IdempotencyGuard;
use anvil::incident_healer::IncidentHealer;
use anvil::incident_sentry::IncidentSentryCircuitBreaker;
use anvil::jittered_backoff::JitteredBackoffGuard;
use anvil::kani_guard::KaniGuard;
use anvil::local_inner_loop::LocalInnerLoopProbe;
use anvil::lockfile_reconciler::LockfileReconciler;
use anvil::mainline_ci_healer::MainlineCiHealer;
use anvil::merge_enlister::MergeEnlister;
use anvil::microbenchmark_ratchet::MicroBenchmarkRatchet;
use anvil::migration_orchestrator::MigrationLifecycleOrchestrator;
use anvil::modularization_guard::ModularizationGuard;
use anvil::monorepo_guard::MonorepoGuard;
use anvil::pre_merge_guard::PreMergeGuard;
use anvil::predictive_test_selector::PredictiveTestSelector;
use anvil::preview_env_reaper::PreviewEnvReaper;
use anvil::progressive_rollout::ProgressiveRingOrchestrator;
use anvil::psa_admission_guard::PsaAdmissionGuard;
use anvil::queue_healer::QueueHealer;
use anvil::remote_cache_optimizer::RemoteCacheOptimizer;
use anvil::replay_harness::DeterministicReplayHarness;
use anvil::review_memory::ReviewMemoryEngine;
use anvil::reviewer::Reviewer;
use anvil::rust_skills_guard::RustSkillsGuard;
use anvil::schema_evolution::SchemaEvolutionRatchet;
use anvil::semantic_abi_ratchet::SemanticAbiRatchet;
use anvil::shadow_traffic_harness::ShadowTrafficHarness;
use anvil::shuffle_shard_simulator::ShuffleShardSimulator;
use anvil::slo_canary_guard::SloCanaryGuard;
use anvil::stacked_diffs::StackedDiffsOrchestrator;
use anvil::state::StateManager;
use anvil::supply_chain_guard::SupplyChainGuard;
use anvil::trace_context_guard::TraceContextGuard;
use anvil::unresolved_review_guard::UnresolvedReviewGuard;
use anvil::upgrade_train::ProactiveUpgradeTrain;
use anvil::vex_scanner::OpenVexReachabilityScanner;
use anvil::wasm_sandbox::WasmPolicySandbox;
use anvil::webhook::AppState;
use anvil::zero_day_patcher::ZeroDayAutoPatcher;
use anvil::zero_trust_workload::ZeroTrustWorkloadGate;

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
    let metrics = Arc::new(anvil::metrics::PrometheusRegistry::new());
    let self_governor = Arc::new(anvil::self_governance::SelfGovernor::new());
    let telemetry_store =
        Arc::new(anvil::telemetry_store::TelemetryStore::new("data/telemetry").await);
    let fleet_observer = Arc::new(anvil::fleet_observer::FleetObserver::new(
        github_client.clone(),
        telemetry_store.clone(),
    ));
    let broadcaster = Arc::new(anvil::webhook::sse::FleetEventBroadcaster::new());
    let verifier = Arc::new(anvil::task_orchestrator::SourceDocVerifier::new());
    let sequencer = Arc::new(anvil::task_orchestrator::TaskDagSequencer::new());
    let fix_engine = Arc::new(anvil::task_orchestrator::AutonomousFixEngine::new(
        git_mgr.clone(),
        github_client.clone(),
        Arc::new(anvil::ai_driver::SubscriptionExecutor::with_pool(Arc::new(self_governor.quota.account_pool.clone()))),
        self_governor.deathloop.clone(),
    ));
    let task_orchestrator = Arc::new(anvil::task_orchestrator::AutonomousTaskOrchestrator::new(
        verifier,
        sequencer,
        fix_engine,
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
        metrics,
        self_governor,
        broadcaster,
        telemetry_store,
        fleet_observer,
        task_orchestrator,
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
