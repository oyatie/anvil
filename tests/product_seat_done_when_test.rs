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
//!      over a kilobyte, while every passing body sits between roughly ninety
//!      bytes and seven hundred. No threshold on total length can separate them.
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
//!   6. **Section boundaries are falsified in both directions.** A previous
//!      revision contained no body with a third section anywhere, so an
//!      extractor that ran the done-when to the next `"\n## "` was never
//!      falsified: an empty `## Done when` followed by `### Testing` came back
//!      `Passed`, which is the pasted-template defect in the shape real
//!      templates produce. `a_third_section_does_not_hide_an_empty_one` places
//!      a real third section after an empty and after a deferred section at
//!      every heading depth an author writes, and its passing counterpart
//!      places one after a genuine bar, so the fix cannot degenerate into
//!      "ignore everything after the marker".
//!   7. **The passing side is wide enough not to block everyone.** See the next
//!      section: a suite whose only passing bodies spelled the marker one exact
//!      way admitted a gate that fails closed on nearly every real change,
//!      which is the fabricated accusation at 100% incidence.
//!   8. **Sections whose lines disagree are pinned, in both directions.** The
//!      revision before this one was exhaustive on *single-content* sections —
//!      all real, all deferral, all blank — and silent on mixed ones, so a gate
//!      reading only the first non-blank line of a section passed every test
//!      and then rejected a three-item bar whose first line was an empty
//!      checkbox. It also passed a gate that rejected any section *containing*
//!      an invisible character, which fails every artifact pasted out of a
//!      document editor. Real content beside a deferral, beside an invisible
//!      character and beside a colon-terminated lead-in are now all pinned as
//!      passing, and the all-deferral counterparts as failing, so the rule the
//!      suite means — strip what is not content, then judge what is left — is
//!      the only rule that satisfies it.
//!   9. **Every family that can be line-ending-sensitive runs over both.** The
//!      boundary and marker families are where CRLF actually bites: a trailing
//!      `\r` defeats the `ends_with` test that recognises `**Testing**`,
//!      `Testing:` and `**Done when**`, and `body.split('\n')` instead of
//!      `body.lines()` is an entirely ordinary way to write an extractor. Under
//!      LF alone the headline defect this file exists to close stayed open in
//!      the exact line endings the GitHub web UI submits.
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
//! each missing artifact) plus that the three shapes of absence render three
//! distinct messages. An earlier revision asserted the negative as a raw
//! substring ban — "a missing-bar message must not contain the word problem" —
//! which turned a correct, helpful implementation red for quoting the offending
//! section back at the author. Do-not-falsely-accuse is enforced where it is
//! mechanically checkable: on the measurement, by `expect_missing` pinning the
//! set exactly. See `three_shapes_of_absence_produce_three_distinct_messages`
//! for what that test does and does not claim.
//!
//! # The marker vocabulary is open; the marker *formatting* is not
//!
//! Which words announce the two sections is left to the implementer: an
//! implementation that also recognises `## Acceptance criteria`, `## Why`, a
//! YAML block or unheaded prose passes unchanged, because no test here requires
//! a body that genuinely states both artifacts to fail.
//!
//! What is no longer left open is the markdown *around* the same two words.
//! A previous revision built every one of its passing bodies from the
//! byte-identical strings `## Problem` and `## Done when`. Paired with the
//! (correct) requirement that a body carrying no bar fails closed, that admits
//! a gate matching exactly two byte strings — one that rejects `## Done When`,
//! `### Done when`, `## Done when:` and `**Done when**`, and therefore withholds
//! certification from essentially every real pull request once wired into
//! `seal()`. This repository has no PULL_REQUEST_TEMPLATE forcing one spelling,
//! so nothing else would have caught it. A false accusation is the same defect
//! as a false green pointed the other way, so
//! `the_same_two_words_are_the_marker_however_the_author_formats_them` pins case,
//! depth, a trailing colon and a bold label as passing — and pins the mirror,
//! that an empty section under each of those spellings still fails, so the
//! widened recognition cannot itself become a fail-open.
//!
//! # What these tests deliberately do NOT pin
//!
//! Synonyms for the two headings, per above. The render order of
//! `missing_artifacts` (the helper sorts before comparing). The prose of the
//! failure messages beyond naming each missing artifact and differing from one
//! another. And the change's *title*: `judge` takes the body alone, because no
//! behavioural test in this suite could distinguish a gate that read the title
//! from one that ignored it, and this suite is not going to require plumbing an
//! input it cannot measure.
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

/// A well-written conventional-commit subject for the change these fixtures
/// describe. It is not an input to `judge` — see the module docs — but it is
/// the companion title for the bodies below, and
/// `the_bet_and_the_bar_are_written_on_the_change_not_left_to_its_title` is
/// what pins that a change whose bet lives only here has not produced the
/// artifact.
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

/// A real acceptance bar written the way most of them are written: as a list of
/// separately checkable criteria. Multi-line on purpose — an extractor that
/// takes only the first line after the marker, and one that takes only the
/// first *non-blank* line, are two different wrong gates and this fixture is
/// what several families below use to separate them.
const MULTILINE_BAR: &str = "- `slo_status` is NotMeasured when no telemetry endpoint is configured\n\
     - `is_admissible()` is false while any gate is NotMeasured\n\
     - the posted scorecard names every unmeasured gate by id";

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

/// The two words of each heading, in the markdown an author actually wraps them
/// in. Every entry is the *same words* — case, depth, a trailing colon and a
/// bold label are formatting, not vocabulary, and a gate that recognises only
/// one of them rejects nearly every real pull request. Synonyms are deliberately
/// absent: which words announce a section is left open, per the module docs.
const DONE_WHEN_MARKERS: &[&str] = &[
    "## Done when",
    "## Done When",
    "## done when",
    "### Done when",
    "# Done when",
    "## Done when:",
    "**Done when**",
];

const PROBLEM_MARKERS: &[&str] = &[
    "## Problem",
    "## problem",
    "### Problem",
    "# Problem",
    "## Problem:",
    "**Problem**",
];

/// Headings an author writes for a third section, at every depth and weight
/// markdown allows. A body with three sections is the commonest filled-in
/// template there is, and until this suite carried one no fixture falsified the
/// gate's section-boundary logic in either direction.
const THIRD_SECTION_HEADERS: &[&str] = &[
    "# Testing",
    "## Testing",
    "### Testing",
    "**Testing**",
    "Testing:",
];

/// What a third section says. It reports what the author did; it states neither
/// what is wrong nor how anyone checks the change is done, so counting it as
/// either artifact is the boundary defect and nothing else.
const THIRD_SECTION_BODY: &str = "Ran `cargo test --all` locally on macOS and again on the CI \
     runner, and re-ran the canary integration suite twice.";

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

/// Both line endings, so a family can be written once and run over each.
const BOTH_EOLS: [Eol; 2] = [Eol::Lf, Eol::Crlf];

/// `body`, re-terminated for `eol`.
///
/// The fixtures below are written in LF and rewritten here rather than
/// threading a separator through every `format!`, which is how the CRLF twins
/// in `awkward_bodies` are built too. The two are the same string: none of
/// these fixtures contains a `\r` of its own.
fn as_eol(body: &str, eol: Eol) -> String {
    match eol {
        Eol::Lf => body.to_string(),
        Eol::Crlf => body.replace('\n', "\r\n"),
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

/// A third section, headed the way `header` heads it.
fn third_section(header: &str) -> String {
    format!("{header}\n\n{THIRD_SECTION_BODY}\n")
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
fn multiply(stems: &[&str]) -> Vec<String> {
    let trailers = ["", ".", "!", "?", ":", "...", " -"];
    let wrappers: [fn(&str) -> String; 4] = [
        |s| s.to_string(),
        |s| format!("- {s}"),
        |s| format!("* {s}"),
        |s| format!("  {s}  "),
    ];

    let mut out: BTreeSet<String> = BTreeSet::new();
    for stem in stems {
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

fn derived_deferrals() -> Vec<String> {
    multiply(DEFERRAL_STEMS)
}

/// The phrase deferrals under the same multiplication.
///
/// Listing the four raw literals and nothing else left the whole family
/// satisfiable by `PHRASES.contains(&section.trim())` — four strings copied
/// straight off the constant. That gate then passes a done-when reading
/// `"See the linked issue."` or `"- see the linked issues"`, which is the
/// sentence-case-and-a-full-stop spelling a human actually types. Multiplying
/// them out makes copying the table strictly more work than normalising.
fn derived_phrase_deferrals() -> Vec<String> {
    multiply(PHRASE_DEFERRALS)
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

/// The measurement, normalised for comparison.
///
/// Render order is presentation, so this sorts before comparing. Duplication is
/// not presentation: an artifact reported twice renders as "a written problem
/// statement and a written problem statement" in the author-facing message, so
/// the raw vector is checked for it rather than quietly deduplicated.
#[track_caller]
fn missing(body: &str) -> Vec<Artifact> {
    let raw = product_bar::missing_artifacts(body);
    let mut got = raw.clone();
    got.sort();
    got.dedup();
    assert_eq!(
        got.len(),
        raw.len(),
        "the measurement named the same missing artifact more than once ({raw:?}); the \
         message rendered from it then repeats itself back at the author. body={body:?}"
    );
    got
}

/// The change produced both artifacts: `judge` passes and the measurement finds
/// nothing missing. Asserting both keeps the verdict and the message rendered
/// from one measurement rather than two disagreeing ones.
#[track_caller]
fn expect_passed(body: &str, context: &str) {
    let status = product_bar::judge(body);
    assert_eq!(
        status,
        GateStatus::Passed,
        "{context}: this change produced the Product artifact; failing it is a \
         fabricated accusation, which is the same defect as a false green pointed \
         the other way. body={body:?}"
    );
    assert!(
        missing(body).is_empty(),
        "{context}: judge() passed the change while missing_artifacts() still reports \
         {:?} absent. The verdict and the message must be rendered from one \
         measurement, or the scorecard and the comment contradict each other",
        missing(body)
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
fn expect_missing(body: &str, expected: &[Artifact], context: &str) -> String {
    let mut want = expected.to_vec();
    want.sort();
    assert_eq!(
        missing(body),
        want,
        "{context}: the gate measured the wrong set of missing artifacts. Naming an \
         artifact the author did write tells them to write the thing they already \
         wrote; failing to name one they did not write hides the work. body={body:?}"
    );
    assert_failed_naming(body, &want, context)
}

/// The change is missing *at least* these artifacts.
///
/// Used where pinning the set exactly would decide something the specification
/// leaves open. A body of unheaded prose states no acceptance bar, so the bar is
/// missing beyond argument; whether that same prose also counts as the written
/// problem is a marker-recognition choice this suite deliberately leaves to the
/// implementer, and pinning it either way would be pinning the marker format.
#[track_caller]
fn expect_at_least_missing(body: &str, expected: &[Artifact], context: &str) -> String {
    let got = missing(body);
    for artifact in expected {
        assert!(
            got.contains(artifact),
            "{context}: the gate did not report the missing {artifact:?}. It reported \
             {got:?}. body={body:?}"
        );
    }
    assert_failed_naming(body, &got, context)
}

/// The shared tail of both: `Failed`, measured, unacceptable, and a message that
/// names every artifact the measurement found missing.
#[track_caller]
fn assert_failed_naming(body: &str, want: &[Artifact], context: &str) -> String {
    assert!(
        !want.is_empty(),
        "{context}: this is the failing side, so the measurement must report at least \
         one missing artifact; use expect_passed otherwise. body={body:?}"
    );

    let status = product_bar::judge(body);
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
        &body_with(PROBLEM, BAR),
        "a written problem and a done-when bar",
    );
}

#[test]
fn a_bar_written_as_measurable_criteria_passes() {
    // The bar is far more often a list than a sentence. A gate that only
    // accepts prose would push authors back to writing nothing.
    expect_passed(
        &body_with(PROBLEM, MULTILINE_BAR),
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
    expect_passed(&body_with(PROBLEM, BAR), "the problem stated first");
    expect_passed(
        &format!("## Done when\n\n{BAR}\n\n## Problem\n\n{PROBLEM}\n"),
        "the done-when stated first",
    );
}

#[test]
fn the_same_two_words_are_the_marker_however_the_author_formats_them() {
    // The defect this closes, reproduced twice by review against a real
    // implementation: a suite whose every passing body used the byte strings
    // "## Problem" and "## Done when" admitted a gate that matched exactly
    // those two byte strings. Combined with the (correct) rule that a body
    // carrying no bar fails closed, that gate rejects "## Done When", "###
    // Done when", "## Done when:" and "**Done when**" — and this repository
    // ships no PULL_REQUEST_TEMPLATE forcing one spelling, so once wired into
    // seal() it withholds certification from essentially every real change.
    // Blocking everyone is not a safe direction to be wrong in; it is the
    // fabricated accusation at full incidence.
    //
    // Every marker below is the SAME TWO WORDS. Case, heading depth, a trailing
    // colon and a bold label are how markdown is written, not what it says.
    // Synonyms are not pinned here — which words announce a section stays open.
    //
    // Run over both line endings, because the two are not the same test. Every
    // marker here that a gate recognises with `ends_with` — `**Done when**`,
    // `## Done when:` — is defeated by the trailing `\r` a browser-submitted
    // body carries, and `body.split('\n')` instead of `body.lines()` is an
    // entirely ordinary way to write the extractor. Under LF alone this whole
    // cross-product is green for a gate that rejects the same complete artifact
    // the moment it is typed into the GitHub web UI.
    for eol in BOTH_EOLS {
        for problem_marker in PROBLEM_MARKERS {
            for done_when_marker in DONE_WHEN_MARKERS {
                expect_passed(
                    &as_eol(
                        &format!("{problem_marker}\n\n{PROBLEM}\n\n{done_when_marker}\n\n{BAR}\n"),
                        eol,
                    ),
                    &format!(
                        "a complete Product artifact under {problem_marker:?} and \
                         {done_when_marker:?}, {eol:?}"
                    ),
                );
            }
        }

        // The mirror, so widening recognition cannot itself become a fail-open:
        // an empty section under any of those spellings is still an empty
        // section.
        for done_when_marker in DONE_WHEN_MARKERS {
            expect_missing(
                &as_eol(
                    &format!("## Problem\n\n{PROBLEM}\n\n{done_when_marker}\n\n"),
                    eol,
                ),
                &[Artifact::DoneWhenBar],
                &format!("{done_when_marker:?} with nothing under it, {eol:?}"),
            );
        }
        for problem_marker in PROBLEM_MARKERS {
            expect_missing(
                &as_eol(
                    &format!("{problem_marker}\n\n\n## Done when\n\n{BAR}\n"),
                    eol,
                ),
                &[Artifact::WrittenProblem],
                &format!("{problem_marker:?} with nothing under it, {eol:?}"),
            );
        }
    }
}

#[test]
fn a_third_section_after_the_artifacts_does_not_hide_them() {
    // The counterpart to `a_third_section_does_not_hide_an_empty_one`. Without
    // this half, the fix for that one degenerates into "ignore everything after
    // the marker" — which reads a bar of zero length and fails every filled-in
    // template instead. The boundary has to be pinned in both directions or the
    // slicing behaviour stays unspecified in both.
    // Both line endings: `**Testing**` and `Testing:` are recognised by an
    // `ends_with` test, which a trailing `\r` defeats, so under CRLF a gate can
    // stop seeing the boundary entirely — and then a complete artifact whose
    // bar is followed by testing notes is judged on a section it mis-sliced.
    for eol in BOTH_EOLS {
        for header in THIRD_SECTION_HEADERS {
            let third = third_section(header);

            expect_passed(
                &as_eol(
                    &format!("## Problem\n\n{PROBLEM}\n\n## Done when\n\n{BAR}\n\n{third}"),
                    eol,
                ),
                &format!("a complete artifact followed by a section headed {header:?}, {eol:?}"),
            );

            // A multi-line bar followed by a third section: an extractor that
            // takes only the first line after the marker still sees a bar here,
            // but one that takes nothing does not.
            expect_passed(
                &as_eol(
                    &format!(
                        "## Problem\n\n{PROBLEM}\n\n## Done when\n\n{MULTILINE_BAR}\n\n{third}"
                    ),
                    eol,
                ),
                &format!("a three-line bar followed by a section headed {header:?}, {eol:?}"),
            );

            // A third section wedged BETWEEN the two artifacts.
            expect_passed(
                &as_eol(
                    &format!("## Problem\n\n{PROBLEM}\n\n{third}\n## Done when\n\n{BAR}\n"),
                    eol,
                ),
                &format!("a section headed {header:?} between the problem and the bar, {eol:?}"),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The measurement: what fails
// ---------------------------------------------------------------------------

#[test]
fn a_third_section_does_not_hide_an_empty_one() {
    // The headline gap in the previous revision, reproduced independently by
    // both reviewers: no fixture in the file had a third section, so an
    // extractor that ran the done-when to the next occurrence of "\n## " was
    // never falsified. It returned Passed for a body whose done-when heading is
    // empty and whose next section is `### Testing`, `**Testing**` or
    // `# Testing`, because none of those is "\n## " — swallowing the testing
    // notes as the acceptance bar.
    //
    // That is the pasted-template defect in the exact shape real templates
    // produce: Problem / Done when / Testing, with the middle one skipped. The
    // author wrote nothing under the heading; what a later section happens to
    // say is not their acceptance bar, however many words it is.
    // Under BOTH line endings, because LF alone leaves this whole family green
    // for a gate that fails open on the shape GitHub's web UI actually submits.
    // `str::lines()` strips a trailing `\r` and `.trim()` eats it, so a gate
    // that finds its boundary with `ends_with("**")` or `ends_with(':')` on
    // `body.split('\n')` sees `**Testing**\r` and `Testing:\r` as ordinary prose
    // — and then swallows the testing notes as the acceptance bar of an empty
    // `## Done when`. That is the headline defect of this file, certified, in
    // the exact line endings a browser produces.
    for eol in BOTH_EOLS {
        for header in THIRD_SECTION_HEADERS {
            let third = third_section(header);

            for filler in ["", "   ", "TBD", "- [ ]"] {
                expect_missing(
                    &as_eol(
                        &format!("## Problem\n\n{PROBLEM}\n\n## Done when\n\n{filler}\n\n{third}"),
                        eol,
                    ),
                    &[Artifact::DoneWhenBar],
                    &format!(
                        "a done-when section holding only {filler:?}, followed by a section \
                         headed {header:?}, {eol:?}"
                    ),
                );

                // The mirror, so the problem-side extractor is pinned the same
                // way.
                expect_missing(
                    &as_eol(
                        &format!("## Problem\n\n{filler}\n\n{third}\n## Done when\n\n{BAR}\n"),
                        eol,
                    ),
                    &[Artifact::WrittenProblem],
                    &format!(
                        "a problem section holding only {filler:?}, followed by a section \
                         headed {header:?}, {eol:?}"
                    ),
                );

                // Both empty, with the third section carrying all the prose in
                // the body. Neither artifact exists; the body is not short.
                expect_missing(
                    &as_eol(
                        &format!("## Problem\n\n{filler}\n\n## Done when\n\n{filler}\n\n{third}"),
                        eol,
                    ),
                    &[Artifact::WrittenProblem, Artifact::DoneWhenBar],
                    &format!(
                        "both sections holding only {filler:?}, with a section headed \
                         {header:?} carrying the only prose in the body, {eol:?}"
                    ),
                );
            }
        }
    }
}

#[test]
fn a_change_with_no_bar_at_all_is_failed_not_merely_unmeasured() {
    expect_missing(
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
        expect_at_least_missing(body, &[Artifact::DoneWhenBar], context);
    }
    for (context, body) in &exactly_both {
        expect_missing(
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
        expect_missing(body, expected, context);
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
            &body_with(problem, placeholder),
            &[Artifact::DoneWhenBar],
            &format!("{family}: the done-when section contains only {placeholder:?}"),
        );
    }

    for bar in [BAR, SHORT_BAR] {
        expect_missing(
            &body_with(placeholder, bar),
            &[Artifact::WrittenProblem],
            &format!("{family}: the problem section contains only {placeholder:?}"),
        );
    }

    expect_missing(
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
            &body_with(&long_prose(), placeholder),
            &[Artifact::DoneWhenBar],
            &format!("four paragraphs of problem above the placeholder {placeholder:?}"),
        );
        expect_missing(
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
    // The prefix-disjointness invariant holds of the raw phrases: nothing in
    // PLACEHOLDERS reaches them by accident. It is asserted on the raw form
    // only, because the bullet wrappers below deliberately give the derived
    // forms the same two-character prefix as `"- "`.
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
    }

    // Multiplied out, exactly as `DEFERRAL_STEMS` is. Listing four lowercase,
    // unpunctuated, unbulleted literals made this family satisfiable by
    // `PHRASES.contains(&section.trim())` — four strings copied off the
    // constant — and that gate then returns Passed for `"See the linked
    // issue."`, `"Same as above!"` and `"- see the linked issues"`. The English
    // sentence a human types is not the literal a table holds.
    let derived = derived_phrase_deferrals();
    let novel = derived
        .iter()
        .filter(|d| !PHRASE_DEFERRALS.contains(&d.as_str()))
        .count();
    assert!(
        novel > 200,
        "fixture invariant: the derived phrase family must contain far more strings \
         than PHRASE_DEFERRALS enumerates, or copying that table into the gate is \
         still a complete implementation; got {novel} novel of {} derived",
        derived.len()
    );

    for phrase in &derived {
        assert_placeholder_fails_in_both_sections(phrase, "derived deferral phrase");
    }
}

#[test]
fn a_pointer_to_somewhere_else_is_not_the_artifact() {
    // The product decision the phrase deferrals were standing in for, stated on
    // the shape it actually takes. "The artifact lives on the change under
    // review" means a section whose entire content is a reference to another
    // place has not produced it: the reviewer, the auditor and the scorecard
    // all read this body, and none of them follows the link.
    //
    // Pinned separately from the phrases because a bare URL and a bare issue
    // reference share no English with any of them, and because they are the
    // commonest form of this defect by a wide margin — an author who defers
    // pastes a link far more often than they write a sentence about deferring.
    //
    // Listed in open_questions as a decision a human can veto: a shop that
    // accepts "the bar is in the linked issue" wants this family deleted, not
    // weakened.
    let pointers = [
        "https://example.invalid/issues/4192",
        "See https://example.invalid/issues/4192",
        "#4192",
        "See #4192",
        "- https://example.invalid/issues/4192",
        "example.invalid/issues/4192",
    ];

    for pointer in pointers {
        assert_placeholder_fails_in_both_sections(pointer, "a pointer to somewhere else");
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

    // THE MIRROR, and without it this family is satisfied by rejecting any
    // section that contains one of these characters at all:
    //
    //     if section.chars().any(|c| matches!(c, '\u{200b}' | '\u{feff}' | …)) {
    //         return false;
    //     }
    //
    // That gate passes every assertion above and then reports both artifacts
    // missing from a complete, well-written one whose only sin is a
    // non-breaking space between two words — which is what every body pasted
    // out of Notion, Google Docs or Confluence carries, and what a leading BOM
    // adds to a body pasted out of a file. High incidence, and a fabricated
    // accusation is the same defect as a false green pointed the other way.
    //
    // The rule these two halves state together is: strip the invisible
    // characters, then judge what is left. Not: reject on sight.
    let nbsp_problem = "Checkout p99 regressed to 40ms\u{00a0}after the cache change, and the \
                        rollout path is now certified against a measurement that never happened.";
    let zwsp_bar = "- p99 under 5ms on the\u{200b} checkout path\n\
                    - the scorecard names the unqueried canary";
    expect_passed(
        &body_with(nbsp_problem, zwsp_bar),
        "a complete artifact carrying a non-breaking space and a zero-width space \
         inside real prose; an invisible character next to real content does not \
         erase the content",
    );

    let bom_problem = format!("\u{feff}{PROBLEM}");
    let bom_bar = format!("\u{feff}{BAR}");
    expect_passed(
        &body_with(&bom_problem, &bom_bar),
        "a complete artifact whose sections open with a byte order mark, which is what \
         a body pasted out of a file carries",
    );

    // And the ideographic and em spaces, which arrive from the same editors.
    expect_passed(
        &body_with(
            &PROBLEM.replacen(' ', "\u{2003}", 1),
            &BAR.replacen(' ', "\u{3000}", 1),
        ),
        "a complete artifact carrying an em space and an ideographic space inside real \
         prose",
    );
}

#[test]
fn a_section_mixing_a_deferral_with_real_content_is_judged_on_the_real_content() {
    // THE DECISION THIS TEST SETTLES. Until it existed, no fixture anywhere in
    // this file had a section whose lines disagreed: every passing section led
    // with real content on every line, and every failing section was a deferral
    // on every line. So `substantive(section)` could legally collapse to
    // `substantive(first non-blank line)` — and the mirror,
    // `section.lines().any(substantive)`, was equally legal. The two disagree
    // about a partially-filled checklist, which is one of the commonest real
    // shapes there is, and the suite decided nothing while reading as though it
    // had.
    //
    // The rule is `any`: a section is the artifact if anything in it is. An
    // author who left the first checkbox blank and then wrote three checkable
    // criteria under it has done Product's job, and failing them is a
    // fabricated accusation over a stray character. The `first line` rule is
    // rejected here explicitly, not left to the implementer.
    let checkbox_then_bar = format!("- [ ]\n{MULTILINE_BAR}");
    expect_passed(
        &body_with(PROBLEM, &checkbox_then_bar),
        "a partially-filled checklist: an empty checkbox above three real, checkable \
         criteria is a bar, and reading only the first line of the section calls it a \
         template paste",
    );

    // The same thing the other way up, so the rule is not "skip the first line"
    // either.
    expect_passed(
        &body_with(PROBLEM, &format!("{MULTILINE_BAR}\n- [ ]")),
        "three real criteria with an empty checkbox left at the bottom",
    );

    // The problem-side mirror, both ways up.
    expect_passed(
        &body_with(&format!("TBD\n\n{PROBLEM}"), BAR),
        "a problem section that opens with a leftover TBD and then states the problem",
    );
    expect_passed(
        &body_with(&format!("{PROBLEM}\n\nTBD"), BAR),
        "a problem section that states the problem and then trails a leftover TBD",
    );

    // And the bound on `any`, so it cannot be satisfied by a stray word: a
    // section several lines long, every line of which is a deferral, is still
    // no artifact. This is the fully-unfilled checklist, which is a template
    // paste however tall it is.
    let all_deferral = "- [ ]\n- [ ]\n- [ ]\n\nTBD\n\nN/A\n\nWIP\n\n???";
    assert!(
        all_deferral
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count()
            > 4,
        "fixture invariant: this section has to be several lines long, or it stops \
         separating `any(substantive)` from `substantive(whole section)`"
    );
    assert_placeholder_fails_in_both_sections(all_deferral, "an unfilled checklist");
}

#[test]
fn a_short_colon_terminated_lead_in_line_is_writing_not_a_section_boundary() {
    // The bound on `THIRD_SECTION_HEADERS`'s `"Testing:"` entry.
    //
    // That entry requires a bare colon-terminated paragraph line to end the
    // section above it, which is the strongest demand this file makes. Until
    // this test existed nothing bounded how aggressive the resulting rule could
    // be: no passing fixture anywhere carried a colon-terminated line INSIDE a
    // section, so "any short line ending in a colon is a heading" satisfied the
    // whole suite — and that gate then reports a missing bar for a done-when
    // that opens `Acceptance:` above its bullets, and a missing problem for one
    // that opens `Today:`. A lead-in ending in a colon is ordinary technical
    // writing; the suite created the pressure to reject it and then never
    // checked the consequence.
    //
    // What separates the two, in these fixtures and in real markdown, is the
    // blank line: a heading is followed by one and a lead-in is not. That is
    // the only difference the fixtures rely on, and it is listed in
    // open_questions as the disambiguation a human may veto — by dropping
    // `"Testing:"` from `THIRD_SECTION_HEADERS` instead, which leaves the other
    // four closing the boundary hole.
    let lead_in_bar = format!("Acceptance:\n{MULTILINE_BAR}");
    let lead_in_problem = format!("Today:\n{PROBLEM}");

    for eol in BOTH_EOLS {
        expect_passed(
            &as_eol(&body_with(PROBLEM, &lead_in_bar), eol),
            &format!("a bar introduced by the lead-in line \"Acceptance:\", {eol:?}"),
        );
        expect_passed(
            &as_eol(&body_with(&lead_in_problem, BAR), eol),
            &format!("a problem introduced by the lead-in line \"Today:\", {eol:?}"),
        );
        expect_passed(
            &as_eol(&body_with(&lead_in_problem, &lead_in_bar), eol),
            &format!("both sections introduced by a colon-terminated lead-in, {eol:?}"),
        );
    }

    // The mirror, so this cannot be satisfied by abandoning the boundary rule
    // altogether: a colon-terminated line with nothing but a lead-in under it
    // is still an empty section.
    expect_missing(
        &body_with(PROBLEM, "Acceptance:"),
        &[Artifact::DoneWhenBar],
        "a done-when section holding a lead-in line and nothing to lead in to",
    );
}

#[test]
fn the_bet_and_the_bar_are_written_on_the_change_not_left_to_its_title() {
    // A well-written conventional-commit subject says what changed. It never
    // says what done looks like, and it is not a written problem statement
    // either — it is a label for one. Accepting it would let every well-titled
    // change certify the Product seat.
    //
    // This is the decision that removed the title from the signature. `judge`
    // takes the body alone, so the strongest form of this pin is the one the
    // compiler enforces: there is no parameter through which TITLE could reach
    // the gate. What is left to assert behaviourally is the product half — that
    // a change whose bet exists only in its title, and whose body carries a
    // real bar and nothing else, is still missing the written problem.
    //
    // Both bodies below have TITLE as their companion subject line: it names
    // the defect ("reporting an unread canary as passed") about as well as a
    // subject line can, and it is still not the artifact.
    expect_missing(
        "",
        &[Artifact::WrittenProblem, Artifact::DoneWhenBar],
        &format!("an empty body under the descriptive title {TITLE:?}"),
    );
    expect_missing(
        &bar_only(BAR),
        &[Artifact::WrittenProblem],
        &format!("a real bar, no written problem, under the title {TITLE:?}"),
    );
}

#[test]
fn three_shapes_of_absence_produce_three_distinct_messages() {
    // What this test checks, exactly: that the three shapes of absence do not
    // share one message. One constant string — "the written problem and the
    // done-when acceptance bar are missing" — satisfies every positive
    // containment assertion in this file, and would tell an author that a gate
    // failed and nothing else.
    //
    // What it deliberately does NOT check, despite an earlier name that claimed
    // it: that each message names *only* the artifact that was missing. There
    // is no non-brittle way to assert that at the level of prose. A correct and
    // helpful implementation may legitimately write "your problem statement is
    // here, your done-when bar is not", or quote the offending section back at
    // the author — and a raw vocabulary ban turns both of those red. The
    // do-not-falsely-accuse property is therefore enforced one level down,
    // where it *is* mechanically checkable: `expect_missing` pins the
    // measurement set exactly, so a gate cannot report an artifact the author
    // wrote. See open_questions.
    let both = expect_missing(
        "",
        &[Artifact::WrittenProblem, Artifact::DoneWhenBar],
        "nothing written at all",
    );
    let no_bar = expect_missing(
        &problem_only(PROBLEM),
        &[Artifact::DoneWhenBar],
        "problem written, bar missing",
    );
    let no_problem = expect_missing(
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
            !missing(&body).is_empty(),
            "{context}: this fixture is on the failing side, so the measurement must \
             report at least one missing artifact; body={body:?}"
        );
        expect_failed(&product_bar::judge(&body), context);
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
        // The three cases above are all `##`-headed with no third section,
        // which is the one shape where CRLF is harmless: `str::lines()` strips
        // the `\r` and `.trim()` eats it, so even a hand-rolled `#` branch
        // survives. The two below are the shapes that actually separate a gate
        // that handles CRLF from one that does not.
        (
            "an empty done-when above a bold third section",
            Box::new(|eol| {
                as_eol(
                    &format!(
                        "## Problem\n\n{PROBLEM}\n\n## Done when\n\n\n**Testing**\n\n{THIRD_SECTION_BODY}\n"
                    ),
                    eol,
                )
            }),
        ),
        (
            "a complete artifact under bold labels rather than hashes",
            Box::new(|eol| {
                as_eol(
                    &format!("**Problem**\n\n{PROBLEM}\n\n**Done when**\n\n{BAR}\n"),
                    eol,
                )
            }),
        ),
    ];

    for (name, build) in &cases {
        let lf = product_bar::judge(&build(Eol::Lf));
        let crlf = product_bar::judge(&build(Eol::Crlf));
        assert_eq!(
            variant(&lf),
            variant(&crlf),
            "{name}: the same change reached a different verdict because its line \
             endings came from a browser rather than an editor. lf={lf:?} crlf={crlf:?}"
        );
        assert_eq!(
            missing(&build(Eol::Lf)),
            missing(&build(Eol::Crlf)),
            "{name}: the gate found different artifacts missing under CRLF than under LF"
        );
    }

    // "Failed under both line endings" would satisfy the loop above, so the
    // absolute verdicts are pinned too — in both directions, on the two shapes
    // that separate a CRLF-aware gate from a CRLF-blind one.
    expect_passed(
        &body_with_eol(PROBLEM, BAR, Eol::Crlf),
        "a complete Product artifact typed into the GitHub web UI; rejecting it \
         blocks the majority of real pull requests",
    );

    // A complete artifact under bold labels, submitted from a browser. A gate
    // that recognises `**Done when**` with `ends_with("**")` over
    // `body.split('\n')` sees `**Done when**\r`, recognises nothing, and
    // reports both artifacts missing from a body that carries both.
    expect_passed(
        &as_eol(
            &format!("**Problem**\n\n{PROBLEM}\n\n**Done when**\n\n{BAR}\n"),
            Eol::Crlf,
        ),
        "a complete Product artifact under bold labels, typed into the GitHub web UI",
    );

    // The mirror, and the one that fails open: an empty `## Done when` whose
    // next section is `**Testing**`. The same CRLF-blind gate misses the
    // boundary and certifies the testing notes as the acceptance bar.
    expect_missing(
        &as_eol(
            &format!(
                "## Problem\n\n{PROBLEM}\n\n## Done when\n\n\n**Testing**\n\n{THIRD_SECTION_BODY}\n"
            ),
            Eol::Crlf,
        ),
        &[Artifact::DoneWhenBar],
        "an empty done-when above a bold third section, typed into the GitHub web UI",
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
        let first = product_bar::judge(body);
        let second = product_bar::judge(body);
        assert_eq!(
            first, second,
            "two calls on the same change disagreed: {first:?} then {second:?}. \
             body={body:?}"
        );
        assert_eq!(
            missing(body),
            missing(body),
            "the measurement is not stable across calls; body={body:?}"
        );
    }

    // The static half. Sanctioned source inspection, same idiom as the wiring
    // tests below; it lives inside this test rather than beside it because on
    // its own — against a module that is still `todo!()` — it would be a test
    // born green, and a test that has never been observed failing publishes
    // assurance it has not earned.
    // The previous revision of this half scanned for eight literal prefixes,
    // and the idiomatic spellings of the very I/O it forbade walked straight
    // past it: `use std::{env, fs};` contains neither "std::env" nor "std::fs",
    // and a grouped import is how anyone actually brings in two std modules.
    // `env!`, `option_env!` and `File::open` were not covered at all. A guard
    // that misreads what it guards is worse than none.
    //
    // So this asserts the positive shape instead. The import list of a module
    // this small is knowable, and an allowlist cannot be evaded by respelling —
    // there is no way to reach the filesystem without naming something outside
    // it. Widening the list is a deliberate act with a diff to argue about,
    // which is the point.
    //
    // The allowlist is a determinism rule, not a style rule, so it must admit
    // every spelling that is deterministic. Two corrections review found:
    //
    //   * `regex` is a first-class dependency of this crate and the house idiom
    //     for exactly this kind of marker parsing — src/cedar_guard.rs,
    //     src/supply_chain_guard.rs, src/clean_architecture_guard.rs,
    //     src/adr_drift_ratchet.rs and src/cell_isolation_guard.rs all open with
    //     `use regex::Regex;`. A compiled regex is pure: it reads the string it
    //     is handed and nothing else. Rejecting it turned a correct
    //     implementation red for a reason this test's own stated property —
    //     "a file, an environment variable, a clock or the network" — does not
    //     cover, and told the implementer to edit a settled specification test
    //     mid-implementation, which is the one thing this project's method
    //     forbids.
    //   * `use std::{cmp, fmt};` is how anyone brings in two std modules. The
    //     group is expanded into its members and each member held to the list on
    //     its own, so the ban on `std::{` is gone: it forbade a spelling, not an
    //     effect.
    //
    // `pub use` is matched too. It re-exports exactly as far as `use` reaches,
    // so leaving it unmatched was a hole in the guard.
    // The allowlist is only as good as the parser that feeds it, so the parser
    // is exercised before it is trusted — including the grouped form that used
    // to be banned outright and the `pub use` form that used to walk past.
    for (line, want) in [
        ("use regex::Regex;", vec!["regex::Regex"]),
        ("pub use regex::Regex;", vec!["regex::Regex"]),
        ("use std::{cmp, fmt};", vec!["std::cmp", "std::fmt"]),
        (
            "use std::{collections::BTreeSet, fmt::Write};",
            vec!["std::collections::BTreeSet", "std::fmt::Write"],
        ),
        ("use std::{env, fs};", vec!["std::env", "std::fs"]),
        (
            "use std::collections::{self, BTreeMap};",
            vec!["std::collections", "std::collections::BTreeMap"],
        ),
        ("use super::GateStatus;", vec!["super::GateStatus"]),
        ("let x = 1;", vec![]),
        ("    // use std::fs;", vec![]),
    ] {
        assert_eq!(
            imported_paths(line),
            want.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            "imported_paths({line:?}) misread the import; an allowlist fed by a \
             parser that cannot read a grouped or re-exported import either bans a \
             correct implementation or lets the filesystem through"
        );
    }

    let src = without_line_comments(&source("src/pre_merge_guard/product_bar.rs"));

    const ALLOWED_IMPORTS: &[&str] = &[
        "super::",
        "crate::",
        "regex",
        "std::borrow",
        "std::cmp",
        "std::collections",
        "std::fmt",
        "std::iter",
        "std::str",
        "std::sync::LazyLock",
        "std::sync::OnceLock",
    ];
    for line in src.lines() {
        let trimmed = line.trim();
        for path in imported_paths(trimmed) {
            assert!(
                ALLOWED_IMPORTS.iter().any(|a| path.starts_with(a)),
                "src/pre_merge_guard/product_bar.rs imports {trimmed:?}, which reaches \
                 {path:?} — outside the allowlist {ALLOWED_IMPORTS:?}. The Product \
                 artifact is authored on the change under review and nowhere else: a \
                 gate that reads a file, an environment variable, a clock or the \
                 network is both a flake this suite could not attribute and a second \
                 source of truth for what the author wrote"
            );
        }
    }

    // The things reachable without a `use` line at all.
    for forbidden in [
        "env!",
        "option_env!",
        "include_str!",
        "include_bytes!",
        "File::open",
        "fs::",
        "env::",
        "process::",
        "Command",
        "::var(",
        "SystemTime",
        "Instant",
        "reqwest",
        "tokio",
    ] {
        assert!(
            !src.contains(forbidden),
            "src/pre_merge_guard/product_bar.rs reaches for {forbidden}. The gate's \
             verdict must be a function of the one string it was handed and nothing \
             else"
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
// commented-out example cannot satisfy one.
//
// These are guards over *facts*, not over formatting. A wiring guard that
// misreads the wiring and then reports a correct implementation as broken is
// worse than no guard, so each assertion below is anchored on the loosest
// spelling that still carries the fact:
//
//   * the evaluator receives the change's **body** — either as a parameter
//     whose name says body (`pr_body`, `body`), or as a field whose name says
//     body on `PrDiffContext`, the change-under-review struct the evaluator
//     already takes. Both are read, and the check is the claim: an earlier
//     revision promised the struct route in this comment and then asserted a
//     literal `"body"` inside the parameter list, so it failed a correct
//     `diff_ctx.pr_body` wiring while accusing it of gating nothing;
//   * `product_bar_status` is derived from a call to product_bar's `judge` —
//     qualified (`product_bar::judge(...)`) or imported (`use
//     super::product_bar::judge;` then `judge(...)`), directly or through a
//     `let` binding, with or without a type annotation;
//   * the pipeline hands that call site the change's body.
//
// `the_wiring_parsers_read_the_shapes_a_wiring_is_actually_written_in` exercises
// those parsers against the realistic spellings, so the guard is tested rather
// than assumed.

fn source(rel: &str) -> String {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `src` with `//` line comments removed, so a commented-out call or a doc
/// comment quoting one cannot satisfy any scan below.
fn without_line_comments(src: &str) -> String {
    src.lines()
        .map(strip_line_comment)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every crate path a `use` line reaches, with a grouped import expanded into
/// its members: `use std::{cmp, fmt};` yields `std::cmp` and `std::fmt`, and
/// `use std::{collections::BTreeSet, fmt::Write};` yields both of those.
///
/// Returns empty for a line that is not an import.
///
/// Expansion rather than a ban on braces: the property the allowlist enforces
/// is that the verdict is a function of the string the gate was handed, and a
/// grouped import reaches exactly the same modules a sequence of single imports
/// would. `pub use` counts as an import for the same reason.
fn imported_paths(line: &str) -> Vec<String> {
    let t = line.trim();
    let rest = t
        .strip_prefix("pub use ")
        .or_else(|| t.strip_prefix("use "))
        .map(|r| r.trim().trim_end_matches(';').trim())
        .map(|r| r.trim_start_matches("::"));
    let Some(rest) = rest else {
        return Vec::new();
    };

    let Some(open) = rest.find('{') else {
        return vec![rest.to_string()];
    };
    let prefix = &rest[..open];
    let Some(close) = rest.rfind('}') else {
        return vec![rest.to_string()];
    };
    let inner = &rest[open + 1..close];

    // Split the group on the commas that are not inside a nested group, then
    // expand each member against the shared prefix. `self` re-imports the
    // prefix itself.
    let mut members: Vec<String> = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for c in inner.chars() {
        match c {
            '{' => {
                depth += 1;
                current.push(c);
            }
            '}' => {
                depth = depth.saturating_sub(1);
                current.push(c);
            }
            ',' if depth == 0 => members.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    members.push(current);

    let mut out = Vec::new();
    for member in members {
        let member = member.trim();
        if member.is_empty() {
            continue;
        }
        if member == "self" {
            out.push(prefix.trim_end_matches("::").to_string());
        } else if member.contains('{') {
            out.extend(imported_paths(&format!("use {prefix}{member};")));
        } else {
            out.push(format!("{prefix}{member}"));
        }
    }
    out
}

/// One line, with any `//` comment on it dropped.
///
/// `//` inside a string literal is not a comment. The naive version of this
/// truncated at the first `//` anywhere on the line, so a single URL in a
/// message string — `"see https://…"` — would silently blind every scan below
/// by cutting the line before the code on it. Nothing in the evaluator or the
/// review pipeline carries one today; the point is that adding one must not
/// quietly turn a wiring guard into a no-op.
fn strip_line_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if in_string => {
                i += 2;
                continue;
            }
            b'"' => in_string = !in_string,
            b'/' if !in_string && bytes.get(i + 1) == Some(&b'/') => return &line[..i],
            _ => {}
        }
        i += 1;
    }
    line
}

/// The identifier a `let` introduces immediately before `head` ends, if the
/// text between the `=` and the end of `head` is still the same statement.
///
/// A type annotation is stripped before the identifier is validated. Without
/// that, `let product_bar_status: GateStatus = product_bar::judge(pr_body);` —
/// a perfectly correct wiring — yielded no binding at all, and the wiring test
/// then failed a correct implementation with a message accusing it of the exact
/// defect it did not have.
fn let_binding_before(head: &str) -> Option<String> {
    let pos = head.rfind("let ")?;
    let tail = &head[pos + "let ".len()..];
    let eq = tail.find('=')?;
    if tail[eq + 1..].contains(';') {
        return None;
    }

    let mut name = tail[..eq].trim();
    name = name.strip_prefix("mut ").unwrap_or(name).trim();
    // `x: GateStatus` -> `x`. A binding name cannot contain `:`, so the first
    // one on the left of the `=` opens the annotation.
    if let Some(colon) = name.find(':') {
        name = name[..colon].trim();
    }

    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    Some(name.to_string())
}

/// Whether `src` brings product_bar's `judge` into scope unqualified.
fn imports_product_bar_judge(src: &str) -> bool {
    src.lines().any(|l| {
        let t = l.trim();
        t.starts_with("use ")
            && t.contains("product_bar")
            && (t.contains("judge") || t.contains("::*"))
    })
}

/// Whether `expr` is, or contains, a call to product_bar's `judge`.
fn is_a_product_bar_judge_call(src: &str, expr: &str) -> bool {
    expr.contains("product_bar::judge(")
        || (imports_product_bar_judge(src) && expr.contains("judge("))
}

/// Every call to product_bar's `judge` in `src`, as (binding, arguments).
///
/// `binding` is `Some(name)` when the call is the initialiser of `let name = `.
///
/// Reaching `judge` through an import is as correct as qualifying it, so a bare
/// `judge(` counts too — but only when `src` actually imports it, so an
/// unrelated gate's `judge` appearing in the evaluator later cannot turn these
/// tests red for a reason that has nothing to do with the Product seat.
fn product_bar_judge_calls(src: &str) -> Vec<(Option<String>, String)> {
    let mut anchors: Vec<&str> = vec!["product_bar::judge("];
    if imports_product_bar_judge(src) {
        anchors.push("judge(");
    }

    let mut out = Vec::new();
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    for anchor in anchors {
        out.extend(calls_at_anchor(src, anchor, &mut seen));
    }
    out
}

/// The calls whose `(` follows `anchor`, skipping opening parens already
/// claimed by an earlier (more specific) anchor.
fn calls_at_anchor(
    src: &str,
    anchor: &str,
    seen: &mut BTreeSet<usize>,
) -> Vec<(Option<String>, String)> {
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(i) = src[from..].find(anchor) {
        let start = from + i;
        let open = start + anchor.len() - 1;
        if !seen.insert(open) {
            from = open + 1;
            continue;
        }
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

/// The field names declared by `struct <name> { .. }` in `src`.
///
/// One level of braces, which is all a plain data struct has.
fn struct_fields(src: &str, name: &str) -> Vec<String> {
    let anchor = format!("struct {name} {{");
    let Some(start) = src.find(&anchor) else {
        return Vec::new();
    };
    let body = &src[start + anchor.len()..];
    let end = body.find('}').unwrap_or(body.len());
    body[..end]
        .lines()
        .filter_map(|l| {
            let t = l.trim().trim_start_matches("pub ").trim();
            let colon = t.find(':')?;
            let name = t[..colon].trim();
            if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Fields on `PrDiffContext` that carry text authored on the change.
///
/// `PrDiffContext` (src/git_manager/diff_context.rs) IS the change-under-review
/// struct: repo, pr_number, base_sha, head_sha, diff_content, changed_files.
/// The evaluator already receives it. Adding `pub pr_body: String` to it,
/// populating it where the context is built, and judging `&diff_ctx.pr_body` is
/// a correct, complete and arguably cleaner wiring than a sixty-ninth
/// parameter — and the previous revision of the two guards below failed it,
/// with a message accusing it of the precise defect it does not have. That is
/// this file's own standard ("a wiring guard that misreads the wiring and then
/// reports a correct implementation as broken is worse than no guard")
/// violated by the guard that states it.
///
/// The fact these guards pin is that the body reaches the gate. Which of the
/// two routes carries it is the implementer's to choose, so both are read.
fn diff_context_body_fields() -> Vec<String> {
    let src = without_line_comments(&source("src/git_manager/diff_context.rs"));
    struct_fields(&src, "PrDiffContext")
        .into_iter()
        .filter(|f| f.contains("body"))
        .collect()
}

/// The `let` statement that binds `name` in `src`, from the `let` to the `;`
/// that closes it.
///
/// Used to ask what a call-site argument was built from: if the body rides on
/// the change-under-review struct rather than on a parameter of its own, the
/// pipeline's obligation is that the statement producing that struct was handed
/// the body.
fn binding_statement(src: &str, name: &str) -> Option<String> {
    let mut from = 0usize;
    while let Some(i) = src[from..].find("let ") {
        let start = from + i;
        let tail = &src[start + "let ".len()..];
        let bound = tail
            .trim_start()
            .trim_start_matches("mut ")
            .trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>();
        if bound == name {
            let end = tail.find(';').map(|e| start + "let ".len() + e + 1);
            return Some(src[start..end.unwrap_or(src.len())].to_string());
        }
        from = start + "let ".len();
    }
    None
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

    // The fact, not the spelling: the evaluator has to be handed the text the
    // Product artifact is written in. A gate cannot measure text the evaluator
    // is never given, and a gate that measures nothing gates nothing.
    //
    // Two routes carry that fact and this test accepts either. A parameter
    // whose name says body is one. The other is a field whose name says body on
    // `PrDiffContext` — the change-under-review struct the evaluator already
    // receives — which is a correct, complete and arguably cleaner wiring than
    // a sixty-ninth positional argument. The previous revision of this test
    // promised that second route in a comment and then asserted the first, so
    // it would have failed the cleaner implementation while accusing it of the
    // one defect it does not have.
    let named_parameter = signature.contains("body");
    let body_fields = diff_context_body_fields();
    let on_the_change_under_review = signature.contains("PrDiffContext") && !body_fields.is_empty();

    assert!(
        named_parameter || on_the_change_under_review,
        "evaluate_pre_merge_gates never receives the change's body, which is where \
         the written problem and the done-when bar are authored. Hand it a parameter \
         whose name says body, or put the body on PrDiffContext — the \
         change-under-review struct this signature already takes — and either \
         satisfies this. PrDiffContext body fields: {body_fields:?}. Signature: \
         {signature:?}"
    );
}

#[test]
fn the_evaluator_computes_product_bar_status_by_judging_that_change() {
    assert_the_wiring_parsers_read_a_real_wiring();

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
            args.contains("body"),
            "the Product gate must be judged over the change's own body, which is \
             where the written problem and the done-when bar are authored; got \
             judge({args})"
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
            is_a_product_bar_judge_call(&src, init) || bindings.contains(&leading_ident(init));
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
fn the_review_pipeline_hands_the_evaluator_the_change_body() {
    // The evaluator can only judge what the pipeline gives it. `body` is
    // already in scope at this call site — it is handed to `review_pr` and to
    // `ensure_documentation_parity` a few lines above — so the only thing that
    // can go wrong is forgetting to pass it, and then the Product gate is a
    // name on the scorecard with nothing behind it.
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

    // Same two routes as the test above, for the same reason. Either the
    // pipeline hands the evaluator an argument that names the body, or the body
    // rides on the change-under-review struct — in which case the pipeline's
    // obligation is that the statement building that struct was handed the
    // body, since a `PrDiffContext` with an empty `pr_body` gates exactly as
    // much as no body at all.
    let handed_directly = args.iter().any(|a| a.contains("body"));

    let body_fields = diff_context_body_fields();
    let built_from_the_body = !body_fields.is_empty()
        && args.iter().any(|a| {
            let ident = leading_ident(a.trim_start_matches('&').trim_start_matches("mut ").trim());
            binding_statement(&src, &ident).is_some_and(|s| s.contains("body"))
        });

    assert!(
        handed_directly || built_from_the_body,
        "the review pipeline never hands the evaluator the pull request body, which \
         is where the written problem and the done-when bar are authored. Pass it as \
         an argument that names the body, or build the PrDiffContext from the body \
         and pass that. The gate otherwise has nothing to read, and a gate that reads \
         nothing gates nothing. Arguments: {args:?}. PrDiffContext body fields: \
         {body_fields:?}"
    );
}

/// Exercises the hand-rolled parsers the wiring guards depend on.
///
/// Not a `#[test]` of its own, deliberately. Nothing here touches the gate, so
/// standing alone it would be green from the moment it was written, and a test
/// that has never been observed failing publishes assurance it has not earned —
/// the same reason the static half of
/// `the_verdict_depends_on_nothing_but_the_change_it_was_handed` lives inside
/// that test. It runs first inside the wiring tests instead, which are red on
/// the absent wiring.
#[track_caller]
fn assert_the_wiring_parsers_read_a_real_wiring() {
    // The wiring guards are the only thing standing between a named Product
    // gate and one that blocks nothing, and they are hand-rolled parsers. A
    // parser that misreads a correct implementation and then reports it as the
    // exact defect it does not have is worse than no guard at all — that is
    // this file's own standard, applied to this file.
    //
    // `let x: GateStatus = judge(..)` is the case that motivated this: the
    // previous revision's identifier check rejected any name containing a
    // character outside [alnum_], which the type annotation supplies, so a
    // correct annotated wiring produced no binding and failed the test.
    for (head, want) in [
        (
            "        let product_bar_status = ",
            Some("product_bar_status"),
        ),
        (
            "        let product_bar_status: GateStatus = ",
            Some("product_bar_status"),
        ),
        (
            "        let mut product_bar_status = ",
            Some("product_bar_status"),
        ),
        (
            "        let mut product_bar_status: GateStatus = ",
            Some("product_bar_status"),
        ),
        (
            "        let product_bar_status = super::",
            Some("product_bar_status"),
        ),
        // Not an initialiser: the statement the `let` opened has already ended.
        (
            "        let earlier = compute();\n        report.field = ",
            None,
        ),
        // No `let` at all: a bare call in expression position.
        ("        gates.push(", None),
    ] {
        assert_eq!(
            let_binding_before(head).as_deref(),
            want,
            "let_binding_before({head:?}) misread the binding"
        );
    }

    // The comment stripper must not treat a URL inside a string literal as the
    // start of a comment; truncating there would blind every scan above.
    for (line, want) in [
        (
            "let x = judge(pr_body); // wire it up",
            "let x = judge(pr_body); ",
        ),
        ("// let x = judge(pr_body);", ""),
        (
            r#"let m = "see https://example.invalid/x"; let x = judge(pr_body);"#,
            r#"let m = "see https://example.invalid/x"; let x = judge(pr_body);"#,
        ),
        (
            r#"let m = "a \" // b"; judge(pr_body)"#,
            r#"let m = "a \" // b"; judge(pr_body)"#,
        ),
    ] {
        assert_eq!(
            strip_line_comment(line),
            want,
            "strip_line_comment({line:?}) cut the line in the wrong place"
        );
    }

    // The call finder must see a `judge` reached through an import, and must
    // not see a `judge` belonging to some other module.
    let imported = "use super::product_bar::judge;\n\
                    let product_bar_status = judge(pr_body);\n";
    assert_eq!(
        product_bar_judge_calls(imported),
        vec![(
            Some("product_bar_status".to_string()),
            "pr_body".to_string()
        )],
        "an imported judge call is as correct as a qualified one and must be found"
    );

    let foreign = "let shape_status = shape::judge(outcome);\n";
    assert!(
        product_bar_judge_calls(foreign).is_empty(),
        "another module's judge must not be mistaken for the Product seat's"
    );

    let qualified = "let product_bar_status = super::product_bar::judge(pr_body);\n";
    assert_eq!(
        product_bar_judge_calls(qualified),
        vec![(
            Some("product_bar_status".to_string()),
            "pr_body".to_string()
        )],
        "a qualified call must be found exactly once, not twice"
    );

    // The struct-field reader, which is what lets the body reach the gate
    // through `PrDiffContext` instead of through a parameter of its own. It has
    // to see a field that is there and not invent one that is not, or the
    // widened guards above become either a no-op or a false accusation.
    let decl = "#[derive(Debug, Clone)]\n\
                pub struct PrDiffContext {\n\
                \x20   pub repo: String,\n\
                \x20   pub pr_number: u64,\n\
                \x20   pub pr_body: String,\n\
                \x20   pub diff_content: String,\n\
                }\n";
    assert_eq!(
        struct_fields(decl, "PrDiffContext"),
        vec!["repo", "pr_number", "pr_body", "diff_content"],
        "struct_fields misread the change-under-review struct"
    );
    assert!(
        struct_fields(decl, "SomeOtherStruct").is_empty(),
        "struct_fields must find nothing for a struct that is not declared here"
    );

    // And it must be pointed at the real struct, not at a file that no longer
    // declares it. `diff_context_body_fields()` returning nothing is how the
    // guards above conclude "the body does not ride on the change-under-review
    // struct", so a moved file or a renamed struct would quietly turn that
    // branch into a permanent no — an answer the guard would give with no
    // evidence behind it.
    let real = struct_fields(
        &without_line_comments(&source("src/git_manager/diff_context.rs")),
        "PrDiffContext",
    );
    for field in ["repo", "pr_number", "diff_content", "changed_files"] {
        assert!(
            real.iter().any(|f| f == field),
            "src/git_manager/diff_context.rs no longer declares PrDiffContext with a \
             {field:?} field, so this file is not reading the change-under-review \
             struct any more and the wiring guards' second route answers 'no' without \
             looking. Found: {real:?}"
        );
    }

    // The binding reader, which is what lets the pipeline satisfy its half by
    // building the diff context from the body rather than passing it separately.
    let pipeline = "    let repo_dir = state.git_mgr.ensure_repo_cloned(repo).await?;\n\
                     \x20   let diff_ctx = state\n\
                     \x20       .git_mgr\n\
                     \x20       .prepare_pr_diff(repo, pr_number, base_sha, head_sha, body)\n\
                     \x20       .await?;\n";
    let bound = binding_statement(pipeline, "diff_ctx").expect("diff_ctx is bound by a let");
    assert!(
        bound.contains("prepare_pr_diff") && bound.contains("body") && bound.ends_with(';'),
        "binding_statement read the wrong span for diff_ctx: {bound:?}"
    );
    assert!(
        !binding_statement(pipeline, "repo_dir")
            .expect("repo_dir is bound by a let")
            .contains("prepare_pr_diff"),
        "binding_statement must stop at the semicolon that closes the statement, or \
         every binding looks like it was built from every later value"
    );
    assert!(
        binding_statement(pipeline, "cert_report").is_none(),
        "binding_statement must find nothing for an identifier no let binds"
    );
}
