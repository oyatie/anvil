//! The Product seat: a change must state its bet and its acceptance bar.
//!
//! ADR-0002 Discover §1 — "Product. Job: the bet and the acceptance bar.
//! Artifact: written problem + done-when. Measurement: Quality cannot sign off
//! without it." The sequence section adds the consequence: "Quality sign-off
//! must fail if Product's bar is missing."
//!
//! # Why absence is `Failed`, not `NotMeasured`
//!
//! `NotMeasured` exists for a gate that was asked to judge and had nothing to
//! read — no telemetry endpoint, no shape spec adopted. That is not this. The
//! artifact is authored on the change under review, by the person opening it.
//! A change with no bar has not withheld evidence from the gate; it has failed
//! to produce the artifact. Reporting that as `NotMeasured` or `Warning` would
//! let every change certify while the seat measures nothing, which is the
//! "named gate, no measurement" pattern the ADR's honesty law forbids.
//!
//! # Why an empty heading must fail
//!
//! A gate that accepts `## Done when` followed by nothing is a shallow check
//! wrapped as a measurement: it rewards pasting a template. The same is true of
//! `TBD`, `N/A`, `todo`, and a bullet with nothing after the dash. The
//! measurement is the content, not the marker.
//!
//! # What these tests deliberately do NOT pin
//!
//! The marker spelling. The fixtures below commit to one pair — `## Problem`
//! and `## Done when` — because a test has to write *something*, but every case
//! is distinguished by what sits **under** those headings, never by the heading
//! text. An implementation that recognises more spellings passes unchanged.
//!
//! Stage discipline: these are red tests, written before the gate exists.
//! `pre_merge_guard::product_bar::judge` is a `todo!()`.

use anvil::pre_merge_guard::product_bar;
use anvil::pre_merge_guard::report::TOTAL_GATES;
use anvil::pre_merge_guard::{GateStatus, PreMergeCertificationReport};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const TITLE: &str = "fix(certify): stop reporting an unread canary as passed";

/// A real problem statement: what is wrong, and why it matters.
const PROBLEM: &str = "The canary gate rebuilds its verdict from `passed`, which is `true` for a \
     canary nobody queried. Every change that touches the rollout path is \
     certified against a measurement that never happened, so the scorecard \
     reads green for the exact condition the gate exists to catch.";

/// A real acceptance bar: the condition under which this is done, stated so
/// that someone other than the author can check it.
const BAR: &str = "An unqueried canary reports NotMeasured and withholds merge-queue admission; a \
     queried canary with divergent P99 reports Failed; the scorecard names which \
     of the two happened.";

/// The marker spelling these fixtures commit to. See the module docs: the
/// behaviour under test is what sits under the headings, never the headings.
fn body_with(problem: &str, done_when: &str) -> String {
    format!("## Problem\n\n{problem}\n\n## Done when\n\n{done_when}\n")
}

fn problem_only(problem: &str) -> String {
    format!("## Problem\n\n{problem}\n")
}

fn bar_only(done_when: &str) -> String {
    format!("## Done when\n\n{done_when}\n")
}

/// Asserts the gate blocked, and returns the message it blocked with.
///
/// `Failed` specifically: `Warning` and `NotMeasured` both certify, and
/// `Errored` would claim the gate tried to read something and could not.
#[track_caller]
fn expect_failed(status: GateStatus, context: &str) -> String {
    match status {
        GateStatus::Failed(msg) => msg,
        other => panic!(
            "{context}: expected Failed, got {other:?}. Absence of Product's bar is the \
             defect itself — reporting it any other way lets the change certify, and \
             quality sign-off is then signing off on nothing.",
        ),
    }
}

fn mentions_the_bar(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("done-when") || m.contains("done when") || m.contains("acceptance")
}

fn mentions_the_problem(msg: &str) -> bool {
    msg.to_lowercase().contains("problem")
}

// ---------------------------------------------------------------------------
// The measurement
// ---------------------------------------------------------------------------

#[test]
fn a_change_carrying_a_written_problem_and_a_done_when_bar_passes() {
    let status = product_bar::judge(TITLE, &body_with(PROBLEM, BAR));
    assert_eq!(
        status,
        GateStatus::Passed,
        "a change that states both the bet and the bar has produced the Product \
         artifact; failing it would be a fabricated accusation, which is the same \
         defect as a false green pointed the other way"
    );
}

#[test]
fn a_bar_written_as_measurable_criteria_passes() {
    // The bar is far more often a list than a sentence. A gate that only
    // accepts prose would push authors back to writing nothing.
    let criteria = "- `slo_status` is NotMeasured when no telemetry endpoint is configured\n\
                    - `is_admissible()` is false while any gate is NotMeasured\n\
                    - the posted scorecard names every unmeasured gate by id";
    let status = product_bar::judge(TITLE, &body_with(PROBLEM, criteria));
    assert_eq!(
        status,
        GateStatus::Passed,
        "an acceptance bar expressed as checkable criteria is the artifact, not a \
         lesser form of it"
    );
}

#[test]
fn a_change_with_no_bar_at_all_is_failed_not_merely_unmeasured() {
    let status = product_bar::judge("", "");
    assert!(
        status.is_measured(),
        "the gate read the change and found no bar; that is a measurement, and \
         recording it as NotMeasured would hide the defect behind honest-looking \
         bookkeeping"
    );
    assert!(
        !status.is_acceptable(),
        "an acceptable status certifies; quality cannot sign off without the bar"
    );
    expect_failed(status, "a change with no problem statement and no bar");
}

#[test]
fn a_problem_statement_with_no_acceptance_bar_fails_however_long_the_prose() {
    // Guards the shallow check "the body is long, so it must say something".
    // This body is four paragraphs of genuine problem analysis and contains no
    // statement of what done looks like.
    let long_prose = format!("{PROBLEM}\n\n{PROBLEM}\n\n{PROBLEM}\n\n{PROBLEM}");
    let status = product_bar::judge(TITLE, &problem_only(&long_prose));
    let msg = expect_failed(status, "a long problem statement with no bar");
    assert!(
        mentions_the_bar(&msg),
        "the message must name the missing artifact so the author can act on it \
         without reading the gate's source; got: {msg:?}"
    );
}

#[test]
fn an_acceptance_bar_with_no_written_problem_fails() {
    // The artifact is "written problem + done-when". A bar with no bet behind
    // it cannot be judged: there is nothing to say whether the bar is the right
    // bar.
    let status = product_bar::judge(TITLE, &bar_only(BAR));
    let msg = expect_failed(status, "a bar with no written problem");
    assert!(
        mentions_the_problem(&msg),
        "the message must name the missing artifact; got: {msg:?}"
    );
}

#[test]
fn a_heading_with_nothing_under_it_fails() {
    let status = product_bar::judge(TITLE, &body_with(PROBLEM, ""));
    expect_failed(
        status,
        "the done-when heading is present with nothing under it",
    );

    let status = product_bar::judge(TITLE, &body_with("", BAR));
    expect_failed(
        status,
        "the problem heading is present with nothing under it",
    );

    let status = product_bar::judge(TITLE, &body_with("", ""));
    expect_failed(
        status,
        "both headings present, both empty — a pasted template is not the artifact",
    );
}

#[test]
fn a_placeholder_bar_fails() {
    // Every one of these has been shipped in a real pull request body. A gate
    // that reads them as an acceptance bar measures the presence of a heading.
    for placeholder in [
        "TBD",
        "tbd",
        "N/A",
        "n/a",
        "TODO",
        "todo",
        "-",
        "- ",
        "*",
        "...",
        "   \n   \n",
    ] {
        let status = product_bar::judge(TITLE, &body_with(PROBLEM, placeholder));
        expect_failed(
            status,
            &format!("the done-when section contains only the placeholder {placeholder:?}"),
        );
    }
}

#[test]
fn a_descriptive_title_is_not_a_substitute_for_the_bar() {
    // A well-written title says what changed. It never says what done looks
    // like, and accepting it would let every conventional-commit title certify
    // the Product seat.
    let status = product_bar::judge(
        "fix(certify): stop reporting an unqueried canary as passed so the scorecard \
         reflects what was actually measured",
        "",
    );
    let msg = expect_failed(status, "a descriptive title with an empty body");
    assert!(
        mentions_the_bar(&msg),
        "the message must name the missing bar; got: {msg:?}"
    );
}

#[test]
fn the_failure_message_names_the_artifact_that_was_missing() {
    // One message per shape of absence, each naming what to write. A single
    // generic string for all three would tell the author a gate failed and
    // nothing else.
    let both = expect_failed(product_bar::judge(TITLE, ""), "nothing written at all");
    assert!(
        mentions_the_problem(&both) && mentions_the_bar(&both),
        "with neither artifact present the message must name both; got: {both:?}"
    );

    let no_bar = expect_failed(
        product_bar::judge(TITLE, &problem_only(PROBLEM)),
        "problem written, bar missing",
    );
    assert!(
        mentions_the_bar(&no_bar),
        "must name the missing bar; got: {no_bar:?}"
    );

    let no_problem = expect_failed(
        product_bar::judge(TITLE, &bar_only(BAR)),
        "bar written, problem missing",
    );
    assert!(
        mentions_the_problem(&no_problem),
        "must name the missing problem statement; got: {no_problem:?}"
    );
}

// ---------------------------------------------------------------------------
// The corpus and the certification verdict
// ---------------------------------------------------------------------------

#[test]
fn the_product_bar_gate_joins_the_corpus_without_desynchronising_the_declared_total() {
    let report = PreMergeCertificationReport::unmeasured("fixture: nothing measured");

    let names: Vec<&str> = report
        .named_statuses()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert!(
        names.contains(&"product_bar_status"),
        "the new gate is a struct field that all_statuses() and named_statuses() \
         cannot see, so seal() cannot gate on it and the scorecard cannot name it. \
         That is how review_verdict_status stopped mattering. Present names: {names:?}"
    );

    assert_eq!(
        report.named_statuses().len(),
        report.all_statuses().len(),
        "the two listings must stay aligned, or a gate is reported in one and \
         invisible in the other"
    );
    assert_eq!(
        report.all_statuses().len(),
        TOTAL_GATES,
        "the corpus grew but TOTAL_GATES did not; TOTAL_GATES is published onto \
         pull requests, so every count claim it feeds is now wrong"
    );
}

#[test]
fn a_missing_product_bar_withholds_certification() {
    // The ADR's consequence, at the level where it bites: "Quality sign-off
    // must fail if Product's bar is missing."
    let mut report = PreMergeCertificationReport::unmeasured("fixture: nothing measured");
    assert!(
        report.is_certified_ready,
        "sanity: NotMeasured is individually acceptable, so this fixture certifies \
         before the Product gate speaks"
    );

    report.product_bar_status =
        GateStatus::Failed("no done-when acceptance bar on the change".to_string());
    report.seal();

    assert!(
        !report.is_certified_ready,
        "a change with no acceptance bar was certified anyway; the Product gate is \
         carried on the report but not wired into the verdict"
    );
}
