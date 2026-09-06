//! `AppState`: every collaborator one webhook request may reach.
//!
//! Its own file because it is a list, not logic. Seventy-odd fields grew
//! `webhook/mod.rs` past ADR-0719 D-35's budget while the routing it exists to
//! do is a page, and a list that has to be edited by every change is the file a
//! change should not also have to read past.

use crate::adr_drift_ratchet::AdrDriftRatchet;
use crate::api_contract_guard::ApiContractGuard;
use crate::attestation_guard::AttestationGuard;
use crate::auto_rollback::AutoRollbackPostmortemEngine;
use crate::automated_canary::AutomatedCanaryAnalysis;
use crate::canary_rollout::CanaryRolloutGuard;
use crate::carbon_aware::CarbonAwareComputeRatchet;
use crate::cedar_guard::CedarGuard;
use crate::cell_isolation_guard::CellIsolationGuard;
use crate::chaos_injector::ChaosFaultInjector;
use crate::chaos_mutation_guard::ChaosMutationGuard;
use crate::ci_runner_economics::CiRunnerEconomicsOptimizer;
use crate::ci_triager::CiTriager;
use crate::ci_wallclock_ratchet::CiWallclockEconomicsRatchet;
use crate::clean_architecture_guard::CleanArchitectureGuard;
use crate::cluster_state_auditor::ClusterStateAuditor;
use crate::compile_time_profiler::CompileTimeProfiler;
use crate::compliance_guard::ComplianceGuard;
use crate::config::Config;
use crate::consistency_guard::ActiveActiveConsistencyGuard;
use crate::constant_work_guard::ConstantWorkGuard;
use crate::cosign_signer::CosignProvenanceSigner;
use crate::coverage_guard::CoverageGuard;
use crate::criterion_bench_ratchet::CriterionBenchRatchet;
use crate::cross_service_impact::CrossServiceImpactEngine;
use crate::deadlock_analyzer::DeadlockStaticAnalyzer;
use crate::debt_shrink_guard::DebtShrinkGuard;
use crate::doc_guard::DocGuard;
use crate::early_exit_cascade::EarlyExitCascadeGuard;
use crate::ephemeral_sandbox::EphemeralSandboxManager;
use crate::ephemeral_secrets::EphemeralSecretInjector;
use crate::feature_flag_ratchet::FeatureFlagRatchet;
use crate::finops_ratchet::FinOpsUnitCostRatchet;
use crate::fixer::Fixer;
use crate::flake_bisector::FlakeBisectorEngine;
use crate::flake_cost_dampener::FlakeCostDampener;
use crate::flake_quarantine::FlakeQuarantineLifecycle;
use crate::formal_verification::FormalVerificationGuard;
use crate::ghost_migration_harness::GhostMigrationHarness;
use crate::git_manager::GitManager;
use crate::github::GitHubClient;
use crate::gitops_drift_reconciler::GitOpsDriftReconciler;
use crate::gitops_promotion::GitOpsPromotionEngine;
use crate::hermetic_build::HermeticBuildValidator;
use crate::idempotency_guard::IdempotencyGuard;
use crate::incident_healer::IncidentHealer;
use crate::incident_sentry::IncidentSentryCircuitBreaker;
use crate::jittered_backoff::JitteredBackoffGuard;
use crate::kani_guard::KaniGuard;
use crate::local_inner_loop::LocalInnerLoopProbe;
use crate::lockfile_reconciler::LockfileReconciler;
use crate::mainline_ci_healer::MainlineCiHealer;
use crate::merge_enlister::MergeEnlister;
use crate::microbenchmark_ratchet::MicroBenchmarkRatchet;
use crate::migration_orchestrator::MigrationLifecycleOrchestrator;
use crate::modularization_guard::ModularizationGuard;
use crate::monorepo_guard::MonorepoGuard;
use crate::pre_merge_guard::PreMergeGuard;
use crate::predictive_test_selector::PredictiveTestSelector;
use crate::preview_env_reaper::PreviewEnvReaper;
use crate::progressive_rollout::ProgressiveRingOrchestrator;
use crate::psa_admission_guard::PsaAdmissionGuard;
use crate::queue_healer::QueueHealer;
use crate::remote_cache_optimizer::RemoteCacheOptimizer;
use crate::replay_harness::DeterministicReplayHarness;
use crate::review_memory::ReviewMemoryEngine;
use crate::reviewer::Reviewer;
use crate::rust_language_policy::RustLanguagePolicy;
use crate::schema_evolution::SchemaEvolutionRatchet;
use crate::semantic_abi_ratchet::SemanticAbiRatchet;
use crate::shadow_traffic_harness::ShadowTrafficHarness;
use crate::shuffle_shard_simulator::ShuffleShardSimulator;
use crate::slo_canary_guard::SloCanaryGuard;
use crate::stacked_diffs::StackedDiffsOrchestrator;
use crate::state::StateManager;
use crate::supply_chain_guard::SupplyChainGuard;
use crate::trace_context_guard::TraceContextGuard;
use crate::unresolved_review_guard::UnresolvedReviewGuard;
use crate::vex_scanner::OpenVexReachabilityScanner;
use crate::wasm_sandbox::WasmPolicySandbox;
use crate::zero_day_patcher::ZeroDayAutoPatcher;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub git_mgr: Arc<GitManager>,
    pub reviewer: Arc<Reviewer>,
    pub fixer: Arc<Fixer>,
    pub doc_guard: Arc<DocGuard>,
    pub cedar_guard: Arc<CedarGuard>,
    pub compliance_guard: Arc<ComplianceGuard>,
    pub api_contract_guard: Arc<ApiContractGuard>,
    pub cell_isolation_guard: Arc<CellIsolationGuard>,
    pub supply_chain_guard: Arc<SupplyChainGuard>,
    pub clean_arch_guard: Arc<CleanArchitectureGuard>,
    pub monorepo_guard: Arc<MonorepoGuard>,
    pub debt_shrink_guard: Arc<DebtShrinkGuard>,
    pub modularization_guard: Arc<ModularizationGuard>,
    pub coverage_guard: Arc<CoverageGuard>,
    pub rust_language_policy: Arc<RustLanguagePolicy>,
    pub kani_guard: Arc<KaniGuard>,
    pub slo_canary_guard: Arc<SloCanaryGuard>,
    pub adr_drift_ratchet: Arc<AdrDriftRatchet>,
    pub shuffle_shard_simulator: Arc<ShuffleShardSimulator>,
    pub trace_context_guard: Arc<TraceContextGuard>,
    pub constant_work_guard: Arc<ConstantWorkGuard>,
    pub idempotency_guard: Arc<IdempotencyGuard>,
    pub finops_ratchet: Arc<FinOpsUnitCostRatchet>,
    pub ghost_migration_harness: Arc<GhostMigrationHarness>,
    pub gitops_promotion_engine: Arc<GitOpsPromotionEngine>,
    pub gitops_drift_reconciler: Arc<GitOpsDriftReconciler>,
    pub canary_rollout_guard: Arc<CanaryRolloutGuard>,
    pub cluster_state_auditor: Arc<ClusterStateAuditor>,
    pub migration_orchestrator: Arc<MigrationLifecycleOrchestrator>,
    pub incident_healer: Arc<IncidentHealer>,
    pub ci_wallclock_ratchet: Arc<CiWallclockEconomicsRatchet>,
    pub predictive_test_selector: Arc<PredictiveTestSelector>,
    pub compile_time_profiler: Arc<CompileTimeProfiler>,
    pub remote_cache_optimizer: Arc<RemoteCacheOptimizer>,
    pub ci_runner_economics: Arc<CiRunnerEconomicsOptimizer>,
    pub early_exit_cascade: Arc<EarlyExitCascadeGuard>,
    pub flake_cost_dampener: Arc<FlakeCostDampener>,
    pub ephemeral_sandbox: Arc<EphemeralSandboxManager>,
    pub cross_service_impact: Arc<CrossServiceImpactEngine>,
    pub ephemeral_secrets: Arc<EphemeralSecretInjector>,
    pub psa_admission_guard: Arc<PsaAdmissionGuard>,
    pub shadow_traffic_harness: Arc<ShadowTrafficHarness>,
    pub flake_bisector: Arc<FlakeBisectorEngine>,
    pub unresolved_review_guard: Arc<UnresolvedReviewGuard>,
    pub local_inner_loop: Arc<LocalInnerLoopProbe>,
    pub semantic_abi_ratchet: Arc<SemanticAbiRatchet>,
    pub incident_sentry: Arc<IncidentSentryCircuitBreaker>,
    pub preview_env_reaper: Arc<PreviewEnvReaper>,
    pub review_memory: Arc<ReviewMemoryEngine>,
    pub zero_day_patcher: Arc<ZeroDayAutoPatcher>,
    pub mainline_ci_healer: Arc<MainlineCiHealer>,
    pub formal_verification: Arc<FormalVerificationGuard>,
    pub deadlock_analyzer: Arc<DeadlockStaticAnalyzer>,
    pub automated_canary: Arc<AutomatedCanaryAnalysis>,
    pub progressive_rollout: Arc<ProgressiveRingOrchestrator>,
    pub hermetic_build: Arc<HermeticBuildValidator>,
    pub vex_scanner: Arc<OpenVexReachabilityScanner>,
    pub cosign_signer: Arc<CosignProvenanceSigner>,
    pub chaos_injector: Arc<ChaosFaultInjector>,
    pub stacked_diffs: Arc<StackedDiffsOrchestrator>,
    pub microbenchmark_ratchet: Arc<MicroBenchmarkRatchet>,
    pub jittered_backoff: Arc<JitteredBackoffGuard>,
    pub schema_evolution: Arc<SchemaEvolutionRatchet>,
    pub auto_rollback: Arc<AutoRollbackPostmortemEngine>,
    pub wasm_sandbox: Arc<WasmPolicySandbox>,
    pub consistency_guard: Arc<ActiveActiveConsistencyGuard>,
    pub flake_quarantine: Arc<FlakeQuarantineLifecycle>,
    pub carbon_aware: Arc<CarbonAwareComputeRatchet>,
    pub replay_harness: Arc<DeterministicReplayHarness>,
    pub chaos_mutation_guard: Arc<ChaosMutationGuard>,
    pub feature_flag_ratchet: Arc<FeatureFlagRatchet>,
    pub criterion_bench_ratchet: Arc<CriterionBenchRatchet>,
    pub attestation_guard: Arc<AttestationGuard>,
    pub pre_merge_guard: Arc<PreMergeGuard>,
    pub merge_enlister: Arc<MergeEnlister>,
    pub queue_healer: Arc<QueueHealer>,
    pub lockfile_reconciler: Arc<LockfileReconciler>,
    pub ci_triager: Arc<CiTriager>,
    pub github_client: Arc<GitHubClient>,
    pub state_mgr: Arc<StateManager>,
    pub metrics: Arc<crate::metrics::PrometheusRegistry>,
    pub self_governor: Arc<crate::self_governance::SelfGovernor>,
    pub broadcaster: Arc<crate::webhook::sse::FleetEventBroadcaster>,
    pub telemetry_store: Arc<crate::telemetry_store::TelemetryStore>,
    pub fleet_observer: Arc<crate::fleet_observer::FleetObserver>,
    /// The control that stops Anvil without stopping the process.
    pub pause: Arc<crate::pause::Pause>,
    pub cloud_native_guard: Arc<crate::cloud_native_guard::CloudNativeGuard>,
    pub stack_whitelist_guard: Arc<crate::stack_whitelist_guard::StackWhitelistGuard>,
}
