use anyhow::Result;
use tracing::info;

use super::matrix::MatrixRenderer;
use super::report::{CertifiedSubject, GateProvenance, GateStatus, PreMergeCertificationReport};
use super::scanner::PreMergeScanner;
use crate::adr_drift_ratchet::AdrReport;
use crate::api_contract_guard::ApiContractReport;
use crate::attestation_guard::AttestationReport;
use crate::auto_rollback::AutoRollbackReport;
use crate::automated_canary::AutomatedCanaryReport;
use crate::canary_rollout::CanaryRolloutReport;
use crate::carbon_aware::CarbonComputeReport;
use crate::cedar_guard::CedarGuardReport;
use crate::cell_isolation_guard::CellIsolationReport;
use crate::chaos_injector::ChaosInjectorReport;
use crate::chaos_mutation_guard::MutationAdequacyReport;
use crate::ci_runner_economics::RunnerEconomicsReport;
use crate::ci_wallclock_ratchet::CiWallclockReport;
use crate::clean_architecture_guard::CleanArchitectureReport;
use crate::cluster_state_auditor::ClusterAuditReport;
use crate::compile_time_profiler::CompileProfileReport;
use crate::compliance_guard::ComplianceGuardReport;
use crate::consistency_guard::ConsistencyReport;
use crate::constant_work_guard::ConstantWorkReport;
use crate::cosign_signer::CosignReport;
use crate::coverage_guard::CoverageReport;
use crate::criterion_bench_ratchet::BenchmarkReport;
use crate::cross_service_impact::ServiceImpactReport;
use crate::deadlock_analyzer::DeadlockReport;
use crate::debt_shrink_guard::DebtShrinkReport;
use crate::doc_guard::DocGuardReport;
use crate::ephemeral_sandbox::SandboxReport;
use crate::ephemeral_secrets::SecretPolicyReport;
use crate::feature_flag_ratchet::FeatureFlagReport;
use crate::finops_ratchet::FinOpsReport;
use crate::flake_quarantine::FlakeQuarantineReport;
use crate::formal_verification::FormalVerificationReport;
use crate::ghost_migration_harness::GhostMigrationReport;
use crate::git_manager::PrDiffContext;
use crate::gitops_drift_reconciler::GitOpsDriftReport;
use crate::gitops_promotion::GitOpsPromotionReport;
use crate::hermetic_build::HermeticBuildReport;
use crate::idempotency_guard::IdempotencyReport;
use crate::jittered_backoff::JitteredBackoffReport;
use crate::kani_guard::KaniGuardReport;
use crate::local_inner_loop::LocalProbeReport;
use crate::microbenchmark_ratchet::MicrobenchmarkReport;
use crate::migration_orchestrator::MigrationLifecycleReport;
use crate::modularization_guard::ModularizationReport;
use crate::monorepo_guard::MonorepoGuardReport;
use crate::predictive_test_selector::PredictiveTestReport;
use crate::progressive_rollout::ProgressiveRingReport;
use crate::psa_admission_guard::PsaAdmissionReport;
use crate::remote_cache_optimizer::CacheReport;
use crate::replay_harness::ReplayHarnessReport;
use crate::rust_language_policy::RustSkillsReport;
use crate::schema_evolution::SchemaEvolutionReport;
use crate::semantic_abi_ratchet::SemanticAbiReport;
use crate::shadow_traffic_harness::ShadowTrafficReport;
use crate::shuffle_shard_simulator::ShuffleShardReport;
use crate::slo_canary_guard::SloCanaryReport;
use crate::stacked_diffs::StackedDiffsReport;
use crate::supply_chain_guard::SupplyChainReport;
use crate::trace_context_guard::TraceContextReport;
use crate::unresolved_review_guard::UnresolvedReviewReport;
use crate::upgrade_train::UpgradeTrainReport;
use crate::vex_scanner::OpenVexReport;
use crate::wasm_sandbox::WasmSandboxReport;
use crate::zero_day_patcher::ZeroDayReport;
use crate::zero_trust_workload::ZeroTrustWorkloadReport;

pub struct PreMergeGuard;

impl Default for PreMergeGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl PreMergeGuard {
    pub fn new() -> Self {
        Self
    }

    /// The test-suite gate's status for a run that happened, or did not.
    ///
    /// Split out and `pub` so the `None` arm is reachable from a test: it is
    /// the arm the review pipeline actually takes, and it was unreachable from
    /// outside while the whole decision sat inline in a 69-argument function.
    pub fn test_suite_gate_status(passed: Option<bool>) -> GateStatus {
        match passed {
            Some(true) => GateStatus::Passed,
            Some(false) => GateStatus::Failed(
                "Test suite reported failures during verification gate.".to_string(),
            ),
            None => GateStatus::NotMeasured {
                gate_id: "test_suite_status".to_string(),
                reason: "no test suite was executed for this pull request, so nothing verifies \
                         that the tests pass"
                    .to_string(),
            },
        }
    }

    /// Evaluates all 70 hyperscale full-lifecycle quality, architecture, GitOps, CI/CD velocity, and security gates
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_pre_merge_gates(
        &self,
        diff_ctx: &PrDiffContext,
        doc_report: &DocGuardReport,
        cedar_report: &CedarGuardReport,
        compliance_report: &ComplianceGuardReport,
        api_contract_report: &ApiContractReport,
        cell_report: &CellIsolationReport,
        supply_chain_report: &SupplyChainReport,
        clean_arch_report: &CleanArchitectureReport,
        monorepo_report: &MonorepoGuardReport,
        debt_report: &DebtShrinkReport,
        modular_report: &ModularizationReport,
        coverage_report: &CoverageReport,
        rust_skills_report: &RustSkillsReport,
        kani_report: &KaniGuardReport,
        slo_report: &SloCanaryReport,
        adr_report: &AdrReport,
        shuffle_report: &ShuffleShardReport,
        trace_report: &TraceContextReport,
        constant_work_report: &ConstantWorkReport,
        idempotency_report: &IdempotencyReport,
        finops_report: &FinOpsReport,
        ghost_migration_report: &GhostMigrationReport,
        gitops_promo_report: &GitOpsPromotionReport,
        gitops_drift_report: &GitOpsDriftReport,
        canary_report: &CanaryRolloutReport,
        cluster_audit_report: &ClusterAuditReport,
        migration_orch_report: &MigrationLifecycleReport,
        ci_wallclock_report: &CiWallclockReport,
        predictive_test_report: &PredictiveTestReport,
        compile_profile_report: &CompileProfileReport,
        remote_cache_report: &CacheReport,
        runner_economics_report: &RunnerEconomicsReport,
        sandbox_report: &SandboxReport,
        cross_service_report: &ServiceImpactReport,
        secret_policy_report: &SecretPolicyReport,
        psa_report: &PsaAdmissionReport,
        shadow_traffic_report: &ShadowTrafficReport,
        unresolved_review_report: &UnresolvedReviewReport,
        local_probe_report: &LocalProbeReport,
        semantic_abi_report: &SemanticAbiReport,
        zero_day_report: &ZeroDayReport,
        formal_report: &FormalVerificationReport,
        deadlock_report: &DeadlockReport,
        aca_report: &AutomatedCanaryReport,
        ring_report: &ProgressiveRingReport,
        hermetic_report: &HermeticBuildReport,
        openvex_report: &OpenVexReport,
        cosign_report: &CosignReport,
        chaos_inj_report: &ChaosInjectorReport,
        stacked_report: &StackedDiffsReport,
        microbench_report: &MicrobenchmarkReport,
        jittered_report: &JitteredBackoffReport,
        schema_evo_report: &SchemaEvolutionReport,
        auto_rollback_report: &AutoRollbackReport,
        wasm_report: &WasmSandboxReport,
        consistency_report: &ConsistencyReport,
        flake_quarantine_report: &FlakeQuarantineReport,
        zero_trust_report: &ZeroTrustWorkloadReport,
        carbon_report: &CarbonComputeReport,
        replay_report: &ReplayHarnessReport,
        upgrade_train_report: &UpgradeTrainReport,
        mutation_report: &MutationAdequacyReport,
        feature_flag_report: &FeatureFlagReport,
        bench_report: &BenchmarkReport,
        attestation_report: &AttestationReport,
        // `None` when no suite was run. This was a plain `bool`, and the review
        // pipeline passed the literal `true`: a gate named for the test suite
        // asserted the suite passed without anything having run it.
        test_suite_passed: Option<bool>,
        review_verdict: &str,
        shape_outcome: &crate::shape::facade::gate::ShapeGateOutcome,
    ) -> Result<PreMergeCertificationReport> {
        info!(
            "Evaluating full-lifecycle quality and GitOps gates for {}#{} ({} gates)...",
            diff_ctx.repo,
            diff_ctx.pr_number,
            crate::pre_merge_guard::report::TOTAL_GATES
        );

        // 1. Doc Parity
        // A probe that could not run is Errored, not Failed: we have no evidence
        // the documentation is deficient, only that we could not judge it.
        // Both block (invariant I1).
        let doc_parity_status = if let Some(err) = &doc_report.errored {
            GateStatus::Errored(err.clone())
        } else if !doc_report.files_created_or_updated.is_empty() {
            GateStatus::AutoUpdated
        } else if doc_report.is_sufficient {
            GateStatus::Passed
        } else {
            GateStatus::Failed(doc_report.summary.clone())
        };

        // 2. Cedar IAM Policy
        let cedar_status = if !cedar_report.files_created_or_updated.is_empty() {
            GateStatus::AutoUpdated
        } else if cedar_report.is_compliant {
            GateStatus::Passed
        } else {
            GateStatus::Failed(cedar_report.summary.clone())
        };

        // 3. Compliance (KR FSS & HIPAA)
        let compliance_status = if compliance_report.is_compliant {
            GateStatus::Passed
        } else {
            GateStatus::Failed(compliance_report.summary.clone())
        };

        // 4. OpenAPI Contract
        let api_contract_status = if api_contract_report.is_intact {
            GateStatus::Passed
        } else {
            GateStatus::Failed(api_contract_report.summary.clone())
        };

        // 5. Cell Boundary
        let cell_isolation_status = if cell_report.is_isolated {
            GateStatus::Passed
        } else {
            GateStatus::Failed(cell_report.summary.clone())
        };

        // 6. Supply Chain Security
        let supply_chain_status = if supply_chain_report.is_secure {
            GateStatus::Passed
        } else {
            GateStatus::Failed(supply_chain_report.summary.clone())
        };

        // 7. Clean Architecture
        // A run that classified no layered file measured nothing: reporting it
        // as Passed would be absent evidence dressed as a pass (invariant I1),
        // and reporting it as Failed would be a fabricated accusation.
        let clean_arch_status = match clean_arch_report.measurement.not_measured_reason() {
            Some(reason) => GateStatus::NotMeasured {
                gate_id: "clean_arch_status".to_string(),
                reason: reason.to_string(),
            },
            None if clean_arch_report.is_clean => GateStatus::Passed,
            None => GateStatus::Failed(clean_arch_report.summary.clone()),
        };

        // 8. Monorepo Guard
        let monorepo_status = if monorepo_report.is_compliant {
            GateStatus::Passed
        } else {
            GateStatus::Failed(monorepo_report.summary.clone())
        };

        // 9. Debt Shrink Guard
        let debt_shrink_status = debt_report.status.clone();

        // 10. Modularization Guard
        let modularization_status = if modular_report.is_modular {
            GateStatus::Passed
        } else {
            GateStatus::Failed(modular_report.summary.clone())
        };

        // 11. Differential Coverage
        // Read the gate's own verdict. Rebuilding it from `is_sufficient` here
        // discarded `NotMeasured` and formatted `f64::NAN` into the accusation
        // "Coverage NaN% is below requirement" -- a fabricated failure published on
        // every PR that adds code without coverage evidence.
        let coverage_status = coverage_report.gate_status();

        // 12. Rust Skills Guard (Upstream 380 Rust Rules)
        let rust_skills_status = if rust_skills_report.is_idiomatic {
            GateStatus::Passed
        } else {
            GateStatus::Failed(rust_skills_report.summary.clone())
        };

        // 13. `// SAFETY:` comment lint over added unsafe blocks
        let kani_status = if kani_report.all_unsafe_blocks_documented {
            GateStatus::Passed
        } else {
            GateStatus::Failed(kani_report.summary.clone())
        };

        // 14. OpenSLO & Error Budget Burn Rate
        // `is_compliant` is true when nothing was measured, so rebuilding from it
        // published absent evidence as `Passed` -- the exact inversion I1 forbids.
        let slo_status = slo_report.status.clone();

        // 15. Living ADR Drift Ratchet
        let adr_status = if adr_report.is_compliant {
            GateStatus::Passed
        } else {
            GateStatus::Failed(adr_report.summary.clone())
        };

        // 16. Cell Shuffle Sharding
        let shuffle_status = if shuffle_report.is_isolated {
            GateStatus::Passed
        } else {
            GateStatus::Failed(shuffle_report.summary.clone())
        };

        // 17. W3C TraceContext Distributed Tracing
        // The guard composes four sentences and decides between four statuses.
        // Rebuilt here from `is_propagated`, three of them were discarded --
        // `GateStatus::Passed` carries no string -- so a diff in which nothing
        // was inspected rendered as a bare tick counted in "N/N gates passed".
        // Same shape as gate 14 above: the guard decides, this clones.
        let trace_status = trace_report.status.clone();

        // 18. Constant-Work Static Bounded Allocations
        let constant_work_status = if constant_work_report.is_bounded {
            GateStatus::Passed
        } else {
            GateStatus::Failed(constant_work_report.summary.clone())
        };

        // 19. Stripe Idempotency Key & Outbox Gate
        let idempotency_status = if idempotency_report.is_idempotent {
            GateStatus::Passed
        } else {
            GateStatus::Warning(idempotency_report.summary.clone())
        };

        // 20. FinOps Unit-Cost Zero-Copy Ratchet
        let finops_status = finops_report.status.clone();

        // 21. Ghost DB Migration & Zero Exclusive Locks
        let ghost_migration_status = ghost_migration_report.status.clone();

        // 22. GitOps Immutable Digest Pinning
        let gitops_promo_status = if gitops_promo_report.is_pinned {
            GateStatus::Passed
        } else {
            GateStatus::Failed(gitops_promo_report.summary.clone())
        };

        // 23. GitOps ArgoCD Manifest Parity
        let gitops_drift_status = gitops_drift_report.status.clone();

        // 24. Progressive Canary Burn-Rate Circuit Breaker
        let canary_status = if canary_report.is_healthy {
            GateStatus::Passed
        } else {
            GateStatus::Failed(canary_report.summary.clone())
        };

        // 25. Live Cluster Readback & Drift Auditor
        let cluster_audit_status = cluster_audit_report.status.clone();

        // 26. Database Expand-Contract Lifecycle
        let migration_orch_status = migration_orch_report.status.clone();

        // 27. CI Wallclock & Compute Cost Ratchet
        let ci_wallclock_status = ci_wallclock_report.status.clone();

        // 28. DAG Predictive Test Selection
        let predictive_test_status = predictive_test_report.status.clone();

        // 29. Compile-Time & Macro Bloat Profiler
        let compile_profile_status = if compile_profile_report.is_lean {
            GateStatus::Passed
        } else {
            GateStatus::Warning(compile_profile_report.summary.clone())
        };

        // 30. Remote Sccache Cache Alignment
        let remote_cache_status = remote_cache_report.status.clone();

        // 31. Runner SKU Tiering
        let runner_economics_status = if runner_economics_report.is_cost_optimal {
            GateStatus::Passed
        } else {
            GateStatus::Warning(runner_economics_report.summary.clone())
        };

        // 32. Ephemeral Sandbox Isolation
        let sandbox_status = sandbox_report.status.clone();

        // 33. Cross-Service Monorepo Blast Radius
        let cross_service_status = if cross_service_report.is_compatible {
            GateStatus::Passed
        } else {
            GateStatus::Failed(cross_service_report.summary.clone())
        };

        // 34. OIDC Zero-Trust Dynamic Credentials
        let ephemeral_secret_status = if secret_policy_report.is_zero_trust {
            GateStatus::Passed
        } else {
            GateStatus::Failed(secret_policy_report.summary.clone())
        };

        // 35. Native Kubernetes PSA Gate (ADR-0710)
        let psa_status = if psa_report.is_compliant {
            GateStatus::Passed
        } else {
            GateStatus::Failed(psa_report.summary.clone())
        };

        // 36. Production Dark-Traffic Shadow Replay
        let shadow_traffic_status = shadow_traffic_report.status.clone();

        // 37. Zero-Unresolved-Comments Review Gate
        let unresolved_review_status = if unresolved_review_report.is_clean {
            GateStatus::Passed
        } else {
            GateStatus::Failed(unresolved_review_report.summary.clone())
        };

        // 38. Sub-100ms Inner-Loop Local Probe
        let local_probe_status = if local_probe_report.is_valid {
            GateStatus::Passed
        } else {
            GateStatus::Failed(local_probe_report.summary.clone())
        };

        // 39. Public Function Signature Stability
        // Published unchanged: the ratchet distinguishes "compared and clean"
        // from "the change touches a layout this gate cannot compute", and a
        // status rebuilt from `is_abi_stable` here would collapse the second
        // into a pass.
        let semantic_abi_status = semantic_abi_report.status.clone();

        // 40. Zero-Day Vulnerability Auto-Patcher
        let zero_day_status = if zero_day_report.is_clean {
            GateStatus::Passed
        } else {
            GateStatus::Warning(zero_day_report.summary.clone())
        };

        // 41. Formal SMT Constraint Verification
        let formal_verification_status = if formal_report.passed {
            GateStatus::Passed
        } else {
            GateStatus::Failed(
                "SMT constraint solver detected unsafe policy or reachability state.".to_string(),
            )
        };

        // 42. Lock Graph & Deadlock Prevention
        // The message carries the cycle the scanner actually found. A fixed
        // sentence would name locks it never looked at, and an author cannot
        // act on an accusation that does not say which locks it is about.
        let deadlock_status = if deadlock_report.passed {
            GateStatus::Passed
        } else {
            GateStatus::Failed(
                deadlock_report
                    .findings
                    .iter()
                    .map(|f| f.description.clone())
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        };

        // 43. Automated Canary Analysis (ACA)
        // Read the gate's own verdict. Rebuilding it from `passed` discarded
        // `NotMeasured` and republished an unqueried canary as `Passed`, while a
        // `false` produced the accusation "detected P99 latency divergence" over
        // samples nobody had read.
        let automated_canary_status = aca_report.status.clone();

        // 44. Progressive Rollout Rings
        let progressive_ring_status = if ring_report.passed {
            GateStatus::Passed
        } else {
            GateStatus::Failed(
                "Progressive ring deployment halted due to unverified canary signals.".to_string(),
            )
        };

        // 45. Hermetic Build Reproducibility
        let hermetic_build_status = hermetic_report.status.clone();

        // 46. OpenVEX Reachability Analysis
        let openvex_status = openvex_report.status.clone();

        // 47. Cosign & Sigstore Provenance Signing
        // This rebuilt the status from `cosign_report.passed`, which was the
        // constant `true` carried out of a fabricated signature bundle. The
        // guard owns the verdict now, so a real Sigstore backend replaces it
        // without touching this wiring.
        let cosign_status = cosign_report.status.clone();

        // 48. Pre-Merge Chaos Injection
        let chaos_injection_status = if chaos_inj_report.passed {
            GateStatus::Passed
        } else {
            GateStatus::Failed("Synthetic chaos fault injection provoked unhandled panic/outage in preview sandbox.".to_string())
        };

        // 49. Stacked Diffs & PR DAG Synchronization
        // As above: `passed` is `plan.atomic_merge_ready`, which is true for a
        // stack that was never read.
        let stacked_diffs_status = stacked_report.status.clone();

        // 50. Microbenchmark Hotpath Ratchet
        // As above: `passed` is arithmetic over a caller-supplied sample, and no
        // benchmark produced one.
        let microbench_status = microbench_report.status.clone();

        // 51. Jittered Exponential Backoff Gate
        let jittered_backoff_status = if jittered_report.passed {
            GateStatus::Passed
        } else {
            GateStatus::Failed(jittered_report.summary.clone())
        };

        // 52. Wire Schema Evolution Ratchet
        let schema_evolution_status = if schema_evo_report.passed {
            GateStatus::Passed
        } else {
            GateStatus::Failed(schema_evo_report.summary.clone())
        };

        // 53. Auto-Rollback & Postmortem Engine
        let auto_rollback_status = auto_rollback_report.status.clone();

        // 54. Dynamic WebAssembly Policy Sandbox
        let wasm_sandbox_status = if wasm_report.passed {
            GateStatus::Passed
        } else {
            GateStatus::Failed(wasm_report.summary.clone())
        };

        // 55. Active-Active Consistency Guard
        let consistency_status = if consistency_report.passed {
            GateStatus::Passed
        } else {
            GateStatus::Failed(consistency_report.summary.clone())
        };

        // 56. Flaky-Test Quarantine Lifecycle
        let flake_quarantine_status = flake_quarantine_report.status.clone();

        // 57. Zero-Trust SPIFFE Workload Identity
        let zero_trust_workload_status = if zero_trust_report.passed {
            GateStatus::Passed
        } else {
            GateStatus::Failed(zero_trust_report.summary.clone())
        };

        // 58. GreenOps Carbon-Aware Compute
        let carbon_compute_status = carbon_report.status.clone();

        // 59. Deterministic Record-and-Replay
        let replay_harness_status = replay_report.status.clone();

        // 60. Proactive Dependency Upgrade Train
        let upgrade_train_status = upgrade_train_report.status.clone();

        // 61. AST Chaos Mutation Test Adequacy
        let mutation_status = if mutation_report.is_adequate {
            GateStatus::Passed
        } else {
            GateStatus::Warning(mutation_report.summary.clone())
        };

        // 62. Feature Flag & Dead Branch Lifecycle
        let feature_flag_report_status = if feature_flag_report.is_clean {
            GateStatus::Passed
        } else {
            GateStatus::Warning(feature_flag_report.summary.clone())
        };

        // 63. Micro-Benchmark & Latency Ratchet
        let bench_status = if bench_report.is_within_budget {
            GateStatus::Passed
        } else {
            GateStatus::Warning(bench_report.summary.clone())
        };

        // 64. Cryptographic Provenance Attestation
        let attestation_status = if attestation_report.is_attested {
            GateStatus::Passed
        } else {
            GateStatus::Failed(attestation_report.summary.clone())
        };

        // 65. Secret & Sensitive Data Scan
        let security_scan_status = PreMergeScanner::scan_for_secrets(&diff_ctx.diff_content);

        // 66. Schema & Breaking Changes Scan
        let schema_compat_status = PreMergeScanner::scan_for_breaking_changes(
            &diff_ctx.diff_content,
            &diff_ctx.changed_files,
        );

        // 67. Concurrency, Performance & Flake Scan
        let performance_concurrency_status =
            PreMergeScanner::scan_for_concurrency_and_flakes(&diff_ctx.diff_content);

        // 68. Test Suite Status
        // `None` is a path on which no suite ran. Reporting a failure there would be
        // an accusation nothing measured, and reporting a pass would be a claim
        // nothing measured; both violate I1, in opposite directions.
        let test_suite_status = Self::test_suite_gate_status(test_suite_passed);

        // 69. AI Code Review & 16-Lens Invariant Gate
        //
        // Only an explicit APPROVE or COMMENT from a successfully parsed response
        // may pass. VERDICT_ERRORED means the harness obtained no review at all —
        // that is Errored, not Failed, because the model did not judge the code
        // adversely; the review simply did not happen. Both block (invariant I1).
        let review_verdict_status = match review_verdict {
            "APPROVE" | "COMMENT" => GateStatus::Passed,
            crate::reviewer::VERDICT_ERRORED => GateStatus::Errored(
                "AI Code Review produced no parseable verdict; the review did not complete"
                    .to_string(),
            ),
            other => GateStatus::Failed(format!(
                "AI Code Review & 16-Lens Matrix issued blocking verdict: {}",
                other
            )),
        };

        // Anvil turns these two inward. Every other gate in this matrix runs
        // against the pull request's repository; these run against Anvil's own
        // tree, because a rule enforced only on other people's code is an
        // assertion about them rather than a property of us.
        let brand_absence_status = {
            let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
            let rep = crate::brand_absence::BrandAbsenceGate::new().scan_tree(repo_root);
            if rep.new_violations.is_empty() {
                GateStatus::Passed
            } else {
                GateStatus::Failed(format!(
                    "{} name(s) or PR-visible string(s) stamp an aspiration instead of naming \
                     what the code verifies",
                    rep.new_violations.len()
                ))
            }
        };

        let migration_boundary_status = match crate::migration::live_tree_violations() {
            Ok(v) if v.is_empty() => GateStatus::Passed,
            Ok(v) => GateStatus::Failed(format!(
                "{} component(s) marked Migrating depend on code oyatie supersedes: {}",
                v.len(),
                v.iter()
                    .map(|x| format!("{} -> {}", x.from, x.to))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            Err(reason) => GateStatus::NotMeasured {
                gate_id: "migration_boundary_status".to_string(),
                reason,
            },
        };

        let shape_status = shape_gate_status(shape_outcome);

        let mut report = PreMergeCertificationReport {
            // Derived by seal(); never a caller-supplied verdict.
            is_certified_ready: false,
            doc_parity_status,
            cedar_status,
            compliance_status,
            api_contract_status,
            cell_isolation_status,
            supply_chain_status,
            clean_arch_status,
            monorepo_status,
            debt_shrink_status,
            modularization_status,
            coverage_status,
            rust_skills_status,
            kani_status,
            slo_status,
            adr_status,
            shuffle_status,
            trace_status,
            constant_work_status,
            idempotency_status,
            finops_status,
            ghost_migration_status,
            gitops_promo_status,
            gitops_drift_status,
            canary_status,
            cluster_audit_status,
            migration_orch_status,
            ci_wallclock_status,
            predictive_test_status,
            compile_profile_status,
            remote_cache_status,
            runner_economics_status,
            sandbox_status,
            cross_service_status,
            ephemeral_secret_status,
            psa_status,
            shadow_traffic_status,
            unresolved_review_status,
            local_probe_status,
            semantic_abi_status,
            zero_day_status,
            formal_verification_status,
            deadlock_status,
            review_verdict_status,
            brand_absence_status,
            migration_boundary_status,
            shape_status,
            automated_canary_status,
            progressive_ring_status,
            hermetic_build_status,
            openvex_status,
            cosign_status,
            chaos_injection_status,
            stacked_diffs_status,
            microbench_status,
            jittered_backoff_status,
            schema_evolution_status,
            auto_rollback_status,
            wasm_sandbox_status,
            consistency_status,
            flake_quarantine_status,
            zero_trust_workload_status,
            carbon_compute_status,
            replay_harness_status,
            upgrade_train_status,
            mutation_status,
            feature_flag_status: feature_flag_report_status,
            bench_status,
            attestation_status,
            security_scan_status,
            schema_compat_status,
            performance_concurrency_status,
            test_suite_status,
            unmeasured_gates: Vec::new(),
            summary_markdown: String::new(),
            // This function is the certification run: these seventy-two
            // statuses are what the gates above reported, not what a caller
            // decided they would have said.
            provenance: GateProvenance::certification_run(),
            // ...and this is what they were reported about. A report with no
            // subject proves "some run produced an all-passing report", never
            // "...for this pull request at this commit", and those are the two
            // claims the merge queue confuses when a head moves mid-run.
            subject: Some(CertifiedSubject {
                repo: diff_ctx.repo.clone(),
                pr_number: diff_ctx.pr_number,
                head_sha: diff_ctx.head_sha.clone(),
            }),
        };
        // A gate the fidelity registry records as Aspirational implements none
        // of the capability its name claims, so whatever it just reported, it
        // has nothing to pass on. Before the verdict, so the withheld gates are
        // in `unmeasured_gates` and in the matrix rather than behind them.
        report.withhold_aspirational_passes();
        // The verdict and the unmeasured list are derived from the statuses just
        // assigned — every field, including the two self-directed gates — so
        // neither can drift from the matrix it summarises.
        report.seal();
        report.summary_markdown = MatrixRenderer::render(&report);
        Ok(report)
    }
}

/// Maps the Shape Program outcome onto the certification vocabulary.
///
/// - No spec adopted: `Warning`, visible on every scorecard, never
///   withholding — a tenant that has not opted in has nothing to measure
///   (owner decision 2026-08-20; precedent: coverage's NothingToMeasure).
/// - Spec present but unreadable: `NotMeasured` (I1 — the gate was asked to
///   measure and could not).
/// - Git failure: `Errored`.
/// - Bootstrap (no baseline at the merge-base) and advisory-only regressions:
///   `Warning` carrying the distance.
/// - Any regression on a blocking rule: `Failed`, first five keys named.
pub fn shape_gate_status(outcome: &crate::shape::facade::gate::ShapeGateOutcome) -> GateStatus {
    use crate::shape::facade::gate::ShapeGateOutcome as O;
    match outcome {
        O::NoSpec { .. } => GateStatus::Warning(
            "no shape spec adopted (.anvil/shape.json absent); see `anvil shape validate-spec`"
                .to_string(),
        ),
        O::SpecUnreadable { reason } => GateStatus::NotMeasured {
            gate_id: "shape_status".to_string(),
            reason: reason.clone(),
        },
        O::Errored { reason } => GateStatus::Errored(reason.clone()),
        O::Bootstrap { .. } => GateStatus::Warning(outcome.summary()),
        O::Judged {
            blocking,
            measurement,
        } => {
            if !blocking.is_empty() {
                let mut first: Vec<&str> = blocking.iter().take(5).map(String::as_str).collect();
                if blocking.len() > 5 {
                    first.push("…");
                }
                GateStatus::Failed(format!(
                    "{} regression(s) on blocking shape rules since the baseline: {}",
                    blocking.len(),
                    first.join("; ")
                ))
            } else if measurement.advisory_regressions > 0 {
                GateStatus::Warning(outcome.summary())
            } else {
                GateStatus::Passed
            }
        }
    }
}
