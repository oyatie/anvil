//! Which gates have demonstrated they can fire, and which have not.
//!
//! # The obligation
//!
//! A check is written by asserting what it should catch, and it passes from the
//! moment it compiles -- so a green tells you nothing about whether it CAN fail.
//! Four checks written in a single session were green for the exact defect they
//! existed to catch, and every one was found by seeding the defect rather than
//! by reading the code.
//!
//! `harness::Rule::fixture` makes that unspellable for anything on the harness:
//! a rule ships a defect it must flag and a twin it must spare, and the harness
//! runs both before trusting a verdict. The seventy-two hand-wired gates are not
//! on the harness, and nothing obliges them to demonstrate anything.
//!
//! This is that obligation, imposed where the harness cannot reach: each gate
//! names the test that seeds its defect and the test that hands it a conformant
//! subject, and both are checked to exist AND to actually exercise that gate.
//!
//! # Why declared rather than inferred
//!
//! The test names follow a convention -- `test_<gate>_red_flag_…`,
//! `test_<gate>_green_…` -- and inferring the mapping from it was tried and
//! abandoned. Fuzzy matching claimed `psa_status` was proven by an ADR test and
//! `slo_status` by the same one. A registry that cites a test which does not
//! exercise the gate is itself green for a defect it exists to catch, which is
//! the class this module is closing. So `exercises` names a symbol the cited
//! test must actually mention, and that is checked.

/// One gate's demonstration that it can fire and that it discriminates.
#[derive(Debug, Clone, Copy)]
pub struct GateProof {
    /// The gate as the certification report names it.
    pub gate_id: &'static str,
    /// A symbol the cited tests must mention -- the guard's type, usually.
    ///
    /// This is what stops a citation being satisfied by any test with a
    /// plausible name. A test that does not name the thing under test is not a
    /// proof of it.
    pub exercises: &'static str,
    /// The test that seeds this gate's defect and asserts it is found.
    pub fires_on: &'static str,
    /// The test that hands it a conformant subject and asserts it is spared.
    ///
    /// Both halves are required. A gate with only the first cannot be shown to
    /// discriminate; one with only the second has never been seen to work.
    pub spares: &'static str,
}

/// Gates that have demonstrated both halves.
///
/// Grows by hand, deliberately. Every row was read before it was written --
/// see the note above on what inference produced.
pub const GATE_PROOFS: &[GateProof] = &[
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

/// Gates with no demonstration that they can fire. EXACT, and it must fall.
///
/// Not every gate can have one. Twenty-six are declared unprovisionable in
/// `admission::ABSENCE_POLICY` -- no telemetry endpoint, no signing backend, no
/// cluster -- and a gate that cannot measure in this deployment cannot be
/// seeded with a defect either. The obligation applies to the gates that CAN
/// fire, and this counts those still owing it.
pub const GATES_WITHOUT_PROOF: usize = 23;
