//! Lane `enlist-authority`, second suite: what the spec suite does not cover.
//!
//! `tests/enlist_authority_test.rs` is the specification for issues #17 and
//! #18 and was written before the implementation. This file is written after
//! it, and deliberately does not repeat it. It adds two things:
//!
//! 1. **Integration.** The spec suite builds every report by hand, through
//!    `from_gate_outcomes`, so it cannot see the wiring: whether the pipeline
//!    puts what a guard actually answered into the report, and whether the
//!    report names the change it measured. The tests below run the corpus —
//!    around forty real guards over a real diff, assembled by the real
//!    `evaluate_pre_merge_gates` — and pin that wiring. Nothing is asserted off
//!    a corpus-built report that a hand-built one would answer the same way:
//!    `admission_refusal` and both publishers take only a
//!    `&PreMergeCertificationReport` and cannot tell the two apart, so
//!    re-asserting the spec suite's refusals here would buy a corpus run and no
//!    discrimination.
//!
//! 2. **Boundary.** The symmetric half of the same invariant, which the
//!    codebase states in its own comments ("a fabricated `Failed` would
//!    accuse") and the spec suite does not test: a gate that produced no
//!    measurement must not be published in the words of one that found a
//!    defect, and a gate whose fix exists only in Anvil's clone must not be
//!    counted among the gates that passed on the commit being merged. Issue
//!    #18's blanket claim is guarded there, on the one admitted fixture where
//!    it would reach a pull request, rather than swept over the corpus:
//!    `measured_lines` iterates `named_statuses()` uniformly, so there is no
//!    implementation that leaks the claim for one gate and not another.
//!
//! Plus one corpus-integrity check that `report.rs`'s own pins cannot make,
//! because they are all on lengths: no two gates may be published under one
//! name.
//!
//! # Issue #17's doors are not re-tested here
//!
//! There were three of them — the CLI `enlist` subcommand, `POST /api/enlist`,
//! and the queue healer's re-enlistment — and every regression written for them
//! here reduced to one call, `enlist_into_merge_queue(repo, 1, evidence)`,
//! which is the first case of the spec suite's
//! `the_merge_queue_entry_point_refuses_the_evidence_it_was_handed`. None of
//! them read the handler they were named after, so a door that reverted to
//! handing over a fabricated passing report left all three green. The spec
//! suite guards the doors where they can actually be guarded, by tracing each
//! one's evidence expression back to a certification run in
//! `every_door_hands_the_merge_queue_evidence_a_certification_run_produced`.
//!
//! Nothing in this file calls `enlist_into_merge_queue`, and nothing in it
//! names a real repository or a real pull request. That is a safety property,
//! not tidiness: under exactly the regression such a test exists to detect,
//! control runs past `admission_refusal` into `gh pr edit`, a formal
//! `submit_pr_review` APPROVE and `gh pr merge --auto`, so a door regression
//! test pointed at the live remote mutates and self-approves a real pull
//! request from a test run.
//!
//! # What the corpus can and cannot produce in this build
//!
//! A report the corpus produces is not admissible in this tree, for reasons
//! that are facts about the deployment rather than about any pull request:
//! eighteen gates at this commit have no data source configured and report
//! `NotMeasured` -- four more than before, since the empty-scope gates stopped
//! certifying corpora they never had -- and
//! `brand_absence_status` scans Anvil's own `src/` and reports `Failed` on the
//! naming debt recorded there. So a corpus run is always refused here, whatever
//! the pull request is, and the integration tests below assert the refusal
//! rather than branching on it.
//!
//! The admitted shapes are reached the cheap way instead, by handing gate
//! outcomes to `from_gate_outcomes` — the spec suite's own route, and the only
//! honest one, since reaching them from a corpus run means overwriting the
//! answers the corpus gave and so putting nothing of the corpus under test.
//!
//! Nothing here touches the network, `gh`, the clock, or any path outside the
//! repository and a private temporary directory: every test may run in
//! parallel with every other.

use anvil::git_manager::PrDiffContext;
use anvil::merge_enlister::MergeEnlister;
use anvil::pre_merge_guard::PreMergeGuard;
use anvil::pre_merge_guard::matrix::{MatrixRenderer, label_for};
use anvil::pre_merge_guard::report::{GateStatus, PreMergeCertificationReport, TOTAL_GATES};
use std::path::Path;

/// The merge strategy the enlistment note is written about. Held constant so
/// that any difference between two notes comes from the report.
const STRATEGY: &str = "Squash & Merge";

// =========================================================================
// Fixtures
// =========================================================================

/// The corpus with every gate `Passed` except the named overrides, built the
/// way a report is built: by handing gate outcomes to the constructor that
/// consumes them.
fn report_with(overrides: &[(&str, GateStatus)]) -> PreMergeCertificationReport {
    let base = PreMergeCertificationReport::unmeasured("fixture baseline");
    let outcomes: Vec<(&str, GateStatus)> = base
        .named_statuses()
        .into_iter()
        .map(|(gate, _)| {
            let status = overrides
                .iter()
                .find(|(name, _)| *name == gate)
                .map(|(_, s)| s.clone())
                .unwrap_or(GateStatus::Passed);
            (gate, status)
        })
        .collect();
    PreMergeCertificationReport::from_gate_outcomes(&outcomes)
        .expect("the fixture hands over an outcome for every gate in the corpus")
}

fn every_gate_passing() -> PreMergeCertificationReport {
    let report = report_with(&[]);
    assert!(
        MergeEnlister::admission_refusal(Some(&report)).is_ok(),
        "fixture sanity: a fully measured, fully passing corpus is admissible"
    );
    report
}

fn gate_names() -> Vec<&'static str> {
    PreMergeCertificationReport::unmeasured("fixture baseline")
        .named_statuses()
        .into_iter()
        .map(|(gate, _)| gate)
        .collect()
}

fn not_measured(gate_id: &str) -> GateStatus {
    GateStatus::NotMeasured {
        gate_id: gate_id.to_string(),
        reason: "no data source configured".to_string(),
    }
}

fn status_of(report: &PreMergeCertificationReport, gate: &str) -> GateStatus {
    report
        .named_statuses()
        .into_iter()
        .find(|(name, _)| *name == gate)
        .map(|(_, status)| status.clone())
        .unwrap_or_else(|| panic!("`{gate}` is not a gate in this corpus"))
}

/// Everything Anvil writes onto a pull request for a report, and nothing else:
/// the approving review body and the enlistment note.
fn published_texts(report: &PreMergeCertificationReport) -> Vec<(&'static str, String)> {
    [
        (
            "approval_summary",
            MergeEnlister::approval_summary(Some(report)),
        ),
        (
            "enlistment_note",
            MergeEnlister::enlistment_note(Some(report), STRATEGY),
        ),
    ]
    .into_iter()
    .filter_map(|(seam, text)| text.map(|t| (seam, t)))
    .collect()
}

/// The two names Anvil has for a gate: the field the report carries it under,
/// and the label it is published under. Which one a publication uses is not any
/// of these tests' business — `MatrixRenderer` uses the label, `measured_lines`
/// happens to use the field name, and both are correct — so every assertion
/// that a text names a gate accepts either.
fn either_name_for(gate: &str) -> (&str, &'static str) {
    let label = label_for(gate).map(|(l, _)| l).unwrap_or("");
    assert!(
        !label.is_empty(),
        "fixture sanity: `{gate}` has no published label, so this test cannot \
         tell a publication that names it from one that does not"
    );
    (gate, label)
}

/// Whether `text` names `gate` under either of its two names.
fn names_gate(text: &str, gate: &str) -> bool {
    let (field, label) = either_name_for(gate);
    text.contains(field) || text.contains(label)
}

/// Whether `n` appears in `text` as a number in its own right, so a claim about
/// three gates is not answered by the "3" inside "23".
fn mentions_number(text: &str, n: usize) -> bool {
    let n = n.to_string();
    text.split(|c: char| !c.is_ascii_digit()).any(|t| t == n)
}

// =========================================================================
// Integration — the corpus, the door and the two publishers, wired together
// =========================================================================

/// A change with something in it for the corpus to read: a source file and its
/// test, in the shape `prepare_pr_diff` hands over.
fn a_change(work_dir: &Path) -> PrDiffContext {
    PrDiffContext {
        repo: A_PULL_REQUEST.0.to_string(),
        pr_number: A_PULL_REQUEST.1,
        base_branch: "main".to_string(),
        base_sha: "1111111111111111111111111111111111111111".to_string(),
        head_sha: A_PULL_REQUEST.2.to_string(),
        previous_head_sha: None,
        repo_working_dir: work_dir.to_path_buf(),
        diff_content: SMALL_DIFF.to_string(),
        changed_files: vec!["src/greeting.rs".to_string()],
        is_incremental: false,
    }
}

const SMALL_DIFF: &str = "diff --git a/src/greeting.rs b/src/greeting.rs\n\
--- a/src/greeting.rs\n\
+++ b/src/greeting.rs\n\
@@ -0,0 +1,9 @@\n\
+/// Returns the greeting for `name`.\n\
+pub fn greeting(name: &str) -> String {\n\
+    format!(\"hello, {name}\")\n\
+}\n\
+\n\
+#[test]\n\
+fn greeting_names_the_person() {\n\
+    assert_eq!(greeting(\"ada\"), \"hello, ada\");\n\
+}\n";

/// The pull request the corpus is run for: repository, number, head commit.
///
/// Deliberately not this repository and not a pull request that exists. Nothing
/// in this file reaches GitHub, but a fixture naming the live remote is one
/// refactor away from a test that does, and the failure mode of that mistake is
/// a real pull request edited and self-approved from a test run.
const A_PULL_REQUEST: (&str, u64, &str) = (
    "anvil-spec/there-is-no-such-repo",
    4242,
    "2222222222222222222222222222222222222222",
);

/// The report the certification pipeline builds, built by the pipeline.
///
/// Every gate `certify_pull_request` can run without a network, a subprocess or
/// a clone is run here by the real guard over `a_change`, and the report is
/// assembled by the real `evaluate_pre_merge_gates`: statuses, verdict,
/// unmeasured list, provenance mark, subject and rendered matrix all derived by
/// production code.
///
/// Six of the corpus's guards are asynchronous and reach outside the process —
/// documentation and cedar policy synthesis, the OpenAPI syncer, the monorepo
/// scan, the GitHub review-thread query and the attestation stamper. Those six
/// are handed the report a satisfied guard returns, written out at the top of
/// this function so a reader can see exactly which part of the corpus is
/// standing in, and that nothing else is.
///
/// `verification_gate` is what `local_verification_gate` returned and
/// `review_verdict` is what the code review reached: the two values the enlist
/// doors compute for themselves, and the two the corpus turns into gate
/// statuses. Varying them is how these tests reach the report shapes that
/// matter without inventing a report.
fn report_from_the_corpus(
    work_dir: &Path,
    verification_gate: Option<bool>,
    review_verdict: &str,
) -> PreMergeCertificationReport {
    let diff_ctx = a_change(work_dir);
    let d = &diff_ctx;
    let dir = work_dir;

    let doc_report = anvil::doc_guard::DocGuardReport {
        is_sufficient: true,
        files_created_or_updated: Vec::new(),
        summary: "documentation parity holds".to_string(),
        errored: None,
    };
    let cedar_report = anvil::cedar_guard::CedarGuardReport {
        is_compliant: true,
        files_created_or_updated: Vec::new(),
        summary: "cedar policies are in parity".to_string(),
    };
    let api_contract_report = anvil::api_contract_guard::ApiContractReport {
        is_intact: true,
        auto_synced_files: Vec::new(),
        summary: "the wire contract is intact".to_string(),
    };
    let monorepo_report = anvil::monorepo_guard::MonorepoGuardReport {
        is_compliant: true,
        violations: Vec::new(),
        summary: "package boundaries hold".to_string(),
    };
    let unresolved_review_report = anvil::unresolved_review_guard::UnresolvedReviewReport {
        is_clean: true,
        unresolved_threads: Vec::new(),
        summary: "no unresolved review threads".to_string(),
    };
    let attestation_report = anvil::attestation_guard::AttestationReport {
        is_attested: true,
        stamped_receipt_path: None,
        summary: "receipt stamped".to_string(),
    };

    let compliance_report = anvil::compliance_guard::ComplianceGuard::new()
        .evaluate_compliance(d)
        .expect("the compliance guard reads the diff");
    let cell_report = anvil::cell_isolation_guard::CellIsolationGuard::new()
        .evaluate_cell_isolation(d)
        .expect("the cell isolation guard reads the diff");
    let supply_chain_report = anvil::supply_chain_guard::SupplyChainGuard::new()
        .audit_supply_chain(dir, d)
        .expect("the supply chain guard reads the diff");
    let clean_arch_report = anvil::clean_architecture_guard::CleanArchitectureGuard::new()
        .evaluate_architecture(d)
        .expect("the clean architecture guard reads the diff");
    let debt_report = anvil::debt_shrink_guard::DebtShrinkGuard::new()
        .evaluate_debt_shrink(dir, d)
        .expect("the debt shrink guard reads the diff");
    let modular_report = anvil::modularization_guard::ModularizationGuard::new()
        .evaluate_modularization(d)
        .expect("the modularization guard reads the diff");
    let coverage_report = anvil::coverage_guard::CoverageGuard::new()
        .evaluate_diff_coverage(dir, d)
        .expect("the coverage guard reads the diff");
    let rust_skills_report = anvil::rust_language_policy::RustLanguagePolicy::new()
        .evaluate_rust_quality(dir, d)
        .expect("the rust language policy reads the diff");
    let kani_report = anvil::kani_guard::KaniGuard::new()
        .lint_unsafe_safety_comments(dir, d)
        .expect("the kani guard reads the diff");
    let slo_report = anvil::slo_canary_guard::SloCanaryGuard::new()
        .evaluate_slo_canary_health(dir, d)
        .expect("the slo canary guard reads the diff");
    let adr_report = anvil::adr_drift_ratchet::AdrDriftRatchet::new()
        .evaluate_adr_parity(dir, d)
        .expect("the adr ratchet reads the diff");
    let shuffle_report = anvil::shuffle_shard_simulator::ShuffleShardSimulator::new()
        .evaluate_shuffle_sharding(dir, d)
        .expect("the shuffle shard simulator reads the diff");
    let trace_report = anvil::trace_context_guard::TraceContextGuard::new()
        .evaluate_trace_propagation(dir, d)
        .expect("the trace context guard reads the diff");
    let constant_work_report = anvil::constant_work_guard::ConstantWorkGuard::new()
        .evaluate_constant_work(dir, d)
        .expect("the constant work guard reads the diff");
    let idempotency_report = anvil::idempotency_guard::IdempotencyGuard::new()
        .evaluate_idempotency(dir, d)
        .expect("the idempotency guard reads the diff");
    let finops_report = anvil::finops_ratchet::FinOpsUnitCostRatchet::new()
        .evaluate_unit_cost(dir, d)
        .expect("the finops ratchet reads the diff");
    let ghost_migration_report = anvil::ghost_migration_harness::GhostMigrationHarness::new()
        .evaluate_migrations(dir, d)
        .expect("the ghost migration harness reads the diff");
    let gitops_promo_report = anvil::gitops_promotion::GitOpsPromotionEngine::new()
        .evaluate_manifest_promotions(dir, d)
        .expect("the gitops promotion engine reads the diff");
    let gitops_drift_report = anvil::gitops_drift_reconciler::GitOpsDriftReconciler::new()
        .evaluate_gitops_drift(dir, d)
        .expect("the gitops drift reconciler reads the diff");
    let canary_report = anvil::canary_rollout::CanaryRolloutGuard::new()
        .evaluate_rollout_health(dir, d)
        .expect("the canary rollout guard reads the diff");
    let cluster_audit_report = anvil::cluster_state_auditor::ClusterStateAuditor::new()
        .evaluate_cluster_parity(dir, d)
        .expect("the cluster state auditor reads the diff");
    let migration_orch_report =
        anvil::migration_orchestrator::MigrationLifecycleOrchestrator::new()
            .evaluate_migration_lifecycle(dir, d)
            .expect("the migration orchestrator reads the diff");
    let ci_wallclock_report = anvil::ci_wallclock_ratchet::CiWallclockEconomicsRatchet::new()
        .evaluate_ci_efficiency(dir, d)
        .expect("the ci wallclock ratchet reads the diff");
    let predictive_test_report = anvil::predictive_test_selector::PredictiveTestSelector::new()
        .evaluate_test_selection(dir, d)
        .expect("the predictive test selector reads the diff");
    let compile_profile_report = anvil::compile_time_profiler::CompileTimeProfiler::new()
        .evaluate_compile_profile(dir, d)
        .expect("the compile time profiler reads the diff");
    let remote_cache_report = anvil::remote_cache_optimizer::RemoteCacheOptimizer::new()
        .evaluate_cache_alignment(dir, d)
        .expect("the remote cache optimizer reads the diff");
    let runner_economics_report = anvil::ci_runner_economics::CiRunnerEconomicsOptimizer::new()
        .evaluate_runner_economics(dir, d)
        .expect("the runner economics optimizer reads the diff");
    let sandbox_report = anvil::ephemeral_sandbox::EphemeralSandboxManager::new()
        .evaluate_sandbox_isolation(dir, d)
        .expect("the ephemeral sandbox manager reads the diff");
    let cross_service_report = anvil::cross_service_impact::CrossServiceImpactEngine::new()
        .evaluate_cross_service_impact(dir, d)
        .expect("the cross service impact engine reads the diff");
    let secret_policy_report = anvil::ephemeral_secrets::EphemeralSecretInjector::new()
        .evaluate_secret_policies(dir, d)
        .expect("the ephemeral secret injector reads the diff");
    let psa_report = anvil::psa_admission_guard::PsaAdmissionGuard::new()
        .evaluate_psa_admission(dir, d)
        .expect("the psa admission guard reads the diff");
    let shadow_traffic_report = anvil::shadow_traffic_harness::ShadowTrafficHarness::new()
        .evaluate_shadow_verification(dir, d)
        .expect("the shadow traffic harness reads the diff");
    let local_probe_report = anvil::local_inner_loop::LocalInnerLoopProbe::new()
        .evaluate_local_probe(dir, d)
        .expect("the local inner loop probe reads the diff");
    let semantic_abi_report = anvil::semantic_abi_ratchet::SemanticAbiRatchet::new()
        .evaluate_abi_stability(dir, d)
        .expect("the semantic abi ratchet reads the diff");
    let zero_day_report = anvil::zero_day_patcher::ZeroDayAutoPatcher::new()
        .evaluate_zero_day_patches(dir, d)
        .expect("the zero day patcher reads the diff");
    let mutation_report = anvil::chaos_mutation_guard::ChaosMutationGuard::new()
        .evaluate_mutation_adequacy(d)
        .expect("the chaos mutation guard reads the diff");
    let feature_flag_report = anvil::feature_flag_ratchet::FeatureFlagRatchet::new()
        .evaluate_feature_flags(dir, d)
        .expect("the feature flag ratchet reads the diff");
    let bench_report = anvil::criterion_bench_ratchet::CriterionBenchRatchet::new()
        .evaluate_benchmarks(dir, d)
        .expect("the criterion bench ratchet reads the diff");

    let formal_report = anvil::formal_verification::FormalVerificationGuard::new()
        .evaluate_formal_invariants(&d.diff_content);
    let deadlock_report = anvil::deadlock_analyzer::DeadlockStaticAnalyzer::new()
        .evaluate_deadlock_invariants(&d.repo, &d.diff_content);
    let aca_report =
        anvil::automated_canary::AutomatedCanaryAnalysis::new().evaluate_without_metrics_source();
    let ring_report = anvil::progressive_rollout::ProgressiveRingOrchestrator::new()
        .evaluate_ring_rollout(
            &anvil::progressive_rollout::DeploymentRing::Ring0Canary,
            aca_report.status.is_acceptable(),
        );
    let hermetic_report = anvil::hermetic_build::HermeticBuildValidator::new()
        .evaluate_hermetic_reproducibility("sha256_clean", "sha256_clean", &d.diff_content);
    let openvex_report = anvil::vex_scanner::OpenVexReachabilityScanner::new().scan_reachability(
        "CVE-NONE",
        "none",
        "symbol_none",
        &d.diff_content,
    );
    let cosign_report =
        anvil::cosign_signer::CosignProvenanceSigner::new().evaluate_without_signing_backend();
    let chaos_inj_report =
        anvil::chaos_injector::ChaosFaultInjector::new().inject_synthetic_chaos(&d.diff_content);
    let stacked_report =
        anvil::stacked_diffs::StackedDiffsOrchestrator::new().evaluate_without_stack_source();
    let microbench_report = anvil::microbenchmark_ratchet::MicroBenchmarkRatchet::new()
        .evaluate_without_criterion_baseline();
    let jittered_report = anvil::jittered_backoff::JitteredBackoffGuard::new()
        .evaluate_backoff_and_jitter(&d.diff_content);
    let schema_evo_report = anvil::schema_evolution::SchemaEvolutionRatchet::new()
        .evaluate_schema_evolution(&d.diff_content);
    let auto_rollback_report = anvil::auto_rollback::AutoRollbackPostmortemEngine::new()
        .evaluate_health_and_rollback(&d.repo, 0.01, 45.0);
    let wasm_report =
        anvil::wasm_sandbox::WasmPolicySandbox::new().execute_sandboxed_policies(&d.diff_content);
    let consistency_report = anvil::consistency_guard::ActiveActiveConsistencyGuard::new()
        .evaluate_active_active_invariants(&d.diff_content);
    let flake_quarantine_report = anvil::flake_quarantine::FlakeQuarantineLifecycle::new()
        .evaluate_quarantine_lifecycle(&d.changed_files);
    let zero_trust_report = anvil::zero_trust_workload::ZeroTrustWorkloadGate::new()
        .evaluate_workload_identity(&d.diff_content);
    let carbon_report =
        anvil::carbon_aware::CarbonAwareComputeRatchet::new().evaluate_compute_carbon(30.0, 12.0);
    let replay_report =
        anvil::replay_harness::DeterministicReplayHarness::new().evaluate_replay_parity(&[]);
    let upgrade_train_report =
        anvil::upgrade_train::ProactiveUpgradeTrain::new().evaluate_upgrade_train(&[]);

    // No `.anvil/shape.json` in this tree, which is what the shape gate reports
    // for a tenant that has not adopted a spec.
    let shape_outcome = anvil::shape::facade::gate::ShapeGateOutcome::NoSpec {
        reason: "no shape spec adopted in this working tree".to_string(),
    };

    PreMergeGuard::new()
        .evaluate_pre_merge_gates(
            d,
            &doc_report,
            &cedar_report,
            &compliance_report,
            &api_contract_report,
            &cell_report,
            &supply_chain_report,
            &clean_arch_report,
            &monorepo_report,
            &debt_report,
            &modular_report,
            &coverage_report,
            &rust_skills_report,
            &kani_report,
            &slo_report,
            &adr_report,
            &shuffle_report,
            &trace_report,
            &constant_work_report,
            &idempotency_report,
            &finops_report,
            &ghost_migration_report,
            &gitops_promo_report,
            &gitops_drift_report,
            &canary_report,
            &cluster_audit_report,
            &migration_orch_report,
            &ci_wallclock_report,
            &predictive_test_report,
            &compile_profile_report,
            &remote_cache_report,
            &runner_economics_report,
            &sandbox_report,
            &cross_service_report,
            &secret_policy_report,
            &psa_report,
            &shadow_traffic_report,
            &unresolved_review_report,
            &local_probe_report,
            &semantic_abi_report,
            &zero_day_report,
            &formal_report,
            &deadlock_report,
            &aca_report,
            &ring_report,
            &hermetic_report,
            &openvex_report,
            &cosign_report,
            &chaos_inj_report,
            &stacked_report,
            &microbench_report,
            &jittered_report,
            &schema_evo_report,
            &auto_rollback_report,
            &wasm_report,
            &consistency_report,
            &flake_quarantine_report,
            &zero_trust_report,
            &carbon_report,
            &replay_report,
            &upgrade_train_report,
            &mutation_report,
            &feature_flag_report,
            &bench_report,
            &attestation_report,
            verification_gate,
            review_verdict,
            &shape_outcome,
        )
        .expect("the corpus produces a report for a change it can read")
}

fn a_working_tree() -> tempfile::TempDir {
    tempfile::tempdir().expect("a private temporary directory")
}

/// INTEGRATION — a change moves through certification, and the merge queue
/// answers for the report that run produced.
///
/// Four properties, all about the wiring rather than about any one gate, so
/// they hold whatever the guards say about this diff:
///
///   - the report names the pull request and the commit it was measured
///     against, which is what the enlistment pins the merge to;
///   - `unmeasured_gates` is exactly the gates that reported no measurement,
///     so the field the refusal is written from cannot drift from the statuses
///     it summarises;
///   - the door refuses this run, the refusal names a gate that is really
///     absent or failing in it, and nothing is published. A refusal that
///     publishes anyway signs Anvil's name onto a change that is not going
///     through;
///   - every gate the cheap pre-flight names as unmeasurable in this build is
///     in fact one this run could not measure, so the doors are not refusing in
///     advance on a claim the corpus does not bear out.
#[test]
fn a_change_that_moves_through_certification_is_answered_for_by_that_report() {
    let work = a_working_tree();
    let report = report_from_the_corpus(work.path(), Some(true), "APPROVE");

    let subject = report
        .subject()
        .expect("a report a certification run produced names what it measured");
    assert_eq!(
        (
            subject.repo.as_str(),
            subject.pr_number,
            subject.head_sha.as_str()
        ),
        A_PULL_REQUEST,
        "the corpus measured one pull request at one commit and the report names \
         another. A report about commit X is not evidence about commit Y, and the \
         enlistment carries this SHA to GitHub as `--match-head-commit`"
    );

    // Compared as sets. `unmeasured_gates` comes from `all_statuses()` and this
    // side from `named_statuses()`, two independently hand-written lists, and
    // nothing pins that they agree on order — nor should it: inserting a gate at
    // a different position in the two, or sorting the refusal so it reads
    // stably, is behaviour-preserving. What is under test is *which* gates the
    // refusal is written from.
    let mut reported_no_measurement: Vec<String> = report
        .named_statuses()
        .into_iter()
        .filter(|(_, status)| matches!(status, GateStatus::NotMeasured { .. }))
        .map(|(gate, _)| gate.to_string())
        .collect();
    reported_no_measurement.sort();
    let mut recorded = report.unmeasured_gates.clone();
    recorded.sort();
    assert_eq!(
        recorded, reported_no_measurement,
        "the list the refusal is written from is not the list of gates that \
         produced no measurement"
    );

    // A corpus run in this tree is always refused (module docs), so this is
    // asserted rather than branched on: a branch whose other arm cannot execute
    // reads as coverage and runs never.
    // The count is interpolated rather than written out: a number in a panic
    // message goes stale the moment a gate changes verdict, and this suite's
    // whole subject is claims outliving the thing they describe.
    let why_refused = format!(
        "{} gates in this build have no data source and `brand_absence_status` \
         reports Anvil's own naming debt, so no corpus run here can admit a \
         pull request. If that changed, this test is now asserting the wrong \
         half of the wiring",
        recorded.len()
    );
    let refusal = MergeEnlister::admission_refusal(Some(&report))
        .expect_err(&why_refused)
        .to_string();
    let absent_or_failing: Vec<&str> = report
        .named_statuses()
        .into_iter()
        .filter(|(_, s)| !s.is_acceptable() || matches!(s, GateStatus::NotMeasured { .. }))
        .map(|(gate, _)| gate)
        .collect();
    assert!(
        absent_or_failing.iter().any(|gate| refusal.contains(gate)),
        "the merge was withheld and the refusal names no gate that is actually \
         absent or failing in the report it was written from, so an operator has \
         nothing to act on.\n  refusal: {refusal}\n  gates: {absent_or_failing:?}"
    );
    assert!(
        published_texts(&report).is_empty(),
        "Anvil endorsed a pull request the same report refuses to admit"
    );

    // The pre-flight the enlist doors read claims certain gates cannot be
    // measured in this build at all. That claim has to be true of the corpus,
    // and this is the corpus run — no second one is needed to check it.
    let blockers = anvil::pre_merge_guard::unmeasurable_gates_in_this_build()
        .expect("this build has a gate no execution of it can measure; see the pre-flight test");
    let mut checked = 0usize;
    for (gate, status) in report.named_statuses() {
        if blockers.contains(gate) {
            checked += 1;
            assert!(
                !status.is_acceptable() || matches!(status, GateStatus::NotMeasured { .. }),
                "`{gate}` is named as a gate this build cannot produce a \
                 measurement for, and this real corpus run reported `{}` for it. \
                 The doors are refusing in advance on a claim the corpus does not \
                 bear out",
                status.badge()
            );
        }
    }
    // The filter reads the pre-flight for the *field* name. A pre-flight
    // reworded to name only the published label would leave the loop above
    // iterating nothing while this test stayed green, so the count is asserted
    // rather than assumed.
    assert!(
        checked > 0,
        "the pre-flight named gates this build cannot measure and none of them \
         matched a gate in the report, so the claim above was checked against \
         nothing: {blockers}"
    );
}

/// INTEGRATION — the verification gate's outcome reaches the merge queue as
/// what it actually was.
///
/// This test begins at the evaluator's three-arm `test_suite_passed` match
/// (src/pre_merge_guard/evaluator.rs), which nothing else in `tests/` and no
/// `mod tests` in `evaluator.rs` pins. It hands `Option<bool>` to
/// `evaluate_pre_merge_gates` and pins that `None` is recorded as `NotMeasured`
/// rather than as a failure, and that the two answers which withhold stay
/// distinguishable in `unmeasured_gates`.
///
/// It does **not** cover the two links upstream of that match, and must not be
/// read as if it did. Commit 032ca6a fixed `QueueHealer::run_local_test_gate`,
/// which classified a spawn failure and the 1800s build timeout as
/// `TestGate::Failed`, and `local_verification_gate`, which mapped that to
/// `Some(false)`. That old code passes this test unchanged, because this test
/// starts downstream of it. Neither link is reachable from an integration test
/// — `run_local_test_gate` is `pub(crate)`, and `local_verification_gate` takes
/// a `GitManager` and produces an ephemeral worktree from a clone — so the
/// `TestGate::Errored` -> `None` mapping is pinned nowhere in `tests/`.
#[test]
fn the_verification_gate_reaches_the_report_as_what_it_actually_did() {
    let work = a_working_tree();

    let mut runs: Vec<(Option<bool>, PreMergeCertificationReport)> = Vec::new();
    for (gate_said, expected, what) in [
        (Some(true), "Passed", "a suite that ran and passed"),
        (
            Some(false),
            "Failed",
            "a suite that ran and reported failures",
        ),
        (None, "NotMeasured", "a gate that never completed"),
    ] {
        let report = report_from_the_corpus(work.path(), gate_said, "APPROVE");
        let actual = match status_of(&report, "test_suite_status") {
            GateStatus::Passed => "Passed",
            GateStatus::Failed(_) => "Failed",
            GateStatus::NotMeasured { .. } => "NotMeasured",
            GateStatus::Errored(_) => "Errored",
            GateStatus::Warning(_) => "Warning",
            GateStatus::AutoUpdated => "AutoUpdated",
        };
        assert_eq!(
            actual, expected,
            "the corpus recorded {what} as `{actual}`. `Some(false)` is a \
             statement about the pull request and `None` is not: a gate that \
             never completed, published as a failing suite, accuses a \
             contributor of something nothing ran and hands them a remediation \
             for it"
        );
        runs.push((gate_said, report));
    }

    // The two answers that withhold must stay distinguishable in the report a
    // refusal is written from. That neither is admitted or endorsed is *not*
    // asserted here: `admission_refusal` and both publishers take only a
    // `&PreMergeCertificationReport` and cannot see how it was built, so a
    // corpus-built report tells them nothing a hand-built one does not, and the
    // spec suite already pins both seams over both shapes.
    let held = |answer: Option<bool>| -> &PreMergeCertificationReport {
        runs.iter()
            .find(|(gate_said, _)| *gate_said == answer)
            .map(|(_, report)| report)
            .expect("the loop above ran the corpus for every answer the gate can give")
    };
    let never_ran = held(None);
    let failed = held(Some(false));
    let unmeasured = "test_suite_status".to_string();
    assert!(
        never_ran.unmeasured_gates.contains(&unmeasured)
            && !failed.unmeasured_gates.contains(&unmeasured),
        "the gate that never completed and the suite that failed are recorded as \
         the same thing, so a refusal cannot tell a reader which happened"
    );
}

/// INTEGRATION — a code review that did not complete is absent evidence, not a
/// blocking verdict.
///
/// The enlist doors are the paths where the review had never run at all; they
/// run it now, and the answer "it did not complete" has to reach the merge
/// queue as `Errored` — which withholds without accusing — rather than as the
/// `Failed` that says the model judged the code adversely.
///
/// What is pinned is the evaluator's `review_verdict` match
/// (src/pre_merge_guard/evaluator.rs): `VERDICT_ERRORED` reaching the report as
/// `Errored` and not as `Failed`, and `REQUEST_CHANGES` reaching it as `Failed`
/// and not as `Errored`. That an errored gate is then refused and published
/// nothing is the spec suite's — `a_report_that_certifies_while_a_gate_errored_is_still_refused`
/// and `nothing_is_endorsed_on_evidence_that_cannot_admit_the_pull_request` —
/// and the door cannot tell a corpus-built report from a hand-built one, so
/// re-asserting it here would add no implementation the suite catches.
///
/// The `unmeasured_gates` note below is a sanity check, not a second claim:
/// `Errored` is the shape `is_admissible()` cannot see, which is why the door
/// asks `admission_refusal` and not the weaker predicate.
#[test]
fn a_review_that_did_not_complete_is_absent_evidence_not_a_blocking_verdict() {
    let work = a_working_tree();
    let errored = report_from_the_corpus(work.path(), Some(true), anvil::reviewer::VERDICT_ERRORED);

    assert!(
        matches!(
            status_of(&errored, "review_verdict_status"),
            GateStatus::Errored(_)
        ),
        "a code review that did not complete must reach the report as a gate \
         that errored; got {:?}",
        status_of(&errored, "review_verdict_status")
    );
    assert!(
        !errored
            .unmeasured_gates
            .contains(&"review_verdict_status".to_string()),
        "fixture sanity: `unmeasured_gates` records `NotMeasured` only, so this \
         gate is invisible to `is_admissible()`. That is the reason the door \
         asks `admission_refusal` instead"
    );

    // A review that did judge the code adversely is a different answer, and the
    // report has to tell them apart: one is a finding against the pull request,
    // the other is a run that did not happen.
    let rejected = report_from_the_corpus(work.path(), Some(true), "REQUEST_CHANGES");
    assert!(
        matches!(
            status_of(&rejected, "review_verdict_status"),
            GateStatus::Failed(_)
        ),
        "a blocking review verdict is a measurement against the pull request and \
         must not be recorded as a review that did not complete; got {:?}",
        status_of(&rejected, "review_verdict_status")
    );
}

/// UNIT — the cheap pre-flight the enlist doors read names the gate that can
/// never pass, and why.
///
/// `unmeasurable_gates_in_this_build` is what the three enlist doors read
/// before running anything: a gate that cannot produce a measurement in this
/// deployment makes every report the corpus can return inadmissible, whatever
/// the pull request is, and the alternative is paying a clone, seventy-two
/// guards, a model turn and a cold `cargo check` to arrive at a refusal the
/// configuration had already fixed. The string it returns is therefore the
/// whole of what an operator gets, and it has to carry both halves: which gate,
/// and why that gate can never pass.
///
/// Both assertions are read off the one `format!` in
/// `unmeasurable_gates_in_this_build` (src/pre_merge_guard/mod.rs). A refusal
/// that says only that something is unmeasurable fails the first; one that
/// names the gate and drops the guard's own reason fails the second.
///
/// That a real corpus run bears the claim out is asserted where a corpus run is
/// already paid for, in
/// `a_change_that_moves_through_certification_is_answered_for_by_that_report`.
/// Nor does anything here assert an ordering: that the doors read this before
/// paying for a corpus is a property of the doors, pinned in the spec suite
/// where the doors are read.
#[test]
fn the_cheap_pre_flight_names_the_gate_that_can_never_pass_and_why() {
    let blockers = anvil::pre_merge_guard::unmeasurable_gates_in_this_build()
        .expect("this build has a gate no execution of it can measure: `slo_status`");
    assert!(
        names_gate(&blockers, "slo_status"),
        "the pre-flight refusal must name the gate that cannot be measured, or \
         an operator is told only that something is wrong: {blockers}"
    );
    let reason = anvil::slo_canary_guard::burn_rate_is_unmeasurable()
        .expect("fixture sanity: the pre-flight's one entry is this guard's answer");
    assert!(
        blockers.contains(reason),
        "the pre-flight names the gate and drops the reason the guard gave for \
         it, so an operator reading a refusal that cost nothing to produce is \
         told a gate can never pass and not why. The reason was: {reason}\n  \
         blockers: {blockers}"
    );
}

// =========================================================================
// Boundary — the half of the invariant that is about not accusing
// =========================================================================

/// BOUNDARY — a gate Anvil auto-corrected is not a gate that passed on the
/// commit being merged.
///
/// `AutoUpdated` means a guard found a deficiency and wrote files to fix it.
/// Those files are staged and committed in Anvil's own shared clone, nothing
/// pushes them to the pull request's branch, and the enlistment pins the merge
/// to the head *without* the fix. Counted among the passes, the approving
/// review says the whole corpus passed about a tree `--match-head-commit`
/// guarantees will not merge.
///
/// The status is acceptable and measured, so the pull request is admitted and
/// the text really is published: of the non-passing shapes, this is the one
/// where a wrong count reaches GitHub.
///
/// This is the test that decides the convention. The spec suite's
/// `an_endorsement_accounts_for_the_gate_that_did_not_simply_pass` computes a
/// local `clean_passes` as `Passed | AutoUpdated`, but that is a variable in a
/// fixture with no `AutoUpdated` gate in it, so it asserts nothing either way
/// and the two suites do not forbid each other's implementations. Here the gate
/// is `AutoUpdated`, and what is asserted is what `--match-head-commit` makes
/// true: a fix that lives only in Anvil's clone did not pass on the commit
/// being merged, so it is not one of the gates the endorsement may count.
#[test]
fn a_gate_that_was_auto_corrected_is_not_counted_among_the_gates_that_passed() {
    let report = report_with(&[
        ("doc_parity_status", GateStatus::AutoUpdated),
        ("cedar_status", GateStatus::AutoUpdated),
    ]);
    assert!(
        MergeEnlister::admission_refusal(Some(&report)).is_ok(),
        "fixture sanity: `AutoUpdated` is acceptable and measured, so this pull \
         request is admitted and whatever is published lands on a merge that \
         really happens"
    );
    assert_eq!(
        report.gate_counts().0,
        TOTAL_GATES,
        "fixture sanity: the ready-made count scores an auto-correction as a \
         pass, which is what makes publishing it a claim about a tree that will \
         not merge"
    );

    let clean = published_texts(&every_gate_passing());
    let texts = published_texts(&report);
    assert_eq!(
        texts.len(),
        clean.len(),
        "an admitted pull request is endorsed, and this one is admitted"
    );
    for ((seam, text), (_, clean_text)) in texts.iter().zip(clean.iter()) {
        assert_ne!(
            text, clean_text,
            "`{seam}` published the same text for a corpus that passed outright \
             and one whose doc-parity and cedar fixes exist only in Anvil's local \
             clone. The two pull requests merge different trees"
        );
        // The negative form, and deliberately not "the text says {TOTAL_GATES}
        // - 2". A publication that names the two auto-corrected gates and gives
        // no count at all is honest, and the spec suite's sibling pins no
        // wording either. What is forbidden is the claim: the whole corpus
        // offered as what passed. `TOTAL_GATES` as the *size* of the corpus is
        // honest and the current text uses it that way, so the ban is on
        // putting that number in front of a reader without the figure that
        // stops it reading as a clean sweep.
        assert!(
            !mentions_number(text, TOTAL_GATES) || mentions_number(text, TOTAL_GATES - 2),
            "`{seam}` published {TOTAL_GATES} — the whole corpus — as what this \
             pull request passed, with no smaller figure beside it. Two of those \
             gates were auto-corrected in Anvil's local clone and are not in the \
             commit `--match-head-commit` pins the merge to. Text was:\n{text}"
        );
        // REGRESSION, issue #18 — the same claim with the number taken out.
        // The defect was one string literal in a function that received no
        // report, so it was signed onto every pull request whatever the gates
        // did. The two sentences it quoted are gone, so looking for them finds
        // nothing any implementation can produce; what has to stay unreachable
        // is the claim under any rewording, and this fixture is where it would
        // do real harm — two gates fixed only in Anvil's clone, on a pull
        // request that is admitted and really is endorsed. The vocabulary is
        // the spec suite's `TOTALITY`. A bare "100%" is not on it: the rendered
        // matrix carries that inside gate descriptions, and banning the number
        // would forbid the most honest derivation available.
        let lower = text.to_lowercase();
        for claim in [
            "100% compliance",
            "100% compliant",
            "100% green",
            "100% certified",
            "100% pass",
            "100% clean",
            "100% of gates",
            "all automated",
            "all gates",
            "all checks",
            "all safety",
            "every gate",
            "fully compliant",
            "fully green",
        ] {
            assert!(
                !lower.contains(claim),
                "`{seam}` asserted \"{claim}\" for a corpus whose doc-parity and \
                 cedar fixes exist only in Anvil's local clone. Nothing in the \
                 report says it, a reader cannot check it against anything, and \
                 it goes onto the pull request permanently.\n  text was:\n{text}"
            );
        }
        assert!(
            names_gate(text, "doc_parity_status") || names_gate(text, "cedar_status"),
            "`{seam}` says nothing about the two gates whose fix is not in the \
             commit being merged — neither under the field name the report \
             carries them as nor under the label they are published under — so a \
             reader cannot discover that the tree that passed and the tree that \
             merges are different. Text was:\n{text}"
        );
    }
}

/// BOUNDARY — the two statuses that mean "nothing was measured" are not
/// published in the words of the one that means "a defect was found".
///
/// The codebase names this obligation repeatedly in its own comments — "a
/// fabricated `Failed` would accuse" — and the spec suite tests only the
/// direction that withholds. On the pull request, a gate that never ran and a
/// gate that found a defect are told apart by the badge in the rendered matrix
/// and by nothing else.
#[test]
fn a_gate_that_produced_no_measurement_is_not_reported_as_a_gate_that_failed() {
    let accusation = GateStatus::Failed("a defect was found".to_string()).badge();
    for absent in [
        not_measured("kani_status"),
        GateStatus::Errored("the probe could not run".to_string()),
    ] {
        assert_ne!(
            absent.badge(),
            accusation,
            "a gate that produced no measurement carries the badge of one that \
             found a defect, so the pull request is accused of something nothing \
             ran: {absent:?}"
        );
        let report = report_with(&[("kani_status", absent.clone())]);
        let rendered = MatrixRenderer::render(&report);
        // The row is selected by the gate, under either of the names Anvil has
        // for it, and the selection is asserted before anything is read off it.
        // A filter on a published spelling turns a relabel — which is
        // behaviour-preserving and none of this test's business — into a filter
        // that matches nothing, and "no row accused this gate" is the passing
        // state, so the test would disarm itself while still reading as cover.
        let rows: Vec<&str> = rendered
            .lines()
            .filter(|line| names_gate(line, "kani_status"))
            .collect();
        assert_eq!(
            rows.len(),
            1,
            "the rendered matrix — the part of the report a reader of the pull \
             request actually sees — carries {} rows for `kani_status`, so this \
             test cannot tell what the gate was published as:\n{rendered}",
            rows.len()
        );
        let row = rows[0];
        assert!(
            row.contains(absent.badge()),
            "the gate's row does not carry what it reported: {absent:?} rendered \
             as {row}"
        );
        assert!(
            !row.contains(accusation),
            "the rendered matrix reports a gate that produced no measurement as \
             a failure: {absent:?} rendered as {row}"
        );
    }
}

// =========================================================================
// Corpus integrity — the one thing report.rs's own pins cannot see
// =========================================================================

/// CORPUS INTEGRITY — every gate is published under its own name.
///
/// `named_statuses()` is the list every refusal and every published line is
/// written from. Two gates sharing a name means one of them can never appear in
/// a refusal, and a reader cannot tell which of the two a published line is
/// about. Copying a `named_statuses()` line and changing the field but not the
/// string is the live way to do that, and it happens exactly when a gate is
/// added.
///
/// This is the one thing `report.rs`'s own pins cannot see. They are all on
/// *lengths* — `named_statuses()` against `all_statuses()`
/// (`tests::named_statuses_and_all_statuses_stay_aligned`), `all_statuses()`
/// against `TOTAL_GATES` (`total_gates_pin::all_statuses_matches_the_declared_total`),
/// `gate_counts()` against a fully passing corpus
/// (`tests::gate_counts_reflect_reality_not_a_constant`) — and two gates under
/// one name keeps every one of those lengths right. No count is re-asserted
/// here, and no order is required of `named_statuses()`: `MatrixRenderer` keeps
/// its own gate order, so reordering fields in the report is
/// behaviour-preserving and must not fail a test.
#[test]
fn every_gate_is_published_under_its_own_name() {
    let named: Vec<String> = gate_names().into_iter().map(str::to_string).collect();

    let mut unique = named.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        named.len(),
        "two gates are published under one name, so one of them can never be \
         named in a refusal and a reader cannot tell which of the two a \
         published line is about"
    );
}

/// A gate the fidelity registry records as `Aspirational` implements none of
/// the capability its name claims, so it has nothing to pass on. The rule was
/// stated in `Fidelity`'s own doc comment and encoded in `may_report_pass()`,
/// and until the certification run started asking it, seven aspirational gates
/// published `Passed` on this very fixture: `deadlock_status`,
/// `openvex_status`, `cosign_status`, `auto_rollback_status`,
/// `carbon_compute_status`, `replay_harness_status` and
/// `upgrade_train_status`.
///
/// `aspirational_gates_cannot_pass_test.rs` pins every branch of the rule
/// against hand-built reports. This is the one that runs the real corpus over a
/// real diff and asks what the pull request would actually have been told —
/// which is the only place the seven were visible, because a hand-built report
/// says whatever it was handed.
#[test]
fn no_aspirational_gate_publishes_a_pass_on_the_change_the_corpus_measured() {
    let work = a_working_tree();
    let report = report_from_the_corpus(work.path(), Some(true), "APPROVE");

    let aspirational: Vec<&str> = anvil::fidelity::registry::AUDITED_GATES
        .iter()
        .filter(|e| !e.fidelity.may_report_pass())
        .map(|e| e.gate_id)
        .collect();
    assert!(
        !aspirational.is_empty(),
        "fixture sanity: with no aspirational gate in the registry this test \
         asks nothing"
    );

    let stamped: Vec<&str> = report
        .named_statuses()
        .into_iter()
        .filter(|(gate, status)| {
            aspirational.contains(gate)
                && matches!(status, GateStatus::Passed | GateStatus::AutoUpdated)
        })
        .map(|(gate, _)| gate)
        .collect();
    assert!(
        stamped.is_empty(),
        "the registry records {stamped:?} as implementing none of the capability \
         they are named for, and the certification run published them as passing \
         on this pull request"
    );

    // Discrimination: a run in which nothing passes would satisfy the above.
    let passing = report
        .named_statuses()
        .into_iter()
        .filter(|(_, s)| matches!(s, GateStatus::Passed))
        .count();
    assert!(
        passing > 20,
        "fixture sanity: only {passing} gates passed, so the assertion above \
         would hold against a corpus that measured nothing"
    );
}
