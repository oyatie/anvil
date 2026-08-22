//! Gate 17 (W3C TraceContext), issue #14: coverage the specification suite does
//! not provide.
//!
//! `tests/trace_gate_claims_only_what_it_measured_test.rs` pins the behaviours
//! the lane was opened for, and it pins them on the guard in isolation: it calls
//! `TraceContextGuard::evaluate_trace_propagation` and reads the
//! `TraceContextReport` that comes back. Two things are left over, and this file
//! is those two.
//!
//! # 1. The verdict has to survive the evaluator
//!
//! The gate-17 mapping lives at `src/pre_merge_guard/evaluator.rs:297`, inside a
//! 68-argument function, and nothing in this repository calls that function from
//! a test: `tests/evaluator_gate_ordering_test.rs` reads its *source text*
//! instead. So a status the guard computes correctly and the evaluator then
//! overwrites -- which is exactly what shipped, `GateStatus::Passed` rebuilt
//! from a boolean -- is invisible to every test here. Section C runs four diffs
//! through the real `PreMergeGuard::evaluate_pre_merge_gates` and reads
//! `PreMergeCertificationReport::trace_status`, which is the value the merge
//! queue and the scorecard actually consume.
//!
//! # 2. Shapes the specification fixtures do not reach
//!
//! Section A is one diff that touches documentation and Rust together. Both
//! halves of the shipped defect meet in it: the chunk filter was
//! `file_diff.contains(".rs")` over the chunk *text*, which the Markdown chunk's
//! own prose supplies, and `spawn_blocking` in the Rust chunk was not a form the
//! scanner knew. So the gate reported a finding against the file with no
//! boundary and none against the file with one -- both counts wrong, in opposite
//! directions, out of one patch.
//!
//! Section B is the two hunk shapes git writes that the fixtures never used -- a
//! file created against `/dev/null`, and a one-line hunk header carrying no
//! count -- because a published `path:line` is a location claim, and a gate that
//! invents one makes the same unbacked assertion this lane exists to remove, in
//! the field a reviewer acts on.
//!
//! # Which of these are regressions, and how that was established
//!
//! The three in sections A and B were run against the gate as it shipped -- the
//! pre-fix `src/trace_context_guard/` restored over this tree, with the
//! two-valued mapping from `evaluator.rs:292-296` scaffolded onto the report so
//! that every failure is a wrong-behaviour assertion and not a compile error --
//! and all three failed. Two of the transcripts:
//!
//! ```text
//! a_diff_touching_docs_and_rust_reports_the_rust_boundary_and_only_that
//!   left: "docs/adr/0002-honesty.md"  right: "src/columnar.rs"
//! a_hunk_header_carrying_no_count_still_locates_the_boundary
//!   left: 6  right: 5   -- a line number counted over the diff chunk
//! ```
//!
//! Section C's `the_verdict_the_guard_reached_is_the_verdict_gate_seventeen_publishes`
//! is a pin rather than a regression and is stated as one: the scaffold above
//! makes the two sides equal by construction, so it cannot go red that way. It
//! goes red on the defect it exists for -- a status rebuilt downstream --
//! which was checked by putting `evaluator.rs:297` back to
//! `if trace_report.is_propagated { Passed } else { Failed(..) }`:
//!
//! ```text
//! nothing in scope: the guard measured this diff and reached
//! Warning("➖ NOTHING TO MEASURE (…)"); the certification report published
//! Passed instead.
//! ```
//!
//! # 3. What this file deliberately does not decide
//!
//! The status for the nothing-in-scope case stays open, exactly as the
//! specification suite left it: `src/slo_canary_guard/mod.rs` answers absent
//! evidence with `NotMeasured` and `src/coverage_guard.rs:139` answers
//! nothing-to-measure with `Passed`, and choosing between them is the owner's
//! call, not a test author's. Section C therefore asserts an equality rather
//! than a table of expected variants: what is required is that whatever the
//! guard decided is what the certification report publishes, whichever of the
//! three it is.
//!
//! No gate is added, no report field is added, `TOTAL_GATES` is untouched, and
//! no test in the specification suite is modified.

use anvil::git_manager::PrDiffContext;
use anvil::pre_merge_guard::{GateStatus, PreMergeCertificationReport, PreMergeGuard};
use anvil::trace_context_guard::{TraceContextGuard, TraceContextReport};
use std::path::{Path, PathBuf};

// -------------------------------------------------------------------------
// Fixtures
// -------------------------------------------------------------------------

/// A diff context carrying exactly the text given, and nothing inferred from it.
///
/// The gate reads `diff_content` and nothing else, so `changed_files` is left
/// empty on purpose: a fixture that also listed the paths would let an
/// implementation pass by reading the list instead of the patch, and the patch
/// is what arrives from the webhook.
fn diff(diff_content: String) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 14,
        base_branch: "main".to_string(),
        base_sha: "aaaaaaa".to_string(),
        head_sha: "bbbbbbb".to_string(),
        diff_content,
        changed_files: Vec::new(),
        repo_working_dir: PathBuf::from("."),
        is_incremental: false,
        previous_head_sha: None,
    }
}

/// One file's chunk of a patch, written the way git writes it: the two path
/// headers are given separately so a creation (`--- /dev/null`) can be spelled
/// out, and the hunk header is given whole so a fixture can use the no-count
/// form git emits for a single line.
fn chunk(path: &str, from: &str, to: &str, hunk: &str, body: &str) -> String {
    format!(
        "diff --git a/{path} b/{path}\nindex 1111111..2222222 100644\n--- {from}\n+++ {to}\n{hunk}\n{body}\n"
    )
}

/// An ordinary edit to a file that exists on both sides of the change.
fn edit(path: &str, hunk: &str, body: &str) -> String {
    chunk(path, &format!("a/{path}"), &format!("b/{path}"), hunk, body)
}

fn run(ctx: &PrDiffContext) -> TraceContextReport {
    TraceContextGuard::new()
        .evaluate_trace_propagation(Path::new("."), ctx)
        .expect("the trace gate must run to completion on a well-formed diff")
}

// -------------------------------------------------------------------------
// A. Regressions: shapes the specification fixtures do not reach
// -------------------------------------------------------------------------

#[test]
fn a_diff_touching_docs_and_rust_reports_the_rust_boundary_and_only_that() {
    // Both halves of the defect in one patch, in the ordinary shape of a real
    // pull request: a code change with a note about it.
    //
    // The Markdown chunk carries a fenced example containing an uninstrumented
    // `tokio::spawn`, and its prose names a `.rs` path -- which is not
    // contrived, the ADR corpus does it. Under a filter that searches the chunk
    // text for `.rs`, that chunk is read as Rust and its example is published as
    // a boundary this pull request wrote, against a Markdown file.
    //
    // The Rust chunk carries `tokio::task::spawn_blocking`, which the shipping
    // scanner did not recognise as a boundary at all: it matched the literal
    // `tokio::spawn` and nothing else. So the gate reported a finding against
    // the file that has no boundary and none against the file that has one --
    // both counts wrong, in opposite directions, out of one patch.
    let doc_body = "\
+Gate 17 is implemented in `src/trace_context_guard/mod.rs`. The block below is
+prose about a task boundary, not a boundary this pull request adds:
+
+```rust
+tokio::spawn(async move { work().await; });
+```";
    let rust_body = "\
 pub fn compact(rows: Vec<Row>) {
+    tokio::task::spawn_blocking(move || {
+        heavy(rows);
+    });
 }";
    let patch = format!(
        "{}{}",
        edit("docs/adr/0002-honesty.md", "@@ -12,1 +12,6 @@", doc_body),
        edit("src/columnar.rs", "@@ -40,2 +40,5 @@", rust_body),
    );
    let report = run(&diff(patch));

    assert_eq!(
        report.tasks_scanned, 1,
        "one task boundary ships in this patch: the `spawn_blocking` in \
         src/columnar.rs. The spawn in the Markdown is prose. Summary was: {}",
        report.summary
    );
    assert_eq!(
        report.detached_findings.len(),
        1,
        "exactly one boundary ships and it carries no span; the gate reported \
         {:?}. Summary was: {}",
        report
            .detached_findings
            .iter()
            .map(|f| format!("{}:{}", f.file_path, f.line_number))
            .collect::<Vec<_>>(),
        report.summary
    );
    assert_eq!(
        report.detached_findings[0].file_path, "src/columnar.rs",
        "a finding names the file it was found in, and a documentation file \
         cannot contain an async boundary this pull request wrote. Summary was: \
         {}",
        report.summary
    );
    assert_eq!(
        report.detached_findings[0].line_number, 41,
        "the `spawn_blocking` is the second line of a hunk beginning at 40. \
         Summary was: {}",
        report.summary
    );
    assert!(
        report.summary.contains("src/columnar.rs"),
        "the sentence a reviewer reads has to name the file that has to change; \
         it said: {}",
        report.summary
    );
}

// -------------------------------------------------------------------------
// B. Hunk shapes git writes that the fixtures never used
// -------------------------------------------------------------------------
//
// A published `file:line` is a location claim about the merged tree, and a
// reviewer acts on it by opening that line. The specification suite pins the
// arithmetic on an ordinary `@@ -a,b +c,d @@` edit of an existing file. These
// are the two other headers git emits, and a gate that reads either of them
// wrongly publishes a number it did not measure -- the same unbacked assertion
// as the sentence, in the field that gets acted on.

#[test]
fn a_file_created_in_this_pull_request_locates_its_boundary_in_the_new_file() {
    // A creation: `--- /dev/null`, and a hunk header whose pre-image side is
    // `-0,0`. The path has to be read off the post-image header, and the line
    // counted from `+1`.
    let body = "\
+pub async fn drain() {
+    tokio::spawn(async move {
+        work().await;
+    });
+}";
    let report = run(&diff(chunk(
        "src/created.rs",
        "/dev/null",
        "b/src/created.rs",
        "@@ -0,0 +1,5 @@",
        body,
    )));

    assert_eq!(
        report.detached_findings.len(),
        1,
        "a file created with an uninstrumented spawn in it is the plainest case \
         gate 17 has. Summary was: {}",
        report.summary
    );
    assert_eq!(
        report.detached_findings[0].file_path, "src/created.rs",
        "the path comes from the post-image header; `/dev/null` is not a file a \
         reviewer can open. Summary was: {}",
        report.summary
    );
    assert_eq!(
        report.detached_findings[0].line_number, 2,
        "the spawn is the second line of the created file. Summary was: {}",
        report.summary
    );
}

#[test]
fn a_hunk_header_carrying_no_count_still_locates_the_boundary() {
    // git omits the count when it is 1: a single line added at line 5 of the
    // post-image is `@@ -4,0 +5 @@`, not `@@ -4,0 +5,1 @@`. A parser that
    // requires the comma reads no position out of this header at all, and then
    // either declines to read the hunk or numbers it from zero -- and a
    // one-line spawn with no span is precisely the change gate 17 is for.
    let report = run(&diff(edit(
        "src/short.rs",
        "@@ -4,0 +5 @@",
        "+    tokio::spawn(async move { work().await; });",
    )));

    assert_eq!(
        report.detached_findings.len(),
        1,
        "one uninstrumented boundary is added by this hunk. Summary was: {}",
        report.summary
    );
    assert_eq!(
        report.detached_findings[0].line_number, 5,
        "the header says the body begins at post-image line 5. Summary was: {}",
        report.summary
    );
}

// -------------------------------------------------------------------------
// C. Integration: the diff reaches a real GateStatus through the evaluator
// -------------------------------------------------------------------------

/// The four diffs that reach the four verdicts the guard can reach, each with a
/// name for what it is. Named rather than numbered so a failure says which
/// measurement disagreed with its published status.
fn arms() -> Vec<(&'static str, PrDiffContext)> {
    vec![
        (
            "nothing in scope",
            diff(edit(
                "src/plain.rs",
                "@@ -1,1 +1,1 @@",
                "+pub const ROWS: usize = 3;",
            )),
        ),
        (
            "inspected and clean",
            diff(edit(
                "src/clean.rs",
                "@@ -1,1 +1,1 @@",
                "+    tokio::spawn(work().instrument(tracing::info_span!(\"w\")));",
            )),
        ),
        (
            "inspected and detached",
            diff(edit(
                "src/detached.rs",
                "@@ -1,1 +1,1 @@",
                "+    tokio::spawn(async move { work().await; });",
            )),
        ),
        (
            "seen and not judged",
            diff(edit(
                "src/unresolved.rs",
                "@@ -1,2 +1,2 @@",
                "+    tokio::spawn(async move {\n+        work().await;",
            )),
        ),
    ]
}

#[test]
fn the_verdict_the_guard_reached_is_the_verdict_gate_seventeen_publishes() {
    // The mapping at `evaluator.rs:297` is the channel through which a
    // measurement becomes a merge decision, and it is the channel that broke:
    // rebuilding the status downstream from `is_propagated` collapsed four
    // measurements onto two verdicts and discarded three sentences, because
    // `GateStatus::Passed` carries no string. Nothing tested it, because
    // reaching it means calling a 68-argument function -- see `stub` below.
    //
    // This is deliberately an equality, not a table of expected variants: which
    // status the nothing-in-scope case deserves is the owner's open question,
    // and a table here would answer it by assertion. What is required is that
    // the guard decides and the report publishes that decision unchanged -- so
    // this passes under `Passed`, `Warning` or `NotMeasured`, and fails on a
    // status invented downstream.
    for (name, ctx) in arms() {
        let measured = run(&ctx);
        let cert = stub::certify(&ctx, &measured);

        assert_eq!(
            cert.trace_status, measured.status,
            "{name}: the guard measured this diff and reached {:?}; the \
             certification report published {:?} instead. The sentence the \
             guard wrote was: {}",
            measured.status, cert.trace_status, measured.summary
        );

        // The other half of publishing a status: `NotMeasured` is the one
        // variant that blocks merge-queue admission by a route of its own, and
        // it only does so if `seal()` sees it. A status that reached the field
        // but not the list is a block that never happens.
        //
        // The id is read out of the status rather than named. Which string gate
        // 17 carries is the guard's own choice -- `recompute_unmeasured`
        // collects whatever id the status hands it -- so a rename is not a
        // behaviour change and must not redden this test. What is required is
        // that a `NotMeasured` status reaches `unmeasured_gates` under whatever
        // id it carries.
        let id = match &cert.trace_status {
            GateStatus::NotMeasured { gate_id, .. } => Some(gate_id.as_str()),
            _ => None,
        };
        assert_eq!(
            id.is_some_and(|id| cert.unmeasured_gates.iter().any(|g| g == id)),
            id.is_some(),
            "{name}: `unmeasured_gates` and `trace_status` disagree about \
             whether gate 17 was measured. Status was {:?}, list was {:?}",
            cert.trace_status,
            cert.unmeasured_gates
        );
    }
}

// -------------------------------------------------------------------------
// Reaching the evaluator
// -------------------------------------------------------------------------

/// A neutral value for every gate report the evaluator wants, so that one gate
/// can be exercised through it.
///
/// `evaluate_pre_merge_gates` takes 64 report structs, one per gate family, and
/// none of them implements `Default`. Writing them out as literals would put a
/// copy of 229 fields belonging to 64 unrelated modules in this file: it would
/// break whenever any other lane adds a field to any of them, which is a build
/// break landed on someone else's pull request for a gate this file does not
/// test. Running the real guards instead is worse -- a dozen of them shell out
/// to `cargo`, walk the working tree or query a forge, and this file must do
/// none of that. That is what the 64 stubs buy: no *stubbed* guard runs.
///
/// It does not make the call hermetic, and this file should not be read as
/// claiming it does. Two gates are computed inside `evaluate_pre_merge_gates`
/// rather than passed to it, so no stub can reach them, and both run against
/// Anvil's own working tree on every call: `evaluator.rs:662-663` walks the
/// tree for brand absence, and `:675` calls
/// `crate::migration::live_tree_violations()`. Section C calls `certify` once
/// per arm over four arms, so one run of this file walks the tree eight times;
/// measured, that test takes under a second (0.86s to 0.88s across runs here).
/// Neither status is asserted on here. The second can come back as
/// `NotMeasured { gate_id: "migration_boundary_status", .. }`, which `seal()`
/// collects into `unmeasured_gates` -- the list section C reads -- so that list
/// is not purely a function of this file's own stubs. That is why the
/// membership check keys on the id carried by gate 17's own status rather than
/// on the list being empty or on its length.
///
/// So the stubs are built through `serde`, which already knows each struct's
/// field list: [`Neutral`] answers every request with the empty value of the
/// type asked for. The twelve reports that do not derive `Deserialize` are
/// written out, and they are small. None of these values is asserted on --
/// gate 17's status is the only thing read back -- they exist to let the
/// function be called at all.
mod stub {
    use super::*;
    use anvil::adr_drift_ratchet::AdrReport;
    use anvil::api_contract_guard::ApiContractReport;
    use anvil::attestation_guard::AttestationReport;
    use anvil::auto_rollback::AutoRollbackReport;
    use anvil::automated_canary::AutomatedCanaryReport;
    use anvil::canary_rollout::CanaryRolloutReport;
    use anvil::carbon_aware::CarbonComputeReport;
    use anvil::cedar_guard::CedarGuardReport;
    use anvil::cell_isolation_guard::CellIsolationReport;
    use anvil::chaos_injector::ChaosInjectorReport;
    use anvil::chaos_mutation_guard::MutationAdequacyReport;
    use anvil::ci_runner_economics::RunnerEconomicsReport;
    use anvil::ci_wallclock_ratchet::CiWallclockReport;
    use anvil::clean_architecture_guard::CleanArchitectureReport;
    use anvil::cluster_state_auditor::ClusterAuditReport;
    use anvil::compile_time_profiler::CompileProfileReport;
    use anvil::compliance_guard::ComplianceGuardReport;
    use anvil::consistency_guard::ConsistencyReport;
    use anvil::constant_work_guard::ConstantWorkReport;
    use anvil::cosign_signer::CosignReport;
    use anvil::coverage_guard::CoverageReport;
    use anvil::criterion_bench_ratchet::BenchmarkReport;
    use anvil::cross_service_impact::ServiceImpactReport;
    use anvil::deadlock_analyzer::DeadlockReport;
    use anvil::debt_shrink_guard::DebtShrinkReport;
    use anvil::doc_guard::DocGuardReport;
    use anvil::ephemeral_sandbox::SandboxReport;
    use anvil::ephemeral_secrets::SecretPolicyReport;
    use anvil::feature_flag_ratchet::FeatureFlagReport;
    use anvil::finops_ratchet::FinOpsReport;
    use anvil::flake_quarantine::FlakeQuarantineReport;
    use anvil::formal_verification::FormalVerificationReport;
    use anvil::ghost_migration_harness::GhostMigrationReport;
    use anvil::gitops_drift_reconciler::GitOpsDriftReport;
    use anvil::gitops_promotion::GitOpsPromotionReport;
    use anvil::hermetic_build::HermeticBuildReport;
    use anvil::idempotency_guard::IdempotencyReport;
    use anvil::jittered_backoff::JitteredBackoffReport;
    use anvil::kani_guard::KaniGuardReport;
    use anvil::local_inner_loop::LocalProbeReport;
    use anvil::microbenchmark_ratchet::MicrobenchmarkReport;
    use anvil::migration_orchestrator::MigrationLifecycleReport;
    use anvil::modularization_guard::ModularizationReport;
    use anvil::monorepo_guard::MonorepoGuardReport;
    use anvil::predictive_test_selector::PredictiveTestReport;
    use anvil::progressive_rollout::ProgressiveRingReport;
    use anvil::psa_admission_guard::PsaAdmissionReport;
    use anvil::remote_cache_optimizer::CacheReport;
    use anvil::replay_harness::ReplayHarnessReport;
    use anvil::rust_language_policy::RustSkillsReport;
    use anvil::schema_evolution::SchemaEvolutionReport;
    use anvil::semantic_abi_ratchet::SemanticAbiReport;
    use anvil::shadow_traffic_harness::ShadowTrafficReport;
    use anvil::shape::facade::gate::ShapeGateOutcome;
    use anvil::shuffle_shard_simulator::ShuffleShardReport;
    use anvil::slo_canary_guard::SloCanaryReport;
    use anvil::stacked_diffs::StackedDiffsReport;
    use anvil::supply_chain_guard::SupplyChainReport;
    use anvil::unresolved_review_guard::UnresolvedReviewReport;
    use anvil::upgrade_train::UpgradeTrainReport;
    use anvil::vex_scanner::OpenVexReport;
    use anvil::wasm_sandbox::WasmSandboxReport;
    use anvil::zero_day_patcher::ZeroDayReport;
    use anvil::zero_trust_workload::ZeroTrustWorkloadReport;
    use serde::de::{
        self, DeserializeSeed, EnumAccess, IntoDeserializer, MapAccess, SeqAccess, VariantAccess,
        Visitor,
    };
    use std::fmt;

    /// What every stubbed report says about itself, so that a string read out of
    /// one of them says where it came from instead of being blank.
    const STUB: &str = "stub: only gate 17 is exercised by this test file";

    /// Runs the real evaluator over one measured trace report.
    pub fn certify(
        ctx: &PrDiffContext,
        trace_report: &TraceContextReport,
    ) -> PreMergeCertificationReport {
        let doc = DocGuardReport {
            is_sufficient: true,
            files_created_or_updated: Vec::new(),
            summary: STUB.to_string(),
            errored: None,
        };
        let cedar = CedarGuardReport {
            is_compliant: true,
            files_created_or_updated: Vec::new(),
            summary: STUB.to_string(),
        };
        let jittered = JitteredBackoffReport {
            passed: true,
            unjittered_retries_detected: 0,
            missing_deadline_calls: 0,
            summary: STUB.to_string(),
        };
        let schema_evo = SchemaEvolutionReport {
            passed: true,
            breaking_field_changes: 0,
            tag_renumbering_detected: false,
            summary: STUB.to_string(),
        };
        let auto_rollback = AutoRollbackReport {
            status: GateStatus::Passed,
            passed: true,
            rollback_triggered: false,
            summary: STUB.to_string(),
        };
        let wasm = WasmSandboxReport {
            passed: true,
            active_wasm_plugins: 0,
            policy_violations: Vec::new(),
            summary: STUB.to_string(),
        };
        let consistency = ConsistencyReport {
            passed: true,
            split_brain_risks: 0,
            unversioned_mutations: 0,
            summary: STUB.to_string(),
        };
        let flake = FlakeQuarantineReport {
            passed: true,
            quarantined_tests_isolated: 0,
            rehabilitated_tests_restored: 0,
            summary: STUB.to_string(),
        };
        let zero_trust = ZeroTrustWorkloadReport {
            passed: true,
            spiffe_id_verified: true,
            mtls_enforced: true,
            unauthenticated_endpoints: 0,
            summary: STUB.to_string(),
        };
        let carbon = CarbonComputeReport {
            status: GateStatus::Passed,
            passed: true,
            estimated_joules_per_build: 0.0,
            green_window_scheduled: true,
            summary: STUB.to_string(),
        };
        let replay = ReplayHarnessReport {
            status: GateStatus::Passed,
            passed: true,
            replayed_fixtures_count: 0,
            divergence_detected: false,
            summary: STUB.to_string(),
        };
        let upgrade = UpgradeTrainReport {
            status: GateStatus::Passed,
            passed: true,
            pending_upgrades_available: 0,
            breaking_major_upgrades: 0,
            summary: STUB.to_string(),
        };

        PreMergeGuard::new()
            .evaluate_pre_merge_gates(
                ctx,
                &doc,
                &cedar,
                &neutral::<ComplianceGuardReport>(),
                &neutral::<ApiContractReport>(),
                &neutral::<CellIsolationReport>(),
                &neutral::<SupplyChainReport>(),
                &neutral::<CleanArchitectureReport>(),
                &neutral::<MonorepoGuardReport>(),
                &neutral::<DebtShrinkReport>(),
                &neutral::<ModularizationReport>(),
                &neutral::<CoverageReport>(),
                &neutral::<RustSkillsReport>(),
                &neutral::<KaniGuardReport>(),
                &neutral::<SloCanaryReport>(),
                &neutral::<AdrReport>(),
                &neutral::<ShuffleShardReport>(),
                trace_report,
                &neutral::<ConstantWorkReport>(),
                &neutral::<IdempotencyReport>(),
                &neutral::<FinOpsReport>(),
                &neutral::<GhostMigrationReport>(),
                &neutral::<GitOpsPromotionReport>(),
                &neutral::<GitOpsDriftReport>(),
                &neutral::<CanaryRolloutReport>(),
                &neutral::<ClusterAuditReport>(),
                &neutral::<MigrationLifecycleReport>(),
                &neutral::<CiWallclockReport>(),
                &neutral::<PredictiveTestReport>(),
                &neutral::<CompileProfileReport>(),
                &neutral::<CacheReport>(),
                &neutral::<RunnerEconomicsReport>(),
                &neutral::<SandboxReport>(),
                &neutral::<ServiceImpactReport>(),
                &neutral::<SecretPolicyReport>(),
                &neutral::<PsaAdmissionReport>(),
                &neutral::<ShadowTrafficReport>(),
                &neutral::<UnresolvedReviewReport>(),
                &neutral::<LocalProbeReport>(),
                &neutral::<SemanticAbiReport>(),
                &neutral::<ZeroDayReport>(),
                &neutral::<FormalVerificationReport>(),
                &neutral::<DeadlockReport>(),
                &neutral::<AutomatedCanaryReport>(),
                &neutral::<ProgressiveRingReport>(),
                &neutral::<HermeticBuildReport>(),
                &neutral::<OpenVexReport>(),
                &neutral::<CosignReport>(),
                &neutral::<ChaosInjectorReport>(),
                &neutral::<StackedDiffsReport>(),
                &neutral::<MicrobenchmarkReport>(),
                &jittered,
                &schema_evo,
                &auto_rollback,
                &wasm,
                &consistency,
                &flake,
                &zero_trust,
                &carbon,
                &replay,
                &upgrade,
                &neutral::<MutationAdequacyReport>(),
                &neutral::<FeatureFlagReport>(),
                &neutral::<BenchmarkReport>(),
                &neutral::<AttestationReport>(),
                Some(true),
                "APPROVE",
                &ShapeGateOutcome::NoSpec {
                    reason: STUB.to_string(),
                },
            )
            .expect("the evaluator must map a set of gate reports without failing")
    }

    /// The empty value of any `Deserialize` type: `false`, `0`, `""`, no
    /// elements, `None`, the first variant.
    fn neutral<T: serde::de::DeserializeOwned>() -> T {
        serde::Deserialize::deserialize(Neutral).unwrap_or_else(|e| {
            panic!(
                "a gate report holds a field this stub cannot fill ({}): write \
                 that report out as a literal above instead. {e}",
                std::any::type_name::<T>()
            )
        })
    }

    #[derive(Debug)]
    pub struct Error(String);

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(&self.0)
        }
    }
    impl std::error::Error for Error {}
    impl de::Error for Error {
        fn custom<T: fmt::Display>(msg: T) -> Self {
            Error(msg.to_string())
        }
    }

    #[derive(Clone, Copy)]
    struct Neutral;

    impl<'de> de::Deserializer<'de> for Neutral {
        type Error = Error;

        fn deserialize_any<V: Visitor<'de>>(self, _v: V) -> Result<V::Value, Error> {
            // Reached only by a field whose `Deserialize` is hand-written and
            // asks what is there rather than what it expects. No gate report has
            // one; if one appears, the panic in `neutral` says what to do.
            Err(de::Error::custom(
                "this stub answers a request for a type, not a self-describing format",
            ))
        }

        fn deserialize_bool<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_bool(false)
        }
        fn deserialize_i8<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_i8(0)
        }
        fn deserialize_i16<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_i16(0)
        }
        fn deserialize_i32<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_i32(0)
        }
        fn deserialize_i64<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_i64(0)
        }
        fn deserialize_u8<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_u8(0)
        }
        fn deserialize_u16<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_u16(0)
        }
        fn deserialize_u32<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_u32(0)
        }
        fn deserialize_u64<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_u64(0)
        }
        fn deserialize_f32<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_f32(0.0)
        }
        fn deserialize_f64<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_f64(0.0)
        }
        fn deserialize_char<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_char(' ')
        }
        fn deserialize_str<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_str("")
        }
        fn deserialize_string<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_str("")
        }
        fn deserialize_bytes<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_bytes(&[])
        }
        fn deserialize_byte_buf<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_bytes(&[])
        }
        fn deserialize_option<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_none()
        }
        fn deserialize_unit<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_unit()
        }
        fn deserialize_unit_struct<V: Visitor<'de>>(
            self,
            _name: &'static str,
            v: V,
        ) -> Result<V::Value, Error> {
            v.visit_unit()
        }
        fn deserialize_newtype_struct<V: Visitor<'de>>(
            self,
            _name: &'static str,
            v: V,
        ) -> Result<V::Value, Error> {
            v.visit_newtype_struct(self)
        }
        fn deserialize_seq<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_seq(Elements(0))
        }
        fn deserialize_tuple<V: Visitor<'de>>(self, len: usize, v: V) -> Result<V::Value, Error> {
            v.visit_seq(Elements(len))
        }
        fn deserialize_tuple_struct<V: Visitor<'de>>(
            self,
            _name: &'static str,
            len: usize,
            v: V,
        ) -> Result<V::Value, Error> {
            v.visit_seq(Elements(len))
        }
        fn deserialize_map<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_map(Fields(&[]))
        }
        fn deserialize_struct<V: Visitor<'de>>(
            self,
            _name: &'static str,
            fields: &'static [&'static str],
            v: V,
        ) -> Result<V::Value, Error> {
            v.visit_map(Fields(fields))
        }
        fn deserialize_enum<V: Visitor<'de>>(
            self,
            name: &'static str,
            variants: &'static [&'static str],
            v: V,
        ) -> Result<V::Value, Error> {
            let first = variants.first().ok_or_else(|| {
                de::Error::custom(format!("{name} has no variant to stand in for it"))
            })?;
            v.visit_enum(FirstVariant(first))
        }
        fn deserialize_identifier<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_str("")
        }
        fn deserialize_ignored_any<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
            v.visit_unit()
        }
    }

    /// `n` elements, each itself neutral. `0` for a sequence -- an empty `Vec`
    /// of findings is what a gate that found nothing holds -- and `len` for a
    /// tuple, which has to be filled to exist at all.
    struct Elements(usize);

    impl<'de> SeqAccess<'de> for Elements {
        type Error = Error;
        fn next_element_seed<T: DeserializeSeed<'de>>(
            &mut self,
            seed: T,
        ) -> Result<Option<T::Value>, Error> {
            if self.0 == 0 {
                return Ok(None);
            }
            self.0 -= 1;
            seed.deserialize(Neutral).map(Some)
        }
        fn size_hint(&self) -> Option<usize> {
            Some(self.0)
        }
    }

    /// The fields serde itself named when it asked for the struct, each answered
    /// with a neutral value. This is what makes the stub survive a field being
    /// added to a report in some other lane.
    struct Fields(&'static [&'static str]);

    impl<'de> MapAccess<'de> for Fields {
        type Error = Error;
        fn next_key_seed<K: DeserializeSeed<'de>>(
            &mut self,
            seed: K,
        ) -> Result<Option<K::Value>, Error> {
            let Some((next, rest)) = self.0.split_first() else {
                return Ok(None);
            };
            self.0 = rest;
            let key: de::value::StrDeserializer<Error> = (*next).into_deserializer();
            seed.deserialize(key).map(Some)
        }
        fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
            seed.deserialize(Neutral)
        }
    }

    /// The first variant of an enum, whatever it is: a stub has no grounds to
    /// prefer one, and none of these values is read back.
    struct FirstVariant(&'static str);

    impl<'de> EnumAccess<'de> for FirstVariant {
        type Error = Error;
        type Variant = Neutral;
        fn variant_seed<V: DeserializeSeed<'de>>(
            self,
            seed: V,
        ) -> Result<(V::Value, Neutral), Error> {
            let name: de::value::StrDeserializer<Error> = self.0.into_deserializer();
            Ok((seed.deserialize(name)?, Neutral))
        }
    }

    impl<'de> VariantAccess<'de> for Neutral {
        type Error = Error;
        fn unit_variant(self) -> Result<(), Error> {
            Ok(())
        }
        fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
            seed.deserialize(Neutral)
        }
        fn tuple_variant<V: Visitor<'de>>(self, len: usize, v: V) -> Result<V::Value, Error> {
            v.visit_seq(Elements(len))
        }
        fn struct_variant<V: Visitor<'de>>(
            self,
            fields: &'static [&'static str],
            v: V,
        ) -> Result<V::Value, Error> {
            v.visit_map(Fields(fields))
        }
    }
}
