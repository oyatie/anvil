//! The proof ledger: one row per gate that has demonstrated both halves.
//!
//! Split from `gate_proof` because the table grows by a row per gate proved and
//! the module crossed the 300-line budget at twenty-seven. The types and the
//! join live next door; this file is data.

use super::GateProof;

/// Gates that have demonstrated both halves.
///
/// Grows by hand, deliberately. Every row is read before it is added --
/// see the note above on what inference produced.
pub const GATE_PROOFS: &[GateProof] = &[
    GateProof {
        gate_id: "ephemeral_secret_status",
        exercises: "EphemeralSecretInjector",
        fires_on: "ephemeral_secret_fires_on_a_static_aws_key_in_a_workflow",
        spares: "ephemeral_secret_spares_a_workflow_that_assumes_a_role_by_oidc",
    },
    GateProof {
        gate_id: "cross_service_status",
        exercises: "CrossServiceImpactEngine",
        fires_on: "cross_service_fires_when_a_required_field_leaves_a_wire_contract",
        spares: "cross_service_spares_a_required_field_being_added",
    },
    GateProof {
        gate_id: "unresolved_review_status",
        exercises: "UnresolvedReviewReport",
        fires_on: "unresolved_review_fires_on_a_thread_github_reports_open",
        spares: "unresolved_review_spares_a_pull_request_with_no_open_threads",
    },
    GateProof {
        gate_id: "idempotency_status",
        exercises: "IdempotencyGuard",
        fires_on: "idempotency_flags_an_added_mutating_route_with_no_key",
        spares: "idempotency_spares_the_same_route_where_the_file_handles_the_key",
    },
    GateProof {
        gate_id: "psa_status",
        exercises: "PsaAdmissionGuard",
        fires_on: "psa_flags_a_namespace_without_an_enforce_label",
        spares: "psa_spares_a_namespace_that_enforces_restricted",
    },
    GateProof {
        gate_id: "cleartext_transport_status",
        exercises: "ZeroTrustWorkloadGate",
        fires_on: "zero_trust_flags_an_added_cleartext_endpoint",
        spares: "zero_trust_spares_the_same_endpoint_over_tls",
    },
    GateProof {
        gate_id: "brand_absence_status",
        exercises: "BrandAbsenceGate",
        fires_on: "brand_absence_flags_an_aspirational_stamp_in_source",
        spares: "brand_absence_spares_source_that_claims_nothing",
    },
    GateProof {
        gate_id: "semantic_abi_status",
        exercises: "SemanticAbiRatchet",
        fires_on: "semantic_abi_flags_a_changed_public_signature",
        spares: "semantic_abi_spares_a_body_change_behind_the_same_signature",
    },
    GateProof {
        gate_id: "cloud_native_status",
        exercises: "CloudNativeGuard",
        fires_on: "cloud_native_flags_a_proprietary_sdk_in_a_core_layer",
        spares: "cloud_native_spares_the_same_sdk_in_an_adapter",
    },
    GateProof {
        gate_id: "stack_whitelist_status",
        exercises: "StackWhitelistGuard",
        fires_on: "stack_whitelist_flags_a_technology_the_approved_list_does_not_name",
        spares: "stack_whitelist_spares_the_approved_stack",
    },
    GateProof {
        gate_id: "cell_isolation_status",
        exercises: "CellIsolationGuard",
        fires_on: "test_cell_isolation_red_flag_unscoped_query",
        spares: "test_cell_isolation_green_scoped_tenant_query",
    },
    GateProof {
        gate_id: "clean_arch_status",
        exercises: "CleanArchitectureGuard",
        fires_on: "test_clean_architecture_red_flag_inward_violation",
        spares: "test_clean_architecture_green_valid_inward_dependency",
    },
    GateProof {
        gate_id: "compliance_status",
        exercises: "ComplianceGuard",
        fires_on: "test_compliance_guard_red_flag_plaintext_rrn",
        spares: "test_compliance_guard_green_tokenized_identifier",
    },
    GateProof {
        gate_id: "consistency_status",
        exercises: "ConsistencyGuard",
        fires_on: "test_consistency_guard_red_flag_blind_overwrite",
        spares: "test_consistency_guard_green_vector_clock_update",
    },
    GateProof {
        gate_id: "constant_work_status",
        exercises: "ConstantWorkGuard",
        fires_on: "test_constant_work_red_flag_unbounded_channel",
        spares: "test_constant_work_green_bounded_buffer",
    },
    GateProof {
        gate_id: "deadlock_status",
        exercises: "DeadlockStaticAnalyzer",
        fires_on: "test_deadlock_analyzer_red_flag_lock_inversion",
        spares: "test_deadlock_analyzer_green_ordered_locks",
    },
    GateProof {
        gate_id: "debt_shrink_status",
        exercises: "DebtShrinkGuard",
        fires_on: "test_debt_shrink_red_flag_blanket_allow",
        spares: "test_debt_shrink_green_clean_code",
    },
    GateProof {
        gate_id: "formal_verification_status",
        exercises: "FormalVerificationGuard",
        fires_on: "test_formal_verification_red_flag_wildcard_permission",
        spares: "test_formal_verification_green_scoped_least_privilege",
    },
    GateProof {
        gate_id: "ghost_migration_status",
        exercises: "GhostMigrationHarness",
        fires_on: "test_ghost_migration_red_flag_exclusive_table_lock",
        spares: "test_ghost_migration_green_concurrent_index",
    },
    GateProof {
        gate_id: "gitops_promo_status",
        exercises: "GitOpsPromotionEngine",
        fires_on: "test_gitops_promotion_red_flag_unpinned_tag",
        spares: "test_gitops_promotion_green_sha256_pinned_digest",
    },
    GateProof {
        gate_id: "jittered_backoff_status",
        exercises: "JitteredBackoffGuard",
        fires_on: "test_jittered_backoff_red_flag_fixed_sleep_retry",
        spares: "test_jittered_backoff_green_full_jitter_retry",
    },
    GateProof {
        gate_id: "kani_status",
        exercises: "KaniGuard",
        fires_on: "test_kani_red_flag_undocumented_unsafe_block",
        spares: "test_kani_green_safe_rust_or_documented_safety",
    },
    GateProof {
        gate_id: "local_probe_status",
        exercises: "FastValidator",
        fires_on: "test_local_probe_red_flag_unconventional_commit_or_secret",
        spares: "test_local_probe_green_nominal_conventional_diff",
    },
    GateProof {
        gate_id: "modularization_status",
        exercises: "ModularizationGuard",
        fires_on: "test_modularization_red_flag_circular_dependency",
        spares: "test_modularization_green_acyclic_dag",
    },
    GateProof {
        gate_id: "rust_skills_status",
        exercises: "RustLanguagePolicy",
        fires_on: "test_rust_skills_red_flag_unwrap_in_production",
        spares: "test_rust_skills_green_question_mark_operator",
    },
    GateProof {
        gate_id: "schema_evolution_status",
        exercises: "SchemaEvolutionRatchet",
        fires_on: "test_schema_evolution_red_flag_deleted_field_without_reserved",
        spares: "test_schema_evolution_green_compatible_field_addition",
    },
    GateProof {
        gate_id: "supply_chain_status",
        exercises: "SupplyChainGuard",
        fires_on: "test_supply_chain_red_flag_advisory_against_a_locked_version",
        spares: "test_supply_chain_green_no_advisory_against_any_locked_version",
    },
    GateProof {
        gate_id: "trace_status",
        exercises: "TraceContextGuard",
        fires_on: "test_trace_context_red_flag_uninstrumented_spawn",
        spares: "test_trace_context_green_instrumented_span",
    },
    GateProof {
        gate_id: "wasm_sandbox_status",
        exercises: "WasmPolicySandbox",
        fires_on: "test_wasm_sandbox_red_flag_dangerous_host_syscall",
        spares: "test_wasm_sandbox_green_safe_wasm_policy",
    },
];
