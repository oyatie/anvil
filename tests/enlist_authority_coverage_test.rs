//! Lane `enlist-authority`, second suite: what the spec suite does not cover.
//!
//! `tests/enlist_authority_test.rs` is the specification for issues #17 and
//! #18 and was written before the implementation. This file is written after
//! it, and deliberately does not repeat it. It adds four things:
//!
//! 1. **Regression.** One test per filed defect, reproducing the *original*
//!    broken behaviour as the thing that must never come back — for #17, each
//!    of the three doors that used to admit a pull request with no evidence at
//!    all, in the shape that door actually holds its evidence; for #18, the
//!    fixed "100% compliance" sentence reaching a published body.
//!
//! 2. **Integration.** The spec suite builds every report by hand, through
//!    `from_gate_outcomes`. The tests below run the corpus: fifty-nine real
//!    guards over a real diff, assembled by the real `evaluate_pre_merge_gates`
//!    — statuses, verdict, unmeasured list, provenance mark, subject and
//!    rendered matrix all derived by production code — and then handed to the
//!    real admission decision and the real publishers. What is pinned is the
//!    wiring between them.
//!
//! 3. **Boundary.** The symmetric half of the same invariant, which the
//!    codebase states in its own comments ("a fabricated `Failed` would
//!    accuse") and the spec suite does not test: a gate that produced no
//!    measurement must not be published in the words of one that found a
//!    defect, and a gate whose fix exists only in Anvil's clone must not be
//!    counted among the gates that passed on the commit being merged.
//!
//! 4. **Corpus integrity.** The change added two non-gate fields to the report
//!    (`provenance`, `subject`). The corpus is the authority for every
//!    published count, so this pins that it is still exactly `TOTAL_GATES`
//!    gates, that the names the report publishes are the fields it declares,
//!    and that the two new fields did not join it.
//!
//! # What the corpus can and cannot produce in this build
//!
//! A report the corpus produces is not admissible in this tree, for reasons
//! that are facts about the deployment rather than about any pull request: ten
//! gates have no data source configured and report `NotMeasured`, and
//! `brand_absence_status` scans Anvil's own `src/` and reports `Failed` on the
//! naming debt recorded there. So the two *admitted* shapes — everything
//! measured, and certified-but-not-admissible — cannot be reached by running
//! the corpus, whatever the pull request is.
//!
//! They are reached instead by taking the statuses a real corpus run produced
//! and handing them back to `from_gate_outcomes` with the unmeasured ones
//! answered, which is what a build with those data sources wired would produce.
//! `as_if_every_gate_had_a_data_source` is that step, in one place, and every
//! test that uses it says so.
//!
//! Nothing here touches the network, `gh`, the clock, or any path outside the
//! repository and a private temporary directory: every test may run in
//! parallel with every other.

use anvil::git_manager::PrDiffContext;
use anvil::github::GitHubClient;
use anvil::merge_enlister::MergeEnlister;
use anvil::pre_merge_guard::PreMergeGuard;
use anvil::pre_merge_guard::matrix::MatrixRenderer;
use anvil::pre_merge_guard::report::{GateStatus, PreMergeCertificationReport, TOTAL_GATES};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The merge strategy the enlistment note is written about. Held constant so
/// that any difference between two notes comes from the report.
const STRATEGY: &str = "Squash & Merge";

/// The sentence `ensure_approving_review` used to sign onto every pull request
/// in the fleet, verbatim from `src/merge_enlister.rs:207-216` as issue #18
/// quotes it, and the one `post_enlistment_note` used to follow it with.
///
/// These are the artefacts, not a wording policy: the fix is free to say
/// anything it can derive, and these two sentences are what it may never say,
/// because nothing measured them. They are quoted here rather than referenced
/// from production precisely because production no longer contains them.
const THE_ORIGINAL_APPROVAL_CLAIM: &str = "All automated review, documentation parity, clean architecture, and safety gates have passed with 100% compliance. Certified for merge queue admission.";
const THE_ORIGINAL_ENLISTMENT_CLAIM: &str = "Pre-Merge Certification 100% Green";

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
fn published_texts(report: Option<&PreMergeCertificationReport>) -> Vec<(&'static str, String)> {
    [
        ("approval_summary", MergeEnlister::approval_summary(report)),
        (
            "enlistment_note",
            MergeEnlister::enlistment_note(report, STRATEGY),
        ),
    ]
    .into_iter()
    .filter_map(|(seam, text)| text.map(|t| (seam, t)))
    .collect()
}

/// Whether `n` appears in `text` as a number in its own right, so a claim about
/// three gates is not answered by the "3" inside "23".
fn mentions_number(text: &str, n: usize) -> bool {
    let n = n.to_string();
    text.split(|c: char| !c.is_ascii_digit()).any(|t| t == n)
}

/// The deepest cause of an error chain: what actually went wrong, under
/// whatever context the callers added on the way out.
fn root_cause(err: &anyhow::Error) -> String {
    err.chain()
        .last()
        .expect("an error has a cause")
        .to_string()
}

fn an_enlister() -> MergeEnlister {
    MergeEnlister::new(Arc::new(GitHubClient::new()))
}

// =========================================================================
// Issue #17 — regression, one per door that used to admit on no evidence
// =========================================================================

/// The shared obligation each door regression checks.
///
/// `evidence` is the value that door holds after trying to obtain a
/// certification report, in the shape that door holds it. The door must refuse,
/// the refusal must say why, and the refusal must be the *first* thing that
/// goes wrong: the original defect was not only that these paths admitted an
/// uncertified pull request but that they acted on the way — `gh pr edit`
/// rewrites the body, `submit_pr_review` signs a formal APPROVE — before
/// anything asked whether the pull request could be admitted at all. A refusal
/// that arrives after those has already published.
async fn assert_the_door_refuses(
    evidence: Option<&PreMergeCertificationReport>,
    door: &str,
) -> String {
    let expected = MergeEnlister::admission_refusal(evidence)
        .expect_err("fixture sanity: this evidence cannot admit a pull request")
        .to_string();

    let err = match an_enlister()
        .enlist_into_merge_queue("oyatie/anvil", 1, evidence)
        .await
    {
        Ok(()) => panic!(
            "{door} admitted a pull request to the merge queue on evidence it \
             does not have. This is issue #17 exactly: the door enlisted, and \
             nothing about the pull request had been measured."
        ),
        Err(e) => e,
    };

    assert!(
        !err.to_string().trim().is_empty(),
        "{door} refused and said nothing: a silent refusal is the no-op the \
         defect already was, with an error type on it"
    );
    assert_eq!(
        root_cause(&err),
        expected,
        "{door} was handed evidence that cannot admit a pull request and the \
         first thing that went wrong was something else. The admission decision \
         has to be reached before the pull request is touched: everything past \
         it — the title and body reconciliation, the approving review, \
         `gh pr merge --auto` — is written onto a pull request that is not going \
         through.\n  expected the refusal: {expected}\n  got: {err:?}"
    );
    expected
}

/// REGRESSION, issue #17, door 2 of 4 — the CLI `enlist` subcommand.
///
/// `Commands::Enlist` called `enlist_into_merge_queue(&repo, pr)` with nothing
/// in front of it. It now runs the certification corpus first and hands over
/// `evidence.as_ref().ok()`, so a certification that could not be obtained is
/// not an error this door can shrug off: `None` is the value it passes, and
/// `None` must refuse. The `Err` here is what `evidence_for_enlistment` returns
/// when no report could be obtained — the state the original door was
/// permanently in, because it never asked for one.
#[tokio::test]
async fn regression_17_the_cli_enlist_subcommand_does_not_admit_without_a_report() {
    let could_not_certify: anyhow::Result<PreMergeCertificationReport> = Err(anyhow::anyhow!(
        "pre-merge certification could not be obtained for oyatie/anvil#1: the repository \
         could not be cloned"
    ));
    assert_the_door_refuses(
        could_not_certify.as_ref().ok(),
        "the CLI `enlist` subcommand",
    )
    .await;
}

/// REGRESSION, issue #17, door 3 of 4 — `POST /api/enlist`.
///
/// The handler spawned the enlistment detached, discarded the result and
/// answered `202 ACCEPTED` regardless, so a refusal reached a log line inside a
/// dropped task and never the person who asked for the merge. Two obligations
/// here, and the second is this door's own: the enlistment must refuse, and the
/// refusal must be *observable* — the value the handler derives its status code
/// and its `success` field from has to be the outcome, not a constant.
#[tokio::test]
async fn regression_17_the_enlist_api_does_not_admit_or_report_success_without_a_report() {
    let could_not_certify: anyhow::Result<PreMergeCertificationReport> = Err(anyhow::anyhow!(
        "pre-merge certification could not be obtained for oyatie/anvil#1: the pull request \
         head could not be read"
    ));
    let evidence = could_not_certify.as_ref().ok();
    let why_no_report = could_not_certify.as_ref().err().map(|e| format!("{e:#}"));

    let refusal = assert_the_door_refuses(evidence, "`POST /api/enlist`").await;

    // The handler's own derivation, as `manual_enlist_handler` writes it:
    // `let enlisted = outcome.is_ok();`. Anything constant here is the 202.
    let outcome = an_enlister()
        .enlist_into_merge_queue("oyatie/anvil", 1, evidence)
        .await;
    assert!(
        outcome.is_err(),
        "`POST /api/enlist` reported an enlistment that did not happen. The \
         original handler answered 202 ACCEPTED for every request because the \
         outcome was discarded, so a caller could not tell a merge queue \
         admission from a refusal."
    );
    assert!(
        why_no_report.is_some_and(|why| !why.trim().is_empty()),
        "the answer must carry the reason no report was obtained: \"no report\" \
         with no cause tells an operator nothing they can act on. The refusal \
         itself was: {refusal}"
    );
}

/// REGRESSION, issue #17, door 4 of 4 — the queue healer's re-enlist.
///
/// The healer pushed a merge commit and put the pull request straight back into
/// the queue. The only thing in front of it was a local `cargo check`, and
/// issue #17 says why that is not certification: one gate's worth of evidence
/// about seventy-two.
///
/// So the evidence here is exactly what the healer used to have — the local
/// verification gate passed, and nothing else was measured — and the door must
/// still refuse. A door that admitted this would satisfy every "no report at
/// all" test in this lane while re-enlisting on precisely the evidence the
/// issue was filed about.
#[tokio::test]
async fn regression_17_a_green_local_test_gate_is_not_certification_for_the_queue_healer() {
    let cargo_check_was_clean: Vec<(&str, GateStatus)> = gate_names()
        .into_iter()
        .map(|gate| {
            let status = if gate == "test_suite_status" {
                GateStatus::Passed
            } else {
                not_measured(gate)
            };
            (gate, status)
        })
        .collect();
    let report = PreMergeCertificationReport::from_gate_outcomes(&cargo_check_was_clean)
        .expect("the corpus is covered; every gate reports what it measured");
    assert!(
        report.is_certified_ready,
        "fixture sanity: `NotMeasured` is individually acceptable, so a corpus \
         that measured one gate still certifies. That is why the healer's \
         `cargo check` looked like enough"
    );

    let refusal = assert_the_door_refuses(Some(&report), "the queue healer's re-enlistment").await;
    assert!(
        refusal.contains("doc_parity_status") || mentions_number(&refusal, TOTAL_GATES - 1),
        "the refusal must name the gates that produced no measurement, or say how \
         many did: the healer is autonomous, and the note it posts is all an \
         operator sees. Got: {refusal}"
    );
}

// =========================================================================
// Issue #18 — regression: the fabricated blanket claim
// =========================================================================

/// REGRESSION, issue #18 — the fixed compliance sentence cannot reach a
/// published body, for any report, anywhere in the corpus.
///
/// The defect was one string literal in a function that received no report, so
/// it was signed onto every pull request whatever its gates did. The sweep is
/// the whole corpus, one gate at a time, in each of the five states a gate can
/// be in other than an outright pass: whatever Anvil publishes for any of them,
/// it is not that sentence and not the enlistment note's version of it.
///
/// The sweep matters rather than one case: a fix that derives the text for the
/// reports a test happens to name, and falls back to the old constant for the
/// rest, is green under a single fixture and is still the defect for the other
/// seventy-one gates.
#[test]
fn regression_18_no_report_publishes_the_fabricated_compliance_claim() {
    let mut published = 0usize;
    for gate in gate_names() {
        for state in [
            GateStatus::Failed("a defect was found".to_string()),
            GateStatus::Errored("the probe could not run".to_string()),
            not_measured(gate),
            GateStatus::Warning("worth a look".to_string()),
            GateStatus::AutoUpdated,
        ] {
            let report = report_with(&[(gate, state.clone())]);
            for (seam, text) in published_texts(Some(&report)) {
                published += 1;
                for claim in [THE_ORIGINAL_APPROVAL_CLAIM, THE_ORIGINAL_ENLISTMENT_CLAIM] {
                    assert!(
                        !text.contains(claim),
                        "`{seam}` published the sentence issue #18 was filed about \
                         while `{gate}` reported `{}`. It is a literal: nothing in \
                         the report says it, and it goes onto the pull request \
                         permanently.\n  claim: {claim}\n  text was:\n{text}",
                        state.badge()
                    );
                }
            }
        }
    }
    // Both publishers withhold for a report that cannot be admitted, so most of
    // the sweep publishes nothing at all. `Warning` and `AutoUpdated` are
    // acceptable and measured, so those reports are admitted and do publish —
    // and if that ever stops being true, this test is asserting over an empty
    // set while reading as coverage.
    assert!(
        published > 0,
        "the sweep published nothing at all, so it checked nothing. Every report \
         in it was withheld, which means the case where the claim can actually \
         reach a pull request is no longer covered"
    );
}

/// REGRESSION, issue #18 — the claim is not merely absent, it is derived.
///
/// Deleting the sentence and publishing an empty body passes the test above and
/// is not the fix: issue #18 asks for text derived from the real report, or for
/// no self-approval at all. So for a report the corpus certified, what is
/// published must carry the count that report actually produced, and it must
/// move when the report moves.
#[test]
fn regression_18_what_is_published_carries_the_count_the_report_produced() {
    let clean = every_gate_passing();
    let texts = published_texts(Some(&clean));
    assert!(
        !texts.is_empty(),
        "nothing was published for a fully certified pull request. Publishing \
         nothing is honest, and issue #18's second option is to drop the \
         self-approval entirely — but then this suite is asserting over an empty \
         set, so delete this test along with the self-approval rather than \
         leaving it green and blind."
    );
    for (seam, text) in &texts {
        assert!(
            mentions_number(text, TOTAL_GATES),
            "`{seam}` published a claim about a fully certified pull request \
             without the number of gates behind it. {TOTAL_GATES} gates were \
             measured and a reader cannot tell that from:\n{text}"
        );
    }

    // One gate moved, and the same seams must say something different. A
    // constant cannot, and neither can `gate_counts()`, which scores a warning
    // as acceptable and reports the whole corpus as passing for both.
    let warned = report_with(&[(
        "bench_status",
        GateStatus::Warning("throughput dipped".into()),
    )]);
    assert_eq!(
        warned.gate_counts().0,
        TOTAL_GATES,
        "fixture sanity: the ready-made count scores this report at the whole \
         corpus, which is what makes publishing it a claim nothing measured"
    );
    let moved = published_texts(Some(&warned));
    assert_eq!(
        moved.len(),
        texts.len(),
        "a pull request with a warned gate is admitted just like a clean one, so \
         the same seams must publish for it. Going silent on the one report whose \
         text carries an obligation is that obligation dodged rather than met"
    );
    for ((seam, clean_text), (_, warned_text)) in texts.iter().zip(moved.iter()) {
        assert_ne!(
            clean_text, warned_text,
            "`{seam}` published the same text for a corpus that passed outright \
             and one with a gate that did not. That is the defect: a sentence \
             that does not move when the evidence moves was not derived from it"
        );
    }
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
const A_PULL_REQUEST: (&str, u64, &str) = (
    "oyatie/anvil",
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
    let rust_skills_report = anvil::rust_language_policy::RustLanguagePolicy::new(dir)
        .evaluate_rust_quality(dir, d)
        .expect("the rust language policy reads the diff");
    let kani_report = anvil::kani_guard::KaniGuard::new()
        .evaluate_unsafe_invariants(dir, d)
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
    let cosign_report = anvil::cosign_signer::CosignProvenanceSigner::new()
        .generate_cosign_attestation(&d.head_sha);
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

/// The statuses a real corpus run produced, with the gates this build has no
/// data source for answered, handed back to the constructor that consumes gate
/// outcomes.
///
/// This is the counterfactual the admitted shapes need and the corpus cannot
/// reach (see the module docs): ten gates report `NotMeasured` because nothing
/// in this deployment can measure them, and `brand_absence_status` reports
/// `Failed` about Anvil's own `src/` rather than about the pull request.
/// Everything else is preserved — every gate that did measure this change keeps
/// exactly the status its guard gave it, and the report is sealed by production
/// code.
///
/// `leave_unmeasured` names gates to leave as they are, so a test can hold one
/// gate at "no measurement" while the rest of the corpus is answered.
fn as_if_every_gate_had_a_data_source(
    report: &PreMergeCertificationReport,
    leave_unmeasured: &[&str],
) -> PreMergeCertificationReport {
    let outcomes: Vec<(&str, GateStatus)> = report
        .named_statuses()
        .into_iter()
        .map(|(gate, status)| {
            if leave_unmeasured.contains(&gate) {
                return (gate, status.clone());
            }
            let answered = match status {
                GateStatus::NotMeasured { .. } => GateStatus::Passed,
                // Anvil's own naming debt, which the corpus reports against
                // every pull request in the fleet. It is not a measurement of
                // this change, so it is answered here with the rest.
                GateStatus::Failed(_) if gate == "brand_absence_status" => GateStatus::Passed,
                other => other.clone(),
            };
            (gate, answered)
        })
        .collect();
    PreMergeCertificationReport::from_gate_outcomes(&outcomes)
        .expect("the statuses of a real run cover the corpus, because a run covers it")
}

/// INTEGRATION — a change moves through certification, and the merge queue
/// answers for the report that run produced.
///
/// Three properties, all about the wiring rather than about any one gate, so
/// they hold whatever the guards say about this diff:
///
///   - the report names the pull request and the commit it was measured
///     against, which is what the enlistment pins the merge to;
///   - `unmeasured_gates` is exactly the gates that reported no measurement,
///     so the field the refusal is written from cannot drift from the statuses
///     it summarises;
///   - the door's answer and the publishers' answers are the same answer. A
///     refusal that publishes anyway signs Anvil's name onto a change that is
///     not going through; an admission that publishes nothing is the
///     self-approval quietly deleted.
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

    let reported_no_measurement: Vec<String> = report
        .named_statuses()
        .into_iter()
        .filter(|(_, status)| matches!(status, GateStatus::NotMeasured { .. }))
        .map(|(gate, _)| gate.to_string())
        .collect();
    assert_eq!(
        report.unmeasured_gates, reported_no_measurement,
        "the list the refusal is written from is not the list of gates that \
         produced no measurement"
    );

    match MergeEnlister::admission_refusal(Some(&report)) {
        Err(refusal) => {
            let refusal = refusal.to_string();
            let absent_or_failing: Vec<&str> = report
                .named_statuses()
                .into_iter()
                .filter(|(_, s)| !s.is_acceptable() || matches!(s, GateStatus::NotMeasured { .. }))
                .map(|(gate, _)| gate)
                .collect();
            assert!(
                absent_or_failing.iter().any(|gate| refusal.contains(gate)),
                "the merge was withheld and the refusal names no gate that is \
                 actually absent or failing in the report it was written from, so \
                 an operator has nothing to act on.\n  refusal: {refusal}\n  \
                 gates: {absent_or_failing:?}"
            );
            assert!(
                published_texts(Some(&report)).is_empty(),
                "Anvil endorsed a pull request the same report refuses to admit"
            );
        }
        Ok(()) => {
            assert!(
                !published_texts(Some(&report)).is_empty(),
                "a pull request the corpus certified was admitted and endorsed by \
                 nothing at all"
            );
        }
    }
}

/// INTEGRATION — everything measured: the corpus certifies, the door admits,
/// and what Anvil publishes is the count that run produced.
///
/// Uses `as_if_every_gate_had_a_data_source`, because this build cannot reach
/// the shape any other way (module docs). Everything the guards did measure
/// about this change survives into the report unchanged, and the count that
/// reaches GitHub is computed from the statuses rather than asserted here.
#[test]
fn a_run_in_which_every_gate_measured_is_admitted_and_endorsed_on_what_it_measured() {
    let work = a_working_tree();
    let measured = as_if_every_gate_had_a_data_source(
        &report_from_the_corpus(work.path(), Some(true), "APPROVE"),
        &[],
    );

    assert!(
        measured.is_certified_ready,
        "a corpus in which every gate measured and none found a defect must \
         certify; the gates that did not simply pass were: {:?}",
        measured
            .named_statuses()
            .into_iter()
            .filter(|(_, s)| !matches!(s, GateStatus::Passed))
            .map(|(g, s)| format!("{g}: {}", s.badge()))
            .collect::<Vec<_>>()
    );
    MergeEnlister::admission_refusal(Some(&measured))
        .expect("a fully measured, fully certified report admits the pull request");

    let clean_passes = measured
        .named_statuses()
        .into_iter()
        .filter(|(_, s)| matches!(s, GateStatus::Passed))
        .count();
    let texts = published_texts(Some(&measured));
    assert!(
        !texts.is_empty(),
        "nothing was published for a pull request the corpus certified"
    );
    for (seam, text) in texts {
        assert!(
            mentions_number(&text, clean_passes),
            "`{seam}` published a claim about this pull request without the \
             number of gates that passed. The corpus produced {clean_passes} \
             clean passes out of {TOTAL_GATES}; the text says:\n{text}"
        );
        assert!(
            mentions_number(&text, TOTAL_GATES),
            "`{seam}` published a count with nothing to read it against: \
             {clean_passes} of what? Text was:\n{text}"
        );
    }
}

/// INTEGRATION — certified, and still withheld.
///
/// The shape invariant I1 exists for: every gate that measured is acceptable,
/// so the report certifies, and one gate produced no measurement, so the merge
/// is withheld anyway. `is_certified_ready` and the admission decision disagree
/// here, and that disagreement is the whole of the invariant.
///
/// The unmeasured gate is the verification gate, reached the way the pipeline
/// reaches it: `local_verification_gate` returned `None`.
#[test]
fn a_certified_run_with_one_gate_unmeasured_is_still_withheld_from_the_queue() {
    let work = a_working_tree();
    let certified_but_incomplete = as_if_every_gate_had_a_data_source(
        &report_from_the_corpus(work.path(), None, "APPROVE"),
        &["test_suite_status"],
    );

    assert!(
        certified_but_incomplete.is_certified_ready,
        "`NotMeasured` is individually acceptable, so this report certifies. If \
         it stops doing so, this test no longer covers the case the two \
         predicates disagree about"
    );
    assert_eq!(
        certified_but_incomplete.unmeasured_gates,
        vec!["test_suite_status".to_string()],
        "exactly one gate produced no measurement in this run"
    );

    let refusal = MergeEnlister::admission_refusal(Some(&certified_but_incomplete))
        .expect_err(
            "a certified report with a gate that produced no measurement must not \
             admit a pull request: absent evidence is not permission",
        )
        .to_string();
    assert!(
        refusal.contains("test_suite_status"),
        "the refusal must name the gate that produced no measurement; got: {refusal}"
    );
    assert!(
        published_texts(Some(&certified_but_incomplete)).is_empty(),
        "Anvil signed a certified-but-incomplete pull request that it is \
         withholding from the merge queue"
    );
}

/// INTEGRATION — the verification gate's outcome reaches the merge queue as
/// what it actually was.
///
/// The chain the reviewers' notes are about, end to end: `run_local_test_gate`
/// distinguishes a suite that failed from a gate that never completed;
/// `local_verification_gate` maps the second to `None`; the corpus maps `None`
/// to `NotMeasured` rather than to a failure; and the merge is withheld with
/// the gate named rather than the pull request accused of failing tests nothing
/// ran.
#[test]
fn the_verification_gate_reaches_the_report_as_what_it_actually_did() {
    let work = a_working_tree();

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
    }

    // The two answers that withhold must stay distinguishable in what reaches
    // the pull request, and neither may be endorsed.
    let never_ran = report_from_the_corpus(work.path(), None, "APPROVE");
    let failed = report_from_the_corpus(work.path(), Some(false), "APPROVE");
    let unmeasured = "test_suite_status".to_string();
    assert!(
        never_ran.unmeasured_gates.contains(&unmeasured)
            && !failed.unmeasured_gates.contains(&unmeasured),
        "the gate that never completed and the suite that failed are recorded as \
         the same thing, so a refusal cannot tell a reader which happened"
    );
    for (what, report) in [("never ran", &never_ran), ("failed", &failed)] {
        assert!(
            MergeEnlister::admission_refusal(Some(report)).is_err()
                && published_texts(Some(report)).is_empty(),
            "a pull request whose verification gate {what} was admitted or endorsed"
        );
    }
}

/// INTEGRATION — a code review that did not complete is absent evidence, not a
/// blocking verdict.
///
/// The enlist doors are the paths where the review had never run at all; they
/// run it now, and the answer "it did not complete" has to reach the merge
/// queue as `Errored` — which withholds without accusing — rather than as the
/// `Failed` that says the model judged the code adversely.
///
/// `Errored` is also the shape `is_admissible()` cannot see, because
/// `unmeasured_gates` does not record it. That is why the door asks
/// `admission_refusal` and not the weaker predicate.
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

    let refusal = MergeEnlister::admission_refusal(Some(&errored))
        .expect_err("a gate that errored produced no measurement; it cannot admit a merge")
        .to_string();
    assert!(
        refusal.contains("review_verdict_status"),
        "the refusal must name the gate that produced no measurement; got: {refusal}"
    );
    assert!(
        published_texts(Some(&errored)).is_empty(),
        "Anvil signed for a pull request whose code review never produced a verdict"
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

/// INTEGRATION — a blocker the configuration already determines is named before
/// the corpus is paid for, and it says nothing about the pull request.
///
/// `unmeasurable_gates_in_this_build` is what the three enlist doors read
/// before running anything: a gate that cannot produce a measurement in this
/// deployment makes every report the corpus can return inadmissible, whatever
/// the pull request is, and the alternative is paying a clone, seventy-two
/// guards, a model turn and a cold `cargo check` to arrive at a refusal the
/// configuration had already fixed.
///
/// The claim has to be true of the corpus, so it is checked against a real run:
/// every gate the pre-flight names must in fact be one that run could not
/// measure. And it must be a statement about the build — naming a pull request
/// in it would be a claim about something nothing measured.
#[test]
fn a_configuration_determined_blocker_is_named_before_the_corpus_runs() {
    let Some(blockers) = anvil::pre_merge_guard::unmeasurable_gates_in_this_build() else {
        // Every gate in this build can be measured, so there is nothing to
        // refuse in advance and nothing here to check.
        return;
    };
    assert!(
        !blockers.trim().is_empty(),
        "the pre-flight refusal must say which gate cannot be measured, or an \
         operator is told only that something is wrong"
    );
    assert!(
        !blockers.contains(A_PULL_REQUEST.0)
            && !mentions_number(&blockers, A_PULL_REQUEST.1 as usize),
        "the pre-flight refusal is a statement about this build, made before \
         anything about a pull request has been read: {blockers}"
    );

    let work = a_working_tree();
    let report = report_from_the_corpus(work.path(), Some(true), "APPROVE");
    for (gate, status) in report.named_statuses() {
        if blockers.contains(gate) {
            assert!(
                !status.is_acceptable() || matches!(status, GateStatus::NotMeasured { .. }),
                "`{gate}` is named as a gate this build cannot produce a \
                 measurement for, and a real corpus run reported `{}` for it. \
                 The doors are refusing in advance on a claim the corpus does not \
                 bear out",
                status.badge()
            );
        }
    }
    assert!(
        MergeEnlister::admission_refusal(Some(&report)).is_err(),
        "a gate that can never be measured is in this report and the door \
         admitted it anyway"
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

    let clean = published_texts(Some(&every_gate_passing()));
    let texts = published_texts(Some(&report));
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
        assert!(
            mentions_number(text, TOTAL_GATES - 2),
            "`{seam}` did not publish the {} gates that passed on the commit \
             being merged. Two were auto-corrected in Anvil's clone and never \
             pushed to this branch. Text was:\n{text}",
            TOTAL_GATES - 2
        );
        assert!(
            text.contains("doc_parity_status") || text.contains("cedar_status"),
            "`{seam}` says nothing about the two gates whose fix is not in the \
             commit being merged, so a reader cannot discover that the tree that \
             passed and the tree that merges are different. Text was:\n{text}"
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
        assert!(
            rendered.contains(absent.badge()),
            "the rendered matrix — the part of the report a reader of the pull \
             request actually sees — does not carry what this gate reported: \
             {absent:?}"
        );
        let accused: Vec<&str> = rendered
            .lines()
            .filter(|line| line.contains("Kani") && line.contains(accusation))
            .collect();
        assert!(
            accused.is_empty(),
            "the rendered matrix reports a gate that produced no measurement as \
             a failure: {absent:?} rendered as {accused:?}"
        );
    }
}

// =========================================================================
// Corpus integrity — the report grew two fields that are not gates
// =========================================================================

/// The gate fields the report declares, read from the type rather than from a
/// list a test maintains beside it.
fn declared_gate_fields() -> Vec<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/pre_merge_guard/report.rs");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} must be readable: {e}", path.display()));
    let start = src
        .find("pub struct PreMergeCertificationReport {")
        .expect("the report type is declared");
    let body = &src[start..];
    let end = body.find("\n}\n").expect("the declaration terminates");
    body[..end]
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let name = line.strip_prefix("pub ")?.strip_suffix(": GateStatus,")?;
            Some(name.to_string())
        })
        .collect()
}

/// CORPUS INTEGRITY — the corpus is exactly the gates the report declares, and
/// the fields this change added did not join it.
///
/// `TOTAL_GATES` is the authority for every count Anvil publishes onto a pull
/// request, and `named_statuses()` is the list every refusal and every
/// published line is written from. A gate that is declared but never named is
/// invisible to both; a name that is not a field is a gate a reader cannot look
/// up. The existing check counts fields against `TOTAL_GATES` and can see
/// neither.
///
/// `provenance` and `subject` are the two fields this change added. Neither is
/// a `GateStatus` and neither may be counted as a gate: they record who
/// measured the report and what it was measured against, not another thing
/// measured.
#[test]
fn the_published_corpus_is_exactly_the_gates_the_report_declares() {
    let declared = declared_gate_fields();
    let named: Vec<String> = gate_names().into_iter().map(str::to_string).collect();

    assert_eq!(
        declared.len(),
        TOTAL_GATES,
        "the report declares {} gate fields and publishes {TOTAL_GATES} as the \
         size of the corpus",
        declared.len()
    );
    assert_eq!(
        named, declared,
        "the names Anvil publishes and refuses by are not the gate fields the \
         report declares, in the order it declares them. A gate that is declared \
         and never named cannot appear in a refusal or in a published count; a \
         name that is not a field is one a reader cannot look up"
    );

    let mut unique = named.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        named.len(),
        "two gates are published under one name, so one of them can never be \
         named in a refusal"
    );

    for added in ["provenance", "subject"] {
        assert!(
            !named.iter().any(|gate| gate == added),
            "`{added}` is counted as a gate. It records where the report came \
             from and what it was measured against — not another gate — and \
             counting it inflates every number Anvil publishes onto a pull request"
        );
    }

    // The count is the corpus, not a constant beside it.
    let report = every_gate_passing();
    assert_eq!(
        report.all_statuses().len(),
        TOTAL_GATES,
        "the report carries a different number of statuses than the corpus whose \
         size it publishes"
    );
    assert_eq!(
        report.gate_counts(),
        (TOTAL_GATES, 0),
        "a fully passing report over the whole corpus does not count as the \
         whole corpus"
    );
}
