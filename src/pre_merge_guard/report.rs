use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreMergeCertificationReport {
    pub is_certified_ready: bool,
    pub doc_parity_status: GateStatus,
    pub cedar_status: GateStatus,
    pub compliance_status: GateStatus,
    pub api_contract_status: GateStatus,
    pub cell_isolation_status: GateStatus,
    pub supply_chain_status: GateStatus,
    pub clean_arch_status: GateStatus,
    pub monorepo_status: GateStatus,
    pub debt_shrink_status: GateStatus,
    pub modularization_status: GateStatus,
    pub coverage_status: GateStatus,
    pub rust_skills_status: GateStatus,
    pub kani_status: GateStatus,
    pub slo_status: GateStatus,
    pub adr_status: GateStatus,
    pub shuffle_status: GateStatus,
    pub trace_status: GateStatus,
    pub constant_work_status: GateStatus,
    pub idempotency_status: GateStatus,
    pub finops_status: GateStatus,
    pub ghost_migration_status: GateStatus,
    pub gitops_promo_status: GateStatus,
    pub gitops_drift_status: GateStatus,
    pub canary_status: GateStatus,
    pub cluster_audit_status: GateStatus,
    pub migration_orch_status: GateStatus,
    pub ci_wallclock_status: GateStatus,
    pub predictive_test_status: GateStatus,
    pub compile_profile_status: GateStatus,
    pub remote_cache_status: GateStatus,
    pub runner_economics_status: GateStatus,
    pub sandbox_status: GateStatus,
    pub cross_service_status: GateStatus,
    pub ephemeral_secret_status: GateStatus,
    pub psa_status: GateStatus,
    pub shadow_traffic_status: GateStatus,
    pub unresolved_review_status: GateStatus,
    pub local_probe_status: GateStatus,
    pub semantic_abi_status: GateStatus,
    pub zero_day_status: GateStatus,
    pub formal_verification_status: GateStatus,
    pub deadlock_status: GateStatus,
    pub automated_canary_status: GateStatus,
    pub progressive_ring_status: GateStatus,
    pub hermetic_build_status: GateStatus,
    pub openvex_status: GateStatus,
    pub cosign_status: GateStatus,
    pub chaos_injection_status: GateStatus,
    pub stacked_diffs_status: GateStatus,
    pub microbench_status: GateStatus,
    pub jittered_backoff_status: GateStatus,
    pub schema_evolution_status: GateStatus,
    pub auto_rollback_status: GateStatus,
    pub wasm_sandbox_status: GateStatus,
    pub consistency_status: GateStatus,
    pub flake_quarantine_status: GateStatus,
    pub zero_trust_workload_status: GateStatus,
    pub carbon_compute_status: GateStatus,
    pub replay_harness_status: GateStatus,
    pub upgrade_train_status: GateStatus,
    pub mutation_status: GateStatus,
    pub feature_flag_status: GateStatus,
    pub bench_status: GateStatus,
    pub attestation_status: GateStatus,
    pub security_scan_status: GateStatus,
    pub schema_compat_status: GateStatus,
    pub performance_concurrency_status: GateStatus,
    pub test_suite_status: GateStatus,
    pub summary_markdown: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateStatus {
    Passed,
    AutoUpdated,
    Warning(String),
    Failed(String),
}

impl GateStatus {
    pub fn badge(&self) -> &'static str {
        match self {
            GateStatus::Passed => "✅ PASSED",
            GateStatus::AutoUpdated => "✨ AUTO-SYNCED",
            GateStatus::Warning(_) => "⚠️ WARNING",
            GateStatus::Failed(_) => "❌ FAILED",
        }
    }

    pub fn is_acceptable(&self) -> bool {
        match self {
            GateStatus::Passed | GateStatus::AutoUpdated => true,
            GateStatus::Warning(_) => true,
            GateStatus::Failed(_) => false,
        }
    }
}
