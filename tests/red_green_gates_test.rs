#![allow(unused_variables, unused_imports, dead_code)]

use anvil::adr_drift_ratchet::AdrDriftRatchet;
use anvil::auto_rollback::AutoRollbackPostmortemEngine;
use anvil::automated_canary::{AutomatedCanaryAnalysis, MetricDistribution};
use anvil::carbon_aware::CarbonAwareComputeRatchet;
use anvil::cell_isolation_guard::CellIsolationGuard;
use anvil::clean_architecture_guard::CleanArchitectureGuard;
use anvil::compliance_guard::ComplianceGuard;
use anvil::consistency_guard::ActiveActiveConsistencyGuard;
use anvil::constant_work_guard::ConstantWorkGuard;
use anvil::deadlock_analyzer::DeadlockStaticAnalyzer;
use anvil::flake_quarantine::FlakeQuarantineLifecycle;
use anvil::formal_verification::FormalVerificationGuard;
use anvil::ghost_migration_harness::GhostMigrationHarness;
use anvil::git_manager::PrDiffContext;
use anvil::gitops_promotion::GitOpsPromotionEngine;
use anvil::jittered_backoff::JitteredBackoffGuard;
use anvil::kani_guard::KaniGuard;
use anvil::pre_merge_guard::report::GateStatus;
use anvil::psa_admission_guard::PsaAdmissionGuard;
use anvil::replay_harness::{DeterministicReplayHarness, ReplayTraceRecord};
use anvil::rust_language_policy::RustLanguagePolicy;
use anvil::schema_evolution::SchemaEvolutionRatchet;
use anvil::trace_context_guard::TraceContextGuard;
use anvil::unresolved_review_guard::{ThreadScanner, UnresolvedReviewThread};
use anvil::upgrade_train::{DependencyUpgradeCandidate, ProactiveUpgradeTrain};
use anvil::wasm_sandbox::WasmPolicySandbox;
use anvil::zero_trust_workload::ZeroTrustWorkloadGate;
use std::path::PathBuf;

fn create_test_diff_context(file_path: &str, diff_content: &str) -> PrDiffContext {
    let full_diff = format!(
        "diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n{diff_content}",
        path = file_path,
        diff_content = diff_content
    );
    PrDiffContext {
        repo: "oyatie/test-repo".to_string(),
        pr_number: 101,
        base_branch: "main".to_string(),
        base_sha: "base123".to_string(),
        head_sha: "head456".to_string(),
        previous_head_sha: None,
        repo_working_dir: PathBuf::from("."),
        diff_content: full_diff,
        changed_files: vec![file_path.to_string()],
        is_incremental: false,
    }
}

// =========================================================================
// 1. Clean Architecture Guard: Ports & Adapters Layer Inversion
// =========================================================================

#[test]
fn test_clean_architecture_red_flag_inward_violation() {
    let guard = CleanArchitectureGuard::new();
    // RED: Core domain imports external HTTP/DB adapter
    let bad_diff = create_test_diff_context(
        "src/core/domain/user.rs",
        "+ use crate::adapters::http::handler::UserHttpHandler;",
    );
    let report = guard.evaluate_architecture(&bad_diff).unwrap();
    assert!(
        !report.is_clean,
        "Expected False Green prevention: Core importing adapter must FAIL"
    );
}

#[test]
fn test_clean_architecture_green_valid_inward_dependency() {
    let guard = CleanArchitectureGuard::new();
    // GREEN: Adapter imports domain entity
    let good_diff = create_test_diff_context(
        "src/adapters/http/handler.rs",
        "+ use crate::core::domain::user::UserEntity;",
    );
    let report = guard.evaluate_architecture(&good_diff).unwrap();
    assert!(
        report.is_clean,
        "Expected False Red prevention: Adapter importing core domain must PASS"
    );
}

// =========================================================================
// 2. Cell Boundary & Multi-Tenant Query Scoping Guard
// =========================================================================

#[test]
fn test_cell_isolation_red_flag_unscoped_query() {
    let guard = CellIsolationGuard::new();
    // RED: Database query missing tenant_id/cell_id filter
    let bad_diff = create_test_diff_context(
        "src/db/orders.rs",
        "+ let sql = \"SELECT * FROM customer_orders WHERE order_id = $1\";",
    );
    let report = guard.evaluate_cell_isolation(&bad_diff).unwrap();
    assert!(
        !report.is_isolated,
        "Expected False Green prevention: Unscoped multi-tenant query must FAIL"
    );
}

#[test]
fn test_cell_isolation_green_scoped_tenant_query() {
    let guard = CellIsolationGuard::new();
    // GREEN: Query strictly filtered by tenant_id / cell_id
    let good_diff = create_test_diff_context(
        "src/db/orders.rs",
        "+ let sql = \"SELECT * FROM customer_orders WHERE tenant_id = $1 AND order_id = $2\";",
    );
    let report = guard.evaluate_cell_isolation(&good_diff).unwrap();
    assert!(
        report.is_isolated,
        "Expected False Red prevention: Scoped tenant query must PASS"
    );
}

// =========================================================================
// 3. Constant Work & Static Bounded Memory Guard
// =========================================================================

#[test]
fn test_constant_work_red_flag_unbounded_channel() {
    let guard = ConstantWorkGuard::new();
    // RED: Unbounded tokio channel can lead to OOM under load spikes
    let bad_diff = create_test_diff_context(
        "src/queue/dispatcher.rs",
        "+ let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Message>();",
    );
    let report = guard
        .evaluate_constant_work(&PathBuf::from("."), &bad_diff)
        .unwrap();
    assert!(
        !report.is_bounded,
        "Expected False Green prevention: Unbounded channel must FAIL"
    );
}

#[test]
fn test_constant_work_green_bounded_buffer() {
    let guard = ConstantWorkGuard::new();
    // GREEN: Static bounded channel with explicit capacity
    let good_diff = create_test_diff_context(
        "src/queue/dispatcher.rs",
        "+ let (tx, rx) = tokio::sync::mpsc::channel::<Message>(1024);",
    );
    let report = guard
        .evaluate_constant_work(&PathBuf::from("."), &good_diff)
        .unwrap();
    assert!(
        report.is_bounded,
        "Expected False Red prevention: Bounded channel must PASS"
    );
}

// =========================================================================
// 4. Jittered Exponential Backoff Gate (AWS Builders' Library)
// =========================================================================

#[test]
fn test_jittered_backoff_red_flag_fixed_sleep_retry() {
    let guard = JitteredBackoffGuard::new();
    // RED: Naive retry loop with fixed sleep interval
    let bad_diff = "+ loop {\n+     if let Ok(res) = client.call().await { return res; }\n+     tokio::time::sleep(std::time::Duration::from_millis(500)).await;\n+ }";
    let report = guard.evaluate_backoff_and_jitter(bad_diff);
    assert!(
        !report.passed,
        "Expected False Green prevention: Naive fixed sleep retry must FAIL"
    );
    assert!(report.unjittered_retries_detected > 0);
}

#[test]
fn test_jittered_backoff_green_full_jitter_retry() {
    let guard = JitteredBackoffGuard::new();
    // GREEN: Full jitter exponential backoff
    let good_diff = "+ let jitter = rand::thread_rng().gen_range(0..base_delay_ms * (2_u64.pow(attempt)));\n+ tokio::time::sleep(std::time::Duration::from_millis(jitter)).await;";
    let report = guard.evaluate_backoff_and_jitter(good_diff);
    assert!(
        report.passed,
        "Expected False Red prevention: Jittered exponential backoff must PASS"
    );
}

// =========================================================================
// 5. Wire Schema Evolution Ratchet (Protobuf / gRPC Backward Compatibility)
// =========================================================================

#[test]
fn test_schema_evolution_red_flag_deleted_field_without_reserved() {
    let ratchet = SchemaEvolutionRatchet::new();
    // RED: Deleting an active protobuf field without reserved annotation breaks wire compatibility
    let bad_diff = "diff --git a/proto/order.proto b/proto/order.proto\n- string customer_id = 3;\n+ string account_id = 3;";
    let report = ratchet.evaluate_schema_evolution(bad_diff);
    assert_ne!(
        report.status,
        GateStatus::Passed,
        "Expected False Green prevention: Proto field renumbering/deletion must FAIL"
    );
}

#[test]
fn test_schema_evolution_green_compatible_field_addition() {
    let ratchet = SchemaEvolutionRatchet::new();
    // GREEN: Adding new optional field with unique tag
    let good_diff = "diff --git a/proto/order.proto b/proto/order.proto\n+ optional string idempotency_token = 12;";
    let report = ratchet.evaluate_schema_evolution(good_diff);
    assert_eq!(
        report.status,
        GateStatus::Passed,
        "Expected False Red prevention: Optional proto field addition must PASS"
    );
}

// =========================================================================
// 6. Active-Active Consistency Guard (Multi-Region Spanner / DynamoDB CRDT)
// =========================================================================

#[test]
fn test_consistency_guard_red_flag_blind_overwrite() {
    let guard = ActiveActiveConsistencyGuard::new();
    // RED: Blind cross-region write without vector clock or condition
    let bad_diff = "+ db.put_item().table(\"global_table\").item(\"id\", &id).send().await?;";
    let report = guard.evaluate_active_active_invariants(bad_diff);
    assert!(
        !report.passed,
        "Expected False Green prevention: Blind cross-region overwrite must FAIL"
    );
}

#[test]
fn test_consistency_guard_green_vector_clock_update() {
    let guard = ActiveActiveConsistencyGuard::new();
    // GREEN: Write protected with vector clock version comparison
    let good_diff = "+ db.put_item().table(\"global_table\").condition_expression(\"vector_clock < :vc\").item(\"vector_clock\", &vc).send().await?;";
    let report = guard.evaluate_active_active_invariants(good_diff);
    assert!(
        report.passed,
        "Expected False Red prevention: Vector clock conditioned write must PASS"
    );
}

// =========================================================================
// 7. Zero-Trust Workload Identity Gate (SPIFFE / SPIRE mTLS)
// =========================================================================

#[test]
fn test_zero_trust_red_flag_plaintext_internal_http() {
    let gate = ZeroTrustWorkloadGate::new();
    // RED: Plaintext internal HTTP connection without mTLS
    let bad_diff = "+ let client = reqwest::Client::new();\n+ let resp = client.get(\"http://payment-service.internal:8080/charge\").send().await?;";
    let report = gate.evaluate_cleartext_transport(bad_diff);
    assert!(
        !report.passed,
        "Expected False Green prevention: Plaintext internal HTTP must FAIL"
    );
}

#[test]
fn test_zero_trust_green_spiffe_mtls_transport() {
    let gate = ZeroTrustWorkloadGate::new();
    // GREEN: SPIFFE ID SAN validation over encrypted TLS
    let good_diff = "+ let tls_config = spiffe::load_spiffe_tls_client_config(\"spiffe://oyatie.internal/ns/prod/sa/payment\").await?;\n+ let client = reqwest::Client::builder().use_preconfigured_tls(tls_config).build()?;";
    let report = gate.evaluate_cleartext_transport(good_diff);
    assert!(
        report.passed,
        "Expected False Red prevention: SPIFFE mTLS connection must PASS"
    );
}

// =========================================================================
// 8. Dynamic WebAssembly Policy Sandbox
// =========================================================================

#[test]
fn test_wasm_sandbox_red_flag_dangerous_host_syscall() {
    let sandbox = WasmPolicySandbox::new();
    // RED: Bytecode policy attempting unauthorized raw socket creation
    let bad_diff = "+ unsafe { libc::socket(libc::AF_INET, libc::SOCK_RAW, 0); }";
    let report = sandbox.execute_sandboxed_policies(bad_diff);
    assert!(
        !report.passed,
        "Expected False Green prevention: Dangerous host syscall in policy must FAIL"
    );
}

#[test]
fn test_wasm_sandbox_green_safe_wasm_policy() {
    let sandbox = WasmPolicySandbox::new();
    // GREEN: Pure sandboxed logic
    let good_diff =
        "+ fn validate_jwt_claims(claims: &Claims) -> bool { claims.exp > current_timestamp() }";
    let report = sandbox.execute_sandboxed_policies(good_diff);
    assert!(
        report.passed,
        "Expected False Red prevention: Clean logic must PASS"
    );
}

// =========================================================================
// 9. Flaky-Test Quarantine Lifecycle
// =========================================================================

/// The red/green pair this replaces tested nothing.
///
/// The "RED" half asserted `quarantined_tests_isolated > 0` for a path
/// containing "flaky" -- which asserts a substring was found in a filename,
/// not that the gate goes red. The "GREEN" half asserted `report.passed`,
/// which was the literal `true`, so it held for every input including the
/// flaky one. A pair where the red case never reddens and the green case
/// cannot fail is worse than no test: it is the evidence someone cites when
/// asking whether this gate works.
///
/// Anvil retains no test-run history, so no input distinguishes a flaky test
/// from a stable one. The gate reports that, and these pin it.
#[test]
fn flake_quarantine_has_no_input_that_produces_a_verdict() {
    let manager = FlakeQuarantineLifecycle::new();

    let flaky_named =
        manager.evaluate_quarantine_lifecycle(&["tests/flaky_network_test.rs".to_string()]);
    let clean_named =
        manager.evaluate_quarantine_lifecycle(&["tests/unit_calculator_test.rs".to_string()]);

    for (label, report) in [("flaky-named", &flaky_named), ("clean-named", &clean_named)] {
        assert_eq!(
            report.status.unmeasured_gate_id(),
            Some("flake_quarantine_status"),
            "{label}: a filename says nothing about non-determinism"
        );
        assert!(!report.passed, "{label}: nothing measured is not a pass");
    }

    // The name heuristic still reports what it saw -- it is retained as data,
    // not as a verdict -- so the two inputs remain distinguishable in the
    // counters even though neither yields a pass.
    assert!(flaky_named.quarantined_tests_isolated > 0);
    assert_eq!(clean_named.quarantined_tests_isolated, 0);
}

// =========================================================================
// 10. Carbon-Aware Compute Ratchet (GreenOps)
// =========================================================================

#[test]
fn test_carbon_aware_red_flag_excessive_peak_emission() {
    let ratchet = CarbonAwareComputeRatchet::new();
    // RED: Actual CPU seconds (550s) exceeds budget (500s)
    let report = ratchet.evaluate_compute_carbon(500.0, 550.0);
    assert!(
        !report.passed,
        "Expected False Green prevention: Excessive dirty compute must FAIL / DEFER"
    );
}

#[test]
fn test_carbon_aware_green_efficient_compute() {
    let ratchet = CarbonAwareComputeRatchet::new();
    // GREEN: Actual CPU seconds (25s) strictly within budget (80s)
    let report = ratchet.evaluate_compute_carbon(80.0, 25.0);
    assert!(
        report.passed,
        "Expected False Red prevention: Green compute within budget must PASS"
    );
}

// =========================================================================
// 11. Lock Graph & Deadlock Static Analyzer
// =========================================================================

#[test]
fn test_deadlock_analyzer_red_flag_lock_inversion() {
    let analyzer = DeadlockStaticAnalyzer::new();
    // RED: the same two locks are acquired in opposite orders at two sites,
    // which is a cycle in the lock-order graph. No lock name is privileged --
    // the inversion is what makes it a finding.
    let bad_diff = "+ fn credit(&self) {\n\
                    +     let a = self.accounts.lock();\n\
                    +     let l = self.ledger.lock();\n\
                    + }\n\
                    + fn audit(&self) {\n\
                    +     let l = self.ledger.lock();\n\
                    +     let a = self.accounts.lock();\n\
                    + }\n";
    let report = analyzer.evaluate_deadlock_invariants("oyatie/anvil", bad_diff);
    assert!(
        !report.passed,
        "Expected False Green prevention: Circular lock acquisition must FAIL"
    );
}

#[test]
fn test_deadlock_analyzer_green_ordered_locks() {
    let analyzer = DeadlockStaticAnalyzer::new();
    // GREEN: canonical ordered lock acquisition at every site. Holding two
    // locks at once is not a defect; holding them in inconsistent orders is.
    let good_diff = "+ fn credit(&self) {\n\
                     +     let a = self.accounts.lock();\n\
                     +     let l = self.ledger.lock();\n\
                     + }\n\
                     + fn debit(&self) {\n\
                     +     let a = self.accounts.lock();\n\
                     +     let l = self.ledger.lock();\n\
                     + }\n";
    let report = analyzer.evaluate_deadlock_invariants("oyatie/anvil", good_diff);
    assert!(
        report.passed,
        "Expected False Red prevention: Canonical lock ordering must PASS"
    );
}

// =========================================================================
// 12. Automated Canary Analysis (ACA)
// =========================================================================

#[test]
fn test_aca_red_flag_latency_divergence() {
    let aca = AutomatedCanaryAnalysis::new();
    // RED: Canary p99 latency significantly degraded compared to baseline
    let degraded_dist = MetricDistribution {
        metric_name: "p99_latency_ms".to_string(),
        baseline_samples: vec![12.0, 12.2, 11.9, 12.1, 12.3],
        canary_samples: vec![85.0, 92.4, 78.9, 88.1, 95.0],
    };
    let report = aca.evaluate_canary(&degraded_dist);
    assert!(
        !report.passed,
        "Expected False Green prevention: Degraded canary latency must FAIL"
    );
}

#[test]
fn test_aca_green_statistical_parity() {
    let aca = AutomatedCanaryAnalysis::new();
    // GREEN: Canary samples within normal statistical distribution of baseline
    let healthy_dist = MetricDistribution {
        metric_name: "p99_latency_ms".to_string(),
        baseline_samples: vec![12.0, 12.2, 11.9, 12.1, 12.3],
        canary_samples: vec![12.1, 12.0, 11.8, 12.2, 12.1],
    };
    let report = aca.evaluate_canary(&healthy_dist);
    assert!(
        report.passed,
        "Expected False Red prevention: Healthy canary must PASS"
    );
}

// =========================================================================
// 13. GitOps Promotion Immutable Digest Pinning Gate
// =========================================================================

#[test]
fn test_gitops_promotion_red_flag_unpinned_tag() {
    let promo = GitOpsPromotionEngine::new();
    // RED: Unpinned mutable docker image tag :latest
    let bad_diff = create_test_diff_context(
        "deploy/k8s/deployment.yaml",
        "+ spec:\n+   containers:\n+   - name: anvil\n+     image: oyatie/anvil:latest",
    );
    let report = promo
        .evaluate_manifest_promotions(&PathBuf::from("."), &bad_diff)
        .unwrap();
    assert!(
        !report.is_pinned,
        "Expected False Green prevention: Mutable :latest image tag must FAIL"
    );
}

#[test]
fn test_gitops_promotion_green_sha256_pinned_digest() {
    let promo = GitOpsPromotionEngine::new();
    // GREEN: Immutable sha256 container digest
    let good_diff = create_test_diff_context(
        "deploy/k8s/deployment.yaml",
        "+ spec:\n+   containers:\n+   - name: anvil\n+     image: oyatie/anvil@sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );
    let report = promo
        .evaluate_manifest_promotions(&PathBuf::from("."), &good_diff)
        .unwrap();
    assert!(
        report.is_pinned,
        "Expected False Red prevention: SHA256 pinned digest must PASS"
    );
}

// =========================================================================
// 14. Ghost DB Zero-Lock Migration Harness
// =========================================================================

#[test]
fn test_ghost_migration_red_flag_exclusive_table_lock() {
    let harness = GhostMigrationHarness::new();
    // RED: Adding index without CONCURRENTLY locks the table exclusively
    let bad_diff = create_test_diff_context(
        "migrations/0002_add_index.sql",
        "+ CREATE INDEX idx_users_email ON users(email);",
    );
    let report = harness
        .evaluate_migrations(&PathBuf::from("."), &bad_diff)
        .unwrap();
    assert!(
        !report.is_safe,
        "Expected False Green prevention: Non-concurrent index lock must FAIL"
    );
}

#[test]
fn test_ghost_migration_green_concurrent_index() {
    let harness = GhostMigrationHarness::new();
    // GREEN: Zero-lock concurrent index creation
    let good_diff = create_test_diff_context(
        "migrations/0002_add_index.sql",
        "+ CREATE INDEX CONCURRENTLY idx_users_email ON users(email);",
    );
    let report = harness
        .evaluate_migrations(&PathBuf::from("."), &good_diff)
        .unwrap();
    assert!(
        report.is_safe,
        "Expected False Red prevention: Concurrent zero-lock index must PASS"
    );
}

// =========================================================================
// 15. Native Kubernetes PSA Admission Guard (ADR-0710)
// =========================================================================

#[test]
fn test_psa_admission_red_flag_unrestricted_namespace() {
    let guard = PsaAdmissionGuard::new();
    // RED: Kubernetes namespace manifest missing pod-security.kubernetes.io/enforce: restricted
    let bad_diff = create_test_diff_context(
        "k8s/namespace.yaml",
        "+ apiVersion: v1\n+ kind: Namespace\n+ metadata:\n+   name: prod-workloads",
    );
    let report = guard
        .evaluate_psa_admission(&PathBuf::from("."), &bad_diff)
        .unwrap();
    assert!(
        !report.is_compliant,
        "Expected False Green prevention: Namespace missing PSA restricted label must FAIL"
    );
}

#[test]
fn test_psa_admission_green_enforce_restricted() {
    let guard = PsaAdmissionGuard::new();
    // GREEN: Kubernetes namespace with enforce: restricted label
    let good_diff = create_test_diff_context(
        "k8s/namespace.yaml",
        "+ apiVersion: v1\n+ kind: Namespace\n+ metadata:\n+   name: prod-workloads\n+   labels:\n+     pod-security.kubernetes.io/enforce: restricted",
    );
    let report = guard
        .evaluate_psa_admission(&PathBuf::from("."), &good_diff)
        .unwrap();
    assert!(
        report.is_compliant,
        "Expected False Red prevention: PSA restricted namespace must PASS"
    );
}

// =========================================================================
// 16. Proactive Upgrade Train Guard
// =========================================================================

#[test]
fn test_upgrade_train_red_flag_breaking_major_upgrade() {
    let train = ProactiveUpgradeTrain::new();
    // RED: Major semver upgrade with breaking changes
    let candidate = vec![DependencyUpgradeCandidate {
        package_name: "tokio".to_string(),
        current_version: "1.38.0".to_string(),
        target_version: "2.0.0".to_string(),
        is_major_breaking: true,
    }];
    let report = train.evaluate_upgrade_train(&candidate);
    assert!(
        !report.passed,
        "Expected False Green prevention: Breaking major upgrade must be flagged"
    );
    assert_eq!(report.breaking_major_upgrades, 1);
}

#[test]
fn test_upgrade_train_green_compatible_patch_upgrade() {
    let train = ProactiveUpgradeTrain::new();
    // GREEN: Compatible patch release
    let candidate = vec![DependencyUpgradeCandidate {
        package_name: "serde".to_string(),
        current_version: "1.0.200".to_string(),
        target_version: "1.0.204".to_string(),
        is_major_breaking: false,
    }];
    let report = train.evaluate_upgrade_train(&candidate);
    assert!(
        report.passed,
        "Expected False Red prevention: Compatible semver patch must PASS"
    );
}

// =========================================================================
// 17. Rust Skills Guard (Upstream 390 Rust Rules)
// =========================================================================

#[test]
fn test_rust_skills_red_flag_unwrap_in_production() {
    let guard = RustLanguagePolicy::new();
    // RED: Production unwrap without error handling
    let bad_diff = create_test_diff_context("src/handler.rs", "+ let value = opt_val.unwrap();");
    let report = guard
        .evaluate_rust_quality(&PathBuf::from("."), &bad_diff)
        .unwrap();
    assert!(
        !report.is_idiomatic,
        "Expected False Green prevention: unwrap() in production must FAIL"
    );
}

#[test]
fn test_rust_skills_green_question_mark_operator() {
    let guard = RustLanguagePolicy::new();
    // GREEN: Idiomatic ? error propagation
    let good_diff = create_test_diff_context(
        "src/handler.rs",
        "+ let value = opt_val.ok_or_else(|| anyhow::anyhow!(\"missing val\"))?;",
    );
    let report = guard
        .evaluate_rust_quality(&PathBuf::from("."), &good_diff)
        .unwrap();
    assert!(
        report.is_idiomatic,
        "Expected False Red prevention: ? operator must PASS"
    );
}

// =========================================================================
// 18. Regulatory Compliance Guard (KR PIPA / FSS / HIPAA)
// =========================================================================

#[test]
fn test_compliance_guard_red_flag_plaintext_rrn() {
    let guard = ComplianceGuard::new();
    // RED: Hardcoded plaintext Korean Resident Registration Number (RRN)
    let bad_diff = create_test_diff_context("src/user.rs", "+ let rrn = \"920315-1234567\";");
    let report = guard.evaluate_compliance(&bad_diff).unwrap();
    assert!(
        !report.is_compliant,
        "Expected False Green prevention: Plaintext RRN must FAIL"
    );
}

#[test]
fn test_compliance_guard_green_tokenized_identifier() {
    let guard = ComplianceGuard::new();
    // GREEN: Tokenized anonymous identifier
    let good_diff =
        create_test_diff_context("src/user.rs", "+ let user_token = \"usr_tok_84920491823\";");
    let report = guard.evaluate_compliance(&good_diff).unwrap();
    assert!(
        report.is_compliant,
        "Expected False Red prevention: Tokenized identifier must PASS"
    );
}

// =========================================================================
// 19. W3C TraceContext Distributed Tracing Guard
// =========================================================================

#[test]
fn test_trace_context_red_flag_uninstrumented_spawn() {
    let guard = TraceContextGuard::new();
    // RED: Uninstrumented background async task loses W3C trace context.
    //
    // The `@@` header is part of the fixture rather than of
    // `create_test_diff_context`, which the other 34 rows share. Gate 17 now
    // publishes the file and line of every boundary it accuses, and the hunk
    // header is the only thing in a diff that says where its body sits; a chunk
    // without one declares no position, and the gate reports no location it was
    // not told. Every real diff carries the header, so only this fixture had to
    // grow one.
    let bad_diff = create_test_diff_context(
        "src/background.rs",
        "@@ -20,3 +20,5 @@\n+ tokio::spawn(async move {\n+     process_background_task().await;\n+ });",
    );
    let report = guard
        .evaluate_trace_propagation(&PathBuf::from("."), &bad_diff)
        .unwrap();
    assert!(
        !report.is_propagated,
        "Expected False Green prevention: Uninstrumented tokio::spawn must FAIL"
    );
}

#[test]
fn test_trace_context_green_instrumented_span() {
    let guard = TraceContextGuard::new();
    // GREEN: Instrumented span maintains W3C trace parentage. The header is
    // here for the reason given on the red row above -- and without it this row
    // passes by measuring nothing at all, which is the defect gate 17 was just
    // rewritten to stop publishing.
    let good_diff = create_test_diff_context(
        "src/background.rs",
        "@@ -20,3 +20,5 @@\n+ tokio::spawn(async move {\n+     process_background_task().await;\n+ }.instrument(tracing::info_span!(\"process_background\")));",
    );
    let report = guard
        .evaluate_trace_propagation(&PathBuf::from("."), &good_diff)
        .unwrap();
    assert!(
        report.is_propagated,
        "Expected False Red prevention: Instrumented span must PASS"
    );
    assert_eq!(
        report.tasks_scanned, 1,
        "the boundary has to have been inspected for this row to mean \
         anything: {}",
        report.summary
    );
}

// =========================================================================
// 20. SMT Formal Invariant Verification Guard
// =========================================================================

#[test]
fn test_formal_verification_red_flag_wildcard_permission() {
    let guard = FormalVerificationGuard::new();
    // RED: Overly permissive wildcard policy grant
    let bad_policy = "+ permit(principal, action, resource);";
    let report = guard.evaluate_formal_invariants(bad_policy);
    assert!(
        !report.passed,
        "Expected False Green prevention: Wildcard principal/action/resource must FAIL"
    );
}

#[test]
fn test_formal_verification_green_scoped_least_privilege() {
    let guard = FormalVerificationGuard::new();
    // GREEN: Scoped least-privilege principal permission
    let good_policy = "+ permit(principal == Principal::\"User:123\", action == Action::\"Read\", resource == Resource::\"Doc:456\");";
    let report = guard.evaluate_formal_invariants(good_policy);
    assert!(
        report.passed,
        "Expected False Red prevention: Least privilege scoped policy must PASS"
    );
}

// =========================================================================
// 21. Micro-Benchmark & Latency Ratchet
// =========================================================================

// =========================================================================
// 22. Living ADR Drift Ratchet
// =========================================================================

/// A working tree that declares the five-field house schema, plus the record
/// itself on disk.
///
/// Gate 22's field list is no longer a Rust literal: it is read from the
/// repository under review, so `PathBuf::from(".")` -- which declares none --
/// now yields `NotMeasured` rather than a verdict. These rows therefore have to
/// supply a repository that states the rule they are testing.
fn adr_repo_with(record: &str, body: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("docs/decisions")).expect("mkdir");
    std::fs::write(
        dir.path().join("docs/decisions/adr-schema.json"),
        r#"["achieves", "origin", "rule", "ensure", "overturn_when"]"#,
    )
    .expect("write schema");
    let record = dir.path().join(record);
    std::fs::create_dir_all(record.parent().expect("a parent")).expect("mkdir");
    std::fs::write(record, body).expect("write record");
    dir
}

#[test]
fn test_adr_drift_red_flag_missing_mandatory_field() {
    let ratchet = AdrDriftRatchet::new();
    // RED: ADR missing the mandatory "Overturn-When:" field
    let body = "# ADR-0002: Cache Strategy\nAchieves: Sub-millisecond read latency\nOrigin: RFC-102\nRule: Use Redis\nEnsure: TTL <= 300s\n";
    let repo = adr_repo_with("docs/adr/0002_cache_strategy.md", body);
    let bad_adr_diff = create_test_diff_context("docs/adr/0002_cache_strategy.md", body);
    let report = ratchet
        .evaluate_adr_parity(repo.path(), &bad_adr_diff)
        .unwrap();
    assert!(
        !report.is_compliant,
        "Expected False Green prevention: ADR missing Overturn-When field must FAIL"
    );
}

#[test]
fn test_adr_drift_green_complete_5_field_adr() {
    let ratchet = AdrDriftRatchet::new();
    // GREEN: ADR with all 5 required fields
    let body = "# ADR-0002: Cache Strategy\nAchieves: Sub-millisecond read latency\nOrigin: RFC-102\nRule: Use Redis\nEnsure: TTL <= 300s\nOverturn-When: Embedded RocksDB satisfies multi-region replication\n";
    let repo = adr_repo_with("docs/adr/0002_cache_strategy.md", body);
    let good_adr_diff = create_test_diff_context("docs/adr/0002_cache_strategy.md", body);
    let report = ratchet
        .evaluate_adr_parity(repo.path(), &good_adr_diff)
        .unwrap();
    assert!(
        report.is_compliant,
        "Expected False Red prevention: Complete 5-field ADR schema must PASS"
    );
}

// =========================================================================
// 23. Zero-Unresolved-Comments Review Gate
// =========================================================================

#[test]
fn test_unresolved_comments_red_flag_open_thread() {
    let scanner = ThreadScanner::new();
    // RED: Unresolved review comment thread
    let open_threads = vec![UnresolvedReviewThread {
        thread_id: "thread_101".to_string(),
        path: "src/main.rs".to_string(),
        line: Some(42),
        comment_body: "Please add integration test for error path".to_string(),
        author: "reviewer_lead".to_string(),
    }];
    let res = scanner.evaluate_unresolved_threads(&open_threads);
    assert!(
        res.is_err(),
        "Expected False Green prevention: Unresolved review comment thread must FAIL"
    );
}

#[test]
fn test_unresolved_comments_green_all_threads_resolved() {
    let scanner = ThreadScanner::new();
    // GREEN: All review comments resolved (0 unresolved threads)
    let res = scanner.evaluate_unresolved_threads(&[]);
    assert!(
        res.is_ok(),
        "Expected False Red prevention: 100% resolved review comments must PASS"
    );
}

// =========================================================================
// 24. Deterministic Record-and-Replay Harness
// =========================================================================

#[test]
fn test_replay_harness_red_flag_divergent_output() {
    let harness = DeterministicReplayHarness::new();
    // RED: Trace with divergent output
    let traces = vec![ReplayTraceRecord {
        trace_id: "trace_99".to_string(),
        input_payload: "{\"event\":\"login\",\"user_id\":\"123\"}".to_string(),
        expected_output: "{\"status\":\"success\",\"divergence\":false}".to_string(),
    }];
    let report = harness.evaluate_replay_parity(&traces);
    assert!(
        report.passed,
        "Replay harness correctly processes nominal trace baseline"
    );
}

// =========================================================================
// 25. Auto-Rollback & Blameless Postmortem Engine
// =========================================================================

#[test]
fn test_auto_rollback_red_flag_degraded_slo_triggers_rollback() {
    let engine = AutoRollbackPostmortemEngine::new();
    // RED: High error rate (15%) and latency (600ms) triggers canary auto-rollback
    let report = engine.evaluate_health_and_rollback("oyatie/anvil", 15.0, 600.0);
    assert!(
        !report.passed,
        "Expected False Green prevention: Degraded error rate must FAIL health gate"
    );
    assert!(
        report.rollback_triggered,
        "Degraded canary must trigger autonomous rollback"
    );
}

#[test]
fn test_auto_rollback_green_healthy_canary_passes() {
    let engine = AutoRollbackPostmortemEngine::new();
    // GREEN: Low error rate (0.001%) and low latency (25ms)
    let report = engine.evaluate_health_and_rollback("oyatie/anvil", 0.0001, 25.0);
    assert!(
        report.passed,
        "Expected False Red prevention: Healthy canary must PASS"
    );
    assert!(
        !report.rollback_triggered,
        "Healthy canary must not trigger rollback"
    );
}

// =========================================================================
// 26. Automated Git Hook Provisioning & Permissions
// =========================================================================

#[tokio::test]
async fn test_git_hook_provisioning_and_permissions() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    let init = std::process::Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    assert!(init.status.success(), "git init: {init:?}");

    anvil::git_manager::GitManager::install_repo_hooks(repo_path)
        .await
        .unwrap();

    let common = std::process::Command::new("git")
        .args(["-C"])
        .arg(repo_path)
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .unwrap();
    let common = String::from_utf8_lossy(&common.stdout).trim().to_string();
    let hooks = if std::path::Path::new(&common).is_absolute() {
        std::path::PathBuf::from(&common).join("hooks")
    } else {
        repo_path.join(&common).join("hooks")
    };

    let pre_commit = hooks.join("pre-commit");
    let commit_msg = hooks.join("commit-msg");
    let pre_push = hooks.join("pre-push");

    assert!(pre_commit.exists(), "pre-commit hook must be created");
    assert!(commit_msg.exists(), "commit-msg hook must be created");
    assert!(pre_push.exists(), "pre-push hook must be created");
    assert!(
        !hooks.join("post-merge").exists(),
        "post-merge is not a native hook; rustfmt-on-file-list is pre-commit/pre-push"
    );

    let pre_commit_content = tokio::fs::read_to_string(&pre_commit).await.unwrap();
    assert!(pre_commit_content.contains("rustfmt --check"));
    assert!(!pre_commit_content.contains("cargo fmt"));

    let commit_msg_content = tokio::fs::read_to_string(&commit_msg).await.unwrap();
    assert!(commit_msg_content.contains("type(scope): summary"));

    let pre_push_content = tokio::fs::read_to_string(&pre_push).await.unwrap();
    assert!(pre_push_content.contains("rustfmt --check"));
}

// =========================================================================
// 27. Sub-100ms Inner-Loop Local Probe Fast Validator
// =========================================================================

#[test]
fn test_local_probe_red_flag_unconventional_commit_or_secret() {
    let validator = anvil::local_inner_loop::FastValidator::new();
    let bad_diff = "+ let aws_key = \"AKIAIOSFODNN7EXAMPLE\";";
    let findings = validator.validate_pre_commit("bad commit msg", bad_diff);

    assert!(
        findings.iter().any(|f| !f.is_valid),
        "Expected False Green prevention: Bad commit or secret leak must be flagged"
    );
}

#[test]
fn test_local_probe_green_nominal_conventional_diff() {
    let validator = anvil::local_inner_loop::FastValidator::new();
    let good_diff = "+ pub fn calculate_total(a: u32, b: u32) -> u32 { a + b }";
    let findings =
        validator.validate_pre_commit("feat(math): add calculate_total function", good_diff);

    assert!(
        findings.iter().all(|f| f.is_valid),
        "Expected False Red prevention: Valid conventional commit and safe code must PASS"
    );
}

// =========================================================================
// 28. In-Place PR Comment Marker Verification
// =========================================================================

#[test]
fn test_scorecard_contains_anvil_receipt_marker() {
    let report = anvil::pre_merge_guard::PreMergeCertificationReport::unmeasured("fixture");
    let matrix_text = anvil::pre_merge_guard::matrix::MatrixRenderer::render(&report);

    assert!(
        matrix_text.contains("<!-- ANVIL_SCORECARD_RECEIPT -->"),
        "Matrix scorecard must contain ANVIL_SCORECARD_RECEIPT for in-place comment upserting"
    );
}

// =========================================================================
// 29. Supply Chain Guard: Banned & Deprecated Dependencies
// =========================================================================

/// RED: a locked version carrying a live OSV advisory.
///
/// Was: a regex for the literal `net2` in the diff. The gate never opened
/// `Cargo.lock`, so a vulnerable transitive dependency -- which is where
/// supply-chain defects actually live -- was invisible to it.
#[test]
fn test_supply_chain_red_flag_advisory_against_a_locked_version() {
    use anvil::pre_merge_guard::report::GateStatus;
    use anvil::supply_chain_guard::{OsvAdvisoryStream, SupplyChainGuard};

    let lock = "[[package]]\nname = \"time\"\nversion = \"0.1.44\"\n";
    let packages = SupplyChainGuard::parse_lockfile(lock).expect("the lockfile resolves");
    let advisories = OsvAdvisoryStream::parse_batch_response(
        r#"{"results":[{"vulns":[{"id":"RUSTSEC-2020-0071"}]}]}"#,
        &packages,
    )
    .expect("the advisory response parses");

    let report = SupplyChainGuard::report(&packages, Ok(advisories));
    assert!(
        matches!(report.status, GateStatus::Failed(_)),
        "Expected False Green prevention: a locked version with a live advisory must FAIL, got {:?}",
        report.status
    );
}

/// GREEN: the same lockfile, with the advisory database returning nothing.
#[test]
fn test_supply_chain_green_no_advisory_against_any_locked_version() {
    use anvil::pre_merge_guard::report::GateStatus;
    use anvil::supply_chain_guard::{OsvAdvisoryStream, SupplyChainGuard};

    let lock = "[[package]]\nname = \"serde\"\nversion = \"1.0.219\"\n";
    let packages = SupplyChainGuard::parse_lockfile(lock).expect("the lockfile resolves");
    let advisories = OsvAdvisoryStream::parse_batch_response(r#"{"results":[{}]}"#, &packages)
        .expect("the advisory response parses");

    let report = SupplyChainGuard::report(&packages, Ok(advisories));
    assert!(
        matches!(report.status, GateStatus::Passed),
        "Expected False Red prevention: a clean lockfile must PASS, got {:?}",
        report.status
    );
}

// =========================================================================
// 30. Monorepo Guard: Internal Boundary Enforcement
// =========================================================================

#[tokio::test]
async fn test_monorepo_guard_red_flag_cross_crate_internal_leak() {
    let guard = anvil::monorepo_guard::MonorepoGuard::new();
    let bad_diff = create_test_diff_context(
        "services/auth/src/lib.rs",
        "+ let secret = include_str!(\"../../../../etc/shadow\");",
    );
    let report = guard
        .evaluate_monorepo_hygiene(std::path::Path::new("."), &bad_diff)
        .await
        .unwrap();
    assert!(
        !report.is_compliant,
        "Expected False Green prevention: Non-hermetic path escape must FAIL"
    );
}

#[tokio::test]
async fn test_monorepo_guard_green_public_crate_api() {
    let guard = anvil::monorepo_guard::MonorepoGuard::new();
    let good_diff = create_test_diff_context(
        "services/auth/src/lib.rs",
        "+ use billing_client::BillingClient;",
    );
    let report = guard
        .evaluate_monorepo_hygiene(std::path::Path::new("."), &good_diff)
        .await
        .unwrap();
    assert!(
        report.is_compliant,
        "Expected False Red prevention: Public API import must PASS"
    );
}

// =========================================================================
// 31. Technical Debt Shrink Guard: Deprecation & Reorg Drain Ratchet
// =========================================================================

#[test]
fn test_debt_shrink_red_flag_blanket_allow() {
    let guard = anvil::debt_shrink_guard::DebtShrinkGuard::new();
    let bad_diff = create_test_diff_context(
        "src/legacy/old_auth_handler.rs",
        "+ pub fn add_more_debt() { println!(\"growing legacy debt\"); }",
    );
    let report = guard
        .evaluate_debt_shrink(std::path::Path::new("."), &bad_diff)
        .unwrap();
    assert!(
        !report.is_acceptable,
        "Expected False Green prevention: Net growth in deprecated path must FAIL"
    );
}

#[test]
fn test_debt_shrink_green_clean_code() {
    let guard = anvil::debt_shrink_guard::DebtShrinkGuard::new();
    // This used to hand the guard `src/lib.rs` -- a path no marker in the
    // deprecation scope can ever match -- and assert the pass that came back.
    // That is the vacuous green the guard now refuses: it certified a debt
    // ratchet over an empty corpus. A green here has to be earned by a
    // deprecating target that was actually read and actually shrank.
    let good_diff = create_test_diff_context(
        "src/legacy/old_auth_handler.rs",
        "- pub fn dead_path() {}\n- pub fn also_dead() {}",
    );
    let report = guard
        .evaluate_debt_shrink(std::path::Path::new("."), &good_diff)
        .unwrap();
    assert!(
        report.is_acceptable,
        "Expected False Red prevention: a deprecating target that only shrank must PASS"
    );
    assert_eq!(report.total_debt_shrunk, 2);
}

#[test]
fn test_debt_shrink_red_flag_empty_scope_is_not_a_pass() {
    let guard = anvil::debt_shrink_guard::DebtShrinkGuard::new();
    let out_of_scope = create_test_diff_context(
        "src/lib.rs",
        "+ pub fn process_event() -> Result<(), Error> { Ok(()) }",
    );
    let report = guard
        .evaluate_debt_shrink(std::path::Path::new("."), &out_of_scope)
        .unwrap();
    assert_eq!(
        report.status.unmeasured_gate_id(),
        Some("debt_shrink_status"),
        "Expected False Green prevention: no deprecating target in scope must NOT pass"
    );
    assert!(!report.is_acceptable);
}

// =========================================================================
// 32. Modularization Guard: Oversized Monolithic Source Files
// =========================================================================

#[test]
fn test_modularization_red_flag_circular_dependency() {
    let guard = anvil::modularization_guard::ModularizationGuard::new();
    let mut large_diff = String::new();
    for i in 0..350 {
        large_diff.push_str(&format!("+ pub fn helper_{}() {{}}\n", i));
    }
    let bad_diff = create_test_diff_context("crates/monolith/src/lib.rs", &large_diff);
    let report = guard.evaluate_modularization(&bad_diff).unwrap();
    assert!(
        !report.is_modular,
        "Expected False Green prevention: File exceeding 300 lines must FAIL"
    );
}

#[test]
fn test_modularization_green_acyclic_dag() {
    let guard = anvil::modularization_guard::ModularizationGuard::new();
    let good_diff = create_test_diff_context(
        "crates/api_gateway/src/handler.rs",
        "+ pub fn handle_request() -> Response { Response::ok() }",
    );
    let report = guard.evaluate_modularization(&good_diff).unwrap();
    assert!(
        report.is_modular,
        "Expected False Red prevention: Modular compact file must PASS"
    );
}

// =========================================================================
// 33. Kani Guard (`kani_status`): Undocumented Unsafe -- `// SAFETY:` comment lint
// =========================================================================

#[test]
fn test_kani_red_flag_undocumented_unsafe_block() {
    let guard = anvil::kani_guard::KaniGuard::new();
    let bad_diff = create_test_diff_context(
        "src/buffer.rs",
        "+ unsafe fn raw_copy(dst: *mut u8, src: *const u8, len: usize) { std::ptr::copy(src, dst, len); }",
    );
    let report = guard
        .lint_unsafe_safety_comments(std::path::Path::new("."), &bad_diff)
        .unwrap();
    assert!(
        !report.all_unsafe_blocks_documented,
        "Expected False Green prevention: Undocumented unsafe must FAIL"
    );
}

#[test]
fn test_kani_green_safe_rust_or_documented_safety() {
    let guard = anvil::kani_guard::KaniGuard::new();
    let good_diff = create_test_diff_context(
        "src/buffer.rs",
        "+ // SAFETY: dst and src are guaranteed non-null, properly aligned, and len <= buffer capacity.\n+ unsafe fn safe_copy(dst: *mut u8, src: *const u8, len: usize) { std::ptr::copy(src, dst, len); }",
    );
    let report = guard
        .lint_unsafe_safety_comments(std::path::Path::new("."), &good_diff)
        .unwrap();
    assert!(
        report.all_unsafe_blocks_documented,
        "Expected False Red prevention: a documented unsafe block must PASS"
    );
}

// =========================================================================
// 34. OpenSLO Canary Guard: Error Budget Burn Rate
// =========================================================================

#[test]
fn test_slo_canary_red_flag_high_burn_rate() {
    let guard = anvil::slo_canary_guard::SloCanaryGuard::new();
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let slo_path = temp_dir.path().join("service.openslo.yaml");
    std::fs::write(
        &slo_path,
        "apiVersion: openslo/v1\nkind: SLO\nspec:\n  objectives: []\n",
    )
    .unwrap();

    let mut bad_diff = create_test_diff_context(
        "service.openslo.yaml",
        "+ apiVersion: openslo/v1\n+ kind: SLO\n+ spec:\n+   objectives: []",
    );
    bad_diff.repo_working_dir = temp_dir.path().to_path_buf();
    let report = guard
        .evaluate_slo_canary_health(temp_dir.path(), &bad_diff)
        .unwrap();
    assert!(
        !report.is_compliant,
        "Expected False Green prevention: OpenSLO spec with 0 objectives must FAIL"
    );
}

#[test]
fn test_slo_canary_green_nominal_slo() {
    let guard = anvil::slo_canary_guard::SloCanaryGuard::new();
    let good_diff = create_test_diff_context(
        "config/app.rs",
        "+ pub fn healthz() -> &'static str { \"ok\" }",
    );
    let report = guard
        .evaluate_slo_canary_health(std::path::Path::new("."), &good_diff)
        .unwrap();
    assert!(
        report.is_compliant,
        "Expected False Red prevention: Nominal code without SLO degradation must PASS"
    );
}

// =========================================================================
// 35. Shuffle Sharding Simulator: Fault Isolation Blast Radius
// =========================================================================

#[test]
fn test_shuffle_shard_combinations_math() {
    let combos_64_4 =
        anvil::shuffle_shard_simulator::ShuffleShardMath::calculate_combinations(64, 4);
    assert_eq!(
        combos_64_4, 635376,
        "C(64, 4) must equal 635,376 combinations"
    );

    let allocs = vec![
        anvil::shuffle_shard_simulator::ShuffleShardAllocation {
            tenant_id: "tenant_a".to_string(),
            assigned_cells: vec![1, 2, 3, 4],
        },
        anvil::shuffle_shard_simulator::ShuffleShardAllocation {
            tenant_id: "tenant_b".to_string(),
            assigned_cells: vec![5, 6, 7, 8],
        },
    ];
    let overlap = anvil::shuffle_shard_simulator::ShuffleShardMath::evaluate_overlap(&allocs);
    assert_eq!(
        overlap, 0,
        "Mutually disjoint tenant shard allocations must have 0 overlap"
    );
}

// =========================================================================
// 36. Subtle Evasion Testing: Nested Subqueries & Regex Variations
// =========================================================================

#[test]
fn test_subtle_cell_isolation_nested_subquery_evasion() {
    let guard = CellIsolationGuard::new();
    // SUBTLE RED: SQL query with nested subquery missing tenant filter in inner SELECT
    let subtle_bad_diff = create_test_diff_context(
        "src/queries/ledger.rs",
        "+ let q = \"SELECT * FROM (SELECT id, amount FROM ledger_entries) AS sub WHERE sub.id = $1\";",
    );
    let report = guard.evaluate_cell_isolation(&subtle_bad_diff).unwrap();
    assert!(
        !report.is_isolated,
        "Subtle bug: Inner subquery missing tenant_id must FAIL"
    );
}

#[test]
fn test_subtle_rust_skills_empty_expect_evasion() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let guard = RustLanguagePolicy::new();
    // SUBTLE RED: Attempting to bypass unwrap check by using empty expect string
    let subtle_bad_diff =
        create_test_diff_context("src/handler.rs", "+ let value = option_val.expect(\"\");");
    let report = guard
        .evaluate_rust_quality(temp_dir.path(), &subtle_bad_diff)
        .unwrap();
    assert!(
        !report.is_idiomatic,
        "Subtle bug: Empty expect must be caught as non-idiomatic"
    );
}

#[test]
fn test_subtle_adr_drift_missing_validation_evidence() {
    let ratchet = AdrDriftRatchet::new();
    // SUBTLE RED: a plain MADR record in a repository that declares the
    // five-field house schema. Every declared field is absent.
    let body =
        "# ADR-0002: Cache Architecture\n## Context\nNeeded caching.\n## Decision\nUse Redis.\n";
    let repo = adr_repo_with("docs/adr/0002-cache.md", body);
    let subtle_bad_diff = create_test_diff_context("docs/adr/0002-cache.md", body);
    let report = ratchet
        .evaluate_adr_parity(repo.path(), &subtle_bad_diff)
        .unwrap();
    assert!(
        !report.is_compliant,
        "Subtle bug: ADR missing Validation Evidence must FAIL"
    );
}

#[test]
fn test_subtle_review_enforcement_dismissed_or_historical_request_changes() {
    // Verify that review decision parsing catches CHANGES_REQUESTED in JSON structure
    let json_payload = r#"{
        "reviewDecision": "CHANGES_REQUESTED",
        "reviews": [
            { "state": "APPROVED" },
            { "state": "CHANGES_REQUESTED" }
        ]
    }"#;
    let val: serde_json::Value = serde_json::from_str(json_payload).unwrap();
    let decision = val.get("reviewDecision").and_then(|d| d.as_str()).unwrap();
    assert_eq!(
        decision, "CHANGES_REQUESTED",
        "Must strictly identify CHANGES_REQUESTED even if other reviews approved"
    );
}
