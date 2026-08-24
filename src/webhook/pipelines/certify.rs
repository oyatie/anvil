//! Pre-merge certification: the gate corpus, and the ways it is run.
//!
//! `certify_pull_request` is the corpus itself, split out of
//! `execute_pr_review` so that every path which hands a pull request to the
//! merge queue can obtain the evidence the review pipeline obtains rather than
//! enlisting on none. Issue #17: three of the four callers of
//! `enlist_into_merge_queue` had no report to pass, because there was nowhere
//! to get one.

use anyhow::{Context, Result};
use std::path::Path;
use tokio::process::Command;
use tracing::{info, warn};

use super::review::execute_pr_review;
use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::PreMergeCertificationReport;
use crate::progressive_rollout::DeploymentRing;
use crate::webhook::AppState;

/// How long an enlist door waits behind another run of the corpus for the same
/// pull request before it gives up and says so.
///
/// Sized to let one whole corpus run ahead of it finish -- a clone, the guards,
/// a model turn under `ExecClass::Model` (600s) and a cold build under
/// `ExecClass::Build` (1800s) -- rather than to any particular gate.
const PR_LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(3600);

pub async fn execute_pr_certify(state: &AppState, repo: &str, pr_number: u64) -> Result<()> {
    info!(
        "Running Pre-Merge Certification for PR #{} on {}...",
        pr_number, repo
    );
    let meta = state
        .github_client
        .fetch_pr_metadata(repo, pr_number)
        .await?;
    execute_pr_review(
        state,
        repo,
        pr_number,
        &meta.title,
        &meta.body.unwrap_or_default(),
        &meta.base_ref_name,
        &meta.base_ref_oid,
        &meta.head_ref_oid,
        true,
    )
    .await
}

/// Runs the whole gate corpus against `diff_ctx` and returns what it produced.
///
/// `review_verdict` is the verdict the AI code review reached for this head.
/// `test_suite_passed` is the outcome of the verification gate, and `None`
/// where the repository offers none -- which reports `NotMeasured` rather than
/// inventing either answer.
///
/// This function mutates the caller's working tree (`git add` excluding Anvil's receipts, `git commit`)
/// and the caller is responsible for holding `acquire_pr_lock` across it. It
/// does not take the lock itself: `execute_pr_review` already holds it here and
/// the mutex is not reentrant.
///
/// It also does not touch review state. The reviewed-SHA stamp belongs to
/// `execute_pr_review`, which sets it and rolls it back; an enlistment running
/// this corpus must not be able to un-stamp a pull request and cause the review
/// pipeline to re-review it.
#[allow(clippy::too_many_arguments)]
pub async fn certify_pull_request(
    state: &AppState,
    repo: &str,
    pr_number: u64,
    title: &str,
    body: &str,
    head_sha: &str,
    repo_dir: &Path,
    diff_ctx: &PrDiffContext,
    review_verdict: &str,
    test_suite_passed: Option<bool>,
) -> Result<PreMergeCertificationReport> {
    // 2. DocGuard: Documentation & Doctrine Parity
    let doc_report = state
        .doc_guard
        .ensure_documentation_parity(repo, repo_dir, diff_ctx, title, body)
        .await?;

    // 3. CedarGuard: offline verification of the Cedar policies this PR touched.
    //
    // Deliberately not `?`. This guard used to propagate its error, so a
    // missing binary failed the whole certification run rather than one gate;
    // it now returns a report whose `NotMeasured` says the same thing without
    // taking the other seventy-one gates down.
    let cedar_report = state
        .cedar_guard
        .evaluate_cedar_policies(repo_dir, diff_ctx)
        .await;

    // 4. ComplianceGuard: Dynamic KR PIPA, FSS & HIPAA Regulatory Engine
    let compliance_report = state.compliance_guard.evaluate_compliance(diff_ctx)?;

    // 5. ApiContractGuard: OpenAPI & Wire Contract Integrity Gate
    let api_contract_report = state
        .api_contract_guard
        .ensure_contract_integrity(repo, repo_dir, diff_ctx)
        .await?;

    // 6. CellIsolationGuard: Cell Boundary & Multi-Tenancy Isolation
    let cell_report = state
        .cell_isolation_guard
        .evaluate_cell_isolation(diff_ctx)?;

    // 7. SupplyChainGuard: SLSA L2+ Dependency Security Audit
    let supply_chain_report = state
        .supply_chain_guard
        .audit_supply_chain(repo_dir, diff_ctx)?;

    // 8. CleanArchitectureGuard: Core -> Ports -> Adapters Boundary Enforcement
    let clean_arch_report = state.clean_arch_guard.evaluate_architecture(diff_ctx)?;

    // 9. MonorepoGuard: Hyperscaler Package Boundaries & Hermeticity
    let monorepo_report = state
        .monorepo_guard
        .evaluate_monorepo_hygiene(repo_dir, diff_ctx)
        .await?;

    // 10. DebtShrinkGuard: Deprecation & Reorg Drain Ratchet
    let debt_shrink_report = state
        .debt_shrink_guard
        .evaluate_debt_shrink(repo_dir, diff_ctx)?;

    // 11. ModularizationGuard: Componentized File Sizing (100-300 Lines)
    let modular_report = state
        .modularization_guard
        .evaluate_modularization(diff_ctx)?;

    // 12. CoverageGuard: Differential Test Coverage (>=85%)
    let coverage_report = state
        .coverage_guard
        .evaluate_diff_coverage(repo_dir, diff_ctx)?;

    // 13. RustLanguagePolicy: 380 Upstream Rust 2024 Edition Rules
    let rust_skills_report = state
        .rust_language_policy
        .evaluate_rust_quality(repo_dir, diff_ctx)?;

    // 14. KaniGuard: `// SAFETY:` comment lint over added unsafe blocks
    let kani_report = state
        .kani_guard
        .lint_unsafe_safety_comments(repo_dir, diff_ctx)?;

    // 15. SloCanaryGuard: OpenSLO Error Budget Burn-Rate Gate
    let slo_report = state
        .slo_canary_guard
        .evaluate_slo_canary_health(repo_dir, diff_ctx)?;

    // 16. AdrDriftRatchet: Living ADR 5-Field Schema Ratchet
    let adr_report = state
        .adr_drift_ratchet
        .evaluate_adr_parity(repo_dir, diff_ctx)?;

    // 17. ShuffleShardSimulator: Cell Shuffle-Sharding & Combinatorial Blast-Radius Gate
    let shuffle_report = state
        .shuffle_shard_simulator
        .evaluate_shuffle_sharding(repo_dir, diff_ctx)?;

    // 18. TraceContextGuard: W3C Distributed Tracing & Span Invariant Gate
    let trace_report = state
        .trace_context_guard
        .evaluate_trace_propagation(repo_dir, diff_ctx)?;

    // 19. ConstantWorkGuard: Bounded Pools, Static Capacities & Anti-Fragility Gate
    let constant_work_report = state
        .constant_work_guard
        .evaluate_constant_work(repo_dir, diff_ctx)?;

    // 20. IdempotencyGuard: Stripe Idempotency Keys & Transactional Outbox Gate
    let idempotency_report = state
        .idempotency_guard
        .evaluate_idempotency(repo_dir, diff_ctx)?;

    // 21. FinOpsUnitCostRatchet: Zero-Copy Hotpaths & Cost-Per-Outcome Ratchet Gate
    let finops_report = state
        .finops_ratchet
        .evaluate_unit_cost(repo_dir, diff_ctx)?;

    // 22. GhostMigrationHarness: Zero-Lock Database Migration Verification Gate
    let ghost_migration_report = state
        .ghost_migration_harness
        .evaluate_migrations(repo_dir, diff_ctx)?;

    // 23. GitOpsPromotionEngine: Deterministic OCI Digest Pinning Gate
    let gitops_promo_report = state
        .gitops_promotion_engine
        .evaluate_manifest_promotions(repo_dir, diff_ctx)?;

    // 24. GitOpsDriftReconciler: Deterministic Manifest Parity & Orphan Prevention Gate
    let gitops_drift_report = state
        .gitops_drift_reconciler
        .evaluate_gitops_drift(repo_dir, diff_ctx)?;

    // 25. CanaryRolloutGuard: Deterministic Traffic Shifter & Burn Breaker Gate
    let canary_report = state
        .canary_rollout_guard
        .evaluate_rollout_health(repo_dir, diff_ctx)?;

    // 26. ClusterStateAuditor: Deterministic Live Readback vs Git Desired-State Gate
    let cluster_audit_report = state
        .cluster_state_auditor
        .evaluate_cluster_parity(repo_dir, diff_ctx)?;

    // 27. MigrationLifecycleOrchestrator: 4-Phase Expand-Contract Database Lifecycle Gate
    let migration_orch_report = state
        .migration_orchestrator
        .evaluate_migration_lifecycle(repo_dir, diff_ctx)?;

    // 28. CiWallclockEconomicsRatchet: Fast CI Target & Regression Prevention Gate
    let ci_wallclock_report = state
        .ci_wallclock_ratchet
        .evaluate_ci_efficiency(repo_dir, diff_ctx)?;

    // 29. PredictiveTestSelector: Deterministic DAG Predictive Test Selection Gate
    let predictive_test_report = state
        .predictive_test_selector
        .evaluate_test_selection(repo_dir, diff_ctx)?;

    // 30. CompileTimeProfiler: Macro Bloat & Slow Build Dependency Profiler Gate
    let compile_profile_report = state
        .compile_time_profiler
        .evaluate_compile_profile(repo_dir, diff_ctx)?;

    // 31. RemoteCacheOptimizer: Deterministic Sccache Key & Cache-Hit Ratchet Gate
    let remote_cache_report = state
        .remote_cache_optimizer
        .evaluate_cache_alignment(repo_dir, diff_ctx)?;

    // 32. CiRunnerEconomicsOptimizer: Deterministic Runner SKU Tiering Gate
    let runner_economics_report = state
        .ci_runner_economics
        .evaluate_runner_economics(repo_dir, diff_ctx)?;

    // 33. EphemeralSandboxManager: Deterministic Sub-Second Micro-Sandbox Gate
    let sandbox_report = state
        .ephemeral_sandbox
        .evaluate_sandbox_isolation(repo_dir, diff_ctx)?;

    // 34. CrossServiceImpactEngine: Cross-Service Monorepo Blast Radius Gate
    let cross_service_report = state
        .cross_service_impact
        .evaluate_cross_service_impact(repo_dir, diff_ctx)?;

    // 35. EphemeralSecretInjector: OIDC Zero-Trust Dynamic Ephemeral Credentials Gate
    let secret_policy_report = state
        .ephemeral_secrets
        .evaluate_secret_policies(repo_dir, diff_ctx)?;

    // 36. PsaAdmissionGuard: Deterministic Native Kubernetes PSA (ADR-0710) Gate
    let psa_report = state
        .psa_admission_guard
        .evaluate_psa_admission(repo_dir, diff_ctx)?;

    // 37. ShadowTrafficHarness: Production Dark-Traffic Shadow Replay Gate
    let shadow_traffic_report = state
        .shadow_traffic_harness
        .evaluate_shadow_verification(repo_dir, diff_ctx)?;

    // 38. UnresolvedReviewGuard: Zero-Unresolved-Comments Review Gate
    let unresolved_review_report = state
        .unresolved_review_guard
        .evaluate_unresolved_reviews(repo, pr_number)
        .await?;

    // 39. LocalInnerLoopProbe: Sub-100ms Inner-Loop Local Probe Gate
    let local_probe_report = state
        .local_inner_loop
        .evaluate_local_probe(repo_dir, diff_ctx)?;

    // 40. SemanticAbiRatchet: Public Library ABI & Semver Stability Gate
    let semantic_abi_report = state
        .semantic_abi_ratchet
        .evaluate_abi_stability(repo_dir, diff_ctx)?;

    // 41. ZeroDayAutoPatcher: Upstream Zero-Day Vulnerability Auto-Patcher Gate
    let zero_day_report = state
        .zero_day_patcher
        .evaluate_zero_day_patches(repo_dir, diff_ctx)?;

    // 42. FormalVerificationGuard: SMT / Z3 Mathematical Policy Invariants
    let formal_report = state
        .formal_verification
        .evaluate_formal_invariants(&diff_ctx.diff_content);

    // 43. DeadlockStaticAnalyzer: Lock Graph Order Inversion & Deadlock Prevention
    let deadlock_report = state
        .deadlock_analyzer
        .evaluate_deadlock_invariants(repo, &diff_ctx.diff_content);

    // 44. AutomatedCanaryAnalysis: Statistical Canary Verification
    // No canary deployment is driven from here and no metrics endpoint is
    // configured, so there are no distributions to compare. The gate reports
    // NotMeasured naming the missing source rather than being handed samples
    // written on this line, whose verdict would describe those samples and not
    // the pull request.
    let aca_report = state.automated_canary.evaluate_without_metrics_source();

    // 45. ProgressiveRingOrchestrator: 4-Ring Progressive Rollout Schedule
    // Consumes the canary verdict. `is_acceptable()` is true for NotMeasured:
    // an unqueried canary is not an unhealthy one, and halting every ring on
    // absent telemetry would be a fabricated accusation. This gate therefore
    // inherits the canary's lack of evidence; `automated_canary_status` in the
    // fidelity registry records what is missing.
    let ring_report = state.progressive_rollout.evaluate_ring_rollout(
        &DeploymentRing::Ring0Canary,
        aca_report.status.is_acceptable(),
    );

    // 46. HermeticBuildValidator: Deterministic Bit-for-Bit Reproducibility
    // Nothing builds this tree twice, so there is no second digest to compare.
    // The two literals passed here were the same string, making the equality
    // check true by construction.
    let hermetic_report = state
        .hermetic_build
        .scan_for_impurity_without_build_pair(&diff_ctx.diff_content);

    // 47. OpenVexReachabilityScanner: Callgraph-Pruned Dead-Code Exploitability
    // No advisory feed is read. The placeholders passed here named a CVE that
    // does not exist, and the scanner clears anything whose symbol is absent.
    let openvex_report = state.vex_scanner.evaluate_without_advisory_source();

    // 48. CosignProvenanceSigner: OIDC Keyless Cryptographic Attestation
    // No OIDC token is requested, no Fulcio certificate is issued and no Rekor
    // entry is submitted, so this artefact carries no attestation. The head sha
    // used to be handed to a signer that invented a certificate and a
    // transparency-log id from it; the gate now reports the absence instead.
    let cosign_report = state.cosign_signer.evaluate_without_signing_backend();

    // 49. ChaosFaultInjector: Pre-Merge Synthetic Fault Simulation
    let chaos_inj_report = state
        .chaos_injector
        .inject_synthetic_chaos(&diff_ctx.diff_content);

    // 50. StackedDiffsOrchestrator: Multi-PR DAG Synchronization
    // No forge query enumerates the PRs stacked on this one, so the stack is
    // unknown. Previously an empty slice literal was passed here, which is the
    // same absence of information dressed as an evaluated stack.
    let stacked_report = state.stacked_diffs.evaluate_without_stack_source();

    // 51. MicroBenchmarkRatchet: Sub-Microsecond Hotpath Criterion Ratchet
    // No criterion harness runs in this repository, so there is no base or head
    // timing to ratchet. The sample that used to be written here compared a
    // literal against itself.
    let microbench_report = state
        .microbenchmark_ratchet
        .evaluate_without_criterion_baseline();

    // 52. JitteredBackoffGuard: AWS Builders' Library Exponential Jitter & Storm Prevention Gate
    let jittered_report = state
        .jittered_backoff
        .evaluate_backoff_and_jitter(&diff_ctx.diff_content);

    // 53. SchemaEvolutionRatchet: Wire Schema Backward/Forward Compatibility Ratchet
    let schema_evolution_report = state
        .schema_evolution
        .evaluate_schema_evolution(&diff_ctx.diff_content);

    // 54. AutoRollbackPostmortemEngine: Canary Auto-Rollback & Postmortem Engine
    // No canary telemetry is queried. The literals passed here sat far below
    // the degradation thresholds, so the rollback branch was unreachable.
    let auto_rollback_report = state.auto_rollback.evaluate_without_telemetry_source();

    // 55. WasmPolicySandbox: WebAssembly Dynamic Bytecode Policy Sandbox Gate
    let wasm_report = state
        .wasm_sandbox
        .execute_sandboxed_policies(&diff_ctx.diff_content);

    // 56. ActiveActiveConsistencyGuard: Multi-Region Active-Active Consistency & Conflict Resolution Gate
    let consistency_report = state
        .consistency_guard
        .evaluate_active_active_invariants(&diff_ctx.diff_content);

    // 57. FlakeQuarantineLifecycle: Flaky-Test Quarantine & Rehabilitation Lifecycle Gate
    let flake_quarantine_report = state
        .flake_quarantine
        .evaluate_quarantine_lifecycle(&diff_ctx.changed_files);

    // 58. ZeroTrustWorkloadGate: Zero-Trust SPIFFE/SPIRE Workload Identity & mTLS Gate
    let zero_trust_report = state
        .zero_trust_workload
        .evaluate_workload_identity(&diff_ctx.diff_content);

    // 59. CarbonAwareComputeRatchet: GreenOps Carbon-Aware Compute Efficiency Ratchet
    // Nothing meters CPU time or grid intensity; the two literals passed here
    // were compared against each other and published as joules.
    let carbon_report = state.carbon_aware.evaluate_without_energy_source();

    // 60. DeterministicReplayHarness: Production Dark-Trace Record-and-Replay Gate
    // No production trace corpus is collected. The empty slice passed here was
    // answered vacuously: an empty slice trivially satisfies the payload check.
    let replay_report = state.replay_harness.evaluate_without_trace_source();

    // 61. ProactiveUpgradeTrain: Proactive Dependency & Security Upgrade Train Gate
    // No dependency manifest or advisory feed is read; `breaking == 0` is
    // trivially true of the empty slice that was passed here.
    let upgrade_train_report = state.upgrade_train.evaluate_without_dependency_source();

    // 62. ChaosMutationGuard: Mutation Adequacy of the Changed Lines
    let mutation_report = state
        .chaos_mutation_guard
        .measure_diff_mutants(repo_dir, diff_ctx)
        .await;

    // 63. FeatureFlagRatchet: Feature Flag & Dead Branch Lifecycle Gate
    let feature_flag_report = state
        .feature_flag_ratchet
        .evaluate_feature_flags(repo_dir, diff_ctx)?;

    // 64. CriterionBenchRatchet: Micro-Benchmark & Latency Ratchet Gate
    let bench_report = state
        .criterion_bench_ratchet
        .evaluate_benchmarks(repo_dir, diff_ctx)?;

    // 65. AttestationGuard: Cryptographic Provenance Receipt Stamper
    let attestation_report = state
        .attestation_guard
        .stamp_lane_receipt(
            repo_dir,
            repo,
            pr_number,
            head_sha,
            crate::attestation_guard::AttestationGuard::VERDICT_PENDING,
            Vec::new(),
        )
        .await?;

    // Stage and commit ONLY substantive domain policy changes (NEVER push attestation receipts in a loop)
    let mut modified_files = Vec::new();
    modified_files.extend(doc_report.files_created_or_updated.clone());
    modified_files.extend(api_contract_report.auto_synced_files.clone());

    if !modified_files.is_empty() {
        info!(
            "Domain guards generated real updates: {:?}. Committing & pushing...",
            modified_files
        );
        // Staging is bounded and fails CLOSED: a hang, a spawn failure or a non-zero
        // exit aborts the review instead of letting the pipeline certify a PR whose
        // auto-synced governance files were never actually committed.
        //
        // The staging command is shared with every other site that stages a
        // clone. This one swept the whole tree, so the lane receipt written
        // into `repo_dir` just above was staged and committed onto the pull
        // request -- the exact loop the comment above forbids.
        let add_cmd = crate::git_manager::stage_excluding_receipts(repo_dir);
        let add_out = crate::exec::run_bounded(
            add_cmd,
            crate::exec::ExecClass::Quick,
            "git add for domain guard auto-sync",
        )
        .await
        .context("Failed to stage auto-synced documentation & contract files")?;
        if !add_out.status.success() {
            anyhow::bail!(
                "git add failed while staging auto-synced governance files on PR #{}: {}",
                pr_number,
                String::from_utf8_lossy(&add_out.stderr).trim()
            );
        }

        let commit_msg = format!(
            "chore(governance): [skip review] auto-sync documentation & wire contracts on PR #{}\n\n\
            X-Anvil-Action: doc-sync\n\
            X-Anvil-Version: 0.1.0\n\n\
            *🤖 Certified by Oyatie Anvil*",
            pr_number
        );
        let mut commit_cmd = Command::new("git");
        commit_cmd
            .current_dir(repo_dir)
            .args(["commit", "-m", &commit_msg]);
        let commit_out = crate::exec::run_bounded(
            commit_cmd,
            crate::exec::ExecClass::Quick,
            "git commit for domain guard auto-sync",
        )
        .await
        .context("Failed to commit auto-synced documentation & contract files")?;

        if commit_out.status.success() {
            // Auto-synced documentation & policies are staged and committed locally for gate verification
            // Pushes are directed to the PR head branch rather than base_branch trunk
            info!(
                "Auto-synced documentation & policies committed locally on PR #{} for certification.",
                pr_number
            );
        } else {
            let stdout = String::from_utf8_lossy(&commit_out.stdout);
            let stderr = String::from_utf8_lossy(&commit_out.stderr);
            let empty_commit = ["nothing to commit", "no changes added to commit"]
                .iter()
                .any(|needle| stdout.contains(needle) || stderr.contains(needle));
            if empty_commit {
                // Benign no-op: the guards rewrote files to content git already has.
                // Nothing was lost, so the review may continue.
                warn!(
                    "Domain guards reported updates {:?} on PR #{}, but git had nothing to commit; the working tree already matched HEAD.",
                    modified_files, pr_number
                );
            } else {
                anyhow::bail!(
                    "git commit failed for auto-synced governance files on PR #{}: {} {}",
                    pr_number,
                    stdout.trim(),
                    stderr.trim()
                );
            }
        }
    }

    // Evaluate the full pre-merge, GitOps, CI-velocity and security certification matrix
    // Shape Program: judge the head against the baseline frozen at the
    // merge-base of the branch this PR targets, and record what was measured.
    let shape_outcome = crate::shape::facade::gate::judge_pr(
        &diff_ctx.repo_working_dir,
        &diff_ctx.base_branch,
        &diff_ctx.head_sha,
        &diff_ctx.repo,
    )
    .await;
    if let Some(m) = shape_outcome.measurement() {
        state
            .telemetry_store
            .record_shape_measurement(crate::telemetry_store::ShapeMeasurementRecord {
                repo: m.repo.clone(),
                rev: m.rev.clone(),
                spec_source: m.spec_source.clone(),
                findings_total: m.distance.findings_total,
                units_total: m.distance.units_total,
                units_conformant: m.distance.units_conformant,
                per_rule: m.per_rule.clone(),
                blocking_regressions: m.blocking_regressions,
                advisory_regressions: m.advisory_regressions,
                recorded_at: chrono::Utc::now(),
            })
            .await;
    }

    let cert_report = state.pre_merge_guard.evaluate_pre_merge_gates(
        diff_ctx,
        &doc_report,
        &cedar_report,
        &compliance_report,
        &api_contract_report,
        &cell_report,
        &supply_chain_report,
        &clean_arch_report,
        &monorepo_report,
        &debt_shrink_report,
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
        &schema_evolution_report,
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
        test_suite_passed,
        review_verdict,
        &shape_outcome,
    )?;

    Ok(cert_report)
}

/// The repository's own verification gate, run against `head_sha` itself.
///
/// `Some(true)`/`Some(false)` is a gate that ran to completion on this commit;
/// `None` is a gate that could not be attributed to it — the repository offers
/// none, no tree at `head_sha` could be produced, the tree that was produced is
/// not at `head_sha`, or the gate never completed — which the corpus records as
/// `NotMeasured` and which withholds the merge.
///
/// `Some(false)` is reserved for a gate that ran and reported failures, because
/// it is published on the pull request as "Test suite reported failures during
/// verification gate" and counted against the pull request in the approving
/// review. Everything else is a failure to measure, and this function does not
/// convert one into the other in either direction.
///
/// The tree is an ephemeral worktree at `head_sha`, which is the whole of the
/// correctness here — and it is *checked*, with `EphemeralWorktree::verify_at`,
/// rather than assumed from the argument that was passed in:
/// `create_ephemeral_worktree` falls back to `FETCH_HEAD` when the object is
/// not local, and `FETCH_HEAD` in the shared clone is whatever ref was fetched
/// last.
///
/// Run in the shared clone that
/// `ensure_repo_cloned` hands out, this gate builds whatever that clone is
/// currently on: nothing on the review or the certify path ever checks a pull
/// request head out into it (`ensure_repo_cloned` only fetches, and
/// `prepare_pr_diff` only fetches the pull ref), while `execute_pr_fix` runs
/// `git checkout -B pr-<N>` in it. So its outcome was the default branch's, or
/// the last PR the fixer touched — published as this pull request's
/// `test_suite_status`, counted in "N of 72 gates passed", and signed into a
/// formal GitHub APPROVE. A green default branch admitted a pull request that
/// does not compile; a clone left dirty by the fixer accused one nothing
/// measured. `QueueHealer::heal_ejected_pr` already ran this gate correctly, on
/// an ephemeral worktree at the head it had just produced; this is the same
/// mechanism, and the reason its result may be called a measurement of the
/// certified commit.
pub async fn local_verification_gate(
    git_mgr: &crate::git_manager::GitManager,
    repo: &str,
    pr_number: u64,
    head_sha: &str,
) -> Option<bool> {
    let worktree = match git_mgr
        .create_ephemeral_worktree(repo, pr_number, head_sha)
        .await
    {
        Ok(worktree) => worktree,
        Err(e) => {
            // `None`, not `Some(false)`: a tree that could not be produced is
            // not a suite that failed. `NotMeasured` withholds the merge and
            // names itself; a fabricated `Failed` would accuse the pull request
            // of something nothing ran.
            warn!(
                "No tree at {} for {}#{}, so the local verification gate was not measured: {:#}",
                head_sha, repo, pr_number, e
            );
            return None;
        }
    };

    // The tree is a tree; this is what makes it *this pull request's* tree.
    // `create_ephemeral_worktree` falls back to `FETCH_HEAD` in the shared
    // clone when the head object is not local, and `FETCH_HEAD` is whatever was
    // fetched last -- another pull request's head from a concurrent
    // `prepare_pr_diff`, or the base branch. Unchecked, the gate's answer would
    // be a measurement of a different commit published as this one's, counted
    // in the approving review and admitted by `admission_refusal`. `None`
    // again, for the same reason as above: a tree Anvil cannot prove is the
    // certified commit measures nothing about it.
    if let Err(e) = worktree.verify_at(head_sha).await {
        warn!(
            "The local verification gate for {}#{} was not measured: {:#}",
            repo, pr_number, e
        );
        if let Err(e) = worktree.cleanup().await {
            warn!(
                "Verification-gate worktree cleanup failed for {}#{}: {}",
                repo, pr_number, e
            );
        }
        return None;
    }

    let outcome = match crate::queue_healer::QueueHealer::run_local_test_gate(
        &worktree.worktree_path,
    )
    .await
    {
        crate::queue_healer::TestGate::Passed(_) => Some(true),
        crate::queue_healer::TestGate::Failed(_) => Some(false),
        // A gate that never completed is not a suite that failed, and this is
        // the arm that reaches a GitHub comment: `Some(false)` becomes
        // `GateStatus::Failed("Test suite reported failures during verification
        // gate.")` on the scorecard, with a remediation telling the contributor
        // to fix tests that were never run. `cargo` missing from the daemon's
        // PATH, the `ExecClass::Build` deadline expiring on a cold check, and
        // the worktree GC reaping this tree mid-build all arrive here.
        crate::queue_healer::TestGate::Errored(label, cause) => {
            warn!(
                "The local verification gate `{}` for {}#{} did not complete, so it was not \
                 measured: {}",
                label, repo, pr_number, cause
            );
            None
        }
        crate::queue_healer::TestGate::Unavailable => None,
    };

    if let Err(e) = worktree.cleanup().await {
        warn!(
            "Verification-gate worktree cleanup failed for {}#{}: {}",
            repo, pr_number, e
        );
    }
    outcome
}

/// The evidence an enlistment path holds, or the reason it holds none.
///
/// The CLI `enlist` subcommand, `POST /api/enlist` and the queue healer each
/// hand a pull request to the merge queue without having reviewed it. They may
/// not enlist on evidence they do not have, so they obtain it here: the same
/// corpus `execute_pr_review` runs, against the pull request's current head,
/// with the review verdict and the verification gate measured rather than
/// asserted absent.
///
/// `expected_head` is the commit the caller believes it is enlisting — the
/// healer knows it, because it just pushed it. GitHub's view of a PR head is
/// eventually consistent immediately after a push, so
/// `fetch_pr_metadata_at` waits a bounded while for the API to name that commit
/// and only then refuses: the race is tolerated, and certifying a *different*
/// head is still refused rather than reported.
///
/// A run that measured nothing is refused too. An empty diff certifies every
/// diff-scanning gate by default, so it is a cause here rather than a report.
///
/// `Err` is fail-closed and carries the cause: the caller turns it into the
/// refusal an operator reads, rather than leaving it in a server log.
pub async fn evidence_for_enlistment(
    state: &AppState,
    repo: &str,
    pr_number: u64,
    expected_head: Option<&str>,
) -> Result<PreMergeCertificationReport> {
    certify_for_enlistment(state, repo, pr_number, expected_head)
        .await
        .with_context(|| {
            format!(
                "pre-merge certification could not be obtained for {}#{}, so nothing may be \
                 enlisted for it",
                repo, pr_number
            )
        })
}

async fn certify_for_enlistment(
    state: &AppState,
    repo: &str,
    pr_number: u64,
    expected_head: Option<&str>,
) -> Result<PreMergeCertificationReport> {
    // Refused before the corpus is paid for, not after.
    //
    // A gate that cannot produce a measurement in this deployment makes every
    // report this function can return inadmissible, for every input, whatever
    // the pull request is -- and each of the three doors that calls it would
    // otherwise pay a full clone, seventy-two guards, a model turn, a cold
    // `cargo check` and a `git add -A` + `git commit` into the shared clone to
    // arrive at a refusal the configuration had already determined. That is the
    // objection this file makes to the previous hardcoded-verdict version a few
    // hundred lines below -- "a door that runs the most expensive operation in
    // the codebase to reach a constant is not a gate" -- and it applies to a
    // constant whose cause is configuration just as well as to one welded into
    // the source.
    //
    // The check is not a substitute for the gate: the review pipeline still
    // runs the corpus and still publishes `NotMeasured` for these gates on the
    // scorecard. This only stops the *enlist* doors, whose whole purpose is
    // admission, from doing futile work to reach a foregone refusal.
    if let Some(blockers) = crate::pre_merge_guard::unmeasurable_gates_in_this_build() {
        anyhow::bail!(
            "no report this deployment can produce would admit {}#{} to the merge queue, so the \
             certification corpus was not run for it: {}. Nothing about the pull request was \
             measured and nothing is claimed about it.",
            repo,
            pr_number,
            blockers
        );
    }

    // The corpus mutates the one shared clone `ensure_repo_cloned` hands out
    // per repository: `git fetch origin pull/N/head --force`, then `git add -A`
    // and `git commit` in that working tree. `execute_pr_review` holds this
    // lock across exactly those mutations for exactly that reason, so a second
    // corpus runner that took no lock would leave it serialising nothing —
    // this path would stage and commit a review run's half-written governance
    // output under its own message, and the review run would then log
    // "nothing to commit" as a benign no-op and certify a tree it did not
    // produce.
    //
    // Taken here rather than inside `certify_pull_request`: `execute_pr_review`
    // already holds it around that call and the mutex is not reentrant.
    //
    // Bounded. Everything else on this path has a deadline -- every child
    // process through `run_bounded`, the model turn through `--print-timeout`
    // -- and this acquisition was the one unbounded await in it, so a second
    // request for the same pull request parked behind the first for as long as
    // the first ran, with nothing to say why. The bound is generous enough to
    // queue behind one whole corpus run and short enough that a caller gets an
    // answer; expiring, it says what it was waiting for rather than reporting
    // anything about the pull request.
    let pr_lock = state.state_mgr.acquire_pr_lock(repo, pr_number).await;
    let _guard = tokio::time::timeout(PR_LOCK_WAIT, pr_lock.lock())
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "another run for {}#{} still held the per-pull-request lock after {}s, so this \
                 one measured nothing and certified nothing",
                repo,
                pr_number,
                PR_LOCK_WAIT.as_secs()
            )
        })?;

    info!(
        "Running pre-merge certification for {}#{} before merge queue admission...",
        repo, pr_number
    );
    let meta = state
        .github_client
        .fetch_pr_metadata_at(repo, pr_number, expected_head)
        .await
        .context("Failed to read pull request metadata before certification")?;

    let repo_dir = state
        .git_mgr
        .ensure_repo_cloned(repo)
        .await
        .context("Failed to ensure repo cloned before certification")?;
    let diff_ctx = state
        .git_mgr
        .prepare_pr_diff(
            repo,
            pr_number,
            &meta.base_ref_name,
            &meta.base_ref_oid,
            &meta.head_ref_oid,
            None,
        )
        .await
        .context("Failed to prepare PR diff context before certification")?;

    // A corpus with nothing to measure certifies nothing. `execute_pr_review`
    // has always refused an empty diff -- it returns before a single gate runs
    // -- and this entry point, shared by the three doors that never reviewed the
    // pull request, had no such refusal. Every diff-scanning guard reports a
    // clean pass over a zero-byte diff, and the run then stamps that with a
    // genuine provenance mark and a subject naming the real head: a fully
    // admissible report over zero measured lines, arriving through the front
    // door. `prepare_pr_diff` no longer swallows the fetch failure that made it
    // reachable; this refuses the state itself, which is the fact that matters
    // however it was arrived at.
    //
    // `bail`, not `return Ok(())`: the caller is about to enlist, so the
    // difference between "nothing to measure" and "everything passed" has to
    // reach it as a cause.
    if diff_ctx.diff_content.trim().is_empty() || diff_ctx.changed_files.is_empty() {
        anyhow::bail!(
            "the corpus has nothing to measure for {}#{} at {}: the diff against {} is {} byte(s) \
             and {} file(s) are listed as changed. A pull request whose changes cannot be read is \
             not a pull request whose gates passed.",
            repo,
            pr_number,
            meta.head_ref_oid,
            meta.base_ref_name,
            diff_ctx.diff_content.trim().len(),
            diff_ctx.changed_files.len()
        );
    }

    let body = meta.body.as_deref().unwrap_or("");

    // The two gates this path used to assert absent. Hardcoding them to
    // `VERDICT_ERRORED` and `None` made every report this function could ever
    // return inadmissible, for every input, in every configuration — after
    // paying a full clone, seventy-two guards and a commit in the shared clone
    // to arrive at a refusal that was fixed before the run started. A door that
    // runs the most expensive operation in the codebase to reach a constant is
    // not a gate. So they are measured, on the path that has to live with the
    // answer.
    //
    // The review is obtained but not submitted: this path is admitting a pull
    // request, not reviewing one, and the approving review it may go on to
    // publish is `MergeEnlister`'s to sign.
    let review_verdict = match state.reviewer.review_pr(&diff_ctx, &meta.title, body).await {
        Ok(review) => review.verdict,
        Err(e) => {
            // Fail-closed, and now for a reason that was measured: the review
            // was attempted for this head and did not complete. The corpus maps
            // this to `Errored`, which withholds the merge.
            warn!(
                "Code review could not complete for {}#{} during enlistment certification: {:#}",
                repo, pr_number, e
            );
            crate::reviewer::VERDICT_ERRORED.to_string()
        }
    };

    let test_suite_passed =
        local_verification_gate(&state.git_mgr, repo, pr_number, &meta.head_ref_oid).await;

    certify_pull_request(
        state,
        repo,
        pr_number,
        &meta.title,
        body,
        &meta.head_ref_oid,
        &repo_dir,
        &diff_ctx,
        &review_verdict,
        test_suite_passed,
    )
    .await
}
