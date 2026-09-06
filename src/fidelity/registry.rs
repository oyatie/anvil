//! The declared fidelity of each audited gate.
//!
//! # Scope of this registry
//!
//! Entries exist ONLY for gates whose implementation was read directly: the
//! 2026-08-19/20 audit, and the 2026-08-30 pass that read the eighteen the
//! first one left. Every gate the certification report carries now has a row,
//! so `unaudited` in the gap report is zero -- and a gate added without one
//! raises it again rather than inheriting anybody's opinion.
//!
//! That rule is the point, not the coverage. Guessing a fidelity would
//! reproduce exactly the failure being corrected: a confident claim with
//! nothing behind it. An honest "not yet audited" is worth more than an
//! invented "Heuristic".

use super::GateFidelity;

mod entries_a;
mod entries_b;
mod entries_c;
mod entries_d;
mod entries_e;
mod entries_f;
mod entries_g;
mod entries_h;
mod entries_i;

/// Gates audited by direct code reading, each gap citing the symbol it rests on.
pub const AUDITED_GATES: &[GateFidelity] = &[
    entries_a::COVERAGE_STATUS,
    entries_a::KANI_STATUS,
    entries_a::SLO_STATUS,
    entries_a::TRACE_STATUS,
    entries_a::REMOTE_CACHE_STATUS,
    entries_a::MUTATION_STATUS,
    entries_a::SUPPLY_CHAIN_STATUS,
    entries_a::FORMAL_VERIFICATION_STATUS,
    entries_a::DEADLOCK_STATUS,
    entries_b::SEMANTIC_ABI_STATUS,
    entries_b::CI_WALLCLOCK_STATUS,
    entries_b::CLUSTER_AUDIT_STATUS,
    entries_b::SHADOW_TRAFFIC_STATUS,
    entries_b::WASM_SANDBOX_STATUS,
    entries_b::CLEARTEXT_TRANSPORT_STATUS,
    entries_b::DOC_PARITY_STATUS,
    entries_b::VENDOR_NEUTRALITY_STATUS,
    entries_b::STACK_WHITELIST_STATUS,
    entries_b::UNRESOLVED_REVIEW_STATUS,
    entries_b::AUTOMATED_CANARY_STATUS,
    entries_b::STACKED_DIFFS_STATUS,
    entries_b::MICROBENCH_STATUS,
    entries_b::SHAPE_STATUS,
    entries_c::REPLAY_HARNESS_STATUS,
    entries_c::CONSISTENCY_STATUS,
    entries_c::JITTERED_BACKOFF_STATUS,
    entries_c::HERMETIC_BUILD_STATUS,
    entries_c::AUTO_ROLLBACK_STATUS,
    entries_c::CARBON_COMPUTE_STATUS,
    entries_c::OPENVEX_STATUS,
    entries_c::FINOPS_STATUS,
    entries_c::SANDBOX_STATUS,
    entries_c::FLAKE_QUARANTINE_STATUS,
    entries_c::PREDICTIVE_TEST_STATUS,
    entries_c::COSIGN_STATUS,
    entries_c::DEBT_SHRINK_STATUS,
    entries_c::GITOPS_DRIFT_STATUS,
    entries_c::MIGRATION_ORCH_STATUS,
    entries_c::GHOST_MIGRATION_STATUS,
    entries_d::TEST_SUITE_STATUS,
    entries_d::RUST_SKILLS_STATUS,
    entries_d::ATTESTATION_STATUS,
    entries_d::CEDAR_STATUS,
    entries_d::SCHEMA_EVOLUTION_STATUS,
    entries_d::ZERO_DAY_STATUS,
    entries_d::FEATURE_FLAG_STATUS,
    entries_e::LOCAL_PROBE_STATUS,
    entries_e::CHAOS_INJECTION_STATUS,
    entries_e::ADR_STATUS,
    entries_e::COMPLIANCE_STATUS,
    entries_e::CROSS_SERVICE_STATUS,
    entries_e::SECURITY_SCAN_STATUS,
    entries_e::CANARY_STATUS,
    entries_f::SHUFFLE_STATUS,
    entries_f::PROGRESSIVE_RING_STATUS,
    entries_g::API_CONTRACT_STATUS,
    entries_g::BENCH_STATUS,
    entries_g::BRAND_ABSENCE_STATUS,
    entries_g::CELL_ISOLATION_STATUS,
    entries_g::CLEAN_ARCH_STATUS,
    entries_g::COMPILE_PROFILE_STATUS,
    entries_h::CONSTANT_WORK_STATUS,
    entries_h::EPHEMERAL_SECRET_STATUS,
    entries_h::GITOPS_PROMO_STATUS,
    entries_h::IDEMPOTENCY_STATUS,
    entries_h::MIGRATION_BOUNDARY_STATUS,
    entries_h::MODULARIZATION_STATUS,
    entries_i::MONOREPO_STATUS,
    entries_i::PERFORMANCE_CONCURRENCY_STATUS,
    entries_i::PSA_STATUS,
    entries_i::REVIEW_VERDICT_STATUS,
    entries_i::RUNNER_ECONOMICS_STATUS,
    entries_i::SCHEMA_COMPAT_STATUS,
];

/// Gate ids whose implementation has NOT been read.
///
/// Empty by construction: anything absent from `AUDITED_GATES` is unaudited.
/// This function exists so the count is derived rather than maintained by hand.
pub fn unaudited_count(total_gates: usize) -> usize {
    total_gates.saturating_sub(AUDITED_GATES.len())
}
