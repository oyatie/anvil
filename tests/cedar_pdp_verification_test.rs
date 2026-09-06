//! Gate 2 (`cedar_status`) paid a model to judge Cedar policy coverage and then
//! threw the answer away.
//!
//! # The defect
//!
//! `evaluate_cedar_policies` had three exits and every one of them set
//! `is_compliant: true` (`cedar_guard.rs:59`, `:75`, `:91`). The third is the
//! interesting one. It is the branch reached *after* `analyze_cedar_coverage`
//! has come back with `is_cedar_compliant: false` -- a model spawned at
//! `ExecClass::Model`, up to 600 seconds, asked for strict JSON and parsed with
//! serde. The gate took that "no", spawned a *second* model turn to write
//! `.cedar` files, and then reported compliant whatever the second turn did:
//!
//! ```text
//! let created_files = self.generate_missing_cedar_policies(...).await?;
//! Ok(CedarGuardReport { is_compliant: true, files_created_or_updated: created_files, ... })
//! ```
//!
//! When `created_files` came back empty -- the model wrote nothing, or wrote
//! outside the `git status` filter -- the evaluator's `!is_empty()` test for
//! `AutoUpdated` was false and `is_compliant` was true, so the published status
//! was `Passed`. A gate whose only judge said "missing policies" published a
//! green. `GateStatus::Failed` at `evaluator.rs:209` was unreachable from
//! production: no code path could construct the report that reaches it.
//!
//! Worse than a constant, because it is a constant that costs two model turns
//! and commits what the second one wrote: `certify.rs` stages
//! `cedar_report.files_created_or_updated` and pushes it. Authorization policy,
//! authored by a model, committed to the repository, parsed by nothing.
//!
//! # Why prompting cannot prevent it
//!
//! The lie is not in the prompt. The prompt is careful -- it demands strict
//! JSON, it fails closed on a parse error. The lie is one struct literal, forty
//! lines below, in a function whose author had already decided that this gate
//! auto-remediates rather than blocks. No instruction reaches the person who
//! writes `is_compliant: true` under a comment that says "auto-generating".
//! Only a test that pins the outcome of a *finding* can.
//!
//! # What the gate is now
//!
//! `cedar check-parse`, the reference implementation's own CLI, over the
//! `.cedar` files the pull request touched. That decides one property, and it
//! decides it soundly: the policy set is grammatical Cedar. Everything the gate
//! is *named* for beyond that -- does a policy cover the route this PR added --
//! needs a schema, and `cedar validate` takes `--schema` as a required
//! argument. This repository has no Cedar schema, so that half is
//! `NotMeasured` and says so.

use anvil::cedar_guard::{
    CEDAR_GATE_ID, CedarGuard, CedarToolOutcome, combine_outcomes, interpret_cedar_outcome,
    policy_files_in_scope, verify,
};
use anvil::git_manager::PrDiffContext;
use anvil::pre_merge_guard::report::GateStatus;

fn files(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|s| s.to_string()).collect()
}

fn a_diff_touching(work_dir: &std::path::Path, changed: &[&str]) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "main".to_string(),
        base_sha: "a".to_string(),
        head_sha: "b".to_string(),
        is_incremental: false,
        previous_head_sha: None,
        repo_working_dir: anvil::git_manager::SubjectRoot::asserted(
            work_dir.to_path_buf(),
            anvil::git_manager::Uncloned::TestFixture,
        ),
        changed_files: files(changed),
        diff_content: String::new(),
    }
}

fn reason_of(status: &GateStatus) -> String {
    match status {
        GateStatus::NotMeasured { gate_id, reason } => {
            assert_eq!(
                gate_id, CEDAR_GATE_ID,
                "a NotMeasured recorded under any other id names a gate that \
                 `unmeasured_gates` cannot resolve back to a field on the report"
            );
            reason.clone()
        }
        other => panic!("expected NotMeasured, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// The headline. This is `cedar_guard.rs:91` restated as an assertion.
// ---------------------------------------------------------------------------

/// THE DEFECT, DIRECTLY. The only judge this gate has said no, and the gate
/// must not publish a status that certifies.
#[test]
fn a_rejected_policy_set_does_not_certify() {
    let diagnostics = "error: invalid policy effect: allow\n  --> tenants.cedar:1:1";
    let scope = files(&["policies/cedar/tenants.cedar"]);
    let report = verify(
        &scope,
        &scope,
        &CedarToolOutcome::Rejected(diagnostics.to_string()),
    );

    assert!(
        !report.status.is_acceptable(),
        "the policy checker rejected the policy set and the gate published an \
         acceptable status: {:?}. This is the exit that set `is_compliant: true` \
         after its own evaluation had come back non-compliant",
        report.status
    );
    assert!(
        matches!(report.status, GateStatus::Failed(_)),
        "a tool that ran and rejected the input is a finding against the pull \
         request, not an absence of evidence: {:?}",
        report.status
    );
    assert!(
        report.summary.contains("invalid policy effect"),
        "the checker's diagnostics are the whole value of the finding; a summary \
         that drops them leaves a reader with an accusation and no way to act on \
         it: {}",
        report.summary
    );
}

/// The same rejection, carried through the field the evaluator reads.
///
/// `evaluator.rs` used to rebuild this gate's verdict from a boolean, which is
/// how a guard's honest answer gets discarded in the wiring where guard-level
/// tests cannot see it. `tests/evaluator_preserves_gate_verdicts_test.rs` pins
/// the wiring; this pins that there is a verdict there to carry.
#[test]
fn the_report_carries_the_verdict_rather_than_a_boolean_to_rebuild_it_from() {
    let scope = files(&["a.cedar"]);
    let rejected = verify(
        &scope,
        &scope,
        &CedarToolOutcome::Rejected("error: unexpected end of input".to_string()),
    );
    let accepted = verify(&scope, &scope, &CedarToolOutcome::Accepted);

    assert_ne!(
        rejected.status, accepted.status,
        "a rejection and an acceptance must be distinguishable from the status \
         alone, or the evaluator has to guess"
    );
    assert_eq!(accepted.status, GateStatus::Passed);
}

// ---------------------------------------------------------------------------
// Absent evidence: never a pass (I1), never an accusation.
// ---------------------------------------------------------------------------

/// The checker is not installed. That is a fact about the runner, not about the
/// pull request, so it is neither a pass nor a finding.
#[test]
fn a_checker_that_could_not_run_is_not_a_pass_and_not_an_accusation() {
    let scope = files(&["policies/cedar/anvil_policies.cedar"]);
    let report = verify(
        &scope,
        &[],
        &CedarToolOutcome::Unavailable(
            "cedar check-parse failed to run: No such file or directory (os error 2)".to_string(),
        ),
    );

    assert!(
        !matches!(report.status, GateStatus::Passed | GateStatus::AutoUpdated),
        "no policy was checked and the gate published a pass: {:?}",
        report.status
    );
    assert!(
        !matches!(report.status, GateStatus::Failed(_)),
        "accusing a pull request of a policy defect nobody observed is the \
         symmetric violation of I1: {:?}",
        report.status
    );

    let reason = reason_of(&report.status);
    assert!(
        reason.contains("No such file or directory") || reason.contains("os error 2"),
        "the reason must carry why the checker produced nothing, verbatim, or an \
         operator cannot tell a missing binary from a timeout: {reason}"
    );
}

/// An empty scope is not a pass -- the rule this repository settled in the four
/// marker-scoped gates.
///
/// Deliberately handed `Accepted`, the friendliest tool answer there is: even a
/// checker that ran and was happy has said nothing about a pull request whose
/// policy files it never saw. The gate's name is coverage, and coverage of the
/// routes this PR *did* touch is exactly what nothing here measured.
#[test]
fn a_pull_request_that_touches_no_policy_file_is_not_certified_by_this_gate() {
    let report = verify(&[], &[], &CedarToolOutcome::Accepted);

    assert!(
        !matches!(report.status, GateStatus::Passed | GateStatus::AutoUpdated),
        "no Cedar policy was in scope, so nothing was verified, and the gate \
         published a pass anyway: {:?}",
        report.status
    );
    let reason = reason_of(&report.status);
    assert!(
        reason.to_lowercase().contains("schema"),
        "the reason must name what is missing before coverage could be decided; \
         `cedar validate` and `cedar symcc` both take a required --schema and \
         this repository has none: {reason}"
    );
}

// ---------------------------------------------------------------------------
// A pass claims only what was checked.
// ---------------------------------------------------------------------------

#[test]
fn a_pass_says_what_was_parsed_and_does_not_claim_the_coverage_it_did_not_check() {
    let scope = files(&["policies/cedar/anvil_policies.cedar", "b.cedar"]);
    let report = verify(&scope, &scope, &CedarToolOutcome::Accepted);

    assert_eq!(report.status, GateStatus::Passed);
    assert!(
        report.summary.contains("anvil_policies.cedar"),
        "a pass must name the corpus it is a pass over, or it is indistinguishable \
         from a constant: {}",
        report.summary
    );
    assert!(
        report.summary.contains("cedar"),
        "the summary must name the tool whose answer it is reporting: {}",
        report.summary
    );

    // The vocabulary the old exits used: "Cedar IAM policy coverage is verified;
    // all actions are bound to authorization rules." Nothing here checked an
    // action, a route or a principal, and no schema exists to check them against.
    let lower = report.summary.to_lowercase();
    for claim in [
        "all actions",
        "all routes",
        "fully compliant",
        "coverage is verified",
        "every action",
        "every route",
        "authorization rules",
    ] {
        assert!(
            !lower.contains(claim),
            "a parse check published \"{claim}\": that is the sentence the gate \
             used to publish from a model's opinion, and no schema exists here \
             for anything to check it against.\n  summary was: {}",
            report.summary
        );
    }
}

/// Catches: a pass that names files the checker never opened.
///
/// A change that deletes one policy and edits another leaves both paths in the
/// diff and one file on disk. `check_parse_all` skips the unreadable one; the
/// pass it produces used to be built from the *scope*, so it published
/// "accepted 2 Cedar policy file(s)" and named a file that does not exist.
/// That is the vacuously-clean shape `combine_outcomes` already guards at zero
/// files parsed -- the same defect at one-of-two, and this is the arm that
/// publishes green.
#[test]
fn a_pass_names_only_the_files_the_checker_actually_read() {
    let scope = files(&["good.cedar", "policies/removed.cedar"]);
    let parsed = files(&["good.cedar"]);
    let report = verify(&scope, &parsed, &CedarToolOutcome::Accepted);

    assert_eq!(report.status, GateStatus::Passed);
    assert!(
        !report.summary.contains("removed.cedar"),
        "the pass names a policy file the checker never opened: {}",
        report.summary
    );
    assert!(
        report.summary.contains("1 Cedar policy file"),
        "the count must be what was parsed, not what was in scope: {}",
        report.summary
    );
}

// ---------------------------------------------------------------------------
// The invocation itself, where the checker exists.
// ---------------------------------------------------------------------------

/// Whether `cedar` is installed and runnable on this machine.
fn cedar_on_path() -> bool {
    std::process::Command::new("cedar")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Catches: a broken invocation that every other test in this file accepts.
///
/// Everything above reaches `verify` with an outcome it was handed. Nothing
/// reaches the spawn, so with the binary installed and on PATH both a mistyped
/// subcommand and a mistyped binary name survived the whole suite: each makes
/// the gate incapable of measuring anything, and each is indistinguishable from
/// "the runner does not have the tool" to a suite whose every end-to-end case
/// accepts `NotMeasured`.
///
/// The early return is the honest form. On a runner without the checker this
/// measures nothing and says nothing; on a runner with it, it is the only thing
/// between the gate and a typo. It also pins `--error-format plain`: `cedar`
/// renders through miette's graphical handler with no TTY gate, and the escape
/// sequences were landing in the published `Failed` summary.
#[tokio::test]
async fn where_the_checker_is_installed_a_bad_policy_fails_and_a_good_one_passes() {
    if !cedar_on_path() {
        eprintln!("cedar is not on PATH: the invocation is unmeasured on this runner");
        return;
    }

    assert_eq!(
        anvil::cedar_guard::check_parse("permit(principal, action, resource);\n").await,
        CedarToolOutcome::Accepted,
        "a grammatical policy set must reach the checker and come back accepted; \
         a mistyped subcommand or binary name lands here as Unavailable"
    );

    match anvil::cedar_guard::check_parse("allow(principal, action, resource);\n").await {
        CedarToolOutcome::Rejected(d) => {
            assert!(
                d.contains("invalid policy effect"),
                "the checker's own diagnostics must survive: {d}"
            );
            assert!(
                !d.contains('\u{1b}'),
                "raw ANSI escapes reached the published finding; the summary goes \
                 onto the scorecard, where a reader cannot act on it: {d:?}"
            );
        }
        other => panic!("an ungrammatical policy set is a rejection, got {other:?}"),
    }

    // The seam: two files in scope, one on disk. The pass must name one.
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("good.cedar"),
        "permit(principal, action, resource);\n",
    )
    .expect("write policy");
    let ctx = a_diff_touching(dir.path(), &["good.cedar", "policies/removed.cedar"]);
    let report = CedarGuard::new()
        .evaluate_cedar_policies(dir.path(), &ctx)
        .await;

    assert_eq!(report.status, GateStatus::Passed, "{}", report.summary);
    assert!(
        !report.summary.contains("removed.cedar"),
        "the pass names a file that was not on disk: {}",
        report.summary
    );
}

// ---------------------------------------------------------------------------
// Scope. An over-broad predicate is the same defect as an empty one.
// ---------------------------------------------------------------------------

/// The old scope was `f.contains("api") || f.contains("route") ||
/// f.contains("handler")`, which is a guess about spelling, not a Cedar policy
/// file. `src/cedar_guard.rs` is a Rust file about Cedar; it is not a policy.
#[test]
fn scope_is_the_cedar_extension_not_the_word_cedar_in_a_path() {
    let scope = policy_files_in_scope(&files(&[
        "src/cedar_guard.rs",
        "src/webhook/manual_handlers.rs",
        "docs/cedar.md",
        "policies/cedar/anvil_policies.cedar",
        "governance/cedar/policy/auth_routes.cedar",
    ]));

    assert_eq!(
        scope,
        files(&[
            "policies/cedar/anvil_policies.cedar",
            "governance/cedar/policy/auth_routes.cedar",
        ]),
        "scope must be the files a Cedar parser can actually read; a Rust file \
         whose name contains `cedar` is what put `src/migration/registry.rs` \
         into schema-migration scope"
    );
}

#[test]
fn a_diff_with_no_policy_file_has_an_empty_scope() {
    assert!(policy_files_in_scope(&files(&["src/main.rs", "README.md"])).is_empty());
}

// ---------------------------------------------------------------------------
// Reading the checker's exit.
// ---------------------------------------------------------------------------

#[test]
fn a_clean_exit_is_an_acceptance() {
    assert_eq!(
        interpret_cedar_outcome(0, "Policy set parses\n", ""),
        CedarToolOutcome::Accepted
    );
}

#[test]
fn a_parse_failure_carries_the_checkers_own_diagnostics() {
    let stderr = "error: invalid policy effect: allow\n  --> input:1:1\n";
    match interpret_cedar_outcome(1, "", stderr) {
        CedarToolOutcome::Rejected(d) => assert!(
            d.contains("invalid policy effect"),
            "the diagnostics must survive verbatim: {d}"
        ),
        other => panic!("a non-zero exit carrying diagnostics is a rejection, got {other:?}"),
    }
}

/// A usage error is Anvil's fault, not the pull request's.
///
/// `cedar` is a clap program, and clap exits 2 when it cannot parse its own
/// command line. If a future flag rename turned that into `Rejected`, every
/// pull request in the fleet would be accused of writing invalid authorization
/// policy on the strength of Anvil mistyping an argument. Absent evidence, not
/// an accusation.
#[test]
fn a_usage_error_from_the_checker_is_absent_evidence_not_a_policy_defect() {
    match interpret_cedar_outcome(2, "", "error: unexpected argument '--policies' found\n") {
        CedarToolOutcome::Unavailable(why) => assert!(
            why.contains("unexpected argument"),
            "the operator has to be able to see that Anvil called the tool \
             wrongly: {why}"
        ),
        other => panic!("clap's usage exit code must not become a finding, got {other:?}"),
    }
}

/// A non-zero exit that printed nothing at all is still a rejection, and the
/// reader is told what little there is to tell.
#[test]
fn a_silent_failure_still_says_what_the_exit_was() {
    match interpret_cedar_outcome(1, "", "") {
        CedarToolOutcome::Rejected(d) => assert!(
            d.contains("exit status 1"),
            "an empty diagnostic must not become an empty finding: {d}"
        ),
        other => panic!("expected a rejection, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Folding per-file answers into one answer for the set.
// ---------------------------------------------------------------------------

/// A finding the checker really produced survives a later file that could not
/// be checked at all. Otherwise one timeout at the end of the list erases the
/// only evidence there was.
#[test]
fn a_rejection_outranks_a_later_unavailability() {
    match combine_outcomes(&files(&["a.cedar: bad effect"]), Some("timed out"), 2) {
        CedarToolOutcome::Rejected(d) => assert!(d.contains("bad effect"), "{d}"),
        other => panic!("the rejection was discarded: {other:?}"),
    }
}

#[test]
fn an_unavailability_with_no_rejection_stays_an_unavailability() {
    assert_eq!(
        combine_outcomes(&[], Some("cedar: not found"), 1),
        CedarToolOutcome::Unavailable("cedar: not found".to_string())
    );
}

/// THE VACUOUS GREEN. Zero rejections out of zero files parsed is not a clean
/// policy set; it is an unexamined one. This is the exact shape that had four
/// other gates in this corpus certifying corpora they never held.
#[test]
fn nothing_parsed_is_not_a_clean_policy_set() {
    match combine_outcomes(&[], None, 0) {
        CedarToolOutcome::Unavailable(why) => assert!(
            why.contains("none of") && why.contains("could be read"),
            "the reason must say that nothing was read, not that nothing was wrong: {why}"
        ),
        other => panic!("an empty parse published {other:?}"),
    }
}

#[test]
fn a_checker_that_ran_and_said_nothing_is_an_acceptance() {
    assert_eq!(combine_outcomes(&[], None, 3), CedarToolOutcome::Accepted);
}

/// End to end: a pull request that DELETES its only Cedar policy file leaves
/// that path in the changed list and nothing on disk. Nothing is parsed, so
/// nothing may be certified on the strength of it.
#[tokio::test]
async fn a_policy_file_the_change_deleted_is_not_parsed_and_not_certified() {
    let dir = tempfile::tempdir().expect("tempdir");
    let diff_ctx = a_diff_touching(dir.path(), &["policies/cedar/gone.cedar"]);

    let report = CedarGuard::new()
        .evaluate_cedar_policies(dir.path(), &diff_ctx)
        .await;

    assert!(
        !matches!(report.status, GateStatus::Passed | GateStatus::AutoUpdated),
        "no policy file was on disk to parse, and the gate certified anyway: {:?}",
        report.status
    );
    reason_of(&report.status);
}

/// The empty-scope path must reach `no_policy_in_scope`, whose reason names the
/// schema, and not the "could not be read" reason that belongs to a scope that
/// had files in it.
#[tokio::test]
async fn a_change_touching_no_policy_file_reports_the_missing_schema() {
    let dir = tempfile::tempdir().expect("tempdir");
    let diff_ctx = a_diff_touching(dir.path(), &["src/main.rs"]);

    let report = CedarGuard::new()
        .evaluate_cedar_policies(dir.path(), &diff_ctx)
        .await;
    let reason = reason_of(&report.status);
    assert!(
        reason.to_lowercase().contains("schema"),
        "an empty scope must say what is missing before coverage could be \
         decided, not that a file could not be read: {reason}"
    );
}

// ---------------------------------------------------------------------------
// The gate must not spawn a model, and must not be able to abort a run.
// ---------------------------------------------------------------------------

/// `src/cedar_guard.rs` with its commentary stripped.
///
/// The module documents what the gate *used to* do, and naming the deleted
/// machinery is how a reader learns why it went. `//! It spawned `agy` at
/// `ExecClass::Model`` is a sentence, not a spawn. This is the repository's own
/// rule, stated in `fidelity_registry_citations_test.rs`: a claim is
/// answerable by code, not by the prose sitting next to it.
fn cedar_guard_production_source() -> String {
    anvil::source_scan::paths::module_source(
        "src/cedar_guard",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .lines()
    .filter(|l| !l.trim_start().starts_with("//"))
    .collect::<Vec<_>>()
    .join("\n")
}

/// Catches: the model comes back.
///
/// Not because a model is useless, but because this gate's question is not one
/// a model can be held to. `PolicySet::from_str` is a decision procedure over
/// the normative grammar and returns Ok on `principal.totallyFakeAttr`, because
/// without a schema that attribute may well be real. A model asked "is this
/// Cedar-compliant?" answers anyway, in both directions, and this repository
/// already has one gate that publishes a model's verdict
/// (`unresolved_review_status` infers from comment text). A second one that
/// *also* writes and commits authorization policy is not a gate.
#[test]
fn the_cedar_gate_spawns_no_model() {
    let src = cedar_guard_production_source();
    for needle in [
        "Command::new(\"agy\")",
        "ExecClass::Model",
        "agy_print_timeout_arg",
        "dangerously-skip-permissions",
    ] {
        assert!(
            !src.contains(needle),
            "src/cedar_guard.rs contains `{needle}`: the gate is paying for a \
             judgement again. Its verdict comes from a parser or from nothing"
        );
    }
}

/// Catches: the gate regains the ability to abort a whole certification run.
///
/// `certify.rs` called this guard with `.await?`, so an absent `agy` binary --
/// `run_bounded` bails with "failed to run: No such file or directory" -- did
/// not fail gate 2. It failed `certify_pull_request`, and the other
/// seventy-one gates never ran. A probe that could not run is `NotMeasured`;
/// it is not an outage.
#[tokio::test]
async fn a_missing_checker_reports_not_measured_instead_of_ending_the_run() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("p.cedar"),
        "permit(principal, action, resource);\n",
    )
    .expect("write policy");

    let diff_ctx = a_diff_touching(dir.path(), &["p.cedar"]);

    // No `?`, no `Result`: the signature itself is the fix. If this stops
    // compiling because a `Result` came back, the gate can abort a run again.
    let report = CedarGuard::new()
        .evaluate_cedar_policies(dir.path(), &diff_ctx)
        .await;

    // `cedar` is not installed on this runner. If it ever is, the policy above
    // is valid Cedar and the gate passes -- both outcomes are honest, and
    // neither is an error the caller has to handle.
    match &report.status {
        GateStatus::NotMeasured { gate_id, .. } => assert_eq!(gate_id, CEDAR_GATE_ID),
        GateStatus::Passed => {}
        other => panic!(
            "a valid policy and an absent checker are the only two things this \
             runner can produce, and neither is {other:?}"
        ),
    }
}

/// The other end of the same wire: the call site must not reintroduce the `?`.
#[test]
fn the_certification_pipeline_cannot_propagate_a_failure_out_of_this_gate() {
    let src = anvil::source_scan::paths::module_source(
        "src/webhook/pipelines/certify",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    let at = src
        .find("evaluate_cedar_policies")
        .expect("certify.rs must still run gate 2");
    let tail = &src[at..];
    let stmt_end = tail.find(';').expect("the call is a statement");
    assert!(
        !tail[..stmt_end].contains('?'),
        "certify.rs propagates this gate's error, so one gate failing to measure \
         takes the other seventy-one down with it:\n{}",
        &tail[..stmt_end]
    );
}
