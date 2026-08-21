//! The Product seat: a change must state its bet and its acceptance bar.
//!
//! ADR-0002 Discover §1 — "Product. Job: the bet and the acceptance bar.
//! Artifact: written problem + done-when. Measurement: Quality cannot sign off
//! without it." The sequence section adds: "Quality sign-off must fail if
//! Product's bar is missing."
//!
//! # Absence is `Failed`, not `NotMeasured`
//!
//! The artifact is authored on the change under review, by the person opening
//! it. A change with no bar has not withheld evidence from the gate; it has
//! failed to produce the artifact. `NotMeasured` and `Warning` both certify, so
//! reporting absence either way is the "named gate, no measurement" pattern the
//! ADR's honesty law forbids. The same holds for a heading with nothing under
//! it, `TBD`, an unticked checkbox and an unfilled template comment: the
//! measurement is the content, not the marker.
//!
//! # Fixture rules a later edit must not simplify away
//!
//! Each closed a hole a real implementation walked through. They constrain the
//! FIXTURES, not the implementation; the detail lives on each fixture.
//!
//!   1. **The must-fail set is derived, never enumerable.** `multiply()` crosses
//!      `DEFERRAL_STEMS` and `PHRASE_DEFERRALS` with trailing punctuation, case
//!      and bullet wrappers into hundreds of strings that appear nowhere here as
//!      literals. Collapse a derived family back to the literals it grew from
//!      and copying the table into the gate is a complete implementation again
//!      — which is how `PHRASES.contains(&section.trim())` certified a done-when
//!      reading "See the linked issue.".
//!   2. **Every must-fail token is ALSO pinned inside real content** — checkbox,
//!      HTML comment, pointer, deferral stem, deferral phrase, invisible
//!      character. Each pair states one rule: strip what is not content, then
//!      judge what is left. Drop either half and "reject on sight" satisfies the
//!      suite, which fails ordinary pull requests at very high incidence. A
//!      fabricated accusation is the same defect as a false green pointed the
//!      other way.
//!   3. **The two sets overlap in length at BOTH ends.** `long_prose()` under a
//!      placeholder must fail; `long_body_with_bar_at_the_end` puts a real bar
//!      two kilobytes and thirty lines into a body that must pass. One side
//!      alone leaves `&body[..450]` and `lines().take(18)` unfalsified.
//!      `SHORT_REAL_CONTENT` bounds it from below in characters AND words, so a
//!      floor bolted on top of a content check does not survive either.
//!   4. **Both sections are held to one standard**, and every failing fixture
//!      goes through `expect_failed`, so `NotMeasured`, `Warning` and `Errored`
//!      are rejected everywhere.
//!   5. **Section boundaries are falsified in both directions**, at every
//!      heading depth and weight: a third section must hide neither an empty
//!      section nor a real one.
//!   6. **The marker is a heading line, not a phrase**, and in those fixtures the
//!      marker-bearing prose sentence is deliberately NOT the last line of the
//!      body — otherwise the spurious section a `contains` predicate opens after
//!      it is empty and the wrong gate reaches the right verdict by accident.
//!   7. **Every line-ending-sensitive family runs over both.** A trailing `\r`
//!      defeats the `ends_with` test that recognises `**Testing**` and
//!      `## Done when:`, and CRLF is what the GitHub web UI submits.
//!   8. **The determinism rule names effects, not spellings** — see
//!      `impure_import`.
//!
//! # The measurement is a function, not a string
//!
//! `missing_artifacts` returns which halves are absent; `judge` renders the
//! verdict and the message from it. The message is asserted positively on the
//! whole message and negatively on the RESIDUE — the message minus the body's
//! own lines — so quoting the offending section back at the author stays legal
//! while naming an artifact they DID write does not. The surviving section's
//! heading is varied over all thirteen spellings in
//! `the_message_holds_its_ground_however_the_surviving_section_is_headed`,
//! because one constant naming both artifacts with `## Problem` and
//! `## Done when` was otherwise subtracted out of its own residue.
//!
//! # What is deliberately NOT pinned
//!
//! Which words announce a section: an implementation that also recognises
//! `## Acceptance criteria`, `## Why` or unheaded prose passes unchanged,
//! because no test here requires a body that genuinely states both artifacts to
//! fail. That is why
//! `a_bold_only_lead_in_line_is_a_heading_and_ends_the_section_above_it` uses
//! `**Testing**`, `**Rollout**` and `**Notes**` over testing prose rather than
//! `**Acceptance criteria**` over a real bar.
//!
//! The markdown AROUND those two words IS pinned, in every case, depth, trailing
//! colon and bold spelling: a gate matching two byte strings withholds
//! certification from essentially every real pull request.
//!
//! Also unpinned: the render order of `missing_artifacts`, the prose of the
//! messages beyond naming each missing artifact and differing from one another,
//! and the change's title — `judge` takes the body alone.

use anvil::pre_merge_guard::product_bar;
use anvil::pre_merge_guard::product_bar::Artifact;
use anvil::pre_merge_guard::report::TOTAL_GATES;
use anvil::pre_merge_guard::{GateStatus, PreMergeCertificationReport};
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// The companion subject line for the bodies below. It is NOT an input to
/// `judge`, which takes the body alone; see the module docs.
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

/// A real acceptance bar written as a list of separately checkable criteria,
/// which is how most of them are written. Multi-line on purpose: an extractor
/// that takes the first line and one that takes the first non-blank line are
/// two different wrong gates, and several families below separate them here.
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

/// A one-line acceptance bar with no bullet, for the shapes where the bar is
/// written on the marker's own line (`## Done when: …`).
const INLINE_BAR: &str = "checkout p99 is under 5ms and the scorecard names the canary it queried";

/// A one-line problem with no bullet, for the same inline shapes.
const INLINE_PROBLEM: &str = "checkout p99 regressed to 40ms when the cache change landed";

/// Genuine content written in as few characters and as few words as anyone
/// writes it. Every one must pass.
///
/// This is the floor. A length rule bolted ON TOP OF a correct content check is
/// a different mistake from a length rule used INSTEAD OF one, and `SHORT_BAR`
/// does not falsify it — `core.chars().count() >= 9`, "the line must contain a
/// space" and "at least three tokens" were all green before these existed.
/// `"- p99<5ms"` has no space at all; `"- 빌드 통과"` is five characters in thirteen
/// bytes, so a floor in characters, bytes or words rejects one of them.
const SHORT_REAL_CONTENT: &[&str] = &["- no 5xx", "- p99<5ms", "- 빌드 통과", "- 결제 실패 급증"];

/// Sections whose first word merely *begins* with a deferral stem, plus one
/// that opens with a deferral token used as an ordinary word. Every one must
/// pass.
///
/// Without them the derived family is satisfiable by
/// `STEMS.iter().any(|s| normalised.starts_with(s))`, which then reports a
/// missing bar for `- Navigation completes in under 200ms` and a missing
/// problem for a section opening `Native TLS …`. The last entry moves the same
/// defect to token level: `TODO` is the first token and the rest of the line is
/// content. What separates it from `"TODO: write the acceptance criteria here"`
/// is the separator, not the token.
const REAL_CONTENT_WITH_A_DEFERRAL_PREFIX: &[&str] = &[
    "- Navigation completes in under 200ms",
    "Native TLS was disabled by the cache change, so every canary poll now falls back to the \
     plaintext listener.",
    "- Wipe the stale rollout entries before the queue admits the change",
    "TODO comments are removed from src/pre_merge_guard/",
];

/// Placeholders shipped in real pull request bodies. A gate that reads any of
/// them as an artifact is measuring the presence of a heading.
///
/// Deliberately **not** the whole must-fail set: copying it into the gate as an
/// exact-match list satisfies these and nothing else — see `derived_deferrals`,
/// `PHRASE_DEFERRALS` and `UNICODE_BLANKS`. Entries that `derived_deferrals()`
/// already produces verbatim are not repeated here; what is left is what that
/// derivation does not reach. The last two are the deferrals announced by a
/// separator, which `real_content_that_merely_begins_with_a_deferral_stem_passes`
/// needs to keep failing, and the longest of them is what bounds
/// `SHORT_REAL_CONTENT` from above.
const PLACEHOLDERS: &[&str] = &[
    "- ",
    "*",
    "...",
    "   \n   \n",
    "- [ ] ",
    "<!-- what problem does this solve? -->",
    "TBD - will fill this in before merge",
    "TODO: write the acceptance criteria here",
];

/// Deferral stems. Never enumerated as finished strings: the tests multiply
/// them by trailing punctuation, case and bullet wrappers, so the hundreds of
/// must-fail sections they produce appear nowhere in this file as literals an
/// implementation could copy.
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
    "todo(jason)",
];

/// Deferrals sharing no prefix with any entry in `PLACEHOLDERS`, so a table
/// copied from that constant cannot reach them. The third and fourth also pin a
/// product decision — the artifact lives on the change under review, so a
/// pointer elsewhere is not the artifact. Listed in open_questions for a veto.
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
    "\u{2003}\u{200b}\n\u{feff}",
    "\u{3000}",
];

/// A Korean problem statement. Short in characters, long in bytes.
const KO_PROBLEM: &str =
    "머지 큐가 조회되지 않은 카나리를 통과로 처리해서, 측정된 적 없는 변경이 승인된다.";

/// A Korean acceptance bar, written as checkable criteria.
const KO_BAR: &str = "- 조회되지 않은 카나리는 NotMeasured 로 보고하고 머지 큐 진입을 막는다\n\
     - 스코어카드가 둘 중 무엇이었는지 이름을 밝힌다";

/// The two words of each heading in the markdown an author wraps them in. Every
/// entry is the SAME WORDS: case, depth, a trailing colon and a bold label are
/// formatting. Synonyms are deliberately absent — which words announce a
/// section is left open, per the module docs.
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
/// markdown allows. Used for the *passing* family: a complete artifact followed
/// by any of these is still a complete artifact.
const THIRD_SECTION_HEADERS: &[&str] = &[
    "# Testing",
    "## Testing",
    "### Testing",
    "**Testing**",
    "Testing:",
];

/// The subset of `THIRD_SECTION_HEADERS` that must TERMINATE the section above
/// it, so an empty `## Done when` cannot swallow the testing notes.
///
/// `"Testing:"` is deliberately absent. A rule making a colon-terminated line
/// followed by a blank line into a heading also rejects `Acceptance criteria:`
/// above a real list of bullets — the two bodies are structurally identical and
/// only the English tells them apart. So the suite decides: a colon-terminated
/// line is never a boundary (pinned passing by
/// `a_short_colon_terminated_lead_in_line_is_writing_not_a_section_boundary`),
/// and a bold-only line always is (pinned by
/// `a_bold_only_lead_in_line_is_a_heading_and_ends_the_section_above_it`, and
/// implied by `**Done when**` being a marker). Both halves are in
/// open_questions; whichever a human vetoes, the pair must stay consistent,
/// which is what `assert_the_boundary_families_state_one_consistent_rule`
/// checks.
const BOUNDARY_HEADERS: &[&str] = &["# Testing", "## Testing", "### Testing", "**Testing**"];

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

/// `body`, re-terminated for `eol`. None of these fixtures contains a `\r` of
/// its own, so the two forms are the same string.
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

/// Two kilobytes of genuine background over thirty-odd lines, saying nothing
/// about what done looks like.
///
/// Bracketing the passing set from ABOVE closes off a length *threshold* and
/// nothing else. It leaves a *truncation* unfalsified, and truncation produces
/// a fabricated accusation: `&pr_body[..450]` and `pr_body.lines().take(18)`
/// both passed every behavioural test here before this existed, because no
/// must-pass body put its bar further in than that. This background carries a
/// real bar at the far end and must pass; the same background with that section
/// emptied must still fail. It contains none of the vocabulary the failure
/// message is judged on — see
/// `assert_the_content_fixtures_carry_none_of_the_message_vocabulary`.
const LONG_BACKGROUND_LINES: &[&str] = &[
    "The merge queue admits a change as soon as every gate reports an acceptable",
    "status, and nine of those gates read their evidence from the rollout",
    "controller rather than from the change itself.",
    "",
    "The controller answers a poll from a cache whenever the upstream store is",
    "slow, and the cache has no notion of staleness. A poll that times out is",
    "served the last document the controller happened to hold, which on a quiet",
    "afternoon can be several hours old.",
    "",
    "What that has cost us across the last three release trains:",
    "",
    "- eleven changes were admitted against a canary window that had already closed",
    "- four of those eleven were rolled back inside the same hour",
    "- the scorecard showed nine green gates for every one of the four",
    "",
    "The rollback that started this was a checkout latency regression. The canary",
    "window had closed forty minutes before the change was queued, so the P99 the",
    "gate compared against was the P99 of the change before it, and the two",
    "changes touched the same code path.",
    "",
    "Background on the cache, for anyone who has not read the controller:",
    "",
    "The controller was written when the upstream store was in-process and a poll",
    "could not fail. The cache was added during the migration to the shared store,",
    "as a way to keep the dashboard responsive while the store warmed up. Nobody",
    "revisited it once the gates began reading from the same endpoint.",
    "",
    "What I have already ruled out, so nobody repeats the work:",
    "",
    "- the upstream store is healthy; its own latency has not moved in six weeks",
    "- the timeout is 250ms and has been since the endpoint was introduced",
    "- raising the timeout to 2s moves the failure rate but not the staleness",
    "- the controller records no metric for how old a served document is",
    "",
    "So the fix has to be on the reading side rather than the serving side: a gate",
    "that cannot tell how old its evidence is must not report a verdict at all.",
];

fn long_background() -> String {
    LONG_BACKGROUND_LINES.join("\n")
}

/// The long background with `done_when` written at the far end of it. Passed a
/// real bar it must pass and is longer than every body here that must fail;
/// passed `""` it is the same body with the section emptied and must fail.
fn long_body_with_bar_at_the_end(done_when: &str) -> String {
    format!(
        "## Problem\n\n{}\n\n## Done when\n\n{done_when}\n",
        long_background()
    )
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
/// bullet wrappers a markdown section arrives in. Derived rather than listed on
/// purpose: satisfying this set by enumeration means enumerating several hundred
/// strings that cannot be read off this file, which is more work than
/// normalising and comparing.
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

/// The phrase deferrals under the same multiplication. The four raw literals on
/// their own left the family satisfiable by `PHRASES.contains(&section.trim())`,
/// which then certified `"See the linked issue."` and `"- see the linked
/// issues"` — the spellings a human actually types.
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

/// `msg` with every non-blank line of `body` subtracted from it: what the gate
/// says ON ITS OWN ACCOUNT.
///
/// Quoting the offending section back at the author is legal and helpful, so the
/// body's text is removed before the message's vocabulary is judged. Without the
/// subtraction the whole message contract is satisfiable by one constant naming
/// both artifacts with the body echoed after it: every positive assertion holds
/// and the three messages differ because the three BODIES differ.
///
/// Longest lines first, so a line containing a shorter one is removed whole, and
/// each removal leaves a space behind so subtracting a `-` cannot join two words
/// into vocabulary nobody wrote.
fn message_residue(msg: &str, body: &str) -> String {
    let mut lines: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    lines.sort_by_key(|l| std::cmp::Reverse(l.len()));

    let mut residue = msg.to_string();
    for line in lines {
        residue = residue.replace(line, " ");
    }
    residue
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

    // The negative, on what the gate said ON ITS OWN ACCOUNT. The positive
    // above is asserted on the whole message, so naming the missing artifact by
    // quoting the empty heading is legal; the negative is asserted on the
    // residue, so naming an artifact the author DID write is not. That is what
    // stops one constant string listing both artifacts, plus an echo of the
    // body, from satisfying this whole file.
    //
    // See open_questions: this also forbids a message that reports the artifact
    // that IS present, which is helpful prose a human may prefer to keep.
    let residue = message_residue(&msg, body);
    for artifact in [Artifact::WrittenProblem, Artifact::DoneWhenBar] {
        if want.contains(&artifact) {
            continue;
        }
        assert!(
            !names(artifact, &residue),
            "{context}: the gate did not find {artifact:?} missing, and the author did \
             write it, but the message names it anyway on its own account — telling \
             them to go and write the thing they already wrote. Quoting the offending \
             section is legal and is subtracted before this check; this is the \
             message minus the body. Missing: {want:?}. Message: {msg:?}. Residue: \
             {residue:?}"
        );
    }

    msg
}

/// The failure-message vocabulary must not be smuggled in from the fixtures:
/// `message_residue` subtracts the body's own lines, so a fixture that itself
/// said "problem" or "acceptance" would exempt the gate from the rule the
/// message is held to. Asserted from inside a test so it is never green on its
/// own.
#[track_caller]
fn assert_the_content_fixtures_carry_none_of_the_message_vocabulary() {
    let background = long_background();
    for (name, fixture) in [
        ("PROBLEM", PROBLEM),
        ("BAR", BAR),
        ("MULTILINE_BAR", MULTILINE_BAR),
        ("SHORT_PROBLEM", SHORT_PROBLEM),
        ("SHORT_BAR", SHORT_BAR),
        ("INLINE_PROBLEM", INLINE_PROBLEM),
        ("INLINE_BAR", INLINE_BAR),
        ("KO_PROBLEM", KO_PROBLEM),
        ("KO_BAR", KO_BAR),
        ("THIRD_SECTION_BODY", THIRD_SECTION_BODY),
        ("LONG_BACKGROUND_LINES", background.as_str()),
        ("SHORT_REAL_CONTENT[0]", SHORT_REAL_CONTENT[0]),
        ("SHORT_REAL_CONTENT[1]", SHORT_REAL_CONTENT[1]),
        ("SHORT_REAL_CONTENT[2]", SHORT_REAL_CONTENT[2]),
        ("SHORT_REAL_CONTENT[3]", SHORT_REAL_CONTENT[3]),
    ] {
        assert!(
            !names_the_problem(fixture) && !names_the_bar(fixture),
            "fixture invariant: {name} must contain none of the vocabulary the failure \
             message is judged on, or subtracting it from the message exempts the gate \
             from the rule that message is held to. Fixture: {fixture:?}"
        );
    }
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
    // A small change that has still done Product's job. `SHORT_BAR` is eleven
    // bytes — shorter than four of the placeholders that must fail — so no
    // length rule used INSTEAD OF a content check admits this and rejects those.
    // A floor bolted ON TOP OF one is a different mistake, closed by
    // `content_passes_however_few_characters_and_words_it_takes`.
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
    // This corpus already carries Korean, so the gate will be handed non-ASCII
    // bodies. Hangul is three bytes per character: a byte-length rule and a
    // character-length rule disagree about this fixture.
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
    // A suite whose every passing body used the byte strings "## Problem" and
    // "## Done when" admitted a gate matching exactly those two, which then
    // rejects "## Done When", "### Done when", "## Done when:" and
    // "**Done when**" — and this repository ships no PULL_REQUEST_TEMPLATE
    // forcing one spelling, so once wired into seal() it blocks essentially
    // every real change. Every marker below is the SAME TWO WORDS.
    //
    // Both line endings, because they are not the same test: every marker a gate
    // recognises with `ends_with` is defeated by the trailing `\r` a
    // browser-submitted body carries.
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

        // THE SAME MARKERS WITH NO BLANK LINE UNDER THEM. Every passing body
        // used to put a blank line between marker and content, so
        // `is_bold_only(line) && next_is_blank` recognised no marker at all in
        //
        //     **Done when**
        //     - p99 < 5ms
        //
        // and reported BOTH artifacts missing from a body carrying both.
        for problem_marker in PROBLEM_MARKERS {
            for done_when_marker in DONE_WHEN_MARKERS {
                expect_passed(
                    &as_eol(
                        &format!(
                            "{problem_marker}\n{PROBLEM}\n\n{done_when_marker}\n{MULTILINE_BAR}\n"
                        ),
                        eol,
                    ),
                    &format!(
                        "a complete Product artifact whose content starts on the line \
                         directly under {problem_marker:?} and {done_when_marker:?}, {eol:?}"
                    ),
                );
            }
        }

        // THE COLON-TERMINATED MARKERS WITH THEIR CONTENT ON THE SAME LINE.
        // `"## Done when:"` is a marker, so an implementer strips the colon and
        // matches the heading text with `==` — which `## Done when: p99 < 5ms`
        // defeats, reporting a missing bar for a change that stated one.
        for problem_marker in PROBLEM_MARKERS.iter().filter(|m| m.ends_with(':')) {
            for done_when_marker in DONE_WHEN_MARKERS.iter().filter(|m| m.ends_with(':')) {
                expect_passed(
                    &as_eol(
                        &format!(
                            "{problem_marker} {INLINE_PROBLEM}\n\n{done_when_marker} {INLINE_BAR}\n"
                        ),
                        eol,
                    ),
                    &format!(
                        "both artifacts written on their own marker lines, \
                         {problem_marker:?} and {done_when_marker:?}, {eol:?}"
                    ),
                );
            }
        }
        // And each of them inline beside an ordinarily-headed counterpart, so
        // the recognition is not pinned only when both sections take the same
        // shape.
        for done_when_marker in DONE_WHEN_MARKERS.iter().filter(|m| m.ends_with(':')) {
            expect_passed(
                &as_eol(
                    &format!("## Problem\n\n{PROBLEM}\n\n{done_when_marker} {INLINE_BAR}\n"),
                    eol,
                ),
                &format!("a bar written on the {done_when_marker:?} line itself, {eol:?}"),
            );
        }
        for problem_marker in PROBLEM_MARKERS.iter().filter(|m| m.ends_with(':')) {
            expect_passed(
                &as_eol(
                    &format!("{problem_marker} {INLINE_PROBLEM}\n\n## Done when\n\n{BAR}\n"),
                    eol,
                ),
                &format!("a problem written on the {problem_marker:?} line itself, {eol:?}"),
            );
        }

        // The mirrors for both widenings, so neither can fail open. A marker
        // whose content starts on the next line still has to be judged on that
        // content, and a marker with a deferral on its own line is a deferral.
        for done_when_marker in DONE_WHEN_MARKERS {
            expect_missing(
                &as_eol(
                    &format!("## Problem\n\n{PROBLEM}\n\n{done_when_marker}\nTBD\n"),
                    eol,
                ),
                &[Artifact::DoneWhenBar],
                &format!("a deferral on the line directly under {done_when_marker:?}, {eol:?}"),
            );
        }
        for done_when_marker in DONE_WHEN_MARKERS.iter().filter(|m| m.ends_with(':')) {
            expect_missing(
                &as_eol(
                    &format!("## Problem\n\n{PROBLEM}\n\n{done_when_marker} TBD\n"),
                    eol,
                ),
                &[Artifact::DoneWhenBar],
                &format!("a deferral written on the {done_when_marker:?} line itself, {eol:?}"),
            );
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
    // this half the fix for that one degenerates into "ignore everything after
    // the marker", which reads a bar of zero length and fails every filled-in
    // template instead. Both line endings, because a trailing `\r` defeats the
    // `ends_with` test that recognises `**Testing**` and `Testing:`.
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
    // THE HEADLINE BOUNDARY DEFECT. An extractor that runs the done-when to the
    // next occurrence of "\n## " returns Passed for a body whose done-when
    // heading is empty and whose next section is `### Testing`, `**Testing**` or
    // `# Testing` — swallowing the testing notes as the acceptance bar. That is
    // the pasted-template defect in the exact shape real templates produce.
    //
    // Both line endings: `str::lines()` strips a trailing `\r` and `.trim()`
    // eats it, so a gate finding its boundary with `ends_with("**")` over
    // `body.split('\n')` sees `**Testing**\r` as prose and certifies.
    //
    // `BOUNDARY_HEADERS`, not `THIRD_SECTION_HEADERS`: see that constant's docs
    // for why `"Testing:"` is not required to end a section, and
    // `assert_the_boundary_families_state_one_consistent_rule` for the invariant
    // that keeps the two families from drifting apart.
    for eol in BOTH_EOLS {
        for header in BOUNDARY_HEADERS {
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
    // THE HEADLINE CASE. Every other failing fixture here is built from this
    // file's own heading template, which is the one shape where an
    // implementation has no temptation to fail open. These are the bodies people
    // actually submit: ordinary prose with no headings, and an unfilled
    // template. A gate answering `NotMeasured` here — "no section I recognise" —
    // is acceptable to `is_certified_ready`, so it certifies the majority of
    // real pull requests while the scorecard names a gate that measured nothing.
    //
    // None of these states an acceptance criterion in any spelling, so none
    // collides with the gate's freedom over the marker format. The first three
    // use `expect_at_least_missing`: whether unheaded prose also counts as the
    // written problem is a recognition choice left open, while the absence of
    // the bar is not open at all.
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
    // The two long cases are half the reason a global length threshold cannot
    // satisfy this suite: `long_prose()` under an empty done-when is longer than
    // every short passing body and must fail. The other half — a passing body
    // longer than every failing one — is
    // `a_bar_at_the_far_end_of_a_long_body_is_still_the_artifact`.
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

#[test]
fn a_bar_at_the_far_end_of_a_long_body_is_still_the_artifact() {
    // The other half of the length property. The test above brackets the passing
    // set from ABOVE, which rules out a threshold and nothing else. It does not
    // rule out a TRUNCATION, and truncation produces a fabricated accusation
    // rather than a false green: `&pr_body[..450.min(pr_body.len())]` and
    // `pr_body.lines().take(18)` each passed every behavioural test in this file
    // before this fixture existed. A bounded regex, a `.take(n)`, a `&body[..N]`
    // clamp and an extractor that stops after two sections are all ordinary ways
    // to write this gate, and each ships a comment telling the author of a long,
    // careful pull request that they wrote no bar.
    //
    // So this brackets from BELOW: a body that must pass, longer than every body
    // here that must fail, with its bar at the far end. Its mirror — the same
    // body with that section emptied — must still fail.
    let passing = long_body_with_bar_at_the_end(MULTILINE_BAR);
    let mirror = long_body_with_bar_at_the_end("");

    // The fixture invariants first, so they are exercised rather than stranded
    // behind the measurement.
    let longest_failing = [
        mirror.clone(),
        body_with(&long_prose(), ""),
        body_with("", &long_bar()),
        body_with(&long_prose(), "TODO: write the acceptance criteria here"),
        problem_only(&long_prose()),
        bar_only(&long_bar()),
    ]
    .iter()
    .map(String::len)
    .max()
    .unwrap();
    assert!(
        passing.len() > longest_failing,
        "fixture invariant: the longest body that must PASS ({} bytes) has to be \
         longer than every body that must fail ({longest_failing} bytes), or the two \
         sets do not overlap in length at this end and a gate that reads only a prefix \
         of the change is unfalsifiable",
        passing.len()
    );

    let marker_byte = passing
        .find("## Done when")
        .expect("fixture invariant: the long body must carry a done-when marker");
    let marker_line = passing
        .lines()
        .position(|l| l.trim() == "## Done when")
        .expect("fixture invariant: the done-when marker must be on a line of its own");
    assert!(
        marker_byte > 1500 && marker_line > 25,
        "fixture invariant: the acceptance bar must sit deep enough into the body that \
         a gate reading a prefix of it cannot see the marker at all; got byte \
         {marker_byte}, line {marker_line}"
    );
    assert!(
        LONG_BACKGROUND_LINES.len() > 30,
        "fixture invariant: the background must be spread over more than thirty lines, \
         or a `.take(n)` extractor is still unfalsified; got {}",
        LONG_BACKGROUND_LINES.len()
    );

    for eol in BOTH_EOLS {
        expect_passed(
            &as_eol(&passing, eol),
            &format!(
                "two kilobytes of genuine background with a real acceptance bar at the \
                 end of it; reading only the opening of a change and reporting the rest \
                 absent is a fabricated accusation aimed squarely at the authors who \
                 wrote the most, {eol:?}"
            ),
        );
        expect_missing(
            &as_eol(&mirror, eol),
            &[Artifact::DoneWhenBar],
            &format!(
                "the same two kilobytes of background with the done-when section \
                 emptied; length is not the measurement in this direction either, \
                 {eol:?}"
            ),
        );
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
    // A gate whose substance check is `!PLACEHOLDERS.contains(section)` passes
    // every enumerated test above and then certifies a done-when reading "TBD.",
    // "tbd!", "N/A.", "Todo:", "WIP", "- [x]" or "xxx". A hardcoded table is a
    // complete implementation only while the must-fail set is enumerable.
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

    // Multiplied out exactly as `DEFERRAL_STEMS` is. Four lowercase,
    // unpunctuated, unbulleted literals made this family satisfiable by
    // `PHRASES.contains(&section.trim())`, which then certified "See the linked
    // issue.", "Same as above!" and "- see the linked issues".
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
    // The product decision the phrase deferrals stand for, on the shape it
    // actually takes: a section whose entire content is a reference to another
    // place has not produced the artifact, because the reviewer, the auditor and
    // the scorecard all read this body and none of them follows the link.
    //
    // Pinned separately from the phrases because a bare URL and a bare issue
    // reference share no English with any of them. Listed in open_questions as a
    // decision a human can veto — a shop that accepts "the bar is in the linked
    // issue" wants this family deleted, not weakened.
    let pointers = [
        "https://example.invalid/issues/4192",
        "See https://example.invalid/issues/4192",
        "#4192",
        "example.invalid/issues/4192",
    ];

    for pointer in pointers {
        assert_placeholder_fails_in_both_sections(pointer, "a pointer to somewhere else");
    }

    // THE MIRROR. No passing fixture anywhere used to contain `http`, a bare
    // domain or a `#1234`, so
    //
    //     !(line.contains("http") || ISSUE_RE.is_match(line)) && …
    //
    // — reject any line CONTAINING a pointer rather than a line that IS one —
    // passed the whole suite and then reported a missing bar for a bar citing
    // the dashboard it will be checked on, and a missing problem for a body
    // opening `Fixes #4192:`. Same hole for `PHRASE_DEFERRALS`, where
    // `section.contains("same as above")` satisfied the derived family.
    //
    // The rule the pair states — a section whose WHOLE content is a pointer is
    // no artifact; a pointer beside real content does not erase the content —
    // is the only one that satisfies both halves.
    for content in [
        "- the checkout p99 panel at https://grafana.invalid/d/canary shows under 5ms for two \
         windows",
        "Fixes #4192: the canary gate rebuilds its verdict from `passed`, which is true for a \
         canary nobody queried.",
        "- the retry budget behaves the same as above for the plaintext listener",
        "- the example.invalid/d/canary panel stays under 5ms for two consecutive windows",
    ] {
        assert_real_content_passes_in_both_sections(content, "real content citing a pointer");
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

    // THE MIRROR. Without it this family is satisfied by rejecting any section
    // that CONTAINS one of these characters, which then reports both artifacts
    // missing from a complete, well-written body whose only sin is a
    // non-breaking space between two words — what every body pasted out of
    // Notion, Google Docs or Confluence carries, and what a leading BOM adds to
    // one pasted out of a file.
    //
    // The rule the two halves state: strip the invisible characters, then judge
    // what is left. Not: reject on sight.
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
    // THE DECISION THIS TEST SETTLES. With every passing section leading with
    // real content on every line and every failing section a deferral on every
    // line, `substantive(section)` could legally collapse to
    // `substantive(first non-blank line)` — and so could
    // `section.lines().any(substantive)`. The two disagree about a
    // partially-filled checklist, one of the commonest real shapes there is.
    //
    // The rule is `any`: a section is the artifact if anything in it is. An
    // author who left the first checkbox blank and then wrote three checkable
    // criteria has done Product's job.
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

    // A CHECKBOX THAT CARRIES CONTENT — the commonest spelling of an acceptance
    // bar there is, and the one every pull request template produces.
    //
    // The only checkbox lines anywhere else in this file are EMPTY ones that
    // must fail, so
    //
    //     !DEFERRAL_STEMS.iter().any(|s| n == *s || n.starts_with(&format!("{s} ")))
    //
    // — treating `- [ ] ` as announcing a deferral the way `TODO: ` does — was
    // green across the whole file and then reported a missing bar for
    // `- [ ] p99 < 5ms`. Strip the checkbox marker, then judge the remainder.
    // Run through both sections, so the rule is not applied to the bar and
    // forgotten on the bet.
    for checklist in [
        "- [ ] p99 < 5ms\n- [ ] the scorecard names the canary it queried",
        "- [x] p99 < 5ms\n- [ ] the scorecard names the canary it queried",
        "- [X] p99 < 5ms\n- [ ] the scorecard names the canary it queried",
    ] {
        assert_real_content_passes_in_both_sections(checklist, "a checklist carrying criteria");
    }

    // A checklist whose first box is empty and whose remaining boxes carry
    // criteria: the partially-filled template, with the marker on every line.
    expect_passed(
        &body_with(
            PROBLEM,
            "- [ ]\n- [x] p99 < 5ms\n- [ ] the scorecard names the canary it queried",
        ),
        "an empty checkbox above two checkboxes that carry real criteria",
    );

    // AN UNFILLED TEMPLATE COMMENT ABOVE REAL CONTENT, which is what a filled-in
    // template actually looks like: authors leave the prompt comment where it is
    // and type underneath it. The comment is pinned as a WHOLE section in
    // `PLACEHOLDERS`, so a section-level `!s.contains("<!--")` passed every
    // fixture here (the mixed cases above mix checkboxes and `TBD`, never a
    // comment) and then reported BOTH artifacts missing from the commonest
    // filled-in template body there is.
    expect_passed(
        &body_with(
            &format!("<!-- what problem does this solve? -->\n{PROBLEM}"),
            &format!("<!-- how will we know this is done? -->\n{MULTILINE_BAR}"),
        ),
        "a filled-in template whose prompt comments were left above the content the \
         author typed under them",
    );
    expect_passed(
        &body_with(
            &format!("{PROBLEM}\n<!-- what problem does this solve? -->"),
            &format!("{MULTILINE_BAR}\n<!-- how will we know this is done? -->"),
        ),
        "the same template comments left trailing below the content instead of above \
         it",
    );
    expect_passed(
        &body_with(
            &format!("<!-- what problem does this solve? -->\n\n{PROBLEM}"),
            &format!("<!-- how will we know this is done? -->\n\n{MULTILINE_BAR}"),
        ),
        "the same template comments with a blank line between the prompt and the \
         content",
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
    // A colon-terminated line is ordinary technical writing, in both spacings:
    // content on the next line, and a blank line between the two.
    //
    // Pinning only the first was pinning the convenient half. With `"Testing:"`
    // in the boundary family, the implementation the suite demanded was
    //
    //     line.starts_with('#') || ((line.ends_with(':') || is_bold_only(line))
    //         && next_is_blank)
    //
    // which passes every test in the file and then reports a missing acceptance
    // bar for `Acceptance criteria:` above a list of bullets — one of the two
    // commonest shapes a done-when takes.
    //
    // The two demands cannot both be met, so the suite decides: `"Testing:"` is
    // out of `BOUNDARY_HEADERS`, a colon-terminated line is never a boundary,
    // and both spacings pass in both sections under both line endings. The veto
    // is in `BOUNDARY_HEADERS`' docs and in open_questions.
    let lead_in_bar = format!("Acceptance:\n{MULTILINE_BAR}");
    let lead_in_problem = format!("Today:\n{PROBLEM}");
    let spaced_lead_in_bar = format!("Acceptance criteria:\n\n{MULTILINE_BAR}");
    let spaced_lead_in_problem = format!("Today:\n\n{PROBLEM}");

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

        // THE MIRROR FIXTURES, and the reason this test was rejected: the same
        // lead-in with a BLANK LINE under it, which is the shape the boundary
        // rule above turns into a heading. Both sections, both line endings.
        expect_passed(
            &as_eol(&body_with(PROBLEM, &spaced_lead_in_bar), eol),
            &format!(
                "a bar introduced by \"Acceptance criteria:\" and a blank line above its \
                 bullets, {eol:?}"
            ),
        );
        expect_passed(
            &as_eol(&body_with(&spaced_lead_in_problem, BAR), eol),
            &format!(
                "a problem introduced by \"Today:\" and a blank line above its prose, \
                 {eol:?}"
            ),
        );
        expect_passed(
            &as_eol(
                &body_with(&spaced_lead_in_problem, &spaced_lead_in_bar),
                eol,
            ),
            &format!(
                "both sections introduced by a colon-terminated lead-in and a blank \
                 line, {eol:?}"
            ),
        );
    }

    // The mirror in the other direction, so this cannot be satisfied by reading
    // a colon-terminated line as content: a lead-in with nothing to lead in to
    // is still an empty section, whichever spacing follows it.
    expect_missing(
        &body_with(PROBLEM, "Acceptance:"),
        &[Artifact::DoneWhenBar],
        "a done-when section holding a lead-in line and nothing to lead in to",
    );
    expect_missing(
        &body_with(PROBLEM, "Acceptance criteria:\n\n"),
        &[Artifact::DoneWhenBar],
        "a done-when section holding a lead-in line, a blank line, and nothing else",
    );
}

#[test]
fn a_bold_only_lead_in_line_is_a_heading_and_ends_the_section_above_it() {
    // The other half of the boundary decision, pinned in the direction that
    // costs an author something — which is why it is a decision, not a detail.
    //
    // `**Done when**` and `**Problem**` are markers here: a bold-only line OPENS
    // a section. It follows that a bold-only line naming a different topic
    // CLOSES the one above it, which is why `BOUNDARY_HEADERS` keeps
    // `"**Testing**"`. The cost: a bold sub-label inside a done-when section
    // starts a new section, so the bar above it is empty. The veto is in
    // `BOUNDARY_HEADERS`' docs — drop that entry and flip these fixtures to
    // `expect_passed`, which re-opens the bold-third-section fail-open.
    //
    // THE SUB-LABELS MUST NOT BE SYNONYMS FOR EITHER ARTIFACT. An earlier
    // revision used `**Acceptance criteria**` above a real three-item bar and
    // `**Background**` above a real problem statement. Both bodies genuinely
    // state both artifacts, so the test contradicted this file's promise that no
    // test requires such a body to fail, and it quietly closed the marker
    // vocabulary the module docs leave open. `Testing`, `Rollout` and `Notes`
    // are synonyms for neither artifact, and what sits under them is testing
    // prose. `**Rollout**` and `**Notes**` are outside `BOUNDARY_HEADERS` on
    // purpose: the rule is that ANY bold-only line is a heading.
    assert_the_boundary_families_state_one_consistent_rule();

    for eol in BOTH_EOLS {
        for label in ["**Testing**", "**Rollout**"] {
            expect_missing(
                &as_eol(
                    &format!(
                        "## Problem\n\n{PROBLEM}\n\n## Done when\n\n{label}\n\n{THIRD_SECTION_BODY}\n"
                    ),
                    eol,
                ),
                &[Artifact::DoneWhenBar],
                &format!(
                    "the bold-only sub-label {label:?} under an otherwise empty done-when \
                     heading, {eol:?}"
                ),
            );
        }
        for label in ["**Testing**", "**Notes**"] {
            expect_missing(
                &as_eol(
                    &format!(
                        "## Problem\n\n{label}\n\n{THIRD_SECTION_BODY}\n\n## Done when\n\n{BAR}\n"
                    ),
                    eol,
                ),
                &[Artifact::WrittenProblem],
                &format!(
                    "the bold-only sub-label {label:?} under an otherwise empty problem \
                     heading, {eol:?}"
                ),
            );
        }
    }
}

/// The two boundary families have to state one rule between them.
///
/// Not a `#[test]`: nothing here touches the gate, so standing alone it would be
/// green from the moment it was written, and a test never observed failing
/// publishes assurance it has not earned. It runs first inside
/// `a_bold_only_lead_in_line_is_a_heading_and_ends_the_section_above_it`.
#[track_caller]
fn assert_the_boundary_families_state_one_consistent_rule() {
    // The boundary families are the one place here where a passing fixture and a
    // failing one are told apart by a rule rather than by their content, so the
    // relationship is asserted rather than left implicit. An edit that puts
    // `"Testing:"` back without flipping the colon lead-in fixtures re-creates
    // two demands no implementation can satisfy at once.
    for header in BOUNDARY_HEADERS {
        assert!(
            THIRD_SECTION_HEADERS.contains(header),
            "{header:?} must terminate a section AND be one of the third-section \
             headings the passing family runs, or the two families are testing \
             different things"
        );
    }
    assert!(
        !BOUNDARY_HEADERS.contains(&"Testing:"),
        "a colon-terminated line is pinned as ordinary writing by \
         a_short_colon_terminated_lead_in_line_is_writing_not_a_section_boundary, so \
         requiring one to terminate a section here demands two incompatible things \
         of the same rule. Flip that test's blank-line fixtures to expect_missing \
         before putting this entry back"
    );
    assert!(
        BOUNDARY_HEADERS.contains(&"**Testing**"),
        "a bold-only line is pinned as a heading by \
         a_bold_only_lead_in_line_is_a_heading_and_ends_the_section_above_it, and by \
         **Done when** and **Problem** being markers. Dropping it here without \
         flipping that test re-opens the bold-third-section fail-open"
    );
}

#[test]
fn the_marker_is_a_heading_line_not_a_phrase_anywhere_in_the_prose() {
    // With the words "problem" and "done when" appearing in no passing fixture
    // except on a marker line, the suite could not tell a marker predicate
    // anchored to a whole heading line apart from one built on `contains` — the
    // cheapest way to satisfy the marker cross-product:
    //
    //     fn is_done_when_marker(l: &str) -> bool {
    //         normalise_heading(l).contains("done when")
    //     }
    //
    // Both directions are pinned, because a `contains` predicate is wrong twice
    // over depending on which match it takes.
    //
    // The marker-bearing prose sentence is deliberately NOT the last line of
    // either body, and that detail is the whole test: when it was, the spurious
    // section a `contains` predicate opens after it was empty either way and the
    // wrong gate reached the right verdict by accident. A further line of
    // ordinary content under each prose sentence is what makes that section
    // non-empty. The two bodies and their invariant are built above the loop so
    // the invariant is exercised rather than stranded behind the measurement.
    let empty_bar_under_rollout_prose = format!(
        "## Problem\n\n{PROBLEM}\n\n## Done when\n\n\n\
         ## Rollout\n\nThe rollout is done when the canary reports two clean \
         windows in a row.\n\
         We will keep the old path behind a flag until then.\n"
    );
    let empty_problem_under_notes_prose = format!(
        "## Problem\n\n\n## Done when\n\n{BAR}\n\n\
         ## Notes\n\nThe problem was introduced by the cache change last \
         quarter, and this only reports it.\n\
         The queue has carried the same defect since the rollout path was \
         split in two.\n"
    );
    assert_the_marker_prose_is_followed_by_content(
        &empty_bar_under_rollout_prose,
        "is done when the canary",
    );
    assert_the_marker_prose_is_followed_by_content(
        &empty_problem_under_notes_prose,
        "The problem was introduced",
    );

    for eol in BOTH_EOLS {
        // FIRST MATCH WINS: an ordinary summary paragraph mentioning the
        // problem, above the real sections. The prose line becomes the marker,
        // its section runs to `## Problem` and is therefore empty, and the gate
        // reports a missing written problem on a change that wrote one.
        expect_passed(
            &as_eol(
                &format!(
                    "## Summary\n\nThis addresses a problem in the canary path.\n\n\
                     ## Problem\n\n{PROBLEM}\n\n## Done when\n\n{BAR}\n"
                ),
                eol,
            ),
            &format!(
                "a summary paragraph mentioning the problem above a real problem and a \
                 real bar, {eol:?}"
            ),
        );
        expect_passed(
            &as_eol(
                &format!(
                    "This addresses a problem in the canary path.\n\n\
                     ## Problem\n\n{PROBLEM}\n\n## Done when\n\n{BAR}\n"
                ),
                eol,
            ),
            &format!(
                "an unheaded opening paragraph mentioning the problem above a real \
                 problem and a real bar, {eol:?}"
            ),
        );

        // LAST MATCH WINS: the fail-open twin. A later section's prose contains
        // the words "done when" and the `## Done when` section above it is
        // empty. Reading that sentence as the bar certifies a template paste.
        expect_missing(
            &as_eol(&empty_bar_under_rollout_prose, eol),
            &[Artifact::DoneWhenBar],
            &format!(
                "an empty done-when section above a later paragraph whose prose \
                 contains the marker words, {eol:?}"
            ),
        );
        expect_missing(
            &as_eol(&empty_problem_under_notes_prose, eol),
            &[Artifact::WrittenProblem],
            &format!(
                "an empty problem section above a later paragraph whose prose contains \
                 the marker word, {eol:?}"
            ),
        );
    }
}

/// The fixture invariant that makes the last-match-wins pair load-bearing: the
/// marker-bearing sentence must exist in the body, and at least one line of
/// ordinary content must follow it before the body ends. Otherwise the spurious
/// section a `contains` predicate opens there is empty and the wrong gate
/// reaches the right verdict by accident.
#[track_caller]
fn assert_the_marker_prose_is_followed_by_content(body: &str, marker_prose: &str) {
    let lines: Vec<&str> = body.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.contains(marker_prose))
        .unwrap_or_else(|| {
            panic!(
                "fixture invariant: {marker_prose:?} must appear in the body, or this \
                 fixture pins nothing about a phrase-matching marker. body={body:?}"
            )
        });
    let followed_by_content = lines[at + 1..]
        .iter()
        .any(|l| !l.trim().is_empty() && !l.trim().starts_with('#'));
    assert!(
        followed_by_content,
        "fixture invariant: at least one line of ordinary content must follow the \
         marker-bearing prose {marker_prose:?}, or the spurious section a `contains` \
         predicate opens there is empty and the wrong gate reaches the right verdict \
         by accident. body={body:?}"
    );
}

/// Runs one section of real content through both positions and both-at-once.
///
/// The mirror of `assert_placeholder_fails_in_both_sections`, and used for the
/// same reason: a rule applied to the bar and not to the bet is half a gate.
#[track_caller]
fn assert_real_content_passes_in_both_sections(content: &str, family: &str) {
    expect_passed(
        &body_with(PROBLEM, content),
        &format!("{family}: the done-when section holds {content:?}"),
    );
    expect_passed(
        &body_with(content, BAR),
        &format!("{family}: the problem section holds {content:?}"),
    );
    expect_passed(
        &body_with(content, content),
        &format!("{family}: both sections hold {content:?}"),
    );
}

#[test]
fn real_content_that_merely_begins_with_a_deferral_stem_passes() {
    // The bound on the deferral vocabulary, and the reason `PLACEHOLDERS` can
    // demand that `"TBD - will fill this in before merge"` and `"TODO: write the
    // acceptance criteria here"` fail without costing an author their bar.
    //
    // Neither normalises to anything in a stem table and `derived_deferrals()`
    // forbids an enumeration, so the cheapest implementation satisfying both is
    // a prefix test on the normalised line — which passes every other fixture
    // here ("Today:" misses "todo" by one character) and then reports a missing
    // bar for `- Navigation completes in under 200ms` and a missing problem for
    // a section opening `Native TLS …`.
    //
    // The last fixture moves the same defect to token level. What separates it
    // from the two placeholders above is the separator after the token, not the
    // token itself.
    for content in REAL_CONTENT_WITH_A_DEFERRAL_PREFIX {
        let normalised = content.trim_start_matches(['-', '*', ' ']).to_lowercase();
        assert!(
            DEFERRAL_STEMS
                .iter()
                .any(|s| normalised.starts_with(&s.to_lowercase())),
            "fixture invariant: {content:?} must normalise to something that STARTS \
             WITH a deferral stem, or it stops separating a prefix rule from a token \
             rule and this whole family is decoration. Normalised: {normalised:?}"
        );
    }

    for content in REAL_CONTENT_WITH_A_DEFERRAL_PREFIX {
        assert_real_content_passes_in_both_sections(content, "real content, deferral prefix");
    }

    // And the mirror, for the same reason. Both are in `PLACEHOLDERS` and fail
    // there too; keeping them here keeps the bound on the vocabulary visible
    // beside the widening it bounds.
    for placeholder in [
        "TODO: write the acceptance criteria here",
        "TBD - will fill this in before merge",
    ] {
        assert_placeholder_fails_in_both_sections(
            placeholder,
            "a deferral announced by its separator",
        );
    }
}

#[test]
fn content_passes_however_few_characters_and_words_it_takes() {
    // The bound on "substantive" from below, and the mirror of
    // `a_bar_at_the_far_end_of_a_long_body_is_still_the_artifact`.
    //
    // With `SHORT_BAR` the shortest thing required to pass, nothing of one or
    // two words was required to pass anywhere, so `core.chars().count() >= 9`
    // appended to an otherwise-correct content predicate passed every
    // behavioural test in this file — and so did "the line must contain a space"
    // and "the line must have at least three tokens". A floor is the natural
    // thing to reach for once the placeholder family is in front of you, and it
    // rejects the terse bars the best-run teams write.
    //
    // The invariants first, so the family cannot quietly stop being short.
    let longest_failing = PLACEHOLDERS
        .iter()
        .chain(PHRASE_DEFERRALS.iter())
        .copied()
        .max_by_key(|p| p.chars().count())
        .expect("fixture invariant: the must-fail vocabulary must not be empty");
    for content in SHORT_REAL_CONTENT {
        assert!(
            content.chars().count() < longest_failing.chars().count()
                && content.split_whitespace().count() < longest_failing.split_whitespace().count(),
            "fixture invariant: {content:?} ({} chars, {} words) must be shorter than \
             the longest string this file requires to FAIL, {longest_failing:?} ({} \
             chars, {} words), in characters AND in words — otherwise a length floor \
             bolted on top of a content check still separates the two sets and \
             \"the measurement is the content, not the length\" is asserted in prose \
             only",
            content.chars().count(),
            content.split_whitespace().count(),
            longest_failing.chars().count(),
            longest_failing.split_whitespace().count(),
        );
    }

    let shortest_passing = SHORT_REAL_CONTENT
        .iter()
        .map(|c| c.chars().count())
        .min()
        .unwrap_or(0);
    let shortest_failing = PLACEHOLDERS
        .iter()
        .map(|p| p.trim().chars().count())
        .min()
        .unwrap_or(0);
    assert!(
        shortest_failing < shortest_passing,
        "fixture invariant: something that must FAIL has to be shorter than everything \
         that must pass ({shortest_failing} vs {shortest_passing} chars), or the two \
         sets are separable by length from below as well and this family only pins the \
         ceiling"
    );

    // One of these has no space in it at all; one is five characters of Hangul
    // in thirteen bytes. A floor in characters, in bytes, in words or in "does
    // it contain a space" rejects at least one of them.
    for content in SHORT_REAL_CONTENT {
        assert_real_content_passes_in_both_sections(content, "content written tersely");
    }

    // And the mirror, in the same length band. These four are pinned as failing
    // by the enumerated and derived families too, so this loop adds no
    // discrimination to the suite — it keeps the failing side of THIS test's
    // rule inside this test, where a later edit to the passing family above can
    // see it.
    for placeholder in ["TBD", "n/a", "...", "- [ ] "] {
        assert_placeholder_fails_in_both_sections(
            placeholder,
            "a deferral as short as the content beside it",
        );
    }
}

#[test]
fn the_bet_and_the_bar_are_written_on_the_change_not_left_to_its_title() {
    // A conventional-commit subject says what changed. It never says what done
    // looks like, and it is a label for a problem statement rather than one.
    // `judge` takes the body alone, so the strongest form of this pin is the one
    // the compiler enforces: there is no parameter through which TITLE could
    // reach the gate. What is left to assert is the product half — a change
    // whose bet exists only in its title is still missing the written problem.
    // The empty-body case is pinned by
    // a_change_with_no_bar_at_all_is_failed_not_merely_unmeasured.
    expect_missing(
        &bar_only(BAR),
        &[Artifact::WrittenProblem],
        &format!("a real bar, no written problem, under the title {TITLE:?}"),
    );
}

#[test]
fn three_shapes_of_absence_produce_three_distinct_messages() {
    // That the three shapes of absence do not share one message. One constant
    // string satisfies every positive containment assertion in this file and
    // would tell an author that a gate failed and nothing else.
    //
    // What this deliberately does NOT check is that each message names ONLY the
    // missing artifact: there is no non-brittle way to assert that at the level
    // of prose, and a raw vocabulary ban turns a correct, helpful implementation
    // red for quoting the offending section. That property is enforced one level
    // down, where it is mechanically checkable — on the measurement by
    // `expect_missing`, and on the message minus the body by
    // `assert_failed_naming`'s residue rule.
    //
    // Distinctness is asserted on the RESIDUE: three different bodies make three
    // different messages for a gate that echoes the body, and the echo would do
    // all the distinguishing.
    assert_the_content_fixtures_carry_none_of_the_message_vocabulary();

    let bodies = ["".to_string(), problem_only(PROBLEM), bar_only(BAR)];
    let both = expect_missing(
        &bodies[0],
        &[Artifact::WrittenProblem, Artifact::DoneWhenBar],
        "nothing written at all",
    );
    let no_bar = expect_missing(
        &bodies[1],
        &[Artifact::DoneWhenBar],
        "problem written, bar missing",
    );
    let no_problem = expect_missing(
        &bodies[2],
        &[Artifact::WrittenProblem],
        "bar written, problem missing",
    );

    let both_said = message_residue(&both, &bodies[0]);
    let no_bar_said = message_residue(&no_bar, &bodies[1]);
    let no_problem_said = message_residue(&no_problem, &bodies[2]);

    assert_ne!(
        both_said, no_bar_said,
        "three different absences cannot share one message; an author has to be \
         able to tell from it which artifact to go and write. These two differ only \
         by the body quoted back at them: {both:?} vs {no_bar:?}"
    );
    assert_ne!(
        both_said, no_problem_said,
        "same, for the missing problem statement: {both:?} vs {no_problem:?}"
    );
    assert_ne!(
        no_bar_said, no_problem_said,
        "the missing-bar message and the missing-problem message must differ in what \
         the GATE says, not merely in the section it quoted: {no_bar:?} vs \
         {no_problem:?}"
    );
}

#[test]
fn the_message_holds_its_ground_however_the_surviving_section_is_headed() {
    // THE HOLE THE RESIDUE RULE DID NOT CLOSE BY ITSELF.
    //
    // `assert_failed_naming` subtracts the body's own lines before asking
    // whether the gate named an artifact the author did write. But every
    // `expect_missing` with one artifact PRESENT used to write that section
    // under the byte-identical `## Problem` or `## Done when`, so a constant
    // spelling the two artifacts with those two literals was subtracted out of
    // the residue for exactly the bodies where the negative rule bites:
    //
    //     GateStatus::Failed(
    //         "This change does not carry the Product artifact. Add a `## Problem` \
    //          section stating the bet and a `## Done when` section stating the \
    //          acceptance bar.".to_string())
    //
    // The positive holds, the negative holds because the surviving heading is in
    // the body, and the three "distinct" residues differ only by which heading
    // got subtracted. The author whose problem statement is present is still
    // told to write one.
    //
    // Varying the surviving section's spelling closes it: no constant embeds
    // thirteen heading spellings. The cross-product below covers both
    // one-artifact-missing families, including the `## Problem` and
    // `## Done when` spellings themselves.
    assert_the_content_fixtures_carry_none_of_the_message_vocabulary();

    // And the same for the two heading depths crossed, so the surviving heading
    // is never the one the missing heading is spelled with either — a constant
    // that embedded `### Problem` instead would otherwise be subtracted by the
    // loops above.
    for problem_marker in PROBLEM_MARKERS {
        for done_when_marker in DONE_WHEN_MARKERS {
            expect_missing(
                &format!("{problem_marker}\n\n{PROBLEM}\n\n{done_when_marker}\n\nTBD\n"),
                &[Artifact::DoneWhenBar],
                &format!(
                    "a written problem headed {problem_marker:?} above a done-when headed \
                     {done_when_marker:?} that defers"
                ),
            );
            expect_missing(
                &format!("{problem_marker}\n\nTBD\n\n{done_when_marker}\n\n{BAR}\n"),
                &[Artifact::WrittenProblem],
                &format!(
                    "a real bar headed {done_when_marker:?} below a problem headed \
                     {problem_marker:?} that defers"
                ),
            );
        }
    }
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
    // `evaluate_pre_merge_gates` and takes the whole review with it. The obvious
    // way to write one is to quote an excerpt back at the author —
    // `&pr_body[..40]` — which is a byte index landing inside a character in
    // several of the bodies below. The other shapes are a marker order the
    // extractor did not expect and a marker with nothing after it.
    //
    // The assertion is `Failed`, not merely "returned": none of these bodies
    // carries both artifacts.
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
    // GitHub's web UI submits textarea content with CRLF and nothing between the
    // webhook payload and the guard layer normalises it. A gate anchored on
    // `"## Done when\n"` fails essentially every human-authored pull request
    // while a suite built only from `\n` stays green.
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
        // The three cases above are all `##`-headed with no third section, the
        // one shape where CRLF is harmless. The two below are the shapes that
        // separate a gate that handles CRLF from one that does not.
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
    // A verdict that depends on anything but the string it was handed is a flake
    // nothing here could attribute, and product_bar.rs's own doc comment promises
    // no network or filesystem call. A gate loading its deferral vocabulary from
    // a config file would satisfy every behavioural assertion above.
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
    // its own, against a `todo!()` module, it would be a test born green.
    //
    // It asserts the SHAPE of the import list through `impure_import` — a
    // denylist of effects, not a whitelist of spellings. A whitelist of eight
    // literal prefixes let `use std::{env, fs};` walk straight past (a grouped
    // import contains neither "std::env" nor "std::fs") while turning correct
    // implementations red over `regex`, the `unicode_*` crates and `std::mem`.
    //
    // The rule is only as good as the parser feeding it, so the parser is
    // exercised first — including the grouped form that used to be banned
    // outright and the `pub use` form that used to walk past.
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

    // And the effect rule itself, exercised on both sides before it is trusted.
    // The pure half of this table is not decoration: every one of these is an
    // import an ordinary line-splitting, string-trimming implementation reaches
    // for, and the whitelist this replaces rejected nine of them.
    for (path, impure) in [
        ("std::mem", false),
        ("std::char", false),
        ("std::ops::Range", false),
        ("std::convert::identity", false),
        ("std::num::NonZeroUsize", false),
        ("std::slice", false),
        ("std::hash::Hash", false),
        ("std::vec::Vec", false),
        ("std::iter::once", false),
        ("std::sync::Arc", false),
        ("std::sync::LazyLock", false),
        ("std::collections::BTreeMap", false),
        ("std::borrow::Cow", false),
        ("std::cmp::Reverse", false),
        ("std::fmt::Write", false),
        ("std::str::FromStr", false),
        ("core::fmt", false),
        ("alloc::string::String", false),
        ("super::GateStatus", false),
        ("crate::pre_merge_guard::GateStatus", false),
        ("self::inner", false),
        ("regex::Regex", false),
        ("unicode_segmentation::UnicodeSegmentation", false),
        ("std::fs", true),
        ("std::fs::File", true),
        ("std::env", true),
        ("std::env::var", true),
        ("std::io::Read", true),
        ("std::net::TcpStream", true),
        ("std::os::unix::fs::PermissionsExt", true),
        ("std::process::Command", true),
        ("std::time::SystemTime", true),
        ("std::thread", true),
        ("std::sync::mpsc::channel", true),
        ("reqwest::Client", true),
        ("tokio::fs", true),
        ("chrono::Utc", true),
        ("rand::random", true),
    ] {
        assert_eq!(
            impure_import(path).is_some(),
            impure,
            "impure_import({path:?}) misjudged the import. A determinism rule that \
             calls a pure module impure accuses a correct implementation and forces a \
             settled specification test to be edited mid-implementation; one that calls \
             an impure module pure lets a second source of truth for what the author \
             wrote in through the front door"
        );
    }

    let src = without_line_comments(&source("src/pre_merge_guard/product_bar.rs"));

    for line in src.lines() {
        let trimmed = line.trim();
        for path in imported_paths(trimmed) {
            if let Some(reason) = impure_import(&path) {
                panic!(
                    "src/pre_merge_guard/product_bar.rs imports {trimmed:?}, which \
                     reaches {path:?} — {reason}. The Product artifact is authored on \
                     the change under review and nowhere else: a gate that reads a \
                     file, an environment variable, a clock or the network is both a \
                     flake this suite could not attribute and a second source of truth \
                     for what the author wrote"
                );
            }
        }
    }

    // The things reachable without a `use` line at all.
    //
    // Swept over source with the string literals blanked out: `"Command"`,
    // `"Instant"` and `"::var("` are all things an author-facing failure message
    // could legitimately contain, and failing the gate for its prose would be a
    // guard misreading what it guards. The parser is exercised first.
    for (line, want) in [
        (
            r#"let msg = "Command"; std::process::Command::new("git")"#,
            r#"let msg = "       "; std::process::Command::new("   ")"#,
        ),
        (
            r#"let m = "a \" Instant"; let t = Instant::now();"#,
            r#"let m = "            "; let t = Instant::now();"#,
        ),
        ("let x = 1;", "let x = 1;"),
    ] {
        assert_eq!(
            without_string_literals(line),
            want,
            "without_string_literals({line:?}) blanked the wrong span; a sweep fed by \
             a parser that cannot tell a message from a syscall either bans a correct \
             implementation for its prose or lets the filesystem through"
        );
    }

    // The things reachable with a `use` that `impure_import` admits — `use std;`
    // plus a full path, or `use std::sync;` then `sync::mpsc`. `::io::` and
    // `::net::` are spelled with both separators so an ordinary identifier
    // ending in those letters (`Ratio::new`) is not mistaken for a syscall.
    let src = without_string_literals(&src);
    for forbidden in [
        "env!",
        "option_env!",
        "include_str!",
        "include_bytes!",
        "File::open",
        "fs::",
        "env::",
        "process::",
        "thread::",
        "mpsc",
        "::io::",
        "::net::",
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
    // copy-paste pairing the new name with a neighbouring field:
    // `("product_bar_status", &self.test_suite_status)`. That passes the name
    // check above and both alignment tests in report.rs, and makes the scorecard
    // report someone else's measurement under the Product seat's name.
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
// `evaluate_pre_merge_gates` takes roughly fifty guard reports, so nothing in
// the suite above can tell a perfect `product_bar::judge` apart from
// `let product_bar_status = GateStatus::Passed`. The existing backstop only
// checks that a computed status is carried, so a literal satisfies it — a named
// gate rendered in the scorecard, counted in TOTAL_GATES, and blocking nothing.
//
// This repository already sanctions source inspection for exactly this
// invariant class (tests/evaluator_gate_ordering_test.rs,
// every_computed_gate_reaches_the_report_test.rs), so these follow that idiom
// over comment-stripped source.
//
// These are guards over FACTS, not over formatting, and each is anchored on the
// loosest spelling that still carries the fact: the evaluator receives the
// body, either as a parameter whose name says body or as a body field on
// `PrDiffContext`; `product_bar_status` is derived from a `judge` call,
// qualified or imported, directly or through a `let`; and the pipeline hands
// that call site the body. A wiring guard that misreads a correct wiring and
// reports it as the defect it does not have is worse than no guard, so
// `assert_the_wiring_parsers_read_a_real_wiring` exercises every parser below
// against the spellings a wiring is actually written in.

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

/// `src` with the contents of every double-quoted string literal blanked out.
///
/// Used only for the forbidden-substring sweep in
/// `the_verdict_depends_on_nothing_but_the_change_it_was_handed`, which bans
/// `"Command"`, `"Instant"` and `"::var("` as reaches for I/O. The gate's own
/// author-facing prose is not a syscall, and code cannot hide inside a string
/// literal, so blanking them costs the sweep nothing.
fn without_string_literals(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut in_string = false;
    let mut escaped = false;
    for c in src.chars() {
        if !in_string {
            out.push(c);
            in_string = c == '"';
            continue;
        }
        if escaped {
            escaped = false;
            out.push(' ');
            continue;
        }
        match c {
            '\\' => {
                escaped = true;
                out.push(' ');
            }
            '"' => {
                in_string = false;
                out.push('"');
            }
            '\n' => out.push('\n'),
            _ => out.push(' '),
        }
    }
    out
}

/// Every crate path a `use` line reaches, with a grouped import expanded into
/// its members: `use std::{cmp, fmt};` yields `std::cmp` and `std::fmt`.
/// Returns empty for a line that is not an import.
///
/// Expansion rather than a ban on braces: a grouped import reaches exactly the
/// modules a sequence of single imports would, and `pub use` re-exports exactly
/// as far as `use`.
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

/// Why importing `path` would make the verdict depend on something other than
/// the change the gate was handed — or `None` if it cannot.
///
/// # Why this is a denylist and not an allowlist
///
/// Three revisions stated the determinism rule as a whitelist of import
/// prefixes, and three times review found a correct implementation turned red by
/// it for a spelling with no effect behind it — `regex`, the `unicode_*` crates,
/// then `std::mem`, `std::char`, `std::ops`, `std::convert`, `std::num`,
/// `std::slice`, `std::hash`, `std::sync::Arc` and `std::vec`. The list forbade
/// a spelling rather than an effect, and each round told the implementer to edit
/// a settled specification test mid-implementation.
///
/// What is checked instead is the stated property: a file, an environment
/// variable, a socket, a clock, another thread or another process. Those live in
/// a short, stable set of `std` subtrees; everything else in `std`, `core` and
/// `alloc` is a pure data structure or a pure text facility. A third-party crate
/// can do anything, so those are admitted by name from a small pure-text list.
///
/// `crate::`, `super::` and `self::` are admitted — the forbidden-token sweep
/// catches a gate reaching for I/O through one of them. That seam is listed in
/// open_questions.
fn impure_import(path: &str) -> Option<&'static str> {
    /// The `std` subtrees that reach outside the process, the machine or the
    /// moment. Matched on `::` segment boundaries, so `std::iter` is not
    /// `std::io` and `std::sync::Arc` is not `std::sync::mpsc`.
    const IMPURE_SUBTREES: &[&str] = &[
        "std::env",
        "std::fs",
        "std::io",
        "std::net",
        "std::os",
        "std::process",
        "std::sync::mpsc",
        "std::thread",
        "std::time",
    ];

    /// Third-party crates that read the string they are handed and nothing else.
    /// `regex` is a first-class dependency and the house idiom for this kind of
    /// marker parsing; the rest are the pure-text crates this suite's
    /// invisible-character demands point an implementer at.
    const PURE_CRATES: &[&str] = &[
        "aho_corasick",
        "itertools",
        "lazy_static",
        "memchr",
        "once_cell",
        "regex",
        "regex_lite",
        "unicode_normalization",
        "unicode_segmentation",
        "unicode_width",
    ];

    let path = path.trim();
    for subtree in IMPURE_SUBTREES {
        if path == *subtree || path.starts_with(&format!("{subtree}::")) {
            return Some(
                "a std subtree that reaches outside the process, the machine or the \
                 moment — a file, an environment variable, a socket, a clock, another \
                 thread or another process",
            );
        }
    }

    let first = path.split("::").next().unwrap_or(path).trim();
    match first {
        "std" | "core" | "alloc" | "crate" | "self" | "super" => None,
        _ if PURE_CRATES.contains(&first) => None,
        _ => Some(
            "a third-party crate outside the small list of pure-text crates, which can \
             do anything at all",
        ),
    }
}

/// One line, with any `//` comment on it dropped.
///
/// `//` inside a string literal is not a comment: truncating at the first `//`
/// anywhere on the line would let a single URL in a message string silently
/// blind every scan below.
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

/// The identifier a `let` introduces immediately before `head` ends, if the text
/// between the `=` and the end of `head` is still the same statement.
///
/// A type annotation is stripped before the identifier is validated; without
/// that, `let product_bar_status: GateStatus = product_bar::judge(pr_body);` — a
/// correct wiring — yielded no binding at all.
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

/// Every call to product_bar's `judge` in `src`, as (binding, arguments), where
/// `binding` is `Some(name)` when the call initialises `let name = `.
///
/// Reaching `judge` through an import is as correct as qualifying it, so a bare
/// `judge(` counts — but only when `src` imports it, so an unrelated gate's
/// `judge` cannot turn these tests red.
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
/// `PrDiffContext` IS the change-under-review struct and the evaluator already
/// receives it, so adding `pub pr_body: String` to it and judging
/// `&diff_ctx.pr_body` is a correct, complete and arguably cleaner wiring than a
/// sixty-ninth parameter. The fact these guards pin is that the body reaches the
/// gate; which route carries it is the implementer's to choose, so both are
/// read.
fn diff_context_body_fields() -> Vec<String> {
    let src = without_line_comments(&source("src/git_manager/diff_context.rs"));
    struct_fields(&src, "PrDiffContext")
        .into_iter()
        .filter(|f| f.contains("body"))
        .collect()
}

/// The `let` statement that binds `name` in `src`, from the `let` to the `;`
/// that closes it. Used to ask what a call-site argument was built from: if the
/// body rides on the change-under-review struct, the pipeline's obligation is
/// that the statement producing that struct was handed the body.
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

/// Every line of `src` that assigns to `name` WITHOUT introducing it.
///
/// `binding_statement` stops at the semicolon that closes the `let` — its own
/// parser test requires that, or every binding would look as though it were
/// built from every later value — so a fail-open written as a later
/// REASSIGNMENT is invisible to `unmeasured_alternatives_in`:
///
///     let mut product_bar_status = product_bar::judge(&diff_ctx.pr_body);
///     if diff_ctx.pr_body.trim().is_empty() {
///         product_bar_status = GateStatus::NotMeasured { .. };
///     }
///
/// That binds the right name, calls `judge` over the body, leaves nothing behind
/// when the call is cut out, and hands the struct the shorthand — so every
/// wiring assertion passes while every pull request with an empty body certifies
/// the Product seat as `NotMeasured`.
///
/// The `let` line itself is not an assignment by this reading, and comparisons
/// are excluded, because a guard that reports a correct wiring as the defect it
/// does not have is worse than no guard.
fn assignments_to(src: &str, name: &str) -> Vec<String> {
    src.lines()
        .filter(|line| {
            let Some((lhs, rhs)) = line.split_once('=') else {
                return false;
            };
            // `==` — a comparison, not a write.
            if rhs.starts_with('=') {
                return false;
            }
            // `!=`, `<=`, `>=`, `+=`-style compounds: the operator's first
            // character sits at the end of what `split_once` called the LHS.
            if lhs.ends_with(['!', '<', '>', '+', '-', '*', '/', '%', '&', '|', '^']) {
                return false;
            }
            lhs.trim() == name
        })
        .map(|l| l.trim().to_string())
        .collect()
}

/// `text` with every product_bar `judge(..)` call expression cut out of it: what
/// the statement does BESIDES measuring. For a correct wiring the residue is
/// `let product_bar_status = ;` and holds nothing.
fn without_judge_calls(src: &str, text: &str) -> String {
    let mut anchors: Vec<&str> = vec!["product_bar::judge("];
    if imports_product_bar_judge(src) {
        anchors.push("judge(");
    }

    let mut out = text.to_string();
    loop {
        let Some((start, anchor)) = anchors
            .iter()
            .filter_map(|a| out.find(*a).map(|i| (i, *a)))
            .min_by_key(|(i, _)| *i)
        else {
            return out;
        };
        let open = start + anchor.len() - 1;
        let mut depth = 0i32;
        let mut end = None;
        for (k, c) in out[open..].char_indices() {
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
        let cut_to = end.unwrap_or(out.len() - 1);
        out.replace_range(start..=cut_to, " ");
    }
}

/// The ways a statement can hold a value in reserve beside the measurement.
///
/// A conditional is the shape an engineer actually writes:
///
///     let product_bar_status = if pr_body.trim().is_empty() {
///         GateStatus::NotMeasured { .. }
///     } else {
///         product_bar::judge(pr_body)
///     };
///
/// `judge` stays perfectly correct so every behavioural test is green, the
/// binding and the arguments and the struct initialiser all check out — and
/// every pull request opened with an empty body certifies the Product seat as
/// `NotMeasured`, which `is_acceptable()` returns true for. That is
/// `a_change_with_no_bar_at_all_is_failed_not_merely_unmeasured` reinstated at
/// the one seam no behavioural test can reach.
const UNMEASURED_ALTERNATIVES: &[&str] = &["GateStatus::", "if ", "match ", "unwrap_or"];

/// Whichever of `UNMEASURED_ALTERNATIVES` survive in `statement` once every
/// product_bar `judge(..)` call has been cut out of it.
fn unmeasured_alternatives_in(src: &str, statement: &str) -> Vec<&'static str> {
    let residue = without_judge_calls(src, statement);
    UNMEASURED_ALTERNATIVES
        .iter()
        .copied()
        .filter(|marker| residue.contains(marker))
        .collect()
}

/// Whether the `let` that binds `ident` in `src` BUILDS the change-under-review
/// struct and was handed the change's body.
///
/// Asking whether ANY argument had a `let` mentioning "body" was satisfied by
/// two bindings that have always existed at the real call site — `&doc_report`
/// and `&review_resp.verdict` — neither of which hands the body to the gate. The
/// moment a body field appeared on `PrDiffContext`, both wiring guards went
/// green with no change to the pipeline, `pr_body` stayed `String::new()` and
/// the gate read `""` for every pull request. A guard satisfied by bindings that
/// predate the change it guards is not a guard.
///
/// So the question is asked of the statement that produces the struct and of
/// nothing else, in the two spellings that really carry the body: passing it to
/// the constructor, and assigning it onto the struct afterwards.
fn diff_context_carries_the_body(src: &str, ident: &str, body_fields: &[String]) -> bool {
    let Some(statement) = binding_statement(src, ident) else {
        return false;
    };
    if !(statement.contains("prepare_pr_diff") || statement.contains("PrDiffContext {")) {
        return false;
    }
    if statement.contains("body") {
        return true;
    }

    body_fields.iter().any(|field| {
        let target = format!("{ident}.{field}");
        src.lines().any(|line| {
            let Some((lhs, rhs)) = line.split_once('=') else {
                return false;
            };
            lhs.trim() == target && rhs.contains("body")
        })
    })
}

/// Whether one of `args` is a change-under-review struct that was built from
/// the change's body.
fn an_argument_carries_the_change_body(src: &str, args: &[String], body_fields: &[String]) -> bool {
    !body_fields.is_empty()
        && args.iter().any(|a| {
            let ident = leading_ident(a.trim_start_matches('&').trim_start_matches("mut ").trim());
            diff_context_carries_the_body(src, &ident, body_fields)
        })
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
    // Two routes carry that fact and either is accepted: a parameter whose name
    // says body, or a body field on `PrDiffContext` — the change-under-review
    // struct this signature already takes.
    //
    // Route two is that the field EXISTS AND IS READ. Declaring
    // `pub pr_body: String` on its own is the "named thing, no measurement"
    // pattern this seat exists to catch. Read with
    // `the_review_pipeline_hands_the_evaluator_the_change_body` and the
    // judge-call check, the chain is unbroken: body -> diff context ->
    // evaluator -> judge.
    let named_parameter = signature.contains("body");
    let body_fields = diff_context_body_fields();
    let reads_the_body_field = body_fields.iter().any(|f| src.contains(&format!(".{f}")));
    let on_the_change_under_review = signature.contains("PrDiffContext") && reads_the_body_field;

    assert!(
        named_parameter || on_the_change_under_review,
        "evaluate_pre_merge_gates never receives the change's body, which is where \
         the written problem and the done-when bar are authored. Hand it a parameter \
         whose name says body, or put the body on PrDiffContext — the \
         change-under-review struct this signature already takes — AND READ IT \
         HERE; a field the evaluator never touches gates as much as no field at \
         all. PrDiffContext body fields: {body_fields:?} (read by this file: \
         {reads_the_body_field}). Signature: {signature:?}"
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
    // on two lines, and by the realistic copy-paste
    // `product_bar_status: doc_parity_status.clone(),`.
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

        // AND THE VALUE IS THE MEASUREMENT, UNCONDITIONALLY. Everything above
        // is satisfied by a conditional whose other branch fails open —
        //
        //     let product_bar_status = if pr_body.trim().is_empty() {
        //         GateStatus::NotMeasured { .. }
        //     } else {
        //         product_bar::judge(pr_body)
        //     };
        //
        // — which binds the right name, calls judge over the body, and hands the
        // struct the shorthand while certifying every change whose author wrote
        // nothing. An empty body is precisely the change with no bar. So the
        // statement is asked what is left of it once the measurement is removed:
        // for a correct wiring, nothing.
        let statement = if is_a_product_bar_judge_call(&src, init) {
            init.clone()
        } else {
            binding_statement(&src, &leading_ident(init)).unwrap_or_else(|| init.clone())
        };
        let smuggled = unmeasured_alternatives_in(&src, &statement);
        assert!(
            smuggled.is_empty(),
            "product_bar_status is not simply the measurement: {smuggled:?} survive in \
             {statement:?} once the product_bar::judge call is cut out of it. The \
             field's value must be the judge call and nothing else. A branch that \
             answers NotMeasured for a body the gate found nothing in certifies every \
             change whose author wrote nothing, which is the one case this seat is \
             for — absence of the bar IS the defect, not an unread measurement. If \
             the body can legitimately be absent, say so in `judge`, where \
             a_change_with_no_bar_at_all_is_failed_not_merely_unmeasured can see it"
        );

        // AND NOTHING LATER OVERWRITES IT. The check above reads the `let` and
        // stops at its semicolon, so the same fail-open written as a
        // reassignment one line down walks straight past it:
        //
        //     let mut product_bar_status = product_bar::judge(&diff_ctx.pr_body);
        //     if diff_ctx.pr_body.trim().is_empty() {
        //         product_bar_status = GateStatus::NotMeasured { .. };
        //     }
        if !is_a_product_bar_judge_call(&src, init) {
            let bound = leading_ident(init);
            let overwrites = assignments_to(&src, &bound);
            assert!(
                overwrites.is_empty(),
                "product_bar_status is measured and then written over: {overwrites:?} \
                 assign to {bound:?} after the let that binds it to the judge call. The \
                 field's value must be the measurement, full stop — a later branch that \
                 substitutes NotMeasured (or Passed, or Warning) for a body the gate \
                 found nothing in certifies every change whose author wrote nothing, \
                 which is the one case this seat exists for. If the body can \
                 legitimately be absent, say so in `judge`, where \
                 a_change_with_no_bar_at_all_is_failed_not_merely_unmeasured can see it"
            );
        }
    }
}

#[test]
fn the_review_pipeline_hands_the_evaluator_the_change_body() {
    // The evaluator can only judge what the pipeline gives it. `body` is already
    // in scope at this call site, so the only thing that can go wrong is
    // forgetting to pass it — and then the Product gate is a name on the
    // scorecard with nothing behind it.
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

    // Same two routes as the test above. Either the pipeline hands the evaluator
    // an argument naming the body, or the body rides on the change-under-review
    // struct — in which case the obligation is that the statement building THAT
    // STRUCT was handed the body, since a `PrDiffContext` with an empty
    // `pr_body` gates exactly as much as no body at all.
    //
    // "That struct", not "some argument": asking whether any of the sixty-eight
    // arguments had a `let` mentioning "body" was satisfied by `&doc_report` and
    // `&review_resp.verdict`, so the instant a body field was declared both
    // guards went green with zero change to the pipeline.
    let handed_directly = args.iter().any(|a| a.contains("body"));

    let body_fields = diff_context_body_fields();
    let built_from_the_body = an_argument_carries_the_change_body(&src, &args, &body_fields);

    assert!(
        handed_directly || built_from_the_body,
        "the review pipeline never hands the evaluator the pull request body, which \
         is where the written problem and the done-when bar are authored. Pass it as \
         an argument that names the body, or hand the body to the statement that \
         builds the PrDiffContext (or assign it onto that struct) and pass that \
         struct. Declaring a body field is not enough on its own: an unpopulated \
         field reads as the empty string for every pull request, and a gate that \
         reads nothing gates nothing. Arguments: {args:?}. PrDiffContext body \
         fields: {body_fields:?}"
    );
}

/// Exercises the hand-rolled parsers the wiring guards depend on.
///
/// Not a `#[test]`, deliberately: nothing here touches the gate, so standing
/// alone it would be green from the moment it was written. It runs first inside
/// the wiring tests instead, which are red on an absent wiring.
#[track_caller]
fn assert_the_wiring_parsers_read_a_real_wiring() {
    // The wiring guards are the only thing standing between a named Product gate
    // and one that blocks nothing, and they are hand-rolled parsers. A parser
    // that misreads a correct implementation and reports it as the exact defect
    // it does not have is worse than no guard — this file's own standard,
    // applied to this file.
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

    // And it must be pointed at the real struct. `diff_context_body_fields()`
    // returning nothing is how the guards conclude "the body does not ride on
    // the change-under-review struct", so a moved file or a renamed struct would
    // quietly turn that branch into a permanent no with no evidence behind it.
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

    // ROUTE TWO, ON THE ARRANGEMENT THAT DEFEATED THE PREVIOUS REVISION. This
    // synthetic pipeline is the real one in miniature: a body-carrying
    // `review_pr(&diff_ctx, title, body)`, a body-carrying `doc_report`, and a
    // `prepare_pr_diff` that never sees the body. The answer has to be "no".
    let body_field = vec!["pr_body".to_string()];
    let unwired = "    let diff_ctx = state\n\
                    \x20       .git_mgr\n\
                    \x20       .prepare_pr_diff(repo, pr_number, base_sha, head_sha)\n\
                    \x20       .await?;\n\
                    \x20   let review_resp = state.reviewer.review_pr(&diff_ctx, title, body).await?;\n\
                    \x20   let doc_report = state.doc_guard.parity(repo, &diff_ctx, title, body).await?;\n";
    let unwired_args = [
        "&diff_ctx".to_string(),
        "&doc_report".to_string(),
        "&review_resp.verdict".to_string(),
    ];
    assert!(
        !an_argument_carries_the_change_body(unwired, &unwired_args, &body_field),
        "route two answered yes for a pipeline whose diff context never received the \
         body. `&doc_report` and `&review_resp.verdict` are bound by statements that \
         mention the body and have nothing to do with the gate; a guard they satisfy \
         is a guard that predates the change it guards, and it would let a \
         permanently empty pr_body certify every pull request"
    );
    assert!(
        !an_argument_carries_the_change_body(unwired, &unwired_args, &[]),
        "with no body field on PrDiffContext at all, route two cannot be open"
    );

    // And the two spellings that really do carry the body, so the fix is not so
    // narrow that it forces the parameter route and decides the implementer's
    // surface for them — which is the mistake the previous round removed.
    let wired_through_the_constructor = unwired.replace(
        "prepare_pr_diff(repo, pr_number, base_sha, head_sha)",
        "prepare_pr_diff(repo, pr_number, base_sha, head_sha, body)",
    );
    assert!(
        an_argument_carries_the_change_body(
            &wired_through_the_constructor,
            &unwired_args,
            &body_field
        ),
        "a diff context built by a call that was handed the body satisfies route two"
    );

    let wired_by_assignment = format!("{unwired}    diff_ctx.pr_body = body.to_string();\n");
    assert!(
        an_argument_carries_the_change_body(&wired_by_assignment, &unwired_args, &body_field),
        "assigning the body onto the diff context after building it carries the body \
         just as far, and must satisfy route two too"
    );
    assert!(
        !an_argument_carries_the_change_body(
            &format!("{unwired}    diff_ctx.pr_body = String::new();\n"),
            &unwired_args,
            &body_field
        ),
        "an assignment that puts something OTHER than the body on the field must not \
         satisfy route two; that is the permanently-empty field this route exists to \
         forbid"
    );

    // The unconditional-initialiser reader. A conditional that keeps a literal
    // status in reserve is the shape an engineer writes, and it is invisible to
    // every other check in these guards.
    let straight = "let product_bar_status = product_bar::judge(pr_body);\n";
    assert_eq!(
        unmeasured_alternatives_in(
            straight,
            &binding_statement(straight, "product_bar_status").expect("bound by a let")
        ),
        Vec::<&str>::new(),
        "a plain judge call must leave nothing behind when the call is cut out of it"
    );

    let annotated = "let product_bar_status: GateStatus = product_bar::judge(&diff_ctx.pr_body);\n";
    assert_eq!(
        unmeasured_alternatives_in(
            annotated,
            &binding_statement(annotated, "product_bar_status").expect("bound by a let")
        ),
        Vec::<&str>::new(),
        "a type annotation is not a value held in reserve, and rejecting it would fail \
         a correct wiring for its spelling"
    );

    let conditional = "let product_bar_status = if pr_body.trim().is_empty() {\n\
                       \x20   GateStatus::NotMeasured { gate_id: \"product_bar_status\".to_string(), reason: \"no body\".to_string() }\n\
                       \x20} else {\n\
                       \x20   product_bar::judge(pr_body)\n\
                       };\n";
    let found = unmeasured_alternatives_in(
        conditional,
        &binding_statement(conditional, "product_bar_status").expect("bound by a let"),
    );
    assert!(
        found.contains(&"if ") && found.contains(&"GateStatus::"),
        "the conditional fail-open must be visible once the judge call is cut out; \
         found {found:?}"
    );

    // THE REASSIGNMENT FORM OF THE SAME FAIL-OPEN, one line away from the
    // conditional above and invisible to every other check here:
    // `binding_statement` stops at the `;`, and `let_binding_before` strips
    // `mut ` on purpose, so this shape leaves an empty residue.
    let reassigned = "    let mut product_bar_status = product_bar::judge(&diff_ctx.pr_body);\n\
                      \x20   if diff_ctx.pr_body.trim().is_empty() {\n\
                      \x20       product_bar_status = GateStatus::NotMeasured { gate_id: \"product_bar_status\".to_string(), reason: \"the pull request has no body\".to_string() };\n\
                      \x20   }\n";
    assert_eq!(
        unmeasured_alternatives_in(
            reassigned,
            &binding_statement(reassigned, "product_bar_status").expect("bound by a let"),
        ),
        Vec::<&str>::new(),
        "sanity, and the reason this check exists: the initialiser reader sees NOTHING \
         wrong with the reassignment fail-open, because the statement it reads ends at \
         the semicolon"
    );
    assert_eq!(
        assignments_to(reassigned, "product_bar_status").len(),
        1,
        "the reassignment that substitutes NotMeasured for the measurement must be \
         seen; got {:?}",
        assignments_to(reassigned, "product_bar_status")
    );

    // And the mirror, so the new check cannot fail a correct wiring for its
    // spelling. A plain `let`, an annotated `let`, a `let mut` that is never
    // written over, and a comparison are all correct and none of them is an
    // overwrite.
    for clean in [
        straight,
        annotated,
        "let mut product_bar_status = product_bar::judge(pr_body);\n",
        "if product_bar_status == GateStatus::Passed {\n",
        "    product_bar_status != GateStatus::Passed\n",
        "let product_bar_status: GateStatus = product_bar::judge(&diff_ctx.pr_body);\n",
    ] {
        assert_eq!(
            assignments_to(clean, "product_bar_status"),
            Vec::<String>::new(),
            "assignments_to saw a write in {clean:?}, which introduces or merely reads \
             the binding. A guard that misreads a correct wiring and reports it as the \
             defect it does not have is worse than no guard"
        );
    }
}
