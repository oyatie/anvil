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
//! `TBD`, `N/A`, `todo`, an unticked checkbox, an unfilled template comment,
//! and a bullet with nothing after the dash. The measurement is the content,
//! not the marker.
//!
//! # How this suite forces the gate to read the content
//!
//! A previous revision of this file was vacuous against a gate that never
//! looked under either heading: every must-pass fixture was longer than every
//! must-fail fixture, so `both headings present && body.len() >= 400` passed
//! the whole suite while accepting a done-when section whose entire content was
//! the literal string `TBD`. Three properties now close that off, and each is a
//! constraint on the fixtures, not on the implementation:
//!
//!   1. **Length is not monotone with the verdict, in either direction.** The
//!      failing set brackets the passing set at both ends: the shortest failing
//!      body is empty and the longest (`long_prose()` under a placeholder) is
//!      about 1.1 KB, while every passing body sits between roughly 90 bytes
//!      and 500. No threshold on total length can separate them.
//!   2. **Length is not monotone per section either.** `SHORT_BAR` is an
//!      eleven-byte bar that must pass; `"TODO: write the acceptance criteria
//!      here"` is a thirty-nine-byte placeholder that must fail. A minimum
//!      section length that admits the first admits the second.
//!   3. **Both sections are held to one standard.** The placeholder table runs
//!      through the problem position and the done-when position, so an
//!      implementation cannot screen the bar for substance and settle for
//!      "non-empty" on the bet.
//!
//! Character count and byte count are also deliberately decoupled: the Korean
//! fixtures are short in characters and long in bytes, so a heuristic in either
//! unit fails one of them.
//!
//! # What these tests deliberately do NOT pin
//!
//! The marker spelling. The fixtures below commit to one pair — `## Problem`
//! and `## Done when` — because a test has to write *something*, but every case
//! is distinguished by what sits **under** those headings, never by the heading
//! text. An implementation that recognises more spellings passes unchanged.
//!
//! Stage discipline: these are red tests, written before the gate exists.
//! `pre_merge_guard::product_bar::judge` is a `todo!()`, and the evaluator
//! carries a placeholder status rather than a call to it, so the wiring tests
//! at the bottom of this file are red for the same reason as the rest.

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

/// A genuine problem statement that happens to be one line long. Paired with
/// `SHORT_BAR` it is the smallest change that has still done Product's job, and
/// it is what stops "substantive" from collapsing into "long".
const SHORT_PROBLEM: &str = "Checkout p99 regressed to 40ms after the cache change.";

/// A genuine acceptance bar, eleven bytes of it. Shorter than four of the
/// placeholders that must fail.
const SHORT_BAR: &str = "- p99 < 5ms";

/// Every one of these has been shipped in a real pull request body. A gate that
/// reads any of them as an artifact is measuring the presence of a heading.
///
/// The last two are longer than `SHORT_BAR`, so a placeholder screen cannot
/// degenerate into a length threshold; the checkbox and the template comment are
/// the empty-bullet defect in the shape PR templates actually produce.
const PLACEHOLDERS: &[&str] = &[
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
    "- [ ] ",
    "<!-- what problem does this solve? -->",
    "TBD - will fill this in before merge",
    "TODO: write the acceptance criteria here",
];

/// A Korean problem statement. Short in characters, long in bytes.
const KO_PROBLEM: &str =
    "머지 큐가 조회되지 않은 카나리를 통과로 처리해서, 측정된 적 없는 변경이 승인된다.";

/// A Korean acceptance bar, written as checkable criteria.
const KO_BAR: &str = "- 조회되지 않은 카나리는 NotMeasured 로 보고하고 머지 큐 진입을 막는다\n\
     - 스코어카드가 둘 중 무엇이었는지 이름을 밝힌다";

const KO_TITLE: &str = "fix(certify): 조회되지 않은 카나리를 통과로 보고하지 않는다";

/// PR bodies authored in the GitHub web UI arrive over the webhook with CRLF,
/// because that is what an HTML textarea submits. Nothing in this repository
/// normalises line endings between the payload and the guard layer, so the
/// fixtures are built over both and the verdict must not depend on which.
#[derive(Clone, Copy, Debug)]
enum Eol {
    Lf,
    Crlf,
}

impl Eol {
    fn seq(self) -> &'static str {
        match self {
            Eol::Lf => "\n",
            Eol::Crlf => "\r\n",
        }
    }
}

/// The marker spelling these fixtures commit to. See the module docs: the
/// behaviour under test is what sits under the headings, never the headings.
fn body_with_eol(problem: &str, done_when: &str, eol: Eol) -> String {
    let n = eol.seq();
    format!("## Problem{n}{n}{problem}{n}{n}## Done when{n}{n}{done_when}{n}")
}

fn body_with(problem: &str, done_when: &str) -> String {
    body_with_eol(problem, done_when, Eol::Lf)
}

fn problem_only_eol(problem: &str, eol: Eol) -> String {
    let n = eol.seq();
    format!("## Problem{n}{n}{problem}{n}")
}

fn problem_only(problem: &str) -> String {
    problem_only_eol(problem, Eol::Lf)
}

fn bar_only(done_when: &str) -> String {
    format!("## Done when\n\n{done_when}\n")
}

/// Four paragraphs of genuine problem analysis and not one word about what done
/// looks like. Used on the *failing* side so that the longest body in the whole
/// suite is one that must fail.
fn long_prose() -> String {
    format!("{PROBLEM}\n\n{PROBLEM}\n\n{PROBLEM}\n\n{PROBLEM}")
}

/// The mirror: a long, real acceptance bar, used where the problem statement is
/// the thing that is missing.
fn long_bar() -> String {
    format!("{BAR}\n\n{BAR}\n\n{BAR}\n\n{BAR}")
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

/// The variant only, with no message. Used where two inputs must reach the same
/// verdict but may legitimately quote different text back at the author.
fn variant(status: &GateStatus) -> &'static str {
    match status {
        GateStatus::Passed => "Passed",
        GateStatus::AutoUpdated => "AutoUpdated",
        GateStatus::Warning(_) => "Warning",
        GateStatus::Failed(_) => "Failed",
        GateStatus::Errored(_) => "Errored",
        GateStatus::NotMeasured { .. } => "NotMeasured",
    }
}

fn names_the_bar(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("done-when") || m.contains("done when") || m.contains("acceptance")
}

fn names_the_problem(msg: &str) -> bool {
    msg.to_lowercase().contains("problem")
}

// ---------------------------------------------------------------------------
// The measurement: what passes
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
fn a_one_line_problem_and_a_one_line_bar_pass() {
    // The smallest change that has still done Product's job. `SHORT_BAR` is
    // eleven bytes — shorter than four of the placeholders in PLACEHOLDERS that
    // must fail — so no minimum length, in bytes or characters, can admit this
    // and reject those. The gate has to discriminate on what the words say.
    assert!(
        SHORT_BAR.len() < "TODO: write the acceptance criteria here".len(),
        "fixture invariant: the legitimate short bar must be shorter than the \
         longest placeholder, or this test stops forcing a content check"
    );

    let status = product_bar::judge(TITLE, &body_with(SHORT_PROBLEM, SHORT_BAR));
    assert_eq!(
        status,
        GateStatus::Passed,
        "a one-line bet and a one-line, checkable bar are the artifact. Rejecting \
         them because they are short measures effort, not the acceptance bar, and \
         accuses an author who did the job"
    );
}

#[test]
fn a_korean_problem_and_bar_pass() {
    // This corpus already carries Korean (src/compliance_guard/statutes.rs and
    // its siblings), so the gate will be handed non-ASCII bodies. Hangul is
    // three bytes per character: a byte-length rule and a character-length rule
    // disagree about this fixture, and the suite refuses to let either stand in
    // for the measurement.
    let body = body_with(KO_PROBLEM, KO_BAR);
    assert_ne!(
        body.len(),
        body.chars().count(),
        "fixture invariant: this body must have more bytes than characters, or it \
         stops separating a byte heuristic from a character heuristic"
    );

    let status = product_bar::judge(KO_TITLE, &body);
    assert_eq!(
        status,
        GateStatus::Passed,
        "a written problem and a done-when bar are the artifact in any language; \
         failing this accuses every author who does not write in English"
    );
}

// ---------------------------------------------------------------------------
// The measurement: what fails
// ---------------------------------------------------------------------------

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
    let status = product_bar::judge(TITLE, &problem_only(&long_prose()));
    let msg = expect_failed(status, "a long problem statement with no bar");
    assert!(
        names_the_bar(&msg),
        "the message must name the missing artifact so the author can act on it \
         without reading the gate's source; got: {msg:?}"
    );
    assert!(
        !names_the_problem(&msg),
        "this change stated its problem at length; telling the author the problem \
         statement is missing is a fabricated accusation, and it hides the one \
         thing they actually have to write; got: {msg:?}"
    );
}

#[test]
fn an_acceptance_bar_with_no_written_problem_fails_however_long_the_bar() {
    // The artifact is "written problem + done-when". A bar with no bet behind
    // it cannot be judged: there is nothing to say whether the bar is the right
    // bar. The long variant is the mirror of the test above — a substantial
    // done-when cannot carry an absent problem statement.
    for bar in [BAR.to_string(), long_bar()] {
        let status = product_bar::judge(TITLE, &bar_only(&bar));
        let msg = expect_failed(status, "a bar with no written problem");
        assert!(
            names_the_problem(&msg),
            "the message must name the missing artifact; got: {msg:?}"
        );
        assert!(
            !names_the_bar(&msg),
            "this change wrote its bar; naming the bar as missing tells the author \
             to write the one thing they already wrote; got: {msg:?}"
        );
    }
}

#[test]
fn a_heading_with_nothing_under_it_fails_however_much_the_other_section_says() {
    // The two long cases are the reason a global length threshold cannot
    // satisfy this suite: `long_prose()` under an empty done-when is the
    // longest body anywhere in this file, and it must fail.
    let cases: Vec<(String, &str)> = vec![
        (
            body_with(PROBLEM, ""),
            "the done-when heading is present with nothing under it",
        ),
        (
            body_with("", BAR),
            "the problem heading is present with nothing under it",
        ),
        (
            body_with("", ""),
            "both headings present, both empty — a pasted template is not the artifact",
        ),
        (
            body_with(&long_prose(), ""),
            "four paragraphs of problem analysis above an empty done-when heading; \
             length is not a bar",
        ),
        (
            body_with("", &long_bar()),
            "a long done-when above an empty problem heading; a bar with no bet \
             cannot be judged",
        ),
    ];

    let longest_failing = cases.iter().map(|(b, _)| b.len()).max().unwrap_or(0);
    assert!(
        longest_failing > body_with(PROBLEM, BAR).len(),
        "fixture invariant: some body that must fail has to be longer than every \
         body that must pass, or total length alone still separates the two sets"
    );

    for (body, context) in cases {
        expect_failed(product_bar::judge(TITLE, &body), context);
    }
}

#[test]
fn a_placeholder_fails_in_either_section_however_much_the_other_one_says() {
    // The table runs through both positions, against a short, a normal and a
    // four-paragraph counterpart. Two defects die here: a gate that screens the
    // done-when for substance and settles for "non-empty" on the problem, and a
    // gate that separates the sets by length.
    for placeholder in PLACEHOLDERS {
        for problem in [PROBLEM.to_string(), long_prose(), SHORT_PROBLEM.to_string()] {
            let msg = expect_failed(
                product_bar::judge(TITLE, &body_with(&problem, placeholder)),
                &format!("the done-when section contains only the placeholder {placeholder:?}"),
            );
            assert!(
                names_the_bar(&msg),
                "the message must name the missing bar; got: {msg:?}"
            );
            assert!(
                !names_the_problem(&msg),
                "the problem statement is present and real here; only the bar is \
                 missing, and the message must say so; got: {msg:?}"
            );
        }

        for bar in [BAR.to_string(), long_bar(), SHORT_BAR.to_string()] {
            let msg = expect_failed(
                product_bar::judge(TITLE, &body_with(placeholder, &bar)),
                &format!("the problem section contains only the placeholder {placeholder:?}"),
            );
            assert!(
                names_the_problem(&msg),
                "a change whose entire problem section is {placeholder:?} wrote no bet \
                 at all; the message must name the missing problem statement, or the \
                 Product seat certifies a template paste; got: {msg:?}"
            );
            assert!(
                !names_the_bar(&msg),
                "the bar is present and real here; got: {msg:?}"
            );
        }

        let msg = expect_failed(
            product_bar::judge(TITLE, &body_with(placeholder, placeholder)),
            &format!("both sections contain only the placeholder {placeholder:?}"),
        );
        assert!(
            names_the_problem(&msg) && names_the_bar(&msg),
            "with neither artifact written the message must name both; got: {msg:?}"
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
        names_the_bar(&msg) && names_the_problem(&msg),
        "an empty body carries neither artifact, so the message must name both; \
         got: {msg:?}"
    );
}

#[test]
fn each_failure_message_names_only_the_artifact_that_was_missing() {
    // One message per shape of absence, each naming what to write and nothing
    // else. The negative assertions are the point: a single generic string
    // naming both artifacts satisfies positive containment while telling an
    // author who wrote a good problem statement that their problem statement is
    // missing. A false accusation is the same defect as a false green, pointed
    // the other way.
    let both = expect_failed(product_bar::judge(TITLE, ""), "nothing written at all");
    assert!(
        names_the_problem(&both) && names_the_bar(&both),
        "with neither artifact present the message must name both; got: {both:?}"
    );

    let no_bar = expect_failed(
        product_bar::judge(TITLE, &problem_only(PROBLEM)),
        "problem written, bar missing",
    );
    assert!(
        names_the_bar(&no_bar),
        "must name the missing bar; got: {no_bar:?}"
    );
    assert!(
        !names_the_problem(&no_bar),
        "must not name the problem statement, which this change wrote; got: {no_bar:?}"
    );

    let no_problem = expect_failed(
        product_bar::judge(TITLE, &bar_only(BAR)),
        "bar written, problem missing",
    );
    assert!(
        names_the_problem(&no_problem),
        "must name the missing problem statement; got: {no_problem:?}"
    );
    assert!(
        !names_the_bar(&no_problem),
        "must not name the bar, which this change wrote; got: {no_problem:?}"
    );

    assert_ne!(
        both, no_bar,
        "three different absences cannot share one message; an author has to be \
         able to tell from it which artifact to go and write"
    );
    assert_ne!(both, no_problem, "same, for the missing problem statement");
    assert_ne!(
        no_bar, no_problem,
        "the missing-bar message and the missing-problem message must differ, or \
         the gate is naming an artifact it did not check"
    );
}

#[test]
fn a_korean_problem_with_no_bar_fails_naming_the_missing_bar() {
    let status = product_bar::judge(KO_TITLE, &problem_only(KO_PROBLEM));
    let msg = expect_failed(status, "a Korean problem statement with no bar");
    assert!(
        names_the_bar(&msg),
        "the message must name the missing bar whatever script the body is in; \
         got: {msg:?}"
    );
}

#[test]
fn judge_returns_a_verdict_for_any_body_and_never_panics() {
    // A panic inside `judge` is not a Failed gate: it unwinds
    // `evaluate_pre_merge_gates` and takes the whole review with it. The
    // obvious way to write one is to quote an excerpt of the body back at the
    // author — `&pr_body[..40]` — which is a byte index, and byte 40 lands
    // inside a character in most of the bodies below.
    let mut awkward: Vec<String> = vec![
        String::new(),
        "카나리".to_string(),
        "## Problem\n\n카나리 게이트가 잘못된 판정을 만든다\n".to_string(),
        "## Problem\n\n한\n\n## Done when\n\nTBD\n".to_string(),
        "🚀 배포가 못 된다".to_string(),
        "## Problem\n\n한국어 problem 混合 テキスト\n\n## Done when\n\n\n".to_string(),
        "\r\r\r".to_string(),
    ];
    // Every byte index from 0 to 47 in this body is either inside "## Problem\n\n"
    // or inside a three-byte Hangul syllable, so any fixed-offset slice of it
    // that is not a character boundary panics.
    awkward.push(format!("## Problem\n\n{}", "가".repeat(12)));

    for body in &awkward {
        // The call itself is the assertion: it must return rather than unwind.
        let status = product_bar::judge(KO_TITLE, body);
        assert_ne!(
            variant(&status),
            "Passed",
            "none of these bodies carries both a written problem and a done-when \
             bar, so none of them may certify the Product seat; body: {body:?}"
        );
    }
}

#[test]
fn the_verdict_is_the_same_whether_the_body_uses_lf_or_crlf() {
    // GitHub's web UI submits textarea content with CRLF, and nothing between
    // the webhook payload and the guard layer normalises it. A gate anchored on
    // `"## Done when\n"` fails essentially every human-authored pull request
    // while a suite built only from `\n` stays green.
    /// A fixture built over whichever line ending it is handed.
    type BodyBuilder = Box<dyn Fn(Eol) -> String>;

    let cases: Vec<(&str, BodyBuilder)> = vec![
        (
            "a written problem and a done-when bar",
            Box::new(|eol| body_with_eol(PROBLEM, BAR, eol)),
        ),
        (
            "a problem statement with no bar",
            Box::new(|eol| problem_only_eol(PROBLEM, eol)),
        ),
        (
            "a done-when heading with nothing under it",
            Box::new(|eol| body_with_eol(PROBLEM, "", eol)),
        ),
    ];

    for (name, build) in &cases {
        let lf = product_bar::judge(TITLE, &build(Eol::Lf));
        let crlf = product_bar::judge(TITLE, &build(Eol::Crlf));
        assert_eq!(
            variant(&lf),
            variant(&crlf),
            "{name}: the same change reached a different verdict because its line \
             endings came from a browser rather than an editor. lf={lf:?} crlf={crlf:?}"
        );
    }

    // Without this, "Failed under both line endings" would satisfy the loop
    // above and the gate could still reject every web-authored change.
    assert_eq!(
        product_bar::judge(TITLE, &body_with_eol(PROBLEM, BAR, Eol::Crlf)),
        GateStatus::Passed,
        "a complete Product artifact typed into the GitHub web UI must pass; \
         rejecting it blocks the majority of real pull requests"
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

// ---------------------------------------------------------------------------
// The wiring: the gate has to run on a real change
// ---------------------------------------------------------------------------
//
// `evaluate_pre_merge_gates` takes roughly fifty guard reports, so calling it
// from a test is not viable and nothing in the suite above can tell a perfect
// `product_bar::judge` apart from `let product_bar_status = GateStatus::Passed`.
// The existing backstop does not close it either:
// `every_computed_gate_reaches_the_report_test` only checks that a computed
// status is carried, so a literal satisfies it. The result would be a named gate
// rendered in the scorecard, counted in TOTAL_GATES, and blocking nothing —
// `review_verdict_status` reproduced one level up.
//
// This repository already sanctions source inspection for exactly this invariant
// class (`tests/evaluator_gate_ordering_test.rs` asserts on the evaluator's text,
// and `every_computed_gate_reaches_the_report_test.rs` parses the report literal),
// so these three follow that idiom.

fn source(rel: &str) -> String {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The argument text of every `judge(...)` call in `src`, parens balanced.
fn judge_call_arguments(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(i) = src[from..].find("judge(") {
        let open = from + i + "judge".len();
        let mut depth = 0i32;
        let mut end = None;
        for (k, c) in src[open..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(open + k);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(end) = end {
            out.push(src[open + 1..end].to_string());
        }
        from = open + 1;
    }
    out
}

#[test]
fn the_evaluator_receives_the_change_under_review() {
    let src = source("src/pre_merge_guard/evaluator.rs");
    let start = src
        .find("pub fn evaluate_pre_merge_gates(")
        .expect("the evaluator declares evaluate_pre_merge_gates");
    let tail = &src[start..];
    let end = tail
        .find(") -> ")
        .expect("evaluate_pre_merge_gates has a return type");
    let signature = &tail[..end];

    assert!(
        signature.contains("pr_title"),
        "evaluate_pre_merge_gates never receives the change's title. The Product \
         seat's artifact is authored on the change under review; a gate cannot \
         measure text the evaluator is never given, and a gate that measures \
         nothing gates nothing"
    );
    assert!(
        signature.contains("pr_body"),
        "evaluate_pre_merge_gates never receives the change's body, which is where \
         the written problem and the done-when bar live"
    );
    assert!(
        signature.find("pr_title") < signature.find("pr_body"),
        "the title parameter must come before the body parameter. Both are &str, \
         so swapping them at any call site compiles in silence and the gate then \
         judges every pull request on its title alone"
    );
}

#[test]
fn the_evaluator_computes_product_bar_status_by_judging_that_change() {
    let src = source("src/pre_merge_guard/evaluator.rs");

    let calls = judge_call_arguments(&src);
    assert!(
        !calls.is_empty(),
        "src/pre_merge_guard/evaluator.rs never calls product_bar::judge, so \
         product_bar_status is not derived from the change under review. A gate \
         computed from nothing gates nothing: it is named on the scorecard, counted \
         in TOTAL_GATES, and blocks no pull request"
    );

    for args in &calls {
        assert!(
            args.contains("pr_title") && args.contains("pr_body"),
            "the Product gate must be judged over the change's own title and body; \
             got judge({args})"
        );
        assert!(
            args.find("pr_title") < args.find("pr_body"),
            "the arguments are swapped. Both are &str so this compiles silently, and \
             the gate then looks for the acceptance bar in the title and the title in \
             the body — every real pull request fails. got judge({args})"
        );
    }

    let hardcoded: Vec<String> = src
        .lines()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim_start();
            !t.starts_with("//") && t.contains("product_bar_status") && t.contains("GateStatus::")
        })
        .map(|(i, l)| format!("{}: {}", i + 1, l.trim()))
        .collect();
    assert!(
        hardcoded.is_empty(),
        "product_bar_status is assigned a literal status instead of the verdict of a \
         measurement: {hardcoded:?}. A literal Passed certifies every change, and a \
         literal NotMeasured leaves a named gate on the scorecard that measures \
         nothing forever — which is the pattern this gate exists to forbid"
    );
}

#[test]
fn the_review_pipeline_hands_the_evaluator_the_title_then_the_body() {
    // The same silent swap is available one level up: the pipeline already
    // passes `title, body` in that order to `ensure_documentation_parity`, and
    // the evaluator must be fed the same way.
    let src = source("src/webhook/pipelines/review.rs");
    let start = src
        .find("evaluate_pre_merge_gates(")
        .expect("the review pipeline evaluates the pre-merge gates");
    let tail = &src[start..];
    let args: Vec<String> = tail
        .lines()
        .skip(1)
        .take_while(|l| !l.trim_start().starts_with(')'))
        .map(|l| l.trim().trim_end_matches(',').to_string())
        .collect();

    let title_at = args.iter().position(|a| a.contains("title"));
    let body_at = args.iter().position(|a| a.contains("body"));

    assert!(
        title_at.is_some(),
        "the review pipeline never hands the evaluator the pull request title, so \
         the Product gate has nothing to read: {args:?}"
    );
    assert!(
        body_at.is_some(),
        "the review pipeline never hands the evaluator the pull request body, which \
         is where the written problem and the done-when bar are authored: {args:?}"
    );
    assert!(
        title_at < body_at,
        "the pipeline passes the body where the evaluator expects the title. Both \
         are &str, so this compiles and every pull request is then judged on the \
         wrong text: {args:?}"
    );
}
