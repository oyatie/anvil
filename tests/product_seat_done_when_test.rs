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
//! This is the whole ballgame, so it is pinned on the input shape where a
//! hurried gate is most tempted to fail open: a body that uses none of the
//! headings the gate happens to recognise. Ordinary prose and an unfilled
//! template are the two commonest real pull request bodies there are, and both
//! must be `Failed` — see `a_real_pull_request_body_with_no_bar_fails_closed`.
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
//! Two earlier revisions of this file were each vacuous against a different
//! cheap gate, and the fixtures now close both off. Every property below is a
//! constraint on the fixtures, not on the implementation.
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
//!   3. **Both sections are held to one standard.** Every placeholder family
//!      runs through the problem position and the done-when position, so an
//!      implementation cannot screen the bar for substance and settle for
//!      "non-empty" on the bet.
//!   4. **The must-fail content is not enumerable.** A previous revision drew
//!      every must-fail fixture from one fifteen-entry table, so a hardcoded
//!      exact-match copy of that table was a complete implementation — and it
//!      shipped a gate that passed a done-when reading `TBD.` or `WIP`.
//!      `derived_deferrals()` multiplies stems by trailing punctuation, letter
//!      case and bullet wrapping into hundreds of strings that appear nowhere
//!      as literals; `PHRASE_DEFERRALS` adds deferrals sharing no prefix with
//!      anything in `PLACEHOLDERS`; and `UNICODE_BLANKS` adds sections that a
//!      `trim().is_empty()` check reads as substantive.
//!   5. **No body may fail open.** Every fixture that carries neither artifact
//!      reaches `expect_failed`, never `assert_ne!(.., "Passed")`, so
//!      `NotMeasured`, `Warning` and `Errored` are rejected everywhere.
//!
//! Character count and byte count are also deliberately decoupled: the Korean
//! fixtures are short in characters and long in bytes, so a heuristic in either
//! unit fails one of them.
//!
//! # Why the measurement is a function and not a string
//!
//! `product_bar::missing_artifacts` returns which halves of the artifact are
//! absent; `judge` renders the verdict and the message from it. The tests
//! assert the set, and assert only *positively* on the message (it must name
//! each missing artifact). An earlier revision asserted the negative as a raw
//! substring ban — "a missing-bar message must not contain the word problem" —
//! which turned a correct, helpful implementation red for quoting the offending
//! section back at the author. Do-not-falsely-accuse is a property of the
//! measurement, not of the vocabulary of the prose.
//!
//! # What these tests deliberately do NOT pin
//!
//! The marker spelling. The fixtures below commit to one pair — `## Problem`
//! and `## Done when` — because a test has to write *something*, but every case
//! is distinguished by what sits **under** those headings, never by the heading
//! text. An implementation that recognises more spellings, or recognises the
//! artifacts in unheaded prose, passes unchanged: no test here requires a
//! marker-less body that genuinely states both artifacts to fail.
//!
//! Stage discipline: these are red tests, written before the gate exists.
//! `pre_merge_guard::product_bar::{judge, missing_artifacts}` are `todo!()`,
//! and the evaluator carries a placeholder status rather than a call to them,
//! so the wiring tests at the bottom of this file are red for the same reason
//! as the rest.

use anvil::pre_merge_guard::product_bar;
use anvil::pre_merge_guard::product_bar::Artifact;
use anvil::pre_merge_guard::report::TOTAL_GATES;
use anvil::pre_merge_guard::{GateStatus, PreMergeCertificationReport};
use std::collections::BTreeSet;

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
///
/// This table is deliberately **not** the whole must-fail set. Copying it into
/// the implementation as an exact-match list satisfies these fifteen and
/// nothing else — see `derived_deferrals`, `PHRASE_DEFERRALS` and
/// `UNICODE_BLANKS`.
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

/// Deferral stems. These are never enumerated as finished strings: the tests
/// multiply them out by trailing punctuation, letter case and bullet wrapping,
/// so the hundreds of must-fail sections they produce appear nowhere in this
/// file as literals an implementation could copy. Normalising before comparing
/// is the cheapest way through, and normalising is what "the measurement is the
/// content" asks for.
const DEFERRAL_STEMS: &[&str] = &[
    "tbd",
    "tba",
    "n/a",
    "na",
    "todo",
    "to do",
    "wip",
    "xxx",
    "???",
    "-",
    "_",
    "[ ]",
    "[x]",
    "- [x]",
    "todo(jason)",
];

/// Deferrals that share no prefix with any entry in `PLACEHOLDERS`, so a table
/// copied from that constant cannot reach them. Each is a real thing authors
/// write in a done-when section instead of an acceptance bar.
///
/// The third and fourth pin a product decision as much as a technical one: the
/// artifact lives on the change under review, so a pointer to somewhere else is
/// not the artifact. That is listed in open_questions for a human to veto.
const PHRASE_DEFERRALS: &[&str] = &[
    "see the linked issue",
    "same as above",
    "will fill this in later",
    "as discussed in standup",
];

/// Sections that are blank to a reader and non-blank to `trim()`. U+200B and
/// U+FEFF are not `char::is_whitespace`, so `!s.trim().is_empty()` reads them
/// as substance; U+00A0, U+2003 and U+3000 arrive whenever anyone pastes out of
/// a document editor.
const UNICODE_BLANKS: &[&str] = &[
    "\u{00a0}",
    "\u{200b}",
    "\u{feff}",
    "\u{00a0}\u{00a0}\n\u{00a0}",
    "\u{2003}\u{200b}\n\u{feff}",
    "\u{3000}",
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

/// `stem` in lower case, upper case and sentence case — the three shapes a
/// human actually types a deferral in.
fn case_shapes(stem: &str) -> Vec<String> {
    let lower = stem.to_lowercase();
    let upper = stem.to_uppercase();
    let sentence = {
        let mut cs = lower.chars();
        match cs.next() {
            Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(),
            None => String::new(),
        }
    };
    vec![lower, upper, sentence]
}

/// Every deferral stem crossed with trailing punctuation, letter case and the
/// bullet wrappers a markdown section arrives in.
///
/// Derived rather than listed on purpose: an implementation that satisfies this
/// set by enumeration has to enumerate several hundred strings it cannot read
/// off this file, which is more work than normalising and comparing.
fn derived_deferrals() -> Vec<String> {
    let trailers = ["", ".", "!", "?", ":", "...", " -"];
    let wrappers: [fn(&str) -> String; 4] = [
        |s| s.to_string(),
        |s| format!("- {s}"),
        |s| format!("* {s}"),
        |s| format!("  {s}  "),
    ];

    let mut out: BTreeSet<String> = BTreeSet::new();
    for stem in DEFERRAL_STEMS {
        for shape in case_shapes(stem) {
            for trailer in trailers {
                let token = format!("{shape}{trailer}");
                for wrap in wrappers {
                    out.insert(wrap(&token));
                }
            }
        }
    }
    out.into_iter().collect()
}

/// Asserts the gate blocked, and returns the message it blocked with.
///
/// `Failed` specifically: `Warning` and `NotMeasured` both certify, and
/// `Errored` would claim the gate tried to read something and could not.
#[track_caller]
fn expect_failed(status: &GateStatus, context: &str) -> String {
    match status {
        GateStatus::Failed(msg) => msg.clone(),
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

fn names(artifact: Artifact, msg: &str) -> bool {
    match artifact {
        Artifact::WrittenProblem => names_the_problem(msg),
        Artifact::DoneWhenBar => names_the_bar(msg),
    }
}

fn missing(title: &str, body: &str) -> Vec<Artifact> {
    let mut got = product_bar::missing_artifacts(title, body);
    got.sort();
    got.dedup();
    got
}

/// The change produced both artifacts: `judge` passes and the measurement finds
/// nothing missing. Asserting both keeps the verdict and the message rendered
/// from one measurement rather than two disagreeing ones.
#[track_caller]
fn expect_passed(title: &str, body: &str, context: &str) {
    let status = product_bar::judge(title, body);
    assert_eq!(
        status,
        GateStatus::Passed,
        "{context}: this change produced the Product artifact; failing it is a \
         fabricated accusation, which is the same defect as a false green pointed \
         the other way. body={body:?}"
    );
    assert!(
        missing(title, body).is_empty(),
        "{context}: judge() passed the change while missing_artifacts() still reports \
         {:?} absent. The verdict and the message must be rendered from one \
         measurement, or the scorecard and the comment contradict each other",
        missing(title, body)
    );
}

/// The change did not produce these artifacts, and only these.
///
/// Asserts the measurement exactly (so naming an artifact the author actually
/// wrote is caught as a fabricated accusation, without banning any word from the
/// prose), asserts the verdict is `Failed` (not `Warning`, `NotMeasured` or
/// `Errored`), and asserts the message names each missing artifact so the author
/// can act on it without reading the gate's source. Returns the message.
#[track_caller]
fn expect_missing(title: &str, body: &str, expected: &[Artifact], context: &str) -> String {
    let mut want = expected.to_vec();
    want.sort();
    assert_eq!(
        missing(title, body),
        want,
        "{context}: the gate measured the wrong set of missing artifacts. Naming an \
         artifact the author did write tells them to write the thing they already \
         wrote; failing to name one they did not write hides the work. body={body:?}"
    );
    assert_failed_naming(title, body, &want, context)
}

/// The change is missing *at least* these artifacts.
///
/// Used where pinning the set exactly would decide something the specification
/// leaves open. A body of unheaded prose states no acceptance bar, so the bar is
/// missing beyond argument; whether that same prose also counts as the written
/// problem is a marker-recognition choice this suite deliberately leaves to the
/// implementer, and pinning it either way would be pinning the marker format.
#[track_caller]
fn expect_at_least_missing(
    title: &str,
    body: &str,
    expected: &[Artifact],
    context: &str,
) -> String {
    let got = missing(title, body);
    for artifact in expected {
        assert!(
            got.contains(artifact),
            "{context}: the gate did not report the missing {artifact:?}. It reported \
             {got:?}. body={body:?}"
        );
    }
    assert_failed_naming(title, body, &got, context)
}

/// The shared tail of both: `Failed`, measured, unacceptable, and a message that
/// names every artifact the measurement found missing.
#[track_caller]
fn assert_failed_naming(title: &str, body: &str, want: &[Artifact], context: &str) -> String {
    assert!(
        !want.is_empty(),
        "{context}: this is the failing side, so the measurement must report at least \
         one missing artifact; use expect_passed otherwise. body={body:?}"
    );

    let status = product_bar::judge(title, body);
    assert!(
        status.is_measured(),
        "{context}: the gate read the change and found {want:?} missing; that is a \
         measurement, and recording it as NotMeasured hides the defect behind \
         honest-looking bookkeeping. body={body:?}"
    );
    assert!(
        !status.is_acceptable(),
        "{context}: an acceptable status certifies, and quality cannot sign off \
         without Product's bar. body={body:?}"
    );
    let msg = expect_failed(&status, context);
    for artifact in want {
        assert!(
            names(*artifact, &msg),
            "{context}: the message must name the missing {artifact:?} so the author \
             can act on it without reading the gate's source; got {msg:?}"
        );
    }
    msg
}

// ---------------------------------------------------------------------------
// The measurement: what passes
// ---------------------------------------------------------------------------

#[test]
fn a_change_carrying_a_written_problem_and_a_done_when_bar_passes() {
    expect_passed(
        TITLE,
        &body_with(PROBLEM, BAR),
        "a written problem and a done-when bar",
    );
}

#[test]
fn a_bar_written_as_measurable_criteria_passes() {
    // The bar is far more often a list than a sentence. A gate that only
    // accepts prose would push authors back to writing nothing.
    let criteria = "- `slo_status` is NotMeasured when no telemetry endpoint is configured\n\
                    - `is_admissible()` is false while any gate is NotMeasured\n\
                    - the posted scorecard names every unmeasured gate by id";
    expect_passed(
        TITLE,
        &body_with(PROBLEM, criteria),
        "an acceptance bar expressed as checkable criteria is the artifact, not a \
         lesser form of it",
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

    expect_passed(
        TITLE,
        &body_with(SHORT_PROBLEM, SHORT_BAR),
        "a one-line bet and a one-line, checkable bar are the artifact; rejecting \
         them because they are short measures effort and accuses an author who did \
         the job",
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

    expect_passed(
        KO_TITLE,
        &body,
        "a written problem and a done-when bar are the artifact in any language; \
         failing this accuses every author who does not write in English",
    );
}

#[test]
fn the_two_sections_may_be_written_in_either_order() {
    // Order is presentation. An author who states the bar first has produced
    // both artifacts, and a section extractor that assumes the problem comes
    // first either mis-slices this body or panics on it — see
    // `judge_returns_a_verdict_for_any_body_and_never_panics`.
    expect_passed(TITLE, &body_with(PROBLEM, BAR), "the problem stated first");
    expect_passed(
        TITLE,
        &format!("## Done when\n\n{BAR}\n\n## Problem\n\n{PROBLEM}\n"),
        "the done-when stated first",
    );
}

// ---------------------------------------------------------------------------
// The measurement: what fails
// ---------------------------------------------------------------------------

#[test]
fn a_change_with_no_bar_at_all_is_failed_not_merely_unmeasured() {
    expect_missing(
        "",
        "",
        &[Artifact::WrittenProblem, Artifact::DoneWhenBar],
        "a change with no problem statement and no bar",
    );
}

#[test]
fn a_real_pull_request_body_with_no_bar_fails_closed() {
    // THE HEADLINE CASE. Every other failing fixture in this file is built from
    // this file's own heading template, which is the one input shape where an
    // implementation has no temptation to fail open. These are the two bodies
    // people actually submit: ordinary prose with no headings at all, and a
    // template nobody filled in. A gate that answers `NotMeasured` here — "the
    // body carries no section I recognise" — is acceptable to
    // `is_certified_ready`, so it certifies the majority of real pull requests
    // while the scorecard names a Product gate that measured nothing. That is
    // precisely the false green this seat exists to prevent.
    //
    // None of these bodies states an acceptance criterion in any spelling, so
    // none of them collides with the gate's freedom over the marker format: the
    // point is that no bar exists, not that no heading exists. For the same
    // reason the first three are pinned with `expect_at_least_missing` —
    // whether unheaded prose also counts as the written problem is a
    // recognition choice this suite leaves open, while the absence of the bar is
    // not open at all.
    let at_least: Vec<(&str, String)> = vec![
        (
            "plain prose with no headings — the commonest real body there is",
            "Refactors the canary poller onto the shared HTTP client so the retry budget \
             is configured in one place instead of three. No behaviour change is intended."
                .to_string(),
        ),
        (
            "a one-line description, the shape most drive-by fixes carry",
            "Bumps the tracing subscriber to 0.3.19.".to_string(),
        ),
        (
            "an emoji-and-link body, which defers the artifact to somewhere else",
            "🚀 see https://example.invalid/issues/4192".to_string(),
        ),
    ];

    // These two carry nothing at all, under any reading, so the set is exact.
    let exactly_both: Vec<(&str, String)> = vec![
        (
            "an unfilled pull request template: HTML comments and nothing else",
            "<!-- Describe your change -->\n\n<!-- Done when? -->\n".to_string(),
        ),
        (
            "whitespace only, which is what an author who deleted the template leaves",
            "   \n\t\n  \r\n  ".to_string(),
        ),
    ];

    for (context, body) in at_least.iter().chain(exactly_both.iter()) {
        assert!(
            !body.contains("Done when") || body.starts_with("<!--"),
            "fixture invariant: {context} must not carry this file's heading over real \
             content, or it stops testing the marker-less shape"
        );
    }

    for (context, body) in &at_least {
        expect_at_least_missing(TITLE, body, &[Artifact::DoneWhenBar], context);
    }
    for (context, body) in &exactly_both {
        expect_missing(
            TITLE,
            body,
            &[Artifact::WrittenProblem, Artifact::DoneWhenBar],
            context,
        );
    }
}

#[test]
fn a_problem_statement_with_no_acceptance_bar_fails_however_long_the_prose() {
    // Guards the shallow check "the body is long, so it must say something".
    // This body is four paragraphs of genuine problem analysis and contains no
    // statement of what done looks like.
    expect_missing(
        TITLE,
        &problem_only(&long_prose()),
        &[Artifact::DoneWhenBar],
        "a long problem statement with no bar",
    );
}

#[test]
fn an_acceptance_bar_with_no_written_problem_fails_however_long_the_bar() {
    // The artifact is "written problem + done-when". A bar with no bet behind
    // it cannot be judged: there is nothing to say whether the bar is the right
    // bar. The long variant is the mirror of the test above — a substantial
    // done-when cannot carry an absent problem statement.
    for bar in [BAR.to_string(), long_bar()] {
        expect_missing(
            TITLE,
            &bar_only(&bar),
            &[Artifact::WrittenProblem],
            "a bar with no written problem",
        );
    }
}

#[test]
fn a_heading_with_nothing_under_it_fails_however_much_the_other_section_says() {
    // The two long cases are the reason a global length threshold cannot
    // satisfy this suite: `long_prose()` under an empty done-when is the
    // longest body anywhere in this file, and it must fail.
    let cases: Vec<(String, Vec<Artifact>, &str)> = vec![
        (
            body_with(PROBLEM, ""),
            vec![Artifact::DoneWhenBar],
            "the done-when heading is present with nothing under it",
        ),
        (
            body_with("", BAR),
            vec![Artifact::WrittenProblem],
            "the problem heading is present with nothing under it",
        ),
        (
            body_with("", ""),
            vec![Artifact::WrittenProblem, Artifact::DoneWhenBar],
            "both headings present, both empty — a pasted template is not the artifact",
        ),
        (
            body_with(&long_prose(), ""),
            vec![Artifact::DoneWhenBar],
            "four paragraphs of problem analysis above an empty done-when heading; \
             length is not a bar",
        ),
        (
            body_with("", &long_bar()),
            vec![Artifact::WrittenProblem],
            "a long done-when above an empty problem heading; a bar with no bet \
             cannot be judged",
        ),
    ];

    let longest_failing = cases.iter().map(|(b, _, _)| b.len()).max().unwrap_or(0);
    assert!(
        longest_failing > body_with(PROBLEM, BAR).len(),
        "fixture invariant: some body that must fail has to be longer than every \
         body that must pass, or total length alone still separates the two sets"
    );

    for (body, expected, context) in &cases {
        expect_missing(TITLE, body, expected, context);
    }
}

/// Runs one must-fail section through both positions and both-at-once.
///
/// Both positions matter: a gate that screens the done-when for substance and
/// settles for "non-empty" on the bet certifies half a template paste.
#[track_caller]
fn assert_placeholder_fails_in_both_sections(placeholder: &str, family: &str) {
    for problem in [PROBLEM, SHORT_PROBLEM] {
        expect_missing(
            TITLE,
            &body_with(problem, placeholder),
            &[Artifact::DoneWhenBar],
            &format!("{family}: the done-when section contains only {placeholder:?}"),
        );
    }

    for bar in [BAR, SHORT_BAR] {
        expect_missing(
            TITLE,
            &body_with(placeholder, bar),
            &[Artifact::WrittenProblem],
            &format!("{family}: the problem section contains only {placeholder:?}"),
        );
    }

    expect_missing(
        TITLE,
        &body_with(placeholder, placeholder),
        &[Artifact::WrittenProblem, Artifact::DoneWhenBar],
        &format!("{family}: both sections contain only {placeholder:?}"),
    );
}

#[test]
fn a_placeholder_fails_in_either_section_however_much_the_other_one_says() {
    // The enumerated table, run through both positions against a short and a
    // normal counterpart. On its own this is copyable; the two tests below are
    // what make copying it insufficient.
    for placeholder in PLACEHOLDERS {
        assert_placeholder_fails_in_both_sections(placeholder, "enumerated placeholder");
    }

    // The long counterparts, so a length threshold cannot separate the sets.
    for placeholder in PLACEHOLDERS {
        expect_missing(
            TITLE,
            &body_with(&long_prose(), placeholder),
            &[Artifact::DoneWhenBar],
            &format!("four paragraphs of problem above the placeholder {placeholder:?}"),
        );
        expect_missing(
            TITLE,
            &body_with(placeholder, &long_bar()),
            &[Artifact::WrittenProblem],
            &format!("the placeholder {placeholder:?} above four paragraphs of bar"),
        );
    }
}

#[test]
fn a_deferral_fails_however_it_is_capitalised_punctuated_or_bulleted() {
    // The defect this kills, verified against a real implementation: a gate
    // whose substance check is `!PLACEHOLDERS.contains(section)` passes every
    // enumerated test above and then returns Passed for a done-when section
    // reading "TBD.", "tbd!", "N/A.", "Todo:", "WIP", "- [x]" or "xxx". A
    // hardcoded table is a complete implementation only while the must-fail set
    // is enumerable, so this one is generated.
    let derived = derived_deferrals();
    let novel = derived
        .iter()
        .filter(|d| !PLACEHOLDERS.contains(&d.as_str()))
        .count();
    assert!(
        novel > 200,
        "fixture invariant: the derived family must contain far more strings than \
         PLACEHOLDERS enumerates, or copying that table into the gate is still a \
         complete implementation; got {novel} novel of {} derived",
        derived.len()
    );

    for placeholder in &derived {
        assert_placeholder_fails_in_both_sections(placeholder, "derived deferral");
    }
}

#[test]
fn a_deferral_phrase_fails_even_though_it_shares_no_prefix_with_the_table() {
    for phrase in PHRASE_DEFERRALS {
        for enumerated in PLACEHOLDERS {
            let n = enumerated.len().min(phrase.len()).min(3);
            assert_ne!(
                phrase.to_lowercase()[..n],
                enumerated.to_lowercase()[..n],
                "fixture invariant: {phrase:?} must share no prefix with the enumerated \
                 placeholder {enumerated:?}, or the table reaches it by accident"
            );
        }
        assert_placeholder_fails_in_both_sections(phrase, "deferral phrase");
    }
}

#[test]
fn a_section_that_is_blank_only_to_a_reader_fails() {
    // U+200B and U+FEFF are not `char::is_whitespace`, so a section holding one
    // survives `trim()` and reads as substance to the cheapest possible check
    // while rendering as an empty heading on GitHub.
    assert!(
        UNICODE_BLANKS
            .iter()
            .any(|b| !b.trim().is_empty() && b.chars().all(|c| !c.is_alphanumeric())),
        "fixture invariant: at least one of these must survive trim() while carrying \
         no readable content, or the family stops separating `trim().is_empty()` from \
         a real substance check"
    );

    for blank in UNICODE_BLANKS {
        assert_placeholder_fails_in_both_sections(blank, "invisible section");
    }
}

#[test]
fn a_descriptive_title_is_not_a_substitute_for_the_bar() {
    // A well-written title says what changed. It never says what done looks
    // like, and accepting it would let every conventional-commit title certify
    // the Product seat.
    expect_missing(
        "fix(certify): stop reporting an unqueried canary as passed so the scorecard \
         reflects what was actually measured",
        "",
        &[Artifact::WrittenProblem, Artifact::DoneWhenBar],
        "a descriptive title with an empty body",
    );
}

#[test]
fn the_title_and_the_body_are_not_interchangeable() {
    // Both parameters are `&str`, so swapping them at a call site compiles in
    // silence. This pins the swap behaviourally rather than by reading the
    // gate's source: fed backwards, a well-formed change must not certify,
    // because a conventional-commit subject line is not an acceptance bar.
    let body = body_with(PROBLEM, BAR);
    expect_passed(TITLE, &body, "sanity: the right way round");

    let swapped = product_bar::judge(&body, TITLE);
    expect_failed(
        &swapped,
        "the title and the body handed to judge() the wrong way round",
    );
}

#[test]
fn each_failure_message_names_only_the_artifact_that_was_missing() {
    // One message per shape of absence. The exact missing-set assertions inside
    // `expect_missing` are what forbid a false accusation; the three
    // `assert_ne!`s below are what forbid one generic string standing in for
    // three measurements, which would satisfy every positive containment check
    // while telling an author who wrote a good problem statement to go and
    // write a problem statement.
    let both = expect_missing(
        TITLE,
        "",
        &[Artifact::WrittenProblem, Artifact::DoneWhenBar],
        "nothing written at all",
    );
    let no_bar = expect_missing(
        TITLE,
        &problem_only(PROBLEM),
        &[Artifact::DoneWhenBar],
        "problem written, bar missing",
    );
    let no_problem = expect_missing(
        TITLE,
        &bar_only(BAR),
        &[Artifact::WrittenProblem],
        "bar written, problem missing",
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
    expect_missing(
        KO_TITLE,
        &problem_only(KO_PROBLEM),
        &[Artifact::DoneWhenBar],
        "a Korean problem statement with no bar",
    );
}

/// Bodies shaped to break a hurried section extractor. Every one of them is
/// missing at least one artifact, so every one of them must be `Failed`.
fn awkward_bodies() -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = vec![
        ("an empty body", String::new()),
        ("one Korean word, no headings", "카나리".to_string()),
        (
            "a Korean problem with no bar",
            "## Problem\n\n카나리 게이트가 잘못된 판정을 만든다\n".to_string(),
        ),
        (
            "a one-syllable problem and a TBD bar",
            "## Problem\n\n한\n\n## Done when\n\nTBD\n".to_string(),
        ),
        (
            "an emoji sentence, no headings",
            "🚀 배포가 못 된다".to_string(),
        ),
        (
            "mixed scripts above an empty done-when",
            "## Problem\n\n한국어 problem 混合 テキスト\n\n## Done when\n\n\n".to_string(),
        ),
        ("bare carriage returns", "\r\r\r".to_string()),
        // Every byte index from 0 to 47 in this body is either inside
        // "## Problem\n\n" or inside a three-byte Hangul syllable, so any
        // fixed-offset slice of it that is not a character boundary panics.
        (
            "a problem of twelve three-byte syllables and no bar",
            format!("## Problem\n\n{}", "가".repeat(12)),
        ),
        // The reversed order. `&body[find("## Problem") + len .. find("## Done when")]`
        // panics here with "byte range starts at 39 but ends at 0", because the
        // done-when marker is now before the problem marker.
        (
            "the done-when first, with an empty problem section",
            "## Done when\n\n- p99 < 5ms\n\n## Problem\n".to_string(),
        ),
        (
            "the done-when first, with a whitespace-only problem section",
            "## Done when\n\n- p99 < 5ms\n\n## Problem\n\n   \n".to_string(),
        ),
        // A body whose last characters are a marker with no trailing newline.
        // GitHub bodies routinely have none, and
        // `&body[find(marker) + marker.len() + 2 ..]` panics with "start byte
        // index 51 is out of bounds for string of length 49".
        (
            "a problem statement and a trailing done-when marker, no newline",
            "## Problem\n\nCheckout p99 regressed.\n\n## Done when".to_string(),
        ),
        (
            "a bar and a trailing problem marker, no newline",
            "## Done when\n\n- p99 < 5ms\n\n## Problem".to_string(),
        ),
        (
            "nothing but the problem marker, no newline",
            "## Problem".to_string(),
        ),
        (
            "nothing but the done-when marker, no newline",
            "## Done when".to_string(),
        ),
        // The same marker twice: the first occurrence empty, the second filled.
        // The other artifact is absent outright, so the verdict is unambiguous
        // whichever occurrence the gate reads.
        (
            "the problem marker twice, first empty, and no bar anywhere",
            "## Problem\n\n\n## Problem\n\nCheckout p99 regressed to 40ms.\n".to_string(),
        ),
        (
            "the done-when marker twice, first empty, and no problem anywhere",
            "## Done when\n\n\n## Done when\n\n- p99 < 5ms\n".to_string(),
        ),
    ];

    // The same fixtures as a browser submits them. A gate anchored on "\n##"
    // mis-slices every one of these.
    let crlf: Vec<(&'static str, String)> = out
        .iter()
        .map(|(name, body)| (*name, body.replace('\n', "\r\n")))
        .collect();
    out.extend(crlf);
    out
}

#[test]
fn judge_returns_a_verdict_for_any_body_and_never_panics() {
    // A panic inside `judge` is not a Failed gate: it unwinds
    // `evaluate_pre_merge_gates` and takes the whole review with it. The
    // obvious way to write one is to quote an excerpt of the body back at the
    // author — `&pr_body[..40]` — which is a byte index, and byte 40 lands
    // inside a character in several of the bodies below. The other two shapes
    // are a marker order the extractor did not expect and a marker with nothing
    // after it.
    //
    // The assertion is `Failed`, not merely "returned": none of these bodies
    // carries both artifacts, so answering `NotMeasured` for any of them
    // certifies a change that produced no acceptance bar.
    for (context, body) in awkward_bodies() {
        assert!(
            !missing(KO_TITLE, &body).is_empty(),
            "{context}: this fixture is on the failing side, so the measurement must \
             report at least one missing artifact; body={body:?}"
        );
        expect_failed(&product_bar::judge(KO_TITLE, &body), context);
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
        assert_eq!(
            missing(TITLE, &build(Eol::Lf)),
            missing(TITLE, &build(Eol::Crlf)),
            "{name}: the gate found different artifacts missing under CRLF than under LF"
        );
    }

    // Without this, "Failed under both line endings" would satisfy the loop
    // above and the gate could still reject every web-authored change.
    expect_passed(
        TITLE,
        &body_with_eol(PROBLEM, BAR, Eol::Crlf),
        "a complete Product artifact typed into the GitHub web UI; rejecting it \
         blocks the majority of real pull requests",
    );
}

#[test]
fn the_verdict_depends_on_nothing_but_the_change_it_was_handed() {
    // The gate runs inside a review that also runs dozens of other gates, and
    // this suite runs in parallel. A verdict that depends on anything other
    // than the two strings it was handed is a flake nothing here could
    // attribute — and product_bar.rs's own doc comment promises it makes no
    // network or filesystem call, which until now nothing pinned. A gate that
    // loaded its deferral vocabulary from a config file would satisfy every
    // behavioural assertion in this file and still be non-deterministic.
    //
    // The behavioural half first, so this test is red on the unimplemented
    // measurement rather than on the source scan.
    let bodies = [
        body_with(PROBLEM, BAR),
        problem_only(PROBLEM),
        bar_only(BAR),
        body_with(PROBLEM, "TBD"),
        String::new(),
    ];
    for body in &bodies {
        let first = product_bar::judge(TITLE, body);
        let second = product_bar::judge(TITLE, body);
        assert_eq!(
            first, second,
            "two calls on the same change disagreed: {first:?} then {second:?}. \
             body={body:?}"
        );
        assert_eq!(
            missing(TITLE, body),
            missing(TITLE, body),
            "the measurement is not stable across calls; body={body:?}"
        );
    }

    // The static half. Sanctioned source inspection, same idiom as the wiring
    // tests below; it lives inside this test rather than beside it because on
    // its own — against a module that is still `todo!()` — it would be a test
    // born green, and a test that has never been observed failing publishes
    // assurance it has not earned.
    let src = without_line_comments(&source("src/pre_merge_guard/product_bar.rs"));
    for forbidden in [
        "std::fs",
        "std::net",
        "std::env",
        "std::process",
        "Command::new",
        "include_str!",
        "reqwest",
        "tokio",
    ] {
        assert!(
            !src.contains(forbidden),
            "src/pre_merge_guard/product_bar.rs reaches for {forbidden}. The Product \
             artifact is authored on the change under review and nowhere else; a gate \
             that reads anything but its two arguments is both a flake and a second \
             source of truth"
        );
    }
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
fn the_product_bar_name_is_bound_to_the_product_bar_field() {
    // Both listings are hand-written lists of seventy-odd near-identical
    // `_status` fields, so the likeliest mistake is not omission but a
    // copy-paste that pairs the new name with a neighbouring field:
    // `("product_bar_status", &self.test_suite_status)`. That passes the name
    // check above and both alignment tests in report.rs, and it makes the
    // scorecard report someone else's measurement under the Product seat's
    // name. Follows the idiom of report.rs's
    // `named_statuses_identifies_which_gates_failed`: mark one field and see
    // which name reports it.
    const PROBE: &str = "probe: the Product seat's own field, marked by this test";

    let mut report = PreMergeCertificationReport::unmeasured("fixture: nothing measured");
    report.product_bar_status = GateStatus::Failed(PROBE.to_string());

    let reporting: Vec<&str> = report
        .named_statuses()
        .into_iter()
        .filter(|(_, s)| matches!(s, GateStatus::Failed(m) if m == PROBE))
        .map(|(n, _)| n)
        .collect();
    assert_eq!(
        reporting,
        vec!["product_bar_status"],
        "exactly one name must report the field this test marked, and it must be \
         product_bar_status. An empty list means named_statuses() reads a different \
         field under that name; a different name means the Product seat's \
         measurement is published under someone else's gate"
    );

    let carried = report
        .all_statuses()
        .into_iter()
        .filter(|s| matches!(s, GateStatus::Failed(m) if m == PROBE))
        .count();
    assert_eq!(
        carried, 1,
        "all_statuses() must carry the product_bar_status field exactly once; \
         seal(), gate_counts() and recompute_unmeasured() all read that listing, so \
         a field missing from it gates nothing and a field listed twice is counted \
         twice"
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
// so these follow that idiom. Every scan runs over comment-stripped source, so a
// commented-out example cannot satisfy one, and every scan is anchored on
// `product_bar::judge(` rather than a bare `judge(`, so an unrelated gate's
// `judge` appearing in the evaluator later cannot turn these red for a reason
// that has nothing to do with the Product seat.

fn source(rel: &str) -> String {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `src` with `//` line comments removed, so a commented-out call or a doc
/// comment quoting one cannot satisfy any scan below.
fn without_line_comments(src: &str) -> String {
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The identifier a `let` introduces immediately before `head` ends, if the
/// text between the `=` and the end of `head` is still the same statement.
fn let_binding_before(head: &str) -> Option<String> {
    let pos = head.rfind("let ")?;
    let tail = &head[pos + "let ".len()..];
    let eq = tail.find('=')?;
    if tail[eq + 1..].contains(';') {
        return None;
    }
    let name = tail[..eq].trim().trim_start_matches("mut ").trim();
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    Some(name.to_string())
}

/// Every `product_bar::judge(...)` call in `src`, as (binding, arguments).
///
/// `binding` is `Some(name)` when the call is the initialiser of `let name = `.
fn product_bar_judge_calls(src: &str) -> Vec<(Option<String>, String)> {
    const ANCHOR: &str = "product_bar::judge(";
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(i) = src[from..].find(ANCHOR) {
        let start = from + i;
        let open = start + ANCHOR.len() - 1;
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
            out.push((
                let_binding_before(&src[..start]),
                src[open + 1..end].to_string(),
            ));
        }
        from = open + 1;
    }
    out
}

/// The initialiser expressions given to `field` in every struct literal in
/// `src`. A shorthand `field,` yields the field's own name.
fn struct_field_initialisers(src: &str, field: &str) -> Vec<String> {
    let shorthand = format!("{field},");
    let labelled = format!("{field}:");
    src.lines()
        .filter_map(|l| {
            let t = l.trim();
            if t == shorthand || t == field {
                Some(field.to_string())
            } else {
                t.strip_prefix(&labelled)
                    .map(|rest| rest.trim().trim_end_matches(',').trim().to_string())
            }
        })
        .collect()
}

/// The leading identifier of an expression, e.g. `pb` in `pb.clone()`.
fn leading_ident(expr: &str) -> String {
    expr.chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

#[test]
fn the_evaluator_receives_the_change_under_review() {
    let src = without_line_comments(&source("src/pre_merge_guard/evaluator.rs"));
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
         judges every pull request on its title alone — which \
         the_title_and_the_body_are_not_interchangeable shows is a fail-open"
    );
}

#[test]
fn the_evaluator_computes_product_bar_status_by_judging_that_change() {
    let src = without_line_comments(&source("src/pre_merge_guard/evaluator.rs"));

    let calls = product_bar_judge_calls(&src);
    assert!(
        !calls.is_empty(),
        "src/pre_merge_guard/evaluator.rs never calls product_bar::judge, so \
         product_bar_status is not derived from the change under review. A gate \
         computed from nothing gates nothing: it is named on the scorecard, counted \
         in TOTAL_GATES, and blocks no pull request"
    );

    for (_, args) in &calls {
        assert!(
            args.contains("pr_title") && args.contains("pr_body"),
            "the Product gate must be judged over the change's own title and body; \
             got product_bar::judge({args})"
        );
        assert!(
            args.find("pr_title") < args.find("pr_body"),
            "the arguments are swapped. Both are &str so this compiles silently, and \
             the gate then looks for the acceptance bar in the title — got \
             product_bar::judge({args})"
        );
    }

    // Binding the field to the call, not merely to the absence of a literal. A
    // scan for "a line holding both product_bar_status and GateStatus::" is
    // satisfied by `let pb = GateStatus::Passed;` plus `product_bar_status: pb,`
    // on two separate lines, and by the realistic copy-paste
    // `product_bar_status: doc_parity_status.clone(),`. Both are caught here:
    // the initialiser must be the judge call itself, or an identifier a `let`
    // bound to one.
    let bindings: Vec<String> = calls.iter().filter_map(|(b, _)| b.clone()).collect();
    let initialisers = struct_field_initialisers(&src, "product_bar_status");
    assert!(
        !initialisers.is_empty(),
        "no struct literal in the evaluator gives product_bar_status a value, so the \
         report cannot carry the Product seat's measurement"
    );
    for init in &initialisers {
        let derived_from_the_call =
            init.contains("product_bar::judge(") || bindings.contains(&leading_ident(init));
        assert!(
            derived_from_the_call,
            "product_bar_status is initialised from {init:?}, which is neither a \
             product_bar::judge call nor an identifier bound to one. A literal Passed \
             certifies every change; a literal NotMeasured leaves a named gate that \
             measures nothing forever; a neighbouring gate's status publishes someone \
             else's measurement under the Product seat's name. Bindings from judge \
             calls: {bindings:?}"
        );
    }
}

#[test]
fn the_review_pipeline_hands_the_evaluator_the_title_then_the_body() {
    // The same silent swap is available one level up: the pipeline already
    // passes `title, body` in that order to `ensure_documentation_parity`, and
    // the evaluator must be fed the same way.
    const CALL: &str = "evaluate_pre_merge_gates(";
    let src = without_line_comments(&source("src/webhook/pipelines/review.rs"));
    let start = src
        .find(CALL)
        .expect("the review pipeline evaluates the pre-merge gates");

    // This parser reads one argument per line. If the call is ever collapsed
    // onto a single line it must fail loudly here rather than silently find no
    // arguments and pass, or report a swap that is not there.
    let after_open = start + CALL.len();
    let first_line_tail = &src[after_open
        ..src[after_open..]
            .find('\n')
            .map(|i| after_open + i)
            .unwrap_or(src.len())];
    assert!(
        first_line_tail.trim().is_empty(),
        "this test reads one argument per line, and the call to \
         evaluate_pre_merge_gates now carries {first_line_tail:?} on its opening \
         line. Fix the test to parse the new shape rather than let it mis-parse — a \
         wiring guard that cannot read the wiring is worse than none"
    );

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
