//! Lane `enlist-doors`: each door is driven, not read.
//!
//! Issue #52. `tests/enlist_authority_test.rs` proves the merge queue entry
//! point behaviourally and traces every door's evidence expression back to a
//! certification run by reading production source. What no test did was *call*
//! a door. A door that reaches its `enlist_into_merge_queue` on one of two
//! branches, or hands over something that also traces to a certification run
//! but is not the evidence for this pull request, was decided by reading text.
//!
//! Three doors are driven here. Each is pointed at a pull request whose
//! certification cannot be obtained, and each must come back refused, with the
//! refusal matched against a reference the test generates rather than a
//! sentence it pastes.
//!
//! # Why no run of this file can touch a pull request
//!
//! `DOOR_REPO` names a repository that cannot exist -- a GitHub account name
//! may hold only ASCII alphanumerics and hyphens, and this owner holds an
//! underscore, so no account, and therefore no repository, can ever answer to
//! it. `certify_for_enlistment` reads pull request metadata through `gh` before
//! it clones anything, so the only subprocess these tests can spawn is a
//! read-only `gh pr view` that resolves nothing, bounded by
//! `crate::exec::run_bounded`. Every write `enlist_into_merge_queue` performs
//! -- `gh pr edit`, an APPROVE review, `gh pr merge --auto` -- sits after
//! `Self::admission_refusal(report)?` on its first line, and the report these
//! drives can produce is `None`.
//!
//! The deleted `regression_17_*` tests are not being restored. They named a
//! live repository and would have self-approved a production pull request
//! whenever they went red. What is restored is the question they asked.
//!
//! # What is driven, and what is not
//!
//! The fourth door is not driven. It sits at the end of `execute_pr_review`,
//! behind a clone, a fetch of the pull request, forty guards and a model turn,
//! and reaching it needs a repository -- which is the one thing these drives
//! must not have. `tests/enlist_authority_coverage_test.rs` runs the corpus
//! through `evaluate_pre_merge_gates` over a real diff, so it covers what that
//! door hands over, not the door. What covers the door is the provenance scan
//! in `tests/enlist_authority_test.rs`: structural, keyed to `KNOWN_DOOR_FILES`
//! so a door that moves fails the test rather than quietly leaving the corpus.
//!
//! The admitting direction of I1 is not asked here either. These drives can
//! only produce absent evidence, so a precondition that refused every pull
//! request in the fleet would satisfy all of them;
//! `a_fully_measured_and_certified_report_admits_the_pull_request` and the
//! admitting case of
//! `the_merge_queue_entry_point_refuses_the_evidence_it_was_handed` are what
//! stop that, on the seam these doors all call.
//!
//! # Parallelism
//!
//! Each test builds its own `StateManager` under its own temporary directory,
//! so the per-pull-request lock `certify_for_enlistment` acquires is never
//! contended across tests and its 3600-second wait can never be reached.

use std::path::Path;
use std::sync::Arc;

use anvil::config::Config;
use anvil::merge_enlister::MergeEnlister;
use anvil::webhook::AppState;

/// A repository no GitHub account can own.
///
/// The owner carries an underscore. GitHub account names are ASCII
/// alphanumerics and hyphens only, so this one is unregistrable, while
/// `webhook::repo_guard::is_syntactically_valid` -- which the `/api/enlist`
/// door applies before anything else -- accepts it. Both properties are
/// asserted by `the_repository_these_doors_are_pointed_at_cannot_exist`, so a
/// later editor cannot "tidy" the underscore away and quietly point three
/// enlist drives at a repository that resolves.
const DOOR_REPO: &str = "no_such_owner_/enlist-door-drive";

/// The pull request number every drive asks about. Nothing resolves it.
const DOOR_PR: u64 = 1;

/// A commit for the queue healer to say it pushed.
const HEALED_HEAD: &str = "0000000000000000000000000000000000000000";

/// Application state for a door drive, with every directory under `dir`.
///
/// `dir` is held because dropping the `TempDir` deletes the tree the state
/// writes into.
struct Doors {
    state: AppState,
    _dir: tempfile::TempDir,
}

/// Anvil's configuration for a door drive.
///
/// A struct literal rather than `Config::from_env`: `from_env` reads the
/// process environment and a `.env` file, so it would make these tests depend
/// on whatever the machine happens to export, and on each other.
fn config(dir: &Path) -> Config {
    Config {
        host: "127.0.0.1".to_string(),
        port: 0,
        // `/api/enlist` refuses an unwatched repository, and refuses it before
        // the door. Unwatched, that test would pass on the allowlist and never
        // reach the enlistment.
        watched_repos: vec![DOOR_REPO.to_string()],
        repos_dir: dir.join("repos"),
        data_dir: dir.join("data"),
        rules_path: None,
        agy_effort: "low".to_string(),
        auto_forward_webhooks: false,
        ai_provider: anvil::ai_driver::ModelProvider::default(),
        specific_model: None,
        webhook_secret: None,
        webhook_secret_previous: None,
        self_repo: anvil::config::SELF_REPO.to_string(),
    }
}

/// The state the three doors run against, built from the same production types
/// `main.rs` boots the daemon with.
///
/// Every field is the real type. Nothing here is a fake or a stub: what these
/// tests exercise is the production path, and a substitute for it would be a
/// second thing to trust. The daemon builds this only inside `main`, so it is
/// rebuilt here; a field added to `AppState` and not added here is a
/// compilation failure, which is the loud half of that duplication.
async fn doors() -> Doors {
    let dir = tempfile::tempdir().expect("temporary directory");
    let config = Arc::new(config(dir.path()));
    let git_mgr = Arc::new(anvil::git_manager::GitManager::new(
        config.repos_dir.clone(),
    ));
    let github_client = Arc::new(anvil::github::GitHubClient::new());
    let merge_enlister = Arc::new(MergeEnlister::new(github_client.clone()));
    let telemetry_store =
        Arc::new(anvil::telemetry_store::TelemetryStore::new(dir.path().join("telemetry")).await);
    let state = AppState {
        config: config.clone(),
        git_mgr: git_mgr.clone(),
        reviewer: Arc::new(anvil::reviewer::Reviewer::new(
            config.to_model_config(),
            config.rules_path.clone(),
        )),
        fixer: Arc::new(anvil::fixer::Fixer::new(
            git_mgr.clone(),
            github_client.clone(),
            config.agy_effort.clone(),
        )),
        doc_guard: Arc::new(anvil::doc_guard::DocGuard::new(config.agy_effort.clone())),
        incident_healer: Arc::new(anvil::incident_healer::IncidentHealer::new(
            config.agy_effort.clone(),
        )),
        unresolved_review_guard: Arc::new(
            anvil::unresolved_review_guard::UnresolvedReviewGuard::new(github_client.clone()),
        ),
        mainline_ci_healer: Arc::new(anvil::mainline_ci_healer::MainlineCiHealer::new(
            github_client.clone(),
        )),
        queue_healer: Arc::new(anvil::queue_healer::QueueHealer::new(
            git_mgr.clone(),
            github_client.clone(),
            merge_enlister.clone(),
            config.agy_effort.clone(),
        )),
        lockfile_reconciler: Arc::new(anvil::lockfile_reconciler::LockfileReconciler::new(
            git_mgr.clone(),
            github_client.clone(),
        )),
        ci_triager: Arc::new(anvil::ci_triager::CiTriager::new(
            github_client.clone(),
            config.agy_effort.clone(),
        )),
        state_mgr: Arc::new(
            anvil::state::StateManager::load(&config.data_dir)
                .await
                .expect("state manager loads under a temporary data directory"),
        ),
        fleet_observer: Arc::new(anvil::fleet_observer::FleetObserver::new(
            github_client.clone(),
            telemetry_store.clone(),
        )),
        pause: Arc::new(anvil::pause::Pause::in_dir(config.data_dir.clone())),
        cloud_native_guard: Arc::new(anvil::cloud_native_guard::CloudNativeGuard::new()),
        stack_whitelist_guard: Arc::new(anvil::stack_whitelist_guard::StackWhitelistGuard::new()),
        telemetry_store,
        merge_enlister,
        github_client,
        cedar_guard: Arc::new(anvil::cedar_guard::CedarGuard::new()),
        compliance_guard: Arc::new(anvil::compliance_guard::ComplianceGuard::new()),
        api_contract_guard: Arc::new(anvil::api_contract_guard::ApiContractGuard::new()),
        cell_isolation_guard: Arc::new(anvil::cell_isolation_guard::CellIsolationGuard::new()),
        supply_chain_guard: Arc::new(anvil::supply_chain_guard::SupplyChainGuard::new()),
        clean_arch_guard: Arc::new(anvil::clean_architecture_guard::CleanArchitectureGuard::new()),
        monorepo_guard: Arc::new(anvil::monorepo_guard::MonorepoGuard::new()),
        debt_shrink_guard: Arc::new(anvil::debt_shrink_guard::DebtShrinkGuard::new()),
        modularization_guard: Arc::new(anvil::modularization_guard::ModularizationGuard::new()),
        coverage_guard: Arc::new(anvil::coverage_guard::CoverageGuard::new()),
        rust_language_policy: Arc::new(anvil::rust_language_policy::RustLanguagePolicy::new()),
        kani_guard: Arc::new(anvil::kani_guard::KaniGuard::new()),
        slo_canary_guard: Arc::new(anvil::slo_canary_guard::SloCanaryGuard::new()),
        adr_drift_ratchet: Arc::new(anvil::adr_drift_ratchet::AdrDriftRatchet::new()),
        shuffle_shard_simulator: Arc::new(
            anvil::shuffle_shard_simulator::ShuffleShardSimulator::new(),
        ),
        trace_context_guard: Arc::new(anvil::trace_context_guard::TraceContextGuard::new()),
        constant_work_guard: Arc::new(anvil::constant_work_guard::ConstantWorkGuard::new()),
        idempotency_guard: Arc::new(anvil::idempotency_guard::IdempotencyGuard::new()),
        finops_ratchet: Arc::new(anvil::finops_ratchet::FinOpsUnitCostRatchet::new()),
        ghost_migration_harness: Arc::new(
            anvil::ghost_migration_harness::GhostMigrationHarness::new(),
        ),
        gitops_promotion_engine: Arc::new(anvil::gitops_promotion::GitOpsPromotionEngine::new()),
        gitops_drift_reconciler: Arc::new(
            anvil::gitops_drift_reconciler::GitOpsDriftReconciler::new(),
        ),
        canary_rollout_guard: Arc::new(anvil::canary_rollout::CanaryRolloutGuard::new()),
        cluster_state_auditor: Arc::new(anvil::cluster_state_auditor::ClusterStateAuditor::new()),
        migration_orchestrator: Arc::new(
            anvil::migration_orchestrator::MigrationLifecycleOrchestrator::new(),
        ),
        ci_wallclock_ratchet: Arc::new(
            anvil::ci_wallclock_ratchet::CiWallclockEconomicsRatchet::new(),
        ),
        predictive_test_selector: Arc::new(
            anvil::predictive_test_selector::PredictiveTestSelector::new(),
        ),
        compile_time_profiler: Arc::new(anvil::compile_time_profiler::CompileTimeProfiler::new()),
        remote_cache_optimizer: Arc::new(anvil::remote_cache_optimizer::RemoteCacheOptimizer::new()),
        ci_runner_economics: Arc::new(anvil::ci_runner_economics::CiRunnerEconomicsOptimizer::new()),
        early_exit_cascade: Arc::new(anvil::early_exit_cascade::EarlyExitCascadeGuard::new()),
        flake_cost_dampener: Arc::new(anvil::flake_cost_dampener::FlakeCostDampener::new()),
        ephemeral_sandbox: Arc::new(anvil::ephemeral_sandbox::EphemeralSandboxManager::new()),
        cross_service_impact: Arc::new(anvil::cross_service_impact::CrossServiceImpactEngine::new()),
        ephemeral_secrets: Arc::new(anvil::ephemeral_secrets::EphemeralSecretInjector::new()),
        psa_admission_guard: Arc::new(anvil::psa_admission_guard::PsaAdmissionGuard::new()),
        shadow_traffic_harness: Arc::new(anvil::shadow_traffic_harness::ShadowTrafficHarness::new()),
        flake_bisector: Arc::new(anvil::flake_bisector::FlakeBisectorEngine::new()),
        local_inner_loop: Arc::new(anvil::local_inner_loop::LocalInnerLoopProbe::new()),
        semantic_abi_ratchet: Arc::new(anvil::semantic_abi_ratchet::SemanticAbiRatchet::new()),
        incident_sentry: Arc::new(anvil::incident_sentry::IncidentSentryCircuitBreaker::new()),
        preview_env_reaper: Arc::new(anvil::preview_env_reaper::PreviewEnvReaper::new()),
        review_memory: Arc::new(anvil::review_memory::ReviewMemoryEngine::new()),
        zero_day_patcher: Arc::new(anvil::zero_day_patcher::ZeroDayAutoPatcher::new()),
        formal_verification: Arc::new(anvil::formal_verification::FormalVerificationGuard::new()),
        deadlock_analyzer: Arc::new(anvil::deadlock_analyzer::DeadlockStaticAnalyzer::new()),
        automated_canary: Arc::new(anvil::automated_canary::AutomatedCanaryAnalysis::new()),
        progressive_rollout: Arc::new(
            anvil::progressive_rollout::ProgressiveRingOrchestrator::new(),
        ),
        hermetic_build: Arc::new(anvil::hermetic_build::HermeticBuildValidator::new()),
        vex_scanner: Arc::new(anvil::vex_scanner::OpenVexReachabilityScanner::new()),
        cosign_signer: Arc::new(anvil::cosign_signer::CosignProvenanceSigner::new()),
        chaos_injector: Arc::new(anvil::chaos_injector::ChaosFaultInjector::new()),
        stacked_diffs: Arc::new(anvil::stacked_diffs::StackedDiffsOrchestrator::new()),
        microbenchmark_ratchet: Arc::new(
            anvil::microbenchmark_ratchet::MicroBenchmarkRatchet::new(),
        ),
        jittered_backoff: Arc::new(anvil::jittered_backoff::JitteredBackoffGuard::new()),
        schema_evolution: Arc::new(anvil::schema_evolution::SchemaEvolutionRatchet::new()),
        auto_rollback: Arc::new(anvil::auto_rollback::AutoRollbackPostmortemEngine::new()),
        wasm_sandbox: Arc::new(anvil::wasm_sandbox::WasmPolicySandbox::new()),
        consistency_guard: Arc::new(anvil::consistency_guard::ActiveActiveConsistencyGuard::new()),
        flake_quarantine: Arc::new(anvil::flake_quarantine::FlakeQuarantineLifecycle::new()),
        carbon_aware: Arc::new(anvil::carbon_aware::CarbonAwareComputeRatchet::new()),
        replay_harness: Arc::new(anvil::replay_harness::DeterministicReplayHarness::new()),
        chaos_mutation_guard: Arc::new(anvil::chaos_mutation_guard::ChaosMutationGuard::new()),
        feature_flag_ratchet: Arc::new(anvil::feature_flag_ratchet::FeatureFlagRatchet::new()),
        criterion_bench_ratchet: Arc::new(
            anvil::criterion_bench_ratchet::CriterionBenchRatchet::new(),
        ),
        attestation_guard: Arc::new(anvil::attestation_guard::AttestationGuard::new()),
        pre_merge_guard: Arc::new(anvil::pre_merge_guard::PreMergeGuard::new()),
        metrics: Arc::new(anvil::metrics::PrometheusRegistry::new()),
        self_governor: Arc::new(anvil::self_governance::SelfGovernor::new()),
        broadcaster: Arc::new(anvil::webhook::sse::FleetEventBroadcaster::new()),
    };
    Doors { state, _dir: dir }
}

/// Every message in an `anyhow` error chain, outermost first.
fn chain(error: &anyhow::Error) -> Vec<String> {
    error.chain().map(|c| c.to_string()).collect()
}

/// The refusal `enlist_into_merge_queue` gives for evidence it never received.
///
/// Generated, not pasted. A door that reaches the entry point with something
/// other than the certification for this pull request -- a hand-built report, a
/// stale one, an optimistic default -- is also refused, with different words.
/// Matching a chain link against this exact string is what tells the two apart;
/// asking whether the chain merely mentions a refusal cannot.
fn refusal_for_absent_evidence() -> String {
    MergeEnlister::admission_refusal(None)
        .expect_err("absent evidence is never a pass")
        .to_string()
}

/// The refusal `evidence_for_enlistment` gives for this pull request, obtained
/// by asking it.
///
/// The queue healer `?`s this error out under its own context, so the outermost
/// link of it must appear in what the healer returns. Generated for the same
/// reason as `refusal_for_absent_evidence`; only that link is compared, because
/// the cause beneath it is whatever `gh` said and that is not the subject here.
async fn refusal_from_the_certification_run(state: &AppState) -> String {
    anvil::webhook::pipelines::certify::evidence_for_enlistment(
        state,
        DOOR_REPO,
        DOOR_PR,
        Some(HEALED_HEAD),
    )
    .await
    .expect_err("no certification can be obtained for a repository that cannot exist")
    .to_string()
}

/// The safety property every drive in this file rests on, asserted rather than
/// assumed.
#[test]
fn the_repository_these_doors_are_pointed_at_cannot_exist() {
    let (owner, name) = DOOR_REPO
        .split_once('/')
        .expect("the door repository is an owner/name pair");
    assert!(
        !name.contains('/'),
        "DOOR_REPO must be two segments; `/api/enlist` rejects anything else \
         before it reaches the door, and a drive that never reaches the door \
         proves nothing about it"
    );
    assert!(
        owner.contains('_'),
        "DOOR_REPO's owner must carry an underscore. That is the whole safety \
         argument of this file: GitHub account names are ASCII alphanumerics \
         and hyphens, so an owner with an underscore can never be registered \
         and these drives can never reach a live pull request. Owner today: \
         {owner:?}"
    );
    assert!(
        anvil::webhook::repo_guard::is_syntactically_valid(DOOR_REPO),
        "DOOR_REPO must pass the syntactic check `/api/enlist` applies first, \
         or that door's drive stops at a 400 and never reaches the enlistment"
    );
}

/// The CLI door: `anvil enlist --repo <r> --pr <n>`.
///
/// The certification cannot be obtained, so the door hands `None` over and the
/// entry point refuses. Asserting the exact refusal is what makes this a check
/// on the door rather than on the machine: a drive that failed because `gh` is
/// absent, or because the fixture is broken, produces a different chain and
/// fails.
#[tokio::test]
async fn the_cli_enlist_door_refuses_a_pull_request_it_could_not_certify() {
    let doors = doors().await;
    let outcome = anvil::cli::enlist::enlist(&doors.state, DOOR_REPO, DOOR_PR).await;
    let error = outcome
        .expect_err("the CLI enlist door obtained no certification and must not enlist on none");
    let links = chain(&error);
    assert!(
        links.contains(&refusal_for_absent_evidence()),
        "the CLI enlist door did not refuse for want of evidence. Its error \
         chain was {links:?}, which does not carry the refusal \
         `enlist_into_merge_queue` gives for `None`: {:?}",
        refusal_for_absent_evidence()
    );
}

/// The `POST /api/enlist` door, called as axum calls it.
///
/// Two claims, and the second is the one a source scan kept being asked for:
/// the handler refuses, and it *answers* the refusal. An enlistment that did
/// not happen must not come back as `success: true` with a 200.
#[tokio::test]
async fn the_api_enlist_door_answers_the_refusal_rather_than_reporting_success() {
    use axum::response::IntoResponse;

    let doors = doors().await;
    let response = anvil::webhook::manual_handlers::manual_enlist_handler(
        axum::extract::State(doors.state.clone()),
        axum::Json(anvil::webhook::manual_handlers::ManualEnlistRequest {
            repo: DOOR_REPO.to_string(),
            pr_number: DOOR_PR,
        }),
    )
    .await
    .into_response();

    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("the enlist API answers with a body");
    let answer: anvil::webhook::ApiResponse =
        serde_json::from_slice(&body).expect("the enlist API answers with an ApiResponse");

    assert_ne!(
        status,
        axum::http::StatusCode::OK,
        "`/api/enlist` answered 200 for an enlistment that was refused. Body: {}",
        answer.message
    );
    assert!(
        !answer.success,
        "`/api/enlist` reported success for an enlistment it did not perform. \
         Body: {}",
        answer.message
    );
    assert!(
        answer.message.contains(&refusal_for_absent_evidence()),
        "`/api/enlist` refused, but not with the refusal the merge queue entry \
         point gives for absent evidence, so what it refused for is not \
         established. It answered {status}: {:?}",
        answer.message
    );
}

/// The queue healer's re-enlist door.
///
/// This one refuses one step earlier than the other two: it `?`s the
/// certification out rather than converting it to `None`, so what must appear
/// in its chain is the certification run's own refusal. A door that fell back
/// to some other report would reach the entry point instead, and carry that
/// refusal rather than this one.
///
/// What this drive cannot reach, because the `?` exits first, is the healer
/// swallowing a refusal the entry point gave it for a certification it *did*
/// obtain -- which needs a corpus run and so needs a repository. That class is
/// covered structurally, by `no_path_drops_a_merge_queue_refusal_on_the_floor`.
#[tokio::test]
async fn the_queue_healer_reenlist_door_refuses_a_heal_it_could_not_certify() {
    let doors = doors().await;
    let expected = refusal_from_the_certification_run(&doors.state).await;
    let outcome = doors
        .state
        .queue_healer
        .certify_and_reenlist(&doors.state, DOOR_REPO, DOOR_PR, HEALED_HEAD)
        .await;
    let error = outcome.expect_err(
        "a heal whose commit could not be certified must not be handed back to the merge queue",
    );
    let links = chain(&error);
    assert!(
        links.contains(&expected),
        "the queue healer's re-enlist door did not refuse for want of a \
         certification. Its error chain was {links:?}, which does not carry \
         {expected:?}"
    );
    assert!(
        !links.contains(&refusal_for_absent_evidence()),
        "the queue healer reached the merge queue entry point carrying \
         something, for a heal it could not certify. Chain: {links:?}"
    );
}
