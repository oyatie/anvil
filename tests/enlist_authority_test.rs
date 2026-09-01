//! Lane `enlist-authority`: Anvil does not admit or endorse a change on
//! evidence it does not have.
//!
//! Two filed defects, one subsystem, one invariant.
//!
//! Issue #17 — `MergeEnlister::enlist_into_merge_queue` has four callers and
//! one of them checks `is_admissible()`. The webhook review pipeline states the
//! rule in a comment ("absent evidence must never merge", invariant I1) and
//! obeys it; the CLI `enlist` subcommand, `POST /api/enlist` and the
//! queue-healer re-enlist walk straight past it. A gate enforced at one of four
//! doors is a convention, not an invariant.
//!
//! Issue #18 — `ensure_approving_review` submits a formal GitHub `APPROVE`
//! whose body asserts "All automated review, documentation parity, clean
//! architecture, and safety gates have passed with 100% compliance", and
//! `post_enlistment_note` follows it with "Pre-Merge Certification 100% Green".
//! Both sentences are string literals in functions that receive no report. They
//! are written onto the pull request, so a reader cannot tell a genuinely
//! certified PR from one enlisted with zero gates run.
//!
//! # Why the entry point takes the evidence
//!
//! The first version of this suite pinned issue #17 entirely by reading
//! production source: it looked for a call to the admission seam near each
//! door. A source window can show that a token is present and has a syntactic
//! consequence. It cannot show that the token is on the path that matters, and
//! an implementation that wrote the guard as
//! `if let Some(r) = self.cached_report(..).await { Self::admission_refusal(Some(&r))?; }`
//! — with `cached_report` returning `None` — satisfied every such scan while
//! admitting every pull request on no evidence at all. The check being
//! conditional on the evidence existing *is* the bug, and the window was opened
//! inside the conditional.
//!
//! So "no report" is made a value the entry point must answer for rather than a
//! state it cannot observe: `enlist_into_merge_queue` takes
//! `Option<&PreMergeCertificationReport>`, and
//! `the_merge_queue_entry_point_refuses_the_evidence_it_was_handed` calls it
//! with `None` and with two reports that must not merge, and requires each to
//! come back refused. Its fourth case is the symmetric one: a certified, fully
//! measured report must get *past* the guard, measured against the failure the
//! machine gives for asking GitHub about the pull request at all — so no
//! wording is pinned and a second precondition bolted on beside the seam
//! cannot silently withhold the whole fleet. That is behaviour, not a scan, and
//! it holds whatever shape the guard is written in. What remains for source
//! scans is the part no in-process call can reach: which callers exist, where
//! what they hand over came from, and what the two publishing functions weld
//! into the text they sign.
//!
//! # Premortem
//!
//! Assume both fixes shipped and then failed. The ways they can have failed,
//! each turned into a test below:
//!
//! P1. The check reads `is_certified_ready` instead of `is_admissible`, so a
//!     report that certifies while three gates produced no measurement still
//!     merges — the exact distinction the two predicates exist to draw.
//!     -> `a_certified_report_with_an_unmeasured_gate_is_refused_and_the_gate_is_named`,
//!        and the same report as a case of the entry-point test.
//! P2. A caller that cannot obtain a report treats "no report" as "nothing
//!     objected" and enlists. Absent configuration is not permission.
//!     -> `evidence_that_was_never_obtained_does_not_admit_a_pull_request`,
//!        and `None` as a case of the entry-point test.
//! P3. The refusal is a silent `return Ok(())`, or the caller throws it away,
//!     or it reaches a log inside a detached task and never the requester.
//!     Nothing merges and nobody can say why.
//!     -> every refusal test asserts a reason;
//!        `no_path_drops_a_merge_queue_refusal_on_the_floor` bans discarding the
//!        outcome at the call site, and
//!        `the_enlist_api_does_not_answer_success_for_an_enlistment_it_has_not_performed`
//!        covers the layer that test cannot see.
//! P4. Over-correction: the precondition refuses everything, including a
//!     genuinely certified, fully measured pull request. I1 cuts both ways —
//!     absent evidence is not a pass and present evidence is not an accusation.
//!     -> `a_fully_measured_and_certified_report_admits_the_pull_request` on the
//!        seam, and the admitting case of the entry-point test on the call.
//! P5. The entry point is fixed and the callers keep passing `None`, so nothing
//!     ever merges again; or a caller manufactures the report it hands over,
//!     which is the same defect wearing a report's clothes — and the cheapest
//!     place to manufacture one is beside the type, where `report.rs` says in a
//!     comment that no "all passed" constructor exists and nothing checks.
//!     -> `a_report_no_certification_run_produced_is_refused_however_well_it_reads`
//!        asks the value itself, which is the only question a spelling cannot
//!        answer, and
//!        `every_door_hands_the_merge_queue_evidence_a_certification_run_produced`
//!        backs it with source: what produced the value, not what the call site
//!        is spelled like.
//! P6. The blanket claim is deleted from `ensure_approving_review` and reappears
//!     a few lines down in the enlistment note, or moves into a `const`, a
//!     helper, or a sibling file — or stays exactly where it was and gets
//!     `format!`ed onto the derived text, which every check on where the
//!     derived value *goes* answers perfectly. The struct is honest, the
//!     published comment is not — and the comment is what a human reads.
//!     -> the note is held to the same derivation rule as the review: both
//!        seams are exercised by `the_endorsement_differs_when_the_evidence_differs`
//!        and both publishers by
//!        `nothing_anvil_publishes_is_written_by_a_function_that_holds_no_report`.
//!        `no_published_string_claims_a_compliance_total_that_no_gate_produced`
//!        is a cheap backstop over eight words, not a substitute for either.
//! P7. `approval_summary` is implemented correctly and never called: the
//!     production path keeps writing its own sentence with no report in scope,
//!     or calls the seam with a literal `None` and publishes a fallback.
//!     -> `nothing_anvil_publishes_is_written_by_a_function_that_holds_no_report`.
//! P8. The claim is reworded rather than derived — a different literal, equally
//!     unmeasured, identical for every pull request in the fleet.
//!     -> `the_endorsement_differs_when_the_evidence_differs`, which pins
//!        derivation without pinning one wording, on a pair of reports that are
//!        *both* admissible so a publication is actually produced for each.
//! P9. The text is derived from `unmeasured_gates` only, so a gate that
//!     `Errored` — configured, attempted, no result — is still described as
//!     having passed.
//!     -> `a_report_that_certifies_while_a_gate_errored_is_still_refused`, which
//!        also requires the refusal to name the gate. This is the side the
//!        defect can still bite from: a report carrying an `Errored` gate is
//!        never admitted and so is never published onto, and the refusal is the
//!        last place Anvil says anything about that gate at all.
//! P10. The text is derived from the ready-made `gate_counts()`, which scores a
//!     `Warning` as acceptable and so reports "72 of 72 gates passed" for a
//!     report where one gate regressed. Honest-looking, asserted on behalf of
//!     nobody's measurement, and it trips no ban on totality wording.
//!     -> `an_endorsement_accounts_for_the_gate_that_did_not_simply_pass`
//!        carries a positive obligation as well as the ban, on the one report
//!        that is both imperfect and admitted.
//! P11. The publications are made honest and are then published over a pull
//!     request that was refused — the note says "Enlisted in Merge Queue" on a
//!     pull request that was not. And a suite that merely reads whatever text an
//!     inadmissible report produces asserts nothing at all once the publishers
//!     sit past the admission decision: the seams answer `None`, the loop runs
//!     zero times, and the coverage is imaginary.
//!     -> `nothing_is_endorsed_on_evidence_that_cannot_admit_the_pull_request`
//!        makes the `None` the assertion.
//!
//! # What the source scans will and will not accept
//!
//! Three questions cannot be asked in-process, because the paths that answer
//! them shell out to `gh` and take an `AppState` of ninety `Arc` fields: which
//! callers hand a pull request to the merge queue, what they hand over with it,
//! and whether the text a publishing function signs is the text a seam derived.
//! Those are read from production source. Comment text *and the contents of
//! string literals* are blanked before any structural scan, so neither a
//! comment documenting the convention nor a `warn!` reminding callers of it can
//! answer a question about whether something is done. `#[cfg(test)]` items are
//! removed by brace matching rather than by truncating the file at the first
//! one, so a test module parked above a door cannot delete that door from the
//! corpus.
//!
//! A `Result` counts as discarded when *its own* value is thrown away: the call
//! is bound to `_` (or to `_anything`), wrapped in `drop(`, or its result —
//! everything after the call's closing parenthesis — is `.ok()`d or
//! `unwrap_or`'d. A `.ok()` inside the argument list belongs to an argument, not
//! to the call, and
//! `Self::admission_refusal(self.report(..).await.ok().as_ref())?` is a guarded
//! door: converting a fetch failure into `None` and refusing on it is exactly
//! what the spec asks for.
//!
//! Values are followed both ways. Forwards, from the seam to the call that
//! publishes, through rebindings and the struct the text gets carried in.
//! Backwards, from what a call is handed to the bindings that produced it —
//! which is how "the seam's value reaches the pull request" becomes "the seam's
//! value is all that reaches it", and how a door's report is asked where it
//! came from rather than what it is called. A call whose callee is written in
//! another file is followed there: extracting a spawned closure into a helper,
//! or obtaining the certification report by calling the pipeline that runs it,
//! are both ordinary and neither may blind a scan.
//!
//! Two exemptions, both deliberate. A struct literal is read field by field, so
//! `ReviewResponse { summary, verdict: "APPROVE".to_string(), .. }` offers the
//! derived text and the verdict separately — the verdict is not a claim about
//! the report. And the arguments of `warn!`, `bail!`, `.context(` and their
//! kin are blanked: an operator-facing message is not signed onto the pull
//! request, and a scan that cannot tell it from a published claim has to accept
//! either both or neither.
//!
//! # What a source scan cannot be asked, and what carries it instead
//!
//! Provenance. A report is evidence when a certification run produced it and an
//! opinion otherwise, and the two are the same seventy-two `Passed` fields:
//! `is_admissible()` says yes to both, `gate_counts()` scores both at the whole
//! corpus. Reading source for the certification run's *name* is answered by a
//! helper merely named after it — `evaluate_pre_merge_gates_from_cache` runs no
//! gate and satisfied every scan the first version of this suite had, while
//! every pull request in the fleet merged on a report zero gates produced.
//!
//! So the report is made to carry where its statuses came from, as something a
//! caller cannot type: `from_gate_outcomes` is the only way gate outcomes enter
//! a report, and `every_gate_passing` and `forged_all_passing` are the same
//! report with and without having gone through it. The scans stay as backstops
//! and both of their holes are closed — the certification run is matched as a
//! call rather than as a substring, and the forgery scan reads every file under
//! `src/pre_merge_guard/` instead of exempting the directory the fabricator was
//! found in.
//!
//! # Scaffolding this suite depends on
//!
//! Three signatures with `todo!()` bodies in `src/merge_enlister.rs` —
//! `admission_refusal`, `approval_summary`, `enlistment_note` — one parameter
//! added to `enlist_into_merge_queue` with every caller passing `None` so the
//! tree compiles, and one signature with a `todo!()` body in
//! `src/pre_merge_guard/report.rs`, `from_gate_outcomes`. No body holds logic,
//! and how provenance is represented is left open.
//!
//! # Green over today's tree, and here anyway
//!
//! Issue #18 says to keep three things that already hold: the enlistment path
//! fails closed on `CHANGES_REQUESTED` and on unresolved review threads, and
//! uses no `--admin`. A guard for them would be a green test in a lane whose
//! rule is that every spec test starts red — so it is not one. They are asserted
//! first inside
//! `nothing_anvil_publishes_is_written_by_a_function_that_holds_no_report`,
//! which is red, exactly as the I2 constructor rule and the forgery scan are
//! asserted first inside the door test. This lane is what puts them at risk:
//! two of the three refusals live in the function this suite compels to be
//! rewritten, and the alternative it sanctions — drop the self-approval —
//! deletes that function. See `assert_the_merge_queue_path_still_fails_closed`.

use anvil::github::GitHubClient;
use anvil::merge_enlister::MergeEnlister;
use anvil::pre_merge_guard::matrix::{MatrixRenderer, label_for};
use anvil::pre_merge_guard::report::{GateStatus, PreMergeCertificationReport, TOTAL_GATES};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Totality words, in the sense a published claim uses them.
///
/// Shared by `assert_no_blanket_claim` and by the source backstop: what Anvil
/// may not assert about a report at runtime is what it may not weld into a
/// literal either. An honest derived body — "69 of 72 gates passed, 3 produced
/// no measurement" — contains none of them.
///
/// Each entry is a *claim*, not a token. A bare "100%" is not one: the report's
/// own rendered gate matrix carries it twice, inside the descriptions of two
/// gates (`matrix.rs`: "100% parity between live cluster and Git trunk" and
/// "100% of review comments and threads must be resolved"). Banning the number
/// makes the most honest derivation available — publishing what the report
/// itself says about every gate — an accusation of asserting something no gate
/// measured, about a word the report wrote. The two sentences in the tree today
/// assert completeness *about this report* — "100% compliance", "100% Green" —
/// and that is what is banned.
const TOTALITY: [&str; 14] = [
    "100% compliance",
    "100% compliant",
    "100% green",
    "100% certified",
    "100% pass",
    "100% clean",
    "100% of gates",
    "all automated",
    "all gates",
    "all checks",
    "all safety",
    "every gate",
    "fully compliant",
    "fully green",
];

/// The merge strategy the enlistment note is written about. Held constant
/// wherever two notes are compared for what the *report* says, so any
/// difference between them comes from the report and not from this. The one
/// place it is varied is
/// `the_endorsement_differs_when_the_evidence_differs`, where the report is
/// held constant instead and the difference can only come from the strategy.
const STRATEGY: &str = "Squash & Merge";

/// The other strategy `enlist_into_merge_queue` merges under: its retry path
/// takes this one whenever GitHub refuses `--squash`.
const OTHER_STRATEGY: &str = "Merge Commit";

// -------------------------------------------------------------------------
// Report fixtures
// -------------------------------------------------------------------------

/// A report in which every gate in the corpus reports `Passed`, produced the
/// way a report is produced: by handing the gate outcomes to the one
/// constructor that consumes them.
///
/// The gate names are read off the corpus rather than typed out, so the fixture
/// stays correct when the corpus grows. What matters is not that the statuses
/// are `Passed` — `forged_all_passing` says exactly the same thing — but that
/// they arrived as outcomes. That difference is the whole of P5, and the pair
/// of fixtures is what makes it a value the tests can put a question to rather
/// than a spelling they have to go looking for in source.
fn every_gate_passing() -> PreMergeCertificationReport {
    let base = PreMergeCertificationReport::unmeasured("fixture baseline");
    let names: Vec<&'static str> = base.named_statuses().into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        names.len(),
        TOTAL_GATES,
        "the fixture must cover the whole corpus; it found {} named gates against \
         TOTAL_GATES={}",
        names.len(),
        TOTAL_GATES
    );
    let outcomes: Vec<(&str, GateStatus)> =
        names.into_iter().map(|n| (n, GateStatus::Passed)).collect();
    let mut report = PreMergeCertificationReport::from_gate_outcomes(&outcomes)
        .expect("the fixture hands over an outcome for every gate in the corpus");
    seal_like_a_run(&mut report);
    assert!(
        report.is_admissible(),
        "fixture sanity: every gate passing must be admissible"
    );
    report
}

/// Every gate in the corpus reporting `Passed` and no gate having run:
/// `unmeasured` round-tripped through serde with the statuses overwritten.
///
/// Well-typed, `is_admissible()` says yes, and nothing in it came from a
/// measurement. This is P5's `optimistic(reason)` fabricator — the one the
/// door scan was walked around by a helper merely *named* after the
/// certification run — written as a test is able to write it, which is the
/// point: if a test can produce this at all, so can a hurried caller, and
/// `admission_refusal` is the only thing standing between it and the fleet.
fn forged_all_passing() -> PreMergeCertificationReport {
    let base = PreMergeCertificationReport::unmeasured("cargo check was clean");
    let mut value = serde_json::to_value(&base).expect("report serialises");
    let object = value.as_object_mut().expect("report is a JSON object");
    let gate_fields: Vec<String> = object
        .keys()
        .filter(|k| k.ends_with("_status"))
        .cloned()
        .collect();
    assert_eq!(
        gate_fields.len(),
        TOTAL_GATES,
        "the fixture must cover the whole corpus; it found {} gate fields against \
         TOTAL_GATES={}",
        gate_fields.len(),
        TOTAL_GATES
    );
    for field in gate_fields {
        object.insert(field, serde_json::json!("Passed"));
    }
    let mut report: PreMergeCertificationReport =
        serde_json::from_value(value).expect("report deserialises");
    seal_like_a_run(&mut report);
    report
}

/// Seals a report the way a certification run seals one: the verdict and the
/// unmeasured list derived from the statuses, and the rendered matrix written
/// back onto `summary_markdown`.
///
/// `evaluate_pre_merge_gates` does exactly this, and a fixture that does only
/// the first half leaves `summary_markdown` empty — the one field a publisher
/// is most likely to reach for, and arguably the most honest derivation
/// available, since it names every gate with what it produced. Left empty it is
/// identical on every report, so a publisher deriving from it would be accused
/// of publishing a constant. The fixture must not be the reason a correct
/// implementation looks wrong.
fn seal_like_a_run(report: &mut PreMergeCertificationReport) {
    report.seal();
    report.summary_markdown = MatrixRenderer::render(report);
}

fn not_measured(gate_id: &str) -> GateStatus {
    GateStatus::NotMeasured {
        gate_id: gate_id.to_string(),
        reason: "no data source configured".to_string(),
    }
}

/// Certified on every measured gate, with one gate that produced no
/// measurement: `is_certified_ready` says yes, `is_admissible()` says no.
fn certified_with_an_unmeasured_gate() -> PreMergeCertificationReport {
    let mut report = every_gate_passing();
    report.kani_status = not_measured("kani_status");
    seal_like_a_run(&mut report);
    report
}

/// A report that asserts certification while a gate errored. Deliberately not
/// sealed: `is_admissible()` alone says yes to it.
fn certified_while_a_gate_errored() -> PreMergeCertificationReport {
    let mut report = every_gate_passing();
    report.slo_status = GateStatus::Errored("prometheus probe timed out".into());
    report.is_certified_ready = true;
    report.recompute_unmeasured();
    report.summary_markdown = MatrixRenderer::render(&report);
    report
}

/// Whether `n` appears in `text` as a number in its own right, so a claim about
/// three gates is not answered by the "3" inside "23".
fn mentions_number(text: &str, n: usize) -> bool {
    let n = n.to_string();
    text.split(|c: char| !c.is_ascii_digit()).any(|t| t == n)
}

// -------------------------------------------------------------------------
// Source-scanning helpers
// -------------------------------------------------------------------------

fn repo_path(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

/// One production source file, split into the code that runs and the strings it
/// carries.
struct Production {
    /// One entry per line, with comment text, the *contents* of every string
    /// literal, and every `#[cfg(test)]` item replaced by spaces. Line numbers
    /// still line up with the file, and the quote characters are kept, so a
    /// surviving `"` marks a literal.
    ///
    /// A token found here is production code. Neither a comment explaining an
    /// invariant, nor a `warn!` reminding callers of one, nor a call made by
    /// the module's own unit tests can satisfy a scan for it.
    code: Vec<String>,
    /// Every string literal outside the test modules, as (1-based line,
    /// contents) — what a scan for a published claim reads.
    literals: Vec<(usize, String)>,
}

/// Blanks a byte range, keeping newlines so line numbers still line up.
fn blank_range(chars: &mut [char], range: std::ops::Range<usize>) {
    for c in &mut chars[range] {
        if *c != '\n' {
            *c = ' ';
        }
    }
}

/// The end of the `#[cfg(test)]`-attributed item starting at `attr`: past the
/// matching `}` of its first block, or past the `;` that ends it if it has
/// none (`#[cfg(test)] use ...;`).
///
/// Brace matching rather than truncation, because truncating at the *first*
/// `#[cfg(test)]` deletes every door below it from the corpus. A test module
/// parked above the CLI `enlist` handler used to remove that door from the scan
/// silently, leaving the door test green over three doors instead of four.
fn cfg_test_item_end(code: &[char], attr: usize) -> usize {
    let mut i = attr;
    let mut depth = 0usize;
    while i < code.len() {
        match code[i] {
            '{' => depth += 1,
            // A `}` at depth zero closes whatever encloses the attributed item,
            // so the item ended before it.
            '}' if depth == 0 => return i,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            ';' if depth == 0 && i > attr => return i + 1,
            _ => {}
        }
        i += 1;
    }
    code.len()
}

/// Reads production source: no comments, no string contents, no test modules.
///
/// Resolved as a MODULE, not a path. `merge_enlister.rs` became
/// `merge_enlister/` to satisfy the oversized-file ratchet, and this read --
/// which names the file -- stopped finding its subject. It failed loudly only
/// because of the `expect`; a scan that returned an empty string would have
/// reported the module clean. `module_source` reads whichever form exists, so
/// the next split changes nothing here.
///
/// Line numbers below are within the module's concatenated source when it is a
/// directory. These checks are about what is published and by whom, not about
/// a coordinate, so that is a report detail rather than a lost fact.
fn production(rel: &str) -> Production {
    let text = anvil::source_scan::paths::module_source(
        rel.trim_end_matches(".rs"),
        Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    let src: Vec<char> = text.chars().collect();
    let mut code: Vec<char> = Vec::with_capacity(src.len());
    let mut literals: Vec<(usize, String)> = Vec::new();
    let mut line = 1usize;
    let mut i = 0usize;

    fn blank(out: &mut Vec<char>, c: char) {
        out.push(if c == '\n' { '\n' } else { ' ' });
    }

    while i < src.len() {
        let c = src[i];
        let next = src.get(i + 1).copied();

        if c == '/' && next == Some('/') {
            while i < src.len() && src[i] != '\n' {
                blank(&mut code, src[i]);
                i += 1;
            }
            continue;
        }

        if c == '/' && next == Some('*') {
            let mut depth = 0usize;
            while i < src.len() {
                if src[i] == '/' && src.get(i + 1) == Some(&'*') {
                    depth += 1;
                    blank(&mut code, src[i]);
                    blank(&mut code, src[i + 1]);
                    i += 2;
                    continue;
                }
                if src[i] == '*' && src.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    blank(&mut code, src[i]);
                    blank(&mut code, src[i + 1]);
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                    continue;
                }
                if src[i] == '\n' {
                    line += 1;
                }
                blank(&mut code, src[i]);
                i += 1;
            }
            continue;
        }

        // Raw string, `r"..."` or `r#"..."#`, not the tail of an identifier.
        if c == 'r' && (i == 0 || !(src[i - 1].is_alphanumeric() || src[i - 1] == '_')) {
            let mut j = i + 1;
            let mut hashes = 0usize;
            while src.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if src.get(j) == Some(&'"') {
                let start_line = line;
                code.extend_from_slice(&src[i..=j]);
                i = j + 1;
                let mut content = String::new();
                while i < src.len() {
                    if src[i] == '"' && (1..=hashes).all(|h| src.get(i + h) == Some(&'#')) {
                        code.extend_from_slice(&src[i..=(i + hashes)]);
                        i += hashes + 1;
                        break;
                    }
                    if src[i] == '\n' {
                        line += 1;
                    }
                    content.push(src[i]);
                    blank(&mut code, src[i]);
                    i += 1;
                }
                literals.push((start_line, content));
                continue;
            }
        }

        if c == '"' {
            let start_line = line;
            code.push('"');
            i += 1;
            let mut content = String::new();
            while i < src.len() && src[i] != '"' {
                if src[i] == '\\' && i + 1 < src.len() {
                    content.push(src[i]);
                    content.push(src[i + 1]);
                    if src[i + 1] == '\n' {
                        line += 1;
                    }
                    blank(&mut code, src[i]);
                    blank(&mut code, src[i + 1]);
                    i += 2;
                    continue;
                }
                if src[i] == '\n' {
                    line += 1;
                }
                content.push(src[i]);
                blank(&mut code, src[i]);
                i += 1;
            }
            if i < src.len() {
                code.push('"');
                i += 1;
            }
            literals.push((start_line, content));
            continue;
        }

        // A char literal, told apart from a lifetime.
        if c == '\''
            && match next {
                Some('\\') => true,
                Some(_) => src.get(i + 2) == Some(&'\''),
                None => false,
            }
        {
            code.push('\'');
            i += 1;
            while i < src.len() && src[i] != '\'' {
                blank(&mut code, src[i]);
                if src[i] == '\\' && i + 1 < src.len() {
                    blank(&mut code, src[i + 1]);
                    i += 1;
                }
                i += 1;
            }
            if i < src.len() {
                code.push('\'');
                i += 1;
            }
            continue;
        }

        if c == '\n' {
            line += 1;
        }
        code.push(c);
        i += 1;
    }

    // Test items are removed after literal blanking, so a `{` or `}` inside a
    // string in a test module cannot throw the brace matching off.
    let needle: Vec<char> = "#[cfg(test)]".chars().collect();
    let mut stripped: Vec<std::ops::Range<usize>> = Vec::new();
    let mut at = 0usize;
    while at + needle.len() <= code.len() {
        if code[at..at + needle.len()] == needle[..] {
            let end = cfg_test_item_end(&code, at);
            stripped.push(at..end);
            at = end;
        } else {
            at += 1;
        }
    }
    let stripped_lines: Vec<std::ops::Range<usize>> = stripped
        .iter()
        .map(|r| {
            let first = code[..r.start].iter().filter(|c| **c == '\n').count() + 1;
            let last = code[..r.end].iter().filter(|c| **c == '\n').count() + 1;
            first..(last + 1)
        })
        .collect();
    for range in stripped {
        blank_range(&mut code, range);
    }
    literals.retain(|(line, _)| !stripped_lines.iter().any(|r| r.contains(line)));

    let joined: String = code.into_iter().collect();
    Production {
        code: joined.lines().map(str::to_string).collect(),
        literals,
    }
}

fn production_lines(rel: &str) -> Vec<String> {
    production(rel).code
}

fn production_source(rel: &str) -> String {
    production(rel).code.join("\n")
}

fn rust_sources_under(dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![repo_path(dir)];
    let root = repo_path("");
    while let Some(p) = stack.pop() {
        let Ok(entries) = fs::read_dir(&p) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|x| x == "rs") {
                let rel = path
                    .strip_prefix(&root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                out.push(rel);
            }
        }
    }
    out.sort();
    out
}

fn char_boundary_at_or_after(text: &str, mut i: usize) -> usize {
    let i_max = text.len();
    while i < i_max && !text.is_char_boundary(i) {
        i += 1;
    }
    i.min(i_max)
}

/// The start of the statement `idx` sits in: back past the previous `;`, `{` or
/// `}`, but *through* the opening brace of a struct literal, so that
/// `let approval = ReviewResponse { summary: f(), .. };` is one statement whose
/// binder is `approval` rather than a fragment beginning at `summary:`.
///
/// A struct literal is told from a block by its opener: `Type {` is preceded by
/// an upper-case-initial path, `if x {` and `else {` are not.
fn statement_start(text: &str, idx: usize) -> usize {
    let mut pos = idx;
    loop {
        let start = text[..pos]
            .rfind([';', '{', '}'])
            .map(|i| i + 1)
            .unwrap_or(0);
        if start == 0 {
            return 0;
        }
        if text.as_bytes()[start - 1] == b'{' {
            let before = text[..start - 1].trim_end();
            let token: String = before
                .chars()
                .rev()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':')
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let head = token.rsplit("::").next().unwrap_or("").to_string();
            if head.chars().next().is_some_and(|c| c.is_uppercase()) {
                pos = start - 1;
                continue;
            }
        }
        return start;
    }
}

/// The end of the statement `idx` sits in: the `;` that closes it, or — when
/// the statement opens a block — past that block's closing `}`.
///
/// The forward half is the point. What becomes of a value is written after it:
/// `...enlist_into_merge_queue(..).await.ok();` throws a refusal away *after*
/// the call, and `if refusal.is_err() { return Ok(()) }` swallows one inside a
/// block. A window that stops at the token sees neither.
fn statement_end(text: &str, idx: usize) -> usize {
    let bytes = text.as_bytes();
    let mut i = idx;
    let mut depth: i32 = 0;
    let mut opened = false;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                depth += 1;
                opened = true;
            }
            b'}' => {
                if !opened {
                    break;
                }
                depth -= 1;
                if depth <= 0 {
                    i += 1;
                    break;
                }
            }
            b';' if !opened => {
                i += 1;
                break;
            }
            _ => {}
        }
        i += 1;
    }
    char_boundary_at_or_after(text, i)
}

/// The whole statement a token sits in.
fn statement(text: &str, idx: usize) -> String {
    text[statement_start(text, idx)..statement_end(text, idx)].to_string()
}

/// A call found by a needle that ends in `(`: the index of the needle, of its
/// open parenthesis, and of the matching close.
struct Call {
    idx: usize,
    open: usize,
    close: usize,
}

fn find_call(text: &str, needle: &str, from: usize) -> Option<Call> {
    debug_assert!(needle.ends_with('('));
    // A needle that starts on an identifier must not match the tail of a longer
    // one: `enlistment_note(` is inside `post_enlistment_note(`, and a scan that
    // takes the function's own signature for a call to the seam reads every
    // parameter as an argument and finds no binding at all.
    let head_is_word = needle
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || c == '_');
    let mut search = from;
    let idx = loop {
        let at = search + text[search..].find(needle)?;
        search = at + 1;
        if !head_is_word
            || !text[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            break at;
        }
    };
    let open = idx + needle.len() - 1;
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    for (offset, b) in bytes[open..].iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(Call {
                        idx,
                        open,
                        close: open + offset,
                    });
                }
            }
            _ => {}
        }
    }
    None
}

/// The arguments of a call, split on the commas that are not inside a nested
/// group.
fn call_arguments(text: &str, call: &Call) -> Vec<String> {
    let inner = &text[call.open + 1..call.close];
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for c in inner.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                args.push(current.trim().to_string());
                current = String::new();
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    if !current.trim().is_empty() {
        args.push(current.trim().to_string());
    }
    args
}

/// Whether a call's *own* `Result` is thrown away.
///
/// Measured against the call, not against everything sharing its statement: the
/// suffix is what follows the call's closing parenthesis, and the prefix is the
/// binding immediately in front of it. A `.ok()` inside the argument list
/// belongs to an argument — `admission_refusal(self.report(..).await.ok().as_ref())?`
/// converts a fetch failure into `None` and refuses on it, which is a guarded
/// door and not a discarded one.
fn discards_call_result(text: &str, call: &Call) -> bool {
    let suffix = &text[call.close + 1..statement_end(text, call.close)];
    if suffix.contains(".ok()") || suffix.contains(".unwrap_or") {
        return true;
    }
    let prefix = text[statement_start(text, call.idx)..call.idx].trim_end();
    // `contains`, not `ends_with`: the door is reached through a receiver, so
    // the text between `drop(` and the call is `self.merge_enlister.`.
    if prefix.contains("drop(") || prefix.contains("_ =") {
        return true;
    }
    let stmt = statement(text, call.idx);
    // `let _ignored = ..` is `let _ = ..` with a comment attached to it.
    if binder(&stmt).is_some_and(|b| b.starts_with('_')) {
        return true;
    }
    binder(&stmt).is_some_and(|name| {
        binder_is_thrown_away_later(text, statement_end(text, call.close), &name)
    })
}

/// The end of the block holding `from`: the first bracket that closes something
/// `from` is inside.
///
/// Bounds a search to the rest of the enclosing function body. Searched over
/// the whole file instead, a binding of the same name in an unrelated function
/// would answer a question about this one.
fn block_end(text: &str, from: usize) -> usize {
    let mut depth = 0i32;
    for (off, c) in text[from..].char_indices() {
        match c {
            '{' | '(' | '[' => depth += 1,
            '}' | ')' | ']' => {
                depth -= 1;
                if depth < 0 {
                    return from + off;
                }
            }
            _ => {}
        }
    }
    text.len()
}

/// Whether `name` is thrown away later in the block that holds `from`.
///
/// `let outcome = door().await; let _ = outcome;` is `let _ = door().await`
/// with one identifier in between, and it is the shape a scan anchored to the
/// call's own statement cannot see: that statement binds a name, so it reads as
/// handled, and the value dies a line later. `drop(outcome)` is the same move
/// spelled differently. Both compile without a `must_use` warning, which is
/// what makes them the silencer to reach for.
fn binder_is_thrown_away_later(text: &str, from: usize, name: &str) -> bool {
    let from = from.min(text.len());
    statements_in(text, from, block_end(text, from))
        .iter()
        .any(|s| {
            let compact = s.split_whitespace().collect::<Vec<_>>().join(" ");
            if compact.contains(&format!("drop({name})")) {
                return true;
            }
            let Some(rest) = compact.strip_prefix("let _") else {
                return false;
            };
            rest.split_once('=')
                .is_some_and(|(_, value)| value.trim().trim_end_matches(';').trim() == name)
        })
}

/// The end of the statement starting at `start`, following `else` chains.
///
/// `statement_end` stops at the `}` that closes the first block, which is right
/// for `let Some(x) = seam() else { .. }` and wrong for
/// `let x = if let Some(s) = seam() { s } else { <a sentence of its own> };` —
/// where the text standing in for an absent derivation is in the branch it
/// stops before.
fn statement_end_through_else(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut i = start;
    let mut depth = 0i32;
    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' => depth -= 1,
            b'}' => {
                depth -= 1;
                if depth <= 0 {
                    if text[i + 1..].trim_start().starts_with("else") {
                        depth = 0;
                        i += 1;
                        continue;
                    }
                    return char_boundary_at_or_after(text, i + 1);
                }
            }
            b';' if depth <= 0 => return char_boundary_at_or_after(text, i + 1),
            _ => {}
        }
        i += 1;
    }
    char_boundary_at_or_after(text, i)
}

/// The statements from the one holding `from` up to `to`, in source order.
fn statements_in(text: &str, from: usize, to: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = statement_start(text, from);
    while i < to.min(text.len()) {
        let end = char_boundary_at_or_after(text, statement_end_through_else(text, i).max(i + 1))
            .min(text.len());
        out.push(text[i..end].to_string());
        let mut j = end;
        while j < text.len() && text.as_bytes()[j].is_ascii_whitespace() {
            j += 1;
        }
        if j <= i {
            break;
        }
        i = j;
    }
    out
}

/// Every word-shaped token in `text`, deduplicated.
fn identifiers(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for c in text.chars().chain(std::iter::once(' ')) {
        if c.is_alphanumeric() || c == '_' {
            current.push(c);
        } else if !current.is_empty() {
            let word = std::mem::take(&mut current);
            if !out.contains(&word) {
                out.push(word);
            }
        }
    }
    out
}

/// The last statement before `upto` that binds `name`.
fn binding_statement(text: &str, name: &str, upto: usize) -> Option<String> {
    let mut found = None;
    let mut from = 0usize;
    while let Some(off) = text[from..].find("let ") {
        let at = from + off;
        from = at + 4;
        if at >= upto {
            break;
        }
        let end = statement_end_through_else(text, at).min(text.len());
        if end <= at {
            continue;
        }
        let stmt = text[at..end].to_string();
        if binder(&stmt).as_deref() == Some(name) {
            found = Some(stmt);
        }
    }
    found
}

/// The values that reach `handed`, followed backwards through the bindings that
/// produced them.
///
/// The forward half of the publication scan asks whether the seam's value
/// reaches the call that publishes. This asks the other half — what *else*
/// reaches it. A publisher that binds the derived text into a sentence of its
/// own answers the forward question perfectly and signs an unmeasured claim
/// anyway, so every value on the way to the pull request has to be accounted
/// for, not only the one that came from the seam.
///
/// Only names this function actually binds are followed: `verdict` and
/// `comments` are field names, not values it computed.
fn traced_values(body: &str, handed: &str, upto: usize) -> Vec<String> {
    let mut traced: Vec<String> = Vec::new();
    let mut queue: Vec<String> = identifiers(handed);
    let mut seen: Vec<String> = Vec::new();
    while let Some(name) = queue.pop() {
        if seen.contains(&name) {
            continue;
        }
        seen.push(name.clone());
        let Some(stmt) = binding_statement(body, &name, upto) else {
            continue;
        };
        traced.push(name);
        let rhs = stmt
            .split_once('=')
            .map(|(_, r)| r.to_string())
            .unwrap_or(stmt);
        queue.extend(identifiers(&rhs));
    }
    traced
}

/// A statement cut into the pieces in which a value and a fixed text can be
/// welded together: a `;` ends one, a `{` ends one, and so does a comma
/// directly inside a brace group.
///
/// So `ReviewResponse { summary, verdict: "APPROVE".to_string(), .. }` offers
/// the derived text and the verdict as separate pieces — the verdict is not a
/// claim about the report, and a scan that cannot separate the two has to
/// accept either both or neither. A `warn!(..)` beside a `return` is likewise
/// not read as part of the binding it shares a statement with.
fn weld_fragments(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut stack: Vec<char> = Vec::new();
    for c in text.chars() {
        match c {
            '(' | '[' => stack.push(c),
            '{' => {
                stack.push(c);
                out.push(std::mem::take(&mut current));
                continue;
            }
            ')' | ']' | '}' => {
                stack.pop();
            }
            ';' => {
                out.push(std::mem::take(&mut current));
                continue;
            }
            ',' if stack.last() == Some(&'{') => {
                out.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    out.push(current);
    out
}

/// Whether `fragment` welds a fixed text onto a value.
///
/// String contents are blanked before any scan, so a surviving `"` is a literal
/// sentence. An ALL-CAPS name is the same sentence moved into a `const`, which
/// is P6's second spelling and trips no ban on vocabulary.
fn welded_text(fragment: &str) -> Option<String> {
    if fragment.contains('"') {
        return Some("a string literal".to_string());
    }
    identifiers(fragment)
        .into_iter()
        .find(|w| {
            w.len() > 1
                && w.chars().any(|c| c.is_ascii_uppercase())
                && w.chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        })
        .map(|w| format!("the constant `{w}`"))
}

/// The bodies of the functions `text` calls, found in `source`, except `skip`.
///
/// A helper on the path from the seam to the publication is part of the
/// publisher: `summary: Self::approval_body(&evidence)` hands the derived text
/// to one line that `format!`s a fixed sentence around it, and a scan that
/// reads only the publisher's own statements sees a pure handover. The door
/// scan and the spawn scan already splice for exactly this reason; the weld
/// scan was the one that did not.
///
/// The seam is never spliced. Holding the report's own words is its whole job.
fn helper_bodies(source: &str, text: &str, skip: &str) -> String {
    let mut out = String::new();
    let mut seen: Vec<String> = vec![skip.to_string()];
    let mut frontier = text.to_string();
    for _ in 0..2 {
        let mut next = String::new();
        for name in called_identifiers(&frontier) {
            if seen.contains(&name) {
                continue;
            }
            seen.push(name.clone());
            let Some(body) = find_fn(source, &format!("fn {name}(")) else {
                continue;
            };
            next.push('\n');
            next.push_str(&body);
        }
        if next.is_empty() {
            break;
        }
        out.push_str(&next);
        frontier = next;
    }
    out
}

/// A fixed text welded onto a value on its way to the pull request, in `span`
/// or in a helper `span` calls.
///
/// Read fragment by fragment rather than over the whole span, and only where a
/// fragment carries a value that is on the path: a struct literal is read field
/// by field, so `ReviewResponse { summary, verdict: "APPROVE".to_string(), .. }`
/// offers the derived text and the verdict separately and the verdict is not a
/// claim about the report. Reading the span whole makes that exemption
/// disappear the moment the seam is scrutinised by an `if let` — the statement
/// is then the entire block, verdict included.
///
/// Where a carrying fragment calls a helper, that helper is read whole: nothing
/// inside it is beside the derived text, it is wrapped around it.
fn welded_on_the_path(source: &str, span: &str, seam: &str, carried: &[String]) -> Option<String> {
    let seam_fn = seam.trim_end_matches('(');
    for fragment in weld_fragments(&blank_unpublished_arguments(span)) {
        if !carried.iter().any(|c| mentions_ident(&fragment, c)) {
            continue;
        }
        if let Some(what) = welded_text(&fragment) {
            return Some(what);
        }
        let helpers = helper_bodies(source, &fragment, seam_fn);
        for inner in weld_fragments(&blank_unpublished_arguments(&helpers)) {
            if let Some(what) = welded_text(&inner) {
                return Some(format!("{what} (in a helper it hands the text to)"));
            }
        }
    }
    None
}

/// `text` with the arguments of the calls that never reach a pull request
/// blanked.
///
/// A log line, an error message and a panic are read by operators; they are not
/// signed onto the pull request, and a scan that cannot tell them from a
/// published claim has to accept either both or neither. Blanking them is what
/// lets the rest of a statement be held to "no fixed text at all".
fn blank_unpublished_arguments(text: &str) -> String {
    let mut out = text.to_string();
    for needle in [
        "warn!(",
        "info!(",
        "error!(",
        "debug!(",
        "trace!(",
        "bail!(",
        "anyhow!(",
        "panic!(",
        ".context(",
        ".with_context(",
    ] {
        let mut from = 0usize;
        while let Some(call) = find_call(&out, needle, from) {
            from = call.close;
            let blanked: String = out[call.open + 1..call.close]
                .chars()
                .map(|c| if c == '\n' { '\n' } else { ' ' })
                .collect();
            out.replace_range(call.open + 1..call.close, &blanked);
        }
    }
    out
}

/// The names `text` calls: an identifier immediately followed by `(`.
fn called_identifiers(text: &str) -> Vec<String> {
    identifiers(text)
        .into_iter()
        .filter(|name| {
            let mut from = 0usize;
            while let Some(off) = text[from..].find(name.as_str()) {
                let at = from + off;
                from = at + name.len();
                let before_ok = text[..at]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
                if before_ok && text[from..].starts_with('(') {
                    return true;
                }
            }
            false
        })
        .collect()
}

/// `text` with the body of every function it calls in `source` spliced in.
///
/// Extracting a spawned closure into a helper is ordinary tidying, and a scan
/// that reads only the closure's own text concludes the detached task no longer
/// enlists anything. It does; the call moved one identifier away. The same is
/// true of the report a door hands over: it is obtained by calling something.
fn with_called_bodies(source: &str, text: &str, depth: usize) -> String {
    const CAP: usize = 400_000;
    let mut out = text.to_string();
    let mut seen: Vec<String> = Vec::new();
    for _ in 0..depth {
        let mut grown = out.clone();
        for name in called_identifiers(&out) {
            if seen.contains(&name) || grown.len() > CAP {
                continue;
            }
            seen.push(name.clone());
            let Some(body) = find_fn(source, &format!("fn {name}(")) else {
                continue;
            };
            grown.push('\n');
            grown.push_str(&body);
        }
        if grown.len() == out.len() {
            break;
        }
        out = grown;
    }
    out
}

/// Every production source under `src/`, as one text.
///
/// A door obtains its evidence by calling something, and what it calls is
/// almost never in its own file — the certification run lives in the review
/// pipeline. Following the call across the file boundary is what separates
/// "obtained from the certification run, wherever that is written" from
/// "manufactured at the door", which is the whole of P5.
fn all_production_source() -> String {
    rust_sources_under("src")
        .iter()
        .map(|rel| production_source(rel))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The whole of one function, signature and braces included, found by a
/// fragment of its signature. Brace-matched, so it works for free functions and
/// for methods at any indentation.
fn find_fn(source: &str, signature_fragment: &str) -> Option<String> {
    let start = source.find(signature_fragment)?;
    let open = start + source[start..].find('{')?;
    let mut depth = 0i32;
    for (offset, b) in source.as_bytes()[open..].iter().enumerate() {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(source[start..=open + offset].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// Whether `text` mentions `ident` as an identifier rather than as a substring
/// of a longer one.
fn mentions_ident(text: &str, ident: &str) -> bool {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut from = 0usize;
    while let Some(off) = text[from..].find(ident) {
        let at = from + off;
        from = at + ident.len();
        let before_ok = text[..at].chars().next_back().is_none_or(|c| !is_word(c));
        let after_ok = text[from..].chars().next().is_none_or(|c| !is_word(c));
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

/// The identifier a `match` arm binds out of `Some(..)` or `Ok(..)`.
///
/// `match seam() { Some(note) => publish(&note), None => return Ok(()) }` binds
/// the derived value exactly as `if let Some(note) = seam()` does, and
/// `assert_absent_text_is_not_papered_over` already anticipates and permits the
/// spelling. A `binder` blind to it reports that the seam's value is bound as
/// nothing at all, the publisher falls back to the substring test, and a
/// function that publishes that value and nothing else is accused of
/// substituting a constant for it — a rejection turning on the spelling and on
/// nothing else.
fn match_arm_binder(statement: &str) -> Option<String> {
    let after = statement.strip_prefix("match ")?;
    // The brace that opens the arms, not one inside the scrutinee.
    let mut depth = 0i32;
    let mut arms = None;
    for (i, c) in after.char_indices() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            '{' if depth == 0 => {
                arms = Some(&after[i + 1..]);
                break;
            }
            _ => {}
        }
    }
    let arms = arms?;
    let mut from = 0usize;
    loop {
        let (at, len) = ["Some(", "Ok("]
            .iter()
            .filter_map(|p| arms[from..].find(p).map(|off| (from + off, p.len())))
            .min_by_key(|(at, _)| *at)?;
        from = at + len;
        let inner = &arms[from..];
        let name: String = inner
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let tail = inner[name.len()..].trim_start();
        // A pattern, not a call: `Some(x) =>` binds a value, `Some(f())` does
        // not, and neither does `Ok(())`.
        if !name.is_empty()
            && name != "_"
            && tail.starts_with(')')
            && tail[1..].trim_start().starts_with("=>")
        {
            return Some(name);
        }
    }
}

/// The identifier a statement binds, in the spellings a value that may be
/// absent is bound with: `let x =`, `let mut x =`, `let Some(x) = .. else`,
/// `if let Some(x) =`, the let-chain `if <cond> && let Some(x) =`, and the
/// `match` arm `Some(x) =>`.
///
/// All of them are the same act. A check that reads one of them and not the
/// others does not pin behaviour, it picks a favourite: the let-chain is the
/// form `src/merge_enlister.rs` itself already reaches for twice, and a scan
/// blind to it accuses a correct implementation of the defect it just fixed.
fn binder(statement: &str) -> Option<String> {
    let mut rest = statement.trim_start();
    for prefix in ["if ", "while "] {
        if let Some(stripped) = rest.strip_prefix(prefix) {
            rest = stripped.trim_start();
            if !rest.starts_with("let ") {
                // Only within the condition: a `let` inside the block belongs
                // to the block, not to the statement that opens it.
                let head = &rest[..rest.find('{').unwrap_or(rest.len())];
                if let Some(at) = head.find("&& let ") {
                    rest = rest[at + "&& ".len()..].trim_start();
                }
            }
        }
    }
    if rest.starts_with("match ") {
        return match_arm_binder(rest);
    }
    let mut rest = rest.strip_prefix("let ")?.trim_start();
    for pattern in ["Some(", "Ok("] {
        if let Some(stripped) = rest.strip_prefix(pattern) {
            rest = stripped.trim_start();
        }
    }
    if let Some(stripped) = rest.strip_prefix("mut ") {
        rest = stripped.trim_start();
    }
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() || name == "_" {
        None
    } else {
        Some(name)
    }
}

/// The identifiers the value produced at `idx` flows into, transitively.
///
/// A value is followed rather than a spelling searched for: whatever
/// `approval_summary` returns is bound, may be rebound (`let Some(s) = summary
/// else ..`), and may be carried inside another value (`let approval =
/// ReviewResponse { summary, .. }`). One of those names has to appear in the
/// statement that hands text to GitHub, or what is published came from
/// somewhere else.
///
/// The search for further bindings starts at the end of the seam *call*, not at
/// the end of the statement enclosing it. When the seam is scrutinised by an
/// `if let` rather than bound by a `let`, that statement is the whole block —
/// and everything the derived value is carried through is inside it, so a
/// search beginning after the block finds nothing and the publisher is accused
/// of publishing something else.
fn value_aliases(body: &str, call: &Call) -> Vec<String> {
    let idx = call.idx;
    let mut aliases: Vec<String> = Vec::new();
    if let Some(name) = binder(&body[statement_start(body, idx)..statement_end(body, idx)]) {
        aliases.push(name);
    }
    if aliases.is_empty() {
        return aliases;
    }
    let mut from = char_boundary_at_or_after(body, call.close + 1);
    while let Some(off) = body[from..].find("let ") {
        let at = from + off;
        from = at + 4;
        let stmt = &body[at..statement_end(body, at)];
        let Some(name) = binder(stmt) else { continue };
        if aliases.contains(&name) {
            continue;
        }
        let rhs = stmt.split_once('=').map(|(_, r)| r).unwrap_or("");
        if aliases.iter().any(|a| mentions_ident(rhs, a)) {
            aliases.push(name);
        }
    }
    aliases
}

/// Every value a struct literal in `text` gives to the field `name`, as
/// (offset of the field, the value).
///
/// The value runs to the comma that closes the field, so a nested call or
/// struct is kept whole.
fn struct_field_values(text: &str, name: &str) -> Vec<(usize, String)> {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(off) = text[from..].find(name) {
        let at = from + off;
        from = at + name.len();
        if text[..at].chars().next_back().is_some_and(is_word) {
            continue;
        }
        let rest = &text[from..];
        let trimmed = rest.trim_start();
        if !trimmed.starts_with(':') || trimmed.starts_with("::") {
            continue;
        }
        let start = text.len() - trimmed.len() + 1;
        let mut depth = 0i32;
        let mut end = text.len();
        for (i, c) in text[start..].char_indices() {
            match c {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' if depth == 0 => {
                    end = start + i;
                    break;
                }
                ')' | ']' | '}' => depth -= 1,
                ',' if depth == 0 => {
                    end = start + i;
                    break;
                }
                _ => {}
            }
        }
        out.push((at, text[start..end].trim().to_string()));
    }
    out
}

/// One place in production source where a pull request is handed to the merge
/// queue.
struct MergeQueueDoor {
    file: String,
    line: usize,
    /// The whole file's production code, so the call can be measured in place.
    code: String,
    call: Call,
}

impl MergeQueueDoor {
    fn at(&self) -> String {
        format!("{}:{}", self.file, self.line)
    }

    fn arguments(&self) -> Vec<String> {
        call_arguments(&self.code, &self.call)
    }

    fn statement(&self) -> String {
        statement(&self.code, self.call.idx)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// The files that hold a door today. Named rather than counted, because the
/// failure mode this pins is a door *disappearing* from the scan: a shrinking
/// list of offenders reads like progress.
const KNOWN_DOOR_FILES: [&str; 4] = [
    "src/cli/enlist.rs",
    "src/queue_healer.rs",
    "src/webhook/manual_handlers.rs",
    "src/webhook/pipelines/review.rs",
];

/// Every call to `enlist_into_merge_queue`, excluding its own definition and
/// any mention of it inside a comment, a string literal or a test module.
fn merge_queue_doors() -> Vec<MergeQueueDoor> {
    const NEEDLE: &str = "enlist_into_merge_queue(";
    let mut doors = Vec::new();
    for rel in rust_sources_under("src") {
        let lines = production_lines(&rel);
        let text = lines.join("\n");
        let mut from = 0usize;
        while let Some(call) = find_call(&text, NEEDLE, from) {
            from = call.idx + NEEDLE.len();
            // A call, not the declaration: the identifier is reached through
            // `.`, ignoring the whitespace and newlines rustfmt puts in a
            // method chain.
            if !text[..call.idx].trim_end().ends_with('.') {
                continue;
            }
            let line = text[..call.idx].matches('\n').count() + 1;
            doors.push(MergeQueueDoor {
                file: rel.clone(),
                line,
                code: text.clone(),
                call,
            });
        }
    }

    let found: Vec<&str> = doors.iter().map(|d| d.file.as_str()).collect();
    let missing: Vec<&str> = KNOWN_DOOR_FILES
        .iter()
        .copied()
        .filter(|f| !found.contains(f))
        .collect();
    assert!(
        missing.is_empty(),
        "the merge queue door scan no longer sees a door in {missing:?}, and found \
         only {found:?}.\n\
         Either those doors were deleted — in which case delete them from \
         KNOWN_DOOR_FILES and say so — or the scan has been blinded and every \
         test built on it is now reporting on a corpus with a hole in it. A door \
         must fail this test when it vanishes, not quietly stop being counted."
    );
    doors
}

/// Source that defines or feeds what Anvil publishes onto a pull request:
/// `merge_enlister.rs` and every module it pulls a name from.
///
/// That is the reachability P6 needs. A blanket sentence moved into a `const`,
/// a helper or a sibling file is still on this path, because the path has to
/// reference the file to publish what is in it. Deliberately not "every file
/// mentioning enlist": the doors are covered by the door and discard scans, and
/// a gate reporting its own measured percentage is not this lane's business.
fn published_text_files() -> Vec<String> {
    const SEED: &str = "src/merge_enlister.rs";
    let mut files = vec![SEED.to_string()];
    let source = production_source(SEED);
    let mut from = 0usize;
    while let Some(off) = source[from..].find("crate::") {
        let idx = from + off + "crate::".len();
        from = idx;
        let path: String = source[idx..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':')
            .collect();
        let mut prefix = String::from("src");
        for segment in path.split("::").filter(|s| !s.is_empty()) {
            prefix.push('/');
            prefix.push_str(segment);
            for candidate in [format!("{prefix}.rs"), format!("{prefix}/mod.rs")] {
                if repo_path(&candidate).is_file() && !files.contains(&candidate) {
                    files.push(candidate);
                }
            }
        }
    }
    files.sort();
    files
}

// =========================================================================
// Issue #17 — the merge queue admits nothing on evidence Anvil does not have
// =========================================================================

/// P2. A caller that could not obtain a report holds no evidence at all, which
/// is the one case the current code cannot express: three of the four doors
/// never had a report to begin with and enlisted anyway.
#[test]
fn evidence_that_was_never_obtained_does_not_admit_a_pull_request() {
    let refusal = MergeEnlister::admission_refusal(None);
    let err = refusal.expect_err(
        "a caller with no certification report must not enlist: absent evidence is \
         not permission",
    );
    assert!(
        !err.to_string().trim().is_empty(),
        "the refusal must say why; a blank refusal is a silent no-op with extra steps"
    );
}

/// P1 and P3. `is_certified_ready` and `is_admissible` differ on exactly this
/// report, and the difference is the whole of invariant I1. The refusal must
/// also name the gate, or an operator watching a pull request sit in limbo has
/// nothing to act on.
#[test]
fn a_certified_report_with_an_unmeasured_gate_is_refused_and_the_gate_is_named() {
    let report = certified_with_an_unmeasured_gate();

    assert!(
        report.is_certified_ready,
        "fixture sanity: NotMeasured is individually acceptable, so this report \
         still certifies — that is why the two predicates exist"
    );
    assert!(
        !report.is_admissible(),
        "fixture sanity: but it is not admissible"
    );

    let err = MergeEnlister::admission_refusal(Some(&report))
        .expect_err("a gate that produced no measurement must withhold the merge");
    assert!(
        err.to_string().contains("kani_status"),
        "the refusal must name the gate that produced no measurement; got: {err}"
    );
}

/// A report carrying a failed gate is not certified and must not be admitted.
/// Distinct from the test above: an implementation that checks only
/// `unmeasured_gates.is_empty()` passes that one and fails this.
#[test]
fn a_report_with_a_failed_gate_is_refused() {
    let mut report = every_gate_passing();
    report.coverage_status = GateStatus::Failed("coverage below the ratchet".into());
    seal_like_a_run(&mut report);

    let err = MergeEnlister::admission_refusal(Some(&report))
        .expect_err("a failing gate must withhold the merge");
    assert!(
        !err.to_string().trim().is_empty(),
        "the refusal must say why"
    );
}

/// P9, on the admission side. `unmeasured_gates` records `NotMeasured` only, so
/// a report that claims certification while a gate `Errored` slips through
/// `is_admissible()` untouched. A gate that was configured, attempted and
/// produced no result is absent evidence in exactly the sense I1 means, and
/// issue #17 names `Errored` alongside `NotMeasured` for that reason.
#[test]
fn a_report_that_certifies_while_a_gate_errored_is_still_refused() {
    let report = certified_while_a_gate_errored();

    assert!(
        report.is_admissible(),
        "fixture sanity: is_admissible() alone says yes to this report"
    );

    let err = MergeEnlister::admission_refusal(Some(&report))
        .expect_err("a gate that errored produced no measurement; it cannot admit a merge");
    let text = err.to_string();
    assert!(!text.trim().is_empty(), "the refusal must say why");
    // P9, on the side where it can still bite. A report carrying an `Errored`
    // gate is never admitted, so it is never published onto; the only place
    // left where Anvil can describe that gate as one that passed is the
    // refusal, and an operator reading "not admissible" with no gate named has
    // nothing to act on.
    assert!(
        text.contains("slo_status") || mentions_number(&text, 1),
        "the refusal must name the gate that errored, or say how many did: an \
         operator watching this pull request sit in limbo is told only that \
         something is wrong. Got: {text}"
    );
}

/// The constructor carries the STATUS it was handed, on the gate it was named for.
///
/// Every other call in this suite hands `from_gate_outcomes` a uniformly
/// passing corpus, which pins the names half of each outcome and leaves the
/// status half free: a constructor that ignored the status entirely and wrote
/// `Passed` into every slot satisfied the whole suite, and so did one whose
/// name-to-field mapping was shifted by a field. Both are invisible under
/// uniform input. A ragged corpus, read back by name, is what tells them apart.
#[test]
fn a_report_carries_back_the_status_each_named_gate_was_given() {
    let base = PreMergeCertificationReport::unmeasured("fixture baseline");
    let names: Vec<&'static str> = base.named_statuses().into_iter().map(|(n, _)| n).collect();

    let failed_on = names[3];
    let errored_on = names[11];
    let warned_on = names[29];
    let unmeasured_on = names[47];

    let ragged: Vec<(&str, GateStatus)> = names
        .iter()
        .map(|n| {
            let status = if *n == failed_on {
                GateStatus::Failed("a defect was found".to_string())
            } else if *n == errored_on {
                GateStatus::Errored("the probe could not run".to_string())
            } else if *n == warned_on {
                GateStatus::Warning("worth a look".to_string())
            } else if *n == unmeasured_on {
                GateStatus::NotMeasured {
                    gate_id: (*n).to_string(),
                    reason: "no telemetry endpoint configured".to_string(),
                }
            } else {
                GateStatus::Passed
            };
            (*n, status)
        })
        .collect();

    let report = PreMergeCertificationReport::from_gate_outcomes(&ragged)
        .expect("a full, well-formed corpus is exactly what a run hands over");

    let back: std::collections::HashMap<&str, &GateStatus> =
        report.named_statuses().into_iter().collect();

    assert!(
        matches!(back.get(failed_on), Some(GateStatus::Failed(_))),
        "{failed_on} was handed Failed and came back {:?}. A constructor that \
         writes Passed into every slot, or one whose name-to-field mapping is \
         shifted, reads exactly like this",
        back.get(failed_on)
    );
    assert!(
        matches!(back.get(errored_on), Some(GateStatus::Errored(_))),
        "{errored_on} was handed Errored and came back {:?}",
        back.get(errored_on)
    );
    assert!(
        matches!(back.get(warned_on), Some(GateStatus::Warning(_))),
        "{warned_on} was handed Warning and came back {:?}",
        back.get(warned_on)
    );
    assert!(
        matches!(
            back.get(unmeasured_on),
            Some(GateStatus::NotMeasured { .. })
        ),
        "{unmeasured_on} was handed NotMeasured and came back {:?}",
        back.get(unmeasured_on)
    );

    assert!(
        !report.is_certified_ready,
        "a corpus carrying a Failed and an Errored gate is not certified"
    );
    assert_eq!(
        report.unmeasured_gates.len(),
        1,
        "one gate was NotMeasured; unmeasured_gates should name it and only it, \
         and it named {:?}",
        report.unmeasured_gates
    );
    assert!(
        report
            .unmeasured_gates
            .iter()
            .any(|g| g.contains(unmeasured_on) || unmeasured_on.contains(g.as_str())),
        "unmeasured_gates should name {unmeasured_on}, and it holds {:?}",
        report.unmeasured_gates
    );
    assert_ne!(
        (report.gate_counts().passed, report.gate_counts().failed),
        (TOTAL_GATES, 0),
        "a ragged corpus counted as a clean sweep means the counts are derived \
         from something other than the statuses"
    );

    assert!(
        MergeEnlister::admission_refusal(Some(&report)).is_err(),
        "this report carries a Failed, an Errored and an unmeasured gate. \
         Admitting it is admitting on evidence Anvil does not have"
    );
}

/// P5 at its own seam: the provenance mark is only worth what the constructor
/// that confers it demands in return.
///
/// `from_gate_outcomes` is the one way gate outcomes enter a report and the one
/// mark `admission_refusal` can trust absolutely, because no caller can type
/// it. Everything else in this suite hands it the whole corpus and reads the
/// report back, which leaves the overlay spelling green:
///
/// ```ignore
/// let mut r = Self { doc_parity_status: GateStatus::Passed, /* ..71 more.. */ };
/// for (name, s) in outcomes { r.set(name, s.clone()); }
/// r.seal();
/// Ok(r)
/// ```
///
/// Seeded from an optimistic skeleton rather than from `unmeasured`. Handed all
/// seventy-two outcomes the skeleton is entirely overwritten and nothing
/// observes it; handed one, it mints a report with seventy-one hardcoded
/// `Passed` gates, a real provenance mark, `is_admissible() == true` and an
/// `admission_refusal` that says `Ok`. That is worse than the `optimistic`
/// fabricator the forgery scan was built to kill, because it carries the mark
/// nothing else can forge — and
/// `every_door_hands_the_merge_queue_evidence_a_certification_run_produced`
/// cannot see it either: the door traces to a genuine certification run, and
/// the hole is inside the constructor that run uses.
///
/// So the corpus is the obligation. An outcome for every gate, named once, or
/// no report. The positive case is kept beside the three refusals so the rule
/// cannot be satisfied by refusing everything — a constructor that never
/// answers `Ok` stops the fleet as surely as one that always does admits it.
#[test]
fn a_report_is_not_produced_from_gate_outcomes_that_do_not_cover_the_corpus() {
    let base = PreMergeCertificationReport::unmeasured("fixture baseline");
    let names: Vec<&'static str> = base.named_statuses().into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        names.len(),
        TOTAL_GATES,
        "this test must cover the whole corpus; it found {} named gates against \
         TOTAL_GATES={}",
        names.len(),
        TOTAL_GATES
    );
    let full: Vec<(&str, GateStatus)> = names.iter().map(|n| (*n, GateStatus::Passed)).collect();

    assert!(
        PreMergeCertificationReport::from_gate_outcomes(&full).is_ok(),
        "one outcome per gate is exactly what a certification run hands over. A \
         constructor that refuses the whole corpus refuses everything, and the \
         rule below would be satisfied by a function that never produces a \
         report at all"
    );

    let nothing: [(&str, GateStatus); 0] = [];
    let err = PreMergeCertificationReport::from_gate_outcomes(&nothing).expect_err(
        "no gate outcome at all is not a report on a passing pull request, it is \
         no report. Answering `Ok` here hands back a corpus nobody measured, \
         carrying the one mark that says somebody did",
    );
    assert!(
        !err.to_string().trim().is_empty(),
        "the refusal must say why"
    );

    let dropped = names[names.len() / 2];
    let short: Vec<(&str, GateStatus)> = full
        .iter()
        .filter(|(name, _)| *name != dropped)
        .cloned()
        .collect();
    assert_eq!(
        short.len(),
        TOTAL_GATES - 1,
        "fixture sanity: this is the whole corpus with `{dropped}` taken out"
    );
    let err = PreMergeCertificationReport::from_gate_outcomes(&short).expect_err(
        "a report with a gate missing is a report with a hole in it, and every \
         predicate downstream reads the hole as whatever the constructor seeded \
         it with rather than as something nothing measured",
    );
    // Named under either of the two names Anvil has for a gate: the field it is
    // carried under and the label it is published under.
    let text = err.to_string();
    let label = label_for(dropped).map(|(l, _)| l).unwrap_or("");
    assert!(
        text.contains(dropped) || (!label.is_empty() && text.contains(label)),
        "the refusal does not say which gate is missing: it names `{dropped}` \
         neither by its field nor as `{label}`, so a caller is told a report \
         could not be built and cannot discover what was not measured. Error \
         was:\n{text}"
    );

    // The right number of outcomes is not the same question as the right
    // outcomes. Seventy-two entries naming one gate twice is a seventy-one gate
    // report with a hole, and a length check alone waves it through.
    let mut duplicated = short.clone();
    duplicated.push((names[0], GateStatus::Passed));
    assert_eq!(
        duplicated.len(),
        TOTAL_GATES,
        "fixture sanity: the corpus count is right and the coverage is not"
    );
    assert!(
        PreMergeCertificationReport::from_gate_outcomes(&duplicated).is_err(),
        "`{}` was named twice and `{dropped}` not at all, and the report came \
         back anyway. Whatever the second mention overwrote, nothing measured \
         `{dropped}` — and the report now carries the mark that says a run did",
        names[0]
    );

    let mut unknown = short.clone();
    unknown.push((
        "a_gate_that_is_not_in_the_corpus_status",
        GateStatus::Passed,
    ));
    assert!(
        PreMergeCertificationReport::from_gate_outcomes(&unknown).is_err(),
        "an outcome for a gate that does not exist was accepted in place of the \
         one for `{dropped}`, which does. An unrecognised name silently dropped \
         is how a renamed gate stops being measured without anything saying so"
    );
}

/// P5, in process, and the reason a report has to know where its statuses came
/// from.
///
/// The two reports here say exactly the same thing about all seventy-two gates.
/// One of them was produced from gate outcomes and one of them was typed. No
/// predicate reading the statuses can separate them — `is_admissible()` says
/// yes to both, `gate_counts()` scores both at the whole corpus — and no source
/// scan can be relied on to, because the fabricator only has to avoid a
/// spelling: a helper *named* `evaluate_pre_merge_gates_from_cache` walked
/// through the door scan untouched while every pull request in the fleet merged
/// on a report zero gates produced.
///
/// So provenance is asked of the value. What a caller cannot type, a caller
/// cannot forge.
#[test]
fn a_report_no_certification_run_produced_is_refused_however_well_it_reads() {
    let forged = forged_all_passing();
    assert!(
        forged.is_admissible(),
        "fixture sanity: is_admissible() reads the statuses and says yes — this is \
         the predicate the fabricator was built to satisfy"
    );
    assert_eq!(
        (forged.gate_counts().passed, forged.gate_counts().failed),
        (TOTAL_GATES, 0),
        "fixture sanity: nothing that counts gates can tell this report from one \
         the run produced. Provenance is the only thing that separates them"
    );

    let err = MergeEnlister::admission_refusal(Some(&forged)).expect_err(
        "a report no certification run produced is the caller's opinion in the \
         shape of evidence, and admitting on it is issue #17 behind a well-typed \
         argument",
    );
    assert!(
        !err.to_string().trim().is_empty(),
        "the refusal must say why"
    );

    let measured = every_gate_passing();
    assert_eq!(
        measured.gate_counts(),
        forged.gate_counts(),
        "fixture sanity: the two reports say the same thing about every gate"
    );
    assert!(
        MergeEnlister::admission_refusal(Some(&measured)).is_ok(),
        "the symmetric half: a report that *did* come from gate outcomes, saying \
         the same thing as the one just refused, must still be admitted. A \
         provenance check that refuses both stops the fleet"
    );
}

/// P4. False-red prevention. A precondition that refuses everything satisfies
/// every test above and stops the fleet.
#[test]
fn a_fully_measured_and_certified_report_admits_the_pull_request() {
    let report = every_gate_passing();
    assert!(
        MergeEnlister::admission_refusal(Some(&report)).is_ok(),
        "a certified, fully measured pull request must still reach the merge queue; \
         refusing on present evidence is the symmetric violation of I1"
    );
}

/// A repo spec the `gh` CLI rejects locally — four path segments — so that a
/// version of `enlist_into_merge_queue` which does *not* refuse fails on its
/// first subprocess rather than reaching GitHub. No test in this suite opens a
/// socket, in either the red state or the green one.
const NO_SUCH_REPO: &str = "anvil-spec/there/is/no/such/repo";

/// P1, P2, P9, and the reason the entry point takes the evidence at all.
///
/// The three reports below are the three shapes of absent evidence: none at
/// all, a gate that was never measured, and a gate that tried and errored. Each
/// must come back refused *from the entry point*, with the refusal the seam
/// gives for it — not from a helper that may or may not have been reached, and
/// not from `gh`.
///
/// This is what a source scan could not establish. A guard written inside
/// `if let Some(r) = self.cached_report(..).await { .. }` reads as present to
/// any window and admits everything; here it simply fails, because `None` goes
/// in and something other than a refusal comes back.
#[tokio::test]
async fn the_merge_queue_entry_point_refuses_the_evidence_it_was_handed() {
    let unmeasured = certified_with_an_unmeasured_gate();
    let errored = certified_while_a_gate_errored();
    let cases: [(&str, Option<&PreMergeCertificationReport>); 3] = [
        ("no certification report at all", None),
        (
            "a report that certifies while a gate produced no measurement",
            Some(&unmeasured),
        ),
        (
            "a report that certifies while a gate errored",
            Some(&errored),
        ),
    ];

    let github = Arc::new(GitHubClient::new());
    let enlister = MergeEnlister::new(github.clone());
    let mut refusals: Vec<String> = Vec::new();
    for (what, report) in cases {
        // Taken first: while the seam is unimplemented this panics here, at the
        // seam, rather than shelling out.
        let refusal = match MergeEnlister::admission_refusal(report) {
            Err(refusal) => refusal.to_string(),
            Ok(()) => panic!("fixture sanity: `admission_refusal` must refuse {what}"),
        };

        let err = enlister
            .enlist_into_merge_queue(NO_SUCH_REPO, 1, report)
            .await
            .expect_err(
                "the merge queue entry point admitted a pull request it was handed \
                 no usable evidence for",
            );
        let chain = format!("{err:?}");
        assert!(
            chain.contains(&refusal),
            "`enlist_into_merge_queue` was handed {what} and did not refuse it. It \
             failed with something else, which means the admission decision is not \
             on the path this call took:\n  wanted the refusal: {refusal}\n  got: {chain}\n\
             Invariant I1 — absent evidence must never merge — has to hold for the \
             call, not for a helper that may or may not have been reached."
        );
        refusals.push(refusal);
    }

    // P4, at the entry point. The reasoning that moved the refusal to the call
    // rather than the helper applies unchanged to the admitting case: a second
    // precondition bolted on beside the seam — a freshness check, a head-SHA
    // check — refuses every certified pull request in the fleet while every
    // test above stays green. So the fourth case is a report that must get
    // *past* the guard.
    //
    // No wording is pinned: what is asserted is that the failure this call
    // comes back with is not one of the three refusals collected above.
    // `NO_SUCH_REPO` is rejected by `gh` on argument parsing, so the call fails
    // locally either way, and if `gh` is absent the spawn error is not a
    // refusal either.
    let clean = every_gate_passing();
    assert!(
        MergeEnlister::admission_refusal(Some(&clean)).is_ok(),
        "fixture sanity: a certified, fully measured report is admitted by the seam"
    );

    // How far past the guard is "past the guard": an admitted pull request must
    // get as far as asking GitHub about itself. Whatever this machine answers
    // for that question — `gh` rejecting the repo spec, or no `gh` at all — is
    // the reference, so no wording is pinned and the two calls fail identically
    // for the same reason.
    let reference = github
        .fetch_pr_metadata(NO_SUCH_REPO, 1)
        .await
        .expect_err("fixture sanity: `gh` cannot answer for this repo spec")
        .to_string();
    let err = enlister
        .enlist_into_merge_queue(NO_SUCH_REPO, 1, Some(&clean))
        .await
        .expect_err(
            "fixture sanity: `gh` cannot answer for this repo spec, so no call to it succeeds",
        );
    let chain = format!("{err:?}");
    for refusal in &refusals {
        assert!(
            !chain.contains(refusal.as_str()),
            "`enlist_into_merge_queue` was handed a certified, fully measured \
             report and answered it with a refusal meant for absent \
             evidence:\n  refusal: {refusal}\n  got: {chain}"
        );
    }
    assert!(
        chain.contains(&reference),
        "`enlist_into_merge_queue` was handed a certified, fully measured report \
         and never got as far as the pull request:\n  expected to reach: \
         {reference}\n  got: {chain}\n\
         I1 cuts both ways. Absent evidence is not a pass and present evidence is \
         not an accusation: a second precondition bolted on beside the admission \
         decision — a freshness check, a head-SHA check — satisfies every refusal \
         case above, withholds every genuinely certified pull request in the \
         fleet, and does it silently."
    );
}

/// P5. The entry point can only refuse on what it is given. A door that hands
/// it `None` for ever has closed the queue rather than guarded it; a door that
/// hands it a report the door itself wrote has reopened issue #17 behind a
/// well-typed argument.
///
/// Three questions, asked of provenance rather than of vocabulary, because a
/// fixed list of banned spellings is a list a hurried engineer walks around
/// without meaning to. A free `optimistic(reason)` next to the type in
/// `report.rs`, round-tripping `unmeasured` through serde with every status set
/// to `Passed`, names no banned construct at the door and sits in the one
/// directory the forgery scan used to exempt wholesale — and every door calling
/// it admits every pull request on a report that zero gates produced.
///
/// So: no function that produces a report may certify without consuming what a
/// gate measured; no file outside the guard may write gate statuses at all; and
/// the value each door hands over must trace back to the certification run.
#[test]
fn every_door_hands_the_merge_queue_evidence_a_certification_run_produced() {
    // Calibration, first, because a scan that reads the wrong thing is worse
    // than no scan: it accuses the fix. `PreMergeCertificationReport {` matches
    // the *signature* rustfmt writes for the shared helper this test's own
    // failure message asks for. These four cases are green today and stay green
    // by being here; the rest of the test is red until the doors carry evidence.
    for literal in [
        "let r = PreMergeCertificationReport { is_certified_ready: false };",
        "Ok(PreMergeCertificationReport {",
        "return crate::pre_merge_guard::report::PreMergeCertificationReport {",
    ] {
        assert!(
            builds_a_report_by_hand(literal),
            "the forgery scan no longer reads a struct literal as one: {literal}"
        );
    }
    for signature in [
        "fn certification_for(state: &AppState) -> PreMergeCertificationReport {",
        "fn certification_for(state: &AppState) -> Result<PreMergeCertificationReport> {",
        "async fn certification_for(s: &AppState) \
         -> crate::pre_merge_guard::report::PreMergeCertificationReport {",
        "fn render(report: &PreMergeCertificationReport) -> String {",
        "impl PreMergeCertificationReport {",
        "pub struct PreMergeCertificationReport {",
    ] {
        assert!(
            !builds_a_report_by_hand(signature),
            "the forgery scan reads a function that *returns* a report as one \
             that builds one, and would accuse the fix this test demands — run \
             the certification on this path and hand over what it produced — of \
             forging the evidence it just obtained: {signature}"
        );
    }
    // The same calibration for the rule below on who may hand out a report.
    // Being told what a gate measured is one way to be entitled to produce one;
    // running the gates is the other, and a parameter list cannot see it. The
    // second case is the round's own fabricator: named after the run, reaching
    // nothing that measures anything.
    for (expected, code) in [
        (
            true,
            "async fn certify_pull_request(g: &PreMergeGuard, d: &PrDiffContext) \
             -> Result<PreMergeCertificationReport> { evaluate_pre_merge_gates(g, d).await }",
        ),
        (
            false,
            "fn evaluate_pre_merge_gates_from_cache(reason: &str) \
             -> PreMergeCertificationReport { \
             let base = PreMergeCertificationReport::unmeasured(reason); \
             serde_json::from_value(all_passing(&base)).unwrap() }",
        ),
    ] {
        let name = code[code.find("fn ").unwrap() + 3..]
            .split('(')
            .next()
            .unwrap()
            .to_string();
        assert_eq!(
            runs_the_certification(code, &name, ""),
            expected,
            "the exemption for a function that runs the gates reads `{name}` \
             wrongly. Read too narrowly it accuses the extraction the three \
             ungated doors need; read too widely it exempts a helper merely \
             named after the run"
        );
    }

    let doors = merge_queue_doors();
    assert!(
        !doors.is_empty(),
        "this scan found no call to `enlist_into_merge_queue`. Either the merge \
         queue entry point was renamed and this test must follow it, or the scan \
         is broken — a mechanism that cannot find its subject reports nothing \
         wrong with anything"
    );

    let everywhere = all_production_source();

    // I2, across the whole guard rather than beside the type. `report.rs` said
    // in a comment that "there is deliberately no 'all passed' constructor" and
    // nothing enforced it; when something finally did, the cheapest place to
    // fabricate evidence moved one file over.
    let producers: Vec<(String, String, String)> = rust_sources_under("src/pre_merge_guard")
        .into_iter()
        .flat_map(|rel| {
            report_producing_fns(&rel)
                .into_iter()
                .map(move |(name, params)| (rel.clone(), name, params))
        })
        .collect();
    for required in ["unmeasured", CERTIFICATION_RUN] {
        assert!(
            producers.iter().any(|(_, name, _)| name == required),
            "this scan found no `{required}` among the functions under \
             src/pre_merge_guard/ that produce a certification report, and found \
             only {:?}. Either it was renamed and this test must follow it, or \
             the scan is broken and would report nothing wrong with anything",
            producers
                .iter()
                .map(|(_, n, _)| n.as_str())
                .collect::<Vec<_>>()
        );
    }
    let fabricators: Vec<String> = producers
        .iter()
        .filter(|(rel, name, params)| {
            name != "unmeasured"
                && name != CERTIFICATION_RUN
                && !params.contains("GateStatus")
                && !runs_the_certification(&production_source(rel), name, &everywhere)
        })
        .map(|(rel, name, params)| {
            format!(
                "{rel}: {name}({})",
                params.split_whitespace().collect::<Vec<_>>().join(" ")
            )
        })
        .collect();
    assert!(
        fabricators.is_empty(),
        "these functions hand out a certification report without being told what \
         any gate measured:\n{}\n\
         A report is what a certification run produced; a constructor that fills \
         the corpus in from a reason string produces the caller's opinion in the \
         shape of evidence, and `admission_refusal` cannot tell the difference. \
         Exempt: `{CERTIFICATION_RUN}`, which is the run; anything that reaches \
         it, which is running it; a constructor told what a gate measured; and \
         `unmeasured`, which can admit nothing.",
        fabricators.join("\n")
    );
    assert!(
        !PreMergeCertificationReport::unmeasured("nothing ran").is_admissible(),
        "`unmeasured` is exempt from the rule above only because it is \
         inadmissible by construction. It is not any more, so the exemption is \
         now a hole: either restore it or take a gate outcome like every other \
         constructor."
    );

    let forged: Vec<String> = rust_sources_under("src")
        .into_iter()
        .filter_map(|rel| code_that_may_not_produce_a_report(&rel).map(|code| (rel, code)))
        .flat_map(|(rel, code)| {
            let mut hits = Vec::new();
            for (needle, what) in [
                (
                    "PreMergeCertificationReport::",
                    "calls a report constructor",
                ),
                (".seal()", "seals a report"),
                (
                    ".recompute_unmeasured()",
                    "rewrites a report's gate summary",
                ),
            ] {
                if code.contains(needle) {
                    hits.push(format!("{rel}: {what} (`{needle}`)"));
                }
            }
            if builds_a_report_by_hand(&code) {
                hits.push(format!("{rel}: builds a report by hand"));
            }
            if assigns_a_gate_status(&code) {
                hits.push(format!("{rel}: assigns a gate status to a report"));
            }
            if code.contains("is_certified_ready =") && !code.contains("is_certified_ready ==") {
                hits.push(format!("{rel}: assigns its own certification verdict"));
            }
            hits
        })
        .collect();
    assert!(
        forged.is_empty(),
        "these production files produce a certification report rather than \
         receiving one:\n{}\n\
         A report a caller wrote is not evidence, it is the caller's opinion in \
         the shape of evidence, and `admission_refusal` cannot tell the \
         difference. Gate statuses come from the certification run. Exempt: \
         `report.rs`, which is the type, and `{CERTIFICATION_RUN}`, which is the \
         run.",
        forged.join("\n")
    );

    let empty_handed: Vec<String> = doors
        .iter()
        .filter_map(|d| {
            let args = d.arguments();
            let why = match args.get(2) {
                None => "hands over no evidence at all".to_string(),
                Some(a) if a == "None" => "hands over `None`".to_string(),
                Some(a) if !evidence_is_certified(d, a, &everywhere) => {
                    format!("hands over `{a}`, which does not come from `{CERTIFICATION_RUN}`")
                }
                Some(_) => return None,
            };
            Some(format!("{} — {why}\n    {}", d.at(), d.statement()))
        })
        .collect();
    assert!(
        empty_handed.is_empty(),
        "these paths hand a pull request to the merge queue without evidence a \
         certification run produced:\n{}\n\
         The entry point refuses what it is handed nothing for, so a `None` here \
         is fail-closed and safe — and it is also a door that can never admit \
         anything again. A report that came from anywhere else is worse: it is \
         well-typed, it is not a measurement, and the predicate cannot tell. Run \
         the certification on this path and hand over what it produced, or \
         delete the door.",
        empty_handed.join("\n")
    );
}

/// The one function that turns gate outcomes into a certification report. What
/// a door hands over is evidence when it came from here and an opinion
/// otherwise.
const CERTIFICATION_RUN: &str = "evaluate_pre_merge_gates";

/// Whether the value a door hands over as evidence traces back to the
/// certification run, through the bindings — and the helpers — of its own file.
///
/// The argument at the call is only the last step:
/// `let evidence = optimistic("cargo check was clean");` one line above satisfies
/// any check made on the spelling at the door.
///
/// What is looked for is a *call*, not the name. A plain substring test is
/// answered by a helper merely *named* `evaluate_pre_merge_gates_from_cache`,
/// which runs no gate at all — and that is the shape a hurried engineer reaches
/// for, because it reads like the thing it is standing in for. This predicate
/// is the cheapest of the three checks on provenance and the easiest to walk
/// around; it is a backstop over the source, behind the forgery scan above and
/// behind `a_report_no_certification_run_produced_is_refused_however_well_it_reads`,
/// which asks the value itself and cannot be answered by any spelling.
fn evidence_is_certified(door: &MergeQueueDoor, argument: &str, everywhere: &str) -> bool {
    let mut queue = identifiers(argument);
    let mut seen: Vec<String> = Vec::new();
    while let Some(name) = queue.pop() {
        if seen.contains(&name) {
            continue;
        }
        seen.push(name.clone());
        if name == CERTIFICATION_RUN {
            return true;
        }
        let Some(stmt) = binding_statement(&door.code, &name, door.call.idx) else {
            continue;
        };
        let rhs = stmt
            .split_once('=')
            .map(|(_, r)| r.to_string())
            .unwrap_or(stmt);
        let spliced = with_called_bodies(everywhere, &rhs, 3);
        if find_call(&spliced, &format!("{CERTIFICATION_RUN}("), 0).is_some() {
            return true;
        }
        queue.extend(identifiers(&rhs));
    }
    false
}

/// Whether the body of `name`, defined in `rel`, reaches a call to the
/// certification run — directly, or through a helper it calls.
///
/// A parameter list cannot see this, and reading only the parameter list is how
/// the rule accuses the fix it demands. The three ungated doors have to obtain
/// evidence from somewhere, and the shortest honest way to give it to them is
/// one helper that runs the gates and hands back what they produced; placed
/// beside `{CERTIFICATION_RUN}` — the obvious home for it — a signature-only
/// rule reports it as a constructor that "fills the corpus in from a reason
/// string", while accepting the identical helper anywhere outside
/// `src/pre_merge_guard/`. That is the rule dictating which file the fix may
/// live in and misdiagnosing when it does not.
///
/// The exemption is for *running* the gates, not for being named after them:
/// `evaluate_pre_merge_gates_from_cache`, which round-trips `unmeasured` into
/// all-`Passed`, calls nothing that measures anything and is still reported.
/// This is the same splice `evidence_is_certified` performs at the doors.
fn runs_the_certification(source: &str, name: &str, everywhere: &str) -> bool {
    let Some(body) = find_fn(source, &format!("fn {name}(")) else {
        return false;
    };
    let spliced = with_called_bodies(everywhere, &body, 3);
    find_call(&spliced, &format!("{CERTIFICATION_RUN}("), 0).is_some()
}

/// Whether `code` builds a certification report literally, `Type { field: .. }`,
/// as opposed to naming the type somewhere a brace happens to follow.
///
/// Matching the type name followed by a brace matches a *signature*: rustfmt
/// writes `) -> PreMergeCertificationReport {`, so the natural way to write the
/// fix this test demands — one helper that runs the certification and returns
/// what it produced — is accused of forging the evidence it just obtained, by a
/// message telling the author to do the thing they did. Worse, whether it fires
/// turns on whether the return type happens to be wrapped:
/// `-> Result<PreMergeCertificationReport>` slips through and the bare one does
/// not. A rule that turns on that is noise. So the brace has to be in
/// expression position.
fn builds_a_report_by_hand(code: &str) -> bool {
    const TYPE: &str = "PreMergeCertificationReport";
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut from = 0usize;
    while let Some(off) = code[from..].find(TYPE) {
        let at = from + off;
        from = at + TYPE.len();
        if code[..at].chars().next_back().is_some_and(is_word) {
            continue;
        }
        if !code[from..].trim_start().starts_with('{') {
            continue;
        }
        // Through any path qualification, so a fully spelled-out literal reads
        // the same as a bare one.
        let mut before = code[..at].trim_end();
        while let Some(head) = before.strip_suffix("::") {
            let head = head.trim_end();
            before = head[..head.len() - head.chars().rev().take_while(|c| is_word(*c)).count()]
                .trim_end();
        }
        if before.ends_with("->") || before.ends_with(':') {
            continue;
        }
        let word: String = before
            .chars()
            .rev()
            .take_while(|c| is_word(*c))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if [
            "impl", "struct", "enum", "trait", "for", "as", "dyn", "type",
        ]
        .contains(&word.as_str())
        {
            continue;
        }
        return true;
    }
    false
}

/// The production code of `rel` that may not bring a certification report into
/// being, or `None` when the whole file is exempt.
///
/// Inside `src/pre_merge_guard/`, `report.rs` *is* the type — its own methods
/// are its definition, not a caller's forgery — and `evaluate_pre_merge_gates`
/// is the run that turns gate outcomes into a report. Every other line under
/// that directory is held to the same rule as every line outside it. Exempting
/// the directory wholesale is what let an all-`Passed`-from-a-reason-string
/// constructor sit one file over from the one the docstring names, unseen by
/// both scans, while every pull request in the fleet merged on it.
fn code_that_may_not_produce_a_report(rel: &str) -> Option<String> {
    if rel == "src/pre_merge_guard/report.rs" {
        return None;
    }
    let mut code = production_source(rel);
    if let Some(run) = find_fn(&code, &format!("fn {CERTIFICATION_RUN}(")) {
        code = code.replace(&run, "");
    }
    Some(code)
}

/// Every function in `rel` that produces a `PreMergeCertificationReport`, as
/// (name, parameter list).
///
/// `-> Self` counts only inside `impl PreMergeCertificationReport`. Read across
/// the whole guard, a bare `Self` is whatever the enclosing `impl` is about,
/// and the evaluator's own `new()` is not a certification report.
fn report_producing_fns(rel: &str) -> Vec<(String, String)> {
    let source = production_source(rel);
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(off) = source[from..].find("fn ") {
        let at = from + off;
        from = at + 3;
        if source[..at]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            continue;
        }
        let name: String = source[at + 3..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            continue;
        }
        let Some(call) = find_call(&source, &format!("{name}("), at) else {
            continue;
        };
        let Some(brace) = source[call.close..].find('{') else {
            continue;
        };
        let returns = &source[call.close + 1..call.close + brace];
        let self_is_the_report = source[..at]
            .rfind("impl ")
            .and_then(|i| source[i..].lines().next())
            .is_some_and(|l| l.contains("PreMergeCertificationReport"));
        if !returns.contains("PreMergeCertificationReport")
            && !(self_is_the_report && returns.contains("Self"))
        {
            continue;
        }
        out.push((name, source[call.open + 1..call.close].to_string()));
    }
    out
}

/// Whether `code` assigns to a `*_status` field reached through a `.`, which is
/// how a caller overwrites what a gate measured. A bare local named
/// `conflict_status` is not that.
fn assigns_a_gate_status(code: &str) -> bool {
    let mut from = 0usize;
    while let Some(off) = code[from..].find("_status") {
        let at = from + off;
        from = at + "_status".len();
        let start = code[..at]
            .rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
            .map(|i| i + 1)
            .unwrap_or(0);
        if start == 0 || code.as_bytes()[start - 1] != b'.' {
            continue;
        }
        let rest = code[from..].trim_start();
        if rest.starts_with('=') && !rest.starts_with("==") {
            return true;
        }
    }
    false
}

/// P3, at the call site. `POST /api/enlist` binds the enlistment to `_` inside
/// a detached task, so the refusal is not merely unreachable by the requester —
/// it is not written down anywhere at all.
///
/// This test claims exactly that much and no more: the `Result` of the
/// enlistment is not thrown away where it is produced. Whether the refusal then
/// reaches the person who asked for the enlistment is a separate question, and
/// `the_enlist_api_does_not_answer_success_for_an_enlistment_it_has_not_performed`
/// is where it is asked.
#[test]
fn no_path_drops_a_merge_queue_refusal_on_the_floor() {
    let doors = merge_queue_doors();
    assert!(
        !doors.is_empty(),
        "this scan found no call to `enlist_into_merge_queue`; see \
         `every_door_hands_the_merge_queue_evidence_a_certification_run_produced`"
    );

    let discarded: Vec<String> = doors
        .iter()
        .filter(|d| discards_call_result(&d.code, &d.call))
        .map(|d| format!("{} — {}", d.at(), d.statement()))
        .collect();

    assert!(
        discarded.is_empty(),
        "these paths throw away the outcome of merge queue enlistment:\n{}\n\
         Handle it: `?`, a `bail!`, or an `if let Err(e) = .. {{ warn!(..) }}` all \
         count. `let _ =`, `drop(..)`, `.ok()` and `.unwrap_or*` on the call's own \
         result do not — thrown away, a refusal is a silent no-op and nothing \
         downstream can tell a withheld pull request from an admitted one.",
        discarded.join("\n")
    );
}

/// What `no_path_drops_a_merge_queue_refusal_on_the_floor` must read as a
/// discard, and what it must not.
///
/// Calibrated in both directions, on literals, before it is pointed at
/// production. A scan that misses the launder passes a door that swallows its
/// refusal; a scan that reads `?` or a logged `Err` as a discard accuses every
/// correct door and gets deleted. Neither failure is visible from the green
/// this test's subject reports over four call sites that are currently fine.
#[test]
fn the_discard_scan_reads_a_laundered_outcome_as_discarded_and_a_handled_one_as_handled() {
    const NEEDLE: &str = "enlist_into_merge_queue(";
    let discarded = |body: &str| {
        let call = find_call(body, NEEDLE, 0).expect("the sample calls the door");
        discards_call_result(body, &call)
    };

    for thrown_away in [
        "fn f() { let _ = e.enlist_into_merge_queue(r, n, x).await; }",
        "fn f() { drop(e.enlist_into_merge_queue(r, n, x).await); }",
        "fn f() { e.enlist_into_merge_queue(r, n, x).await.ok(); }",
        // The launder: bound, so the call's own statement reads as handled,
        // and discarded one statement later.
        "fn f() { let out = e.enlist_into_merge_queue(r, n, x).await; let _ = out; Ok(()) }",
        "fn f() { let out = e.enlist_into_merge_queue(r, n, x).await; drop(out); Ok(()) }",
    ] {
        assert!(
            discarded(thrown_away),
            "the discard scan reads this as handling the enlistment outcome, and \
             nothing in it does: {thrown_away}"
        );
    }

    for handled in [
        "fn f() { e.enlist_into_merge_queue(r, n, x).await?; Ok(()) }",
        "fn f() { e.enlist_into_merge_queue(r, n, x).await }",
        "fn f() { let out = e.enlist_into_merge_queue(r, n, x).await; out }",
        "fn f() { let out = e.enlist_into_merge_queue(r, n, x).await; \
         if let Err(e) = out { warn!(\"{e}\"); } Ok(()) }",
        // A binder discarded inside another function is not this one's outcome.
        "fn f() { let out = e.enlist_into_merge_queue(r, n, x).await; out } \
         fn g() { let out = h(); let _ = out; }",
    ] {
        assert!(
            !discarded(handled),
            "the discard scan accuses a door that handles its outcome, which is \
             how a scan gets deleted rather than fixed: {handled}"
        );
    }
}

/// P3, one layer out. The refusal not being dropped at the call site is a third
/// of the problem: `manual_enlist_handler` spawns the enlistment into a
/// detached task and answers `202 ACCEPTED` with `success: true` before the
/// task has done anything. Logging the refusal inside that task satisfies the
/// discard scan and still tells the operator — or the automation calling the
/// API — that an enlistment happened when it was refused.
///
/// Either the handler waits for the outcome and answers with it, or it stops
/// asserting an outcome it does not have.
///
/// Nothing here is measured by position, and nothing is waved through for want
/// of a match. Building the answer *above* the spawn moves the claim without
/// changing it; extracting the spawned closure into a helper — ordinary tidying,
/// not evasion — moves the enlistment one identifier away. Both used to pass,
/// the second one vacuously.
#[test]
fn the_enlist_api_does_not_answer_success_for_an_enlistment_it_has_not_performed() {
    const HANDLER: &str = "fn manual_enlist_handler(";
    const ENLIST: &str = "enlist_into_merge_queue(";
    let source = production_source("src/webhook/manual_handlers");

    let Some(body) = find_fn(&source, HANDLER) else {
        // Dropping the endpoint is an honest way to close this half of #17 —
        // but gone is not the same as moved, and a handler that merely changed
        // file still answers the request.
        let still_named: Vec<String> = rust_sources_under("src")
            .into_iter()
            .filter(|rel| production_source(rel).contains("manual_enlist_handler"))
            .collect();
        assert!(
            still_named.is_empty(),
            "`manual_enlist_handler` is no longer in src/webhook/manual_handlers.rs \
             but production code still names it, in {still_named:?}. This test must \
             follow the endpoint to wherever it now lives; a scan that stops \
             finding its subject is not a fix."
        );
        return;
    };

    // What the detached tasks do, with the bodies of the helpers they call
    // spliced in, and what the handler does on the request's own thread.
    let mut detached = String::new();
    let mut from = 0usize;
    while let Some(spawn) = find_call(&body, "tokio::spawn(", from) {
        from = spawn.close;
        detached.push('\n');
        detached.push_str(&with_called_bodies(
            &source,
            &body[spawn.open..=spawn.close],
            2,
        ));
    }
    let enlists_detached = detached.contains(ENLIST);
    let enlists_inline =
        with_called_bodies(&source, &body, 2).contains(ENLIST) && !enlists_detached;

    assert!(
        enlists_detached || enlists_inline,
        "`manual_enlist_handler` no longer reaches the merge queue at all. If the \
         endpoint was retired, delete the handler and say so; a handler that \
         answers a request it no longer acts on is the same lie with the work \
         removed."
    );

    if enlists_inline {
        // The outcome is in hand by the time the answer is written, so there is
        // nothing left for this test to hold: whatever it answers, it answers
        // about something that has happened.
        return;
    }

    // What the answer claims about the enlistment, traced back to whatever
    // produced it. `success: true` is one spelling of the defect and pinning it
    // pins nothing: `let accepted = true; .. success: accepted`,
    // `const QUEUED: bool = true`, and a helper that builds the whole response
    // all answer the same unmeasured thing, and a scan looking for one token
    // waves the other three through — in a suite whose own rule is that nothing
    // is waved through for want of a match.
    //
    // `false` is exempt. It claims nothing, and it is true of a job that has not
    // run.
    let answered = format!(
        "{body}{}",
        helper_bodies(
            &source,
            &body,
            HANDLER.trim_start_matches("fn ").trim_end_matches('(')
        )
    );
    let mut claimed: Vec<String> = Vec::new();
    for (at, value) in struct_field_values(&answered, "success") {
        if value == "false" {
            continue;
        }
        let mut origin = value.clone();
        for name in traced_values(&answered, &value, at) {
            if let Some(stmt) = binding_statement(&answered, &name, at) {
                origin.push_str("\n    <- ");
                origin.push_str(
                    stmt.split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .as_str(),
                );
            }
        }
        if !mentions_ident(&origin, ENLIST.trim_end_matches('(')) {
            claimed.push(format!("  success: {origin}"));
        }
    }
    assert!(
        claimed.is_empty(),
        "`manual_enlist_handler` detaches the enlistment and answers with a \
         success that did not come from it:\n{}\n\
         The requester is told the pull request was enlisted when it may have \
         been refused, and a refusal that reaches only a log line inside a \
         detached task has not been surfaced to anybody who asked. A literal, a \
         constant, or a value bound from neither says the same thing whatever \
         happens. Wait for the outcome and answer with it, or answer something \
         that is true of a job that has not run yet.",
        claimed.join("\n")
    );
}

// =========================================================================
// Issue #18 — Anvil endorses nothing it did not measure
// =========================================================================

/// The two seams that derive what Anvil publishes onto a pull request, so that
/// every test below reads both. The enlistment note is published onto the same
/// pull request as the approving review, by the same run, and is read by the
/// same people: holding one to derivation and the other to a word list is how
/// "Pre-Merge Certification 100% Green" survives a fix to the review body.
fn publication(
    report: Option<&PreMergeCertificationReport>,
) -> Vec<(&'static str, Option<String>)> {
    vec![
        ("approval_summary", MergeEnlister::approval_summary(report)),
        (
            "enlistment_note",
            MergeEnlister::enlistment_note(report, STRATEGY),
        ),
    ]
}

/// What Anvil actually publishes for a report: the seams that produced text.
fn published_texts(report: &PreMergeCertificationReport) -> Vec<(&'static str, String)> {
    publication(Some(report))
        .into_iter()
        .filter_map(|(seam, text)| text.map(|t| (seam, t)))
        .collect()
}

/// The honest answer when there is no report is to publish nothing. Today both
/// functions that publish receive no report and publish anyway.
#[test]
fn nothing_is_endorsed_when_nothing_was_measured() {
    for (seam, text) in publication(None) {
        assert_eq!(
            text, None,
            "`{seam}` was given no certification report and still produced text to \
             publish. With nothing to derive a claim from, Anvil must publish \
             nothing at all"
        );
    }
}

/// Publishing nothing at all is always honest, so two `None`s assert nothing.
/// Anything else must differ: a publication present on one report and absent on
/// the other has already discriminated.
fn assert_publications_differ(
    a: &PreMergeCertificationReport,
    b: &PreMergeCertificationReport,
    what: &str,
) {
    let on_a = publication(Some(a));
    let on_b = publication(Some(b));
    for ((seam, text_a), (_, text_b)) in on_a.into_iter().zip(on_b) {
        if text_a.is_none() && text_b.is_none() {
            continue;
        }
        assert_ne!(text_a, text_b, "`{seam}`: {what}");
    }
}

/// P8. The defect is not the wording, it is that the wording is a constant: the
/// same sentence is signed onto every pull request in the fleet whatever its
/// gates did. Two reports that differ must not produce one publication.
///
/// The second pair is the one that bites. Both reports are admissible, so an
/// implementation that publishes only for admissible pull requests produces a
/// sentence for each and has to make them differ — which a constant cannot do,
/// and neither can `gate_counts()`, which scores both at 72 of 72. A pair with
/// an inadmissible side lets that implementation answer `None` and assert
/// nothing.
///
/// The third comparison turns it around: one report, two merge strategies. The
/// note is written about how the pull request went into the queue as well as
/// about what was measured, and `enlistment_note` is handed that. A parameter
/// no test can make matter is scaffolding — and this one can be got wrong, on
/// the retry path, permanently, on the pull request.
#[test]
fn the_endorsement_differs_when_the_evidence_differs() {
    let clean = every_gate_passing();

    let mut ragged = every_gate_passing();
    ragged.kani_status = not_measured("kani_status");
    ragged.coverage_status = GateStatus::Failed("coverage below the ratchet".into());
    seal_like_a_run(&mut ragged);
    assert_publications_differ(
        &clean,
        &ragged,
        "the same text was published for a pull request whose gates all passed and \
         for one with a failed gate and a gate that produced no measurement. A \
         claim identical across both is derived from neither",
    );

    let mut warned = every_gate_passing();
    warned.bench_status = GateStatus::Warning("throughput regressed within tolerance".into());
    seal_like_a_run(&mut warned);
    assert!(
        warned.is_admissible(),
        "fixture sanity: a Warning is acceptable and measured, so this report is \
         still admissible — whatever the clean report is published with, this one \
         is published with too"
    );
    assert_publications_differ(
        &clean,
        &warned,
        "the same text was published for a pull request with a clean bench gate and \
         for one whose bench gate reported a warning. Both are admissible, so both \
         are published — with one constant sentence, which is issue #18 restored \
         verbatim",
    );

    // The strategy is the one field of the note that production can genuinely
    // get wrong. `enlist_into_merge_queue` retries as a merge commit whenever
    // `--squash` is refused, and `post_enlistment_note` is handed whichever one
    // happened — so a note that keeps today's welded `- **Strategy**: Squash &
    // Merge` line, or derives every gate honestly and hardcodes the strategy
    // beside it, tells the reader the pull request was squashed when it was
    // not. That is this lane's defect class exactly: a claim Anvil signs onto a
    // pull request that is not derived from what actually happened. Here the
    // report is the constant and the strategy is the variable, so a difference
    // can come from nowhere else.
    //
    // Publishing no note at all remains honest, and two `None`s assert nothing.
    let squashed = MergeEnlister::enlistment_note(Some(&clean), STRATEGY);
    let committed = MergeEnlister::enlistment_note(Some(&clean), OTHER_STRATEGY);
    if squashed.is_some() || committed.is_some() {
        assert_ne!(
            squashed, committed,
            "`enlistment_note` published the same note for a pull request merged \
             as `{STRATEGY}` and for one merged as `{OTHER_STRATEGY}`. The \
             strategy is a parameter it was handed and did not read; the note it \
             signs now says how the merge happened on the authority of nothing"
        );
        for (strategy, other, note) in [
            (STRATEGY, OTHER_STRATEGY, &squashed),
            (OTHER_STRATEGY, STRATEGY, &committed),
        ] {
            let Some(note) = note else { continue };
            let lower = note.to_lowercase();
            assert!(
                lower.contains(&strategy.to_lowercase()),
                "the note published for a pull request merged as `{strategy}` does \
                 not say so. A reader of the pull request cannot discover how it \
                 went in. Note was:\n{note}"
            );
            assert!(
                !lower.contains(&other.to_lowercase()),
                "the note published for a pull request merged as `{strategy}` says \
                 `{other}`. It is written onto the pull request permanently and it \
                 is not what happened. Note was:\n{note}"
            );
        }
    }
}

/// P8 and P10, on the only report that reaches a publication at all.
///
/// A `Warning` is measured and acceptable, so this pull request is admitted and
/// whatever Anvil writes onto it is written onto a merge that really happens.
/// It is also the only shape of imperfect evidence that can be published over:
/// `NotMeasured` and `Errored` are both refused at the door, and a `Failed`
/// report never certifies. So this carries both halves of the obligation — the
/// ban on sweeping the corpus into a total, and the positive requirement to
/// account for the gate that did not simply pass.
///
/// The positive half pins no wording: naming the gate, or saying how many were
/// not clean passes, both satisfy it. Publishing the ready-made `gate_counts()`
/// figure — which scores a `Warning` as acceptable and so reports the whole
/// corpus — does not.
#[test]
fn an_endorsement_accounts_for_the_gate_that_did_not_simply_pass() {
    let mut report = every_gate_passing();
    report.bench_status = GateStatus::Warning("throughput regressed within tolerance".into());
    seal_like_a_run(&mut report);
    assert!(
        report.is_admissible(),
        "fixture sanity: this pull request is admitted, so whatever Anvil publishes \
         onto it is published onto a merge that really happens"
    );
    let clean_passes = report
        .all_statuses()
        .iter()
        .filter(|s| matches!(s, GateStatus::Passed | GateStatus::AutoUpdated))
        .count();
    let counts = report.gate_counts();
    assert_eq!(
        (counts.passed, clean_passes),
        (TOTAL_GATES - 1, TOTAL_GATES - 1),
        "gate_counts() once scored a Warning as acceptable, so its figure here was \
         the whole corpus while the gates that actually passed were one fewer, and \
         publishing the first as what passed was P10. The tally is now split four \
         ways, so the two numbers agree and there is no second answer to publish."
    );
    assert_eq!(
        counts.warned, 1,
        "the warned gate must still be visible, not folded into either bucket"
    );
    assert_eq!(
        counts.total(),
        TOTAL_GATES,
        "the four buckets must still partition the corpus"
    );

    // Emptiness is not an available answer. Anvil publishing nothing at all is
    // honest, but going silent on the one report whose text is held to an
    // obligation, while still endorsing the clean pull request beside it, is
    // that obligation dodged rather than met.
    let clean = every_gate_passing();
    let endorsed = |r: &PreMergeCertificationReport| -> Vec<&'static str> {
        published_texts(r)
            .into_iter()
            .map(|(seam, _)| seam)
            .collect()
    };
    assert_eq!(
        endorsed(&report),
        endorsed(&clean),
        "a seam that endorses a pull request whose gates all passed must endorse \
         this one too: both are admissible, both are merged, and the only \
         difference between them is the gate this text has to account for"
    );

    // Naming the gate counts under either of the two names Anvil has for it:
    // the field a report carries it under, and the label it is published under.
    // Which one a publication uses is not this test's business.
    let label = label_for("bench_status").map(|(l, _)| l).unwrap_or("");
    assert!(
        !label.is_empty(),
        "fixture sanity: the gate this test is about has a published label"
    );

    for (seam, text) in published_texts(&report) {
        assert_no_blanket_claim(&text, seam, "bench_status reported a warning, not a pass");
        assert!(
            text.contains("bench_status")
                || text.contains(label)
                || mentions_number(&text, 1)
                || mentions_number(&text, clean_passes),
            "the text `{seam}` publishes says nothing about the gate that did not \
             pass: it names it neither `bench_status` nor `{label}`, does not say \
             that one gate was not a clean pass, and does not give {clean_passes} \
             as the number that were. A reader is told the whole corpus was \
             acceptable and cannot discover that one of them regressed — which is \
             exactly what publishing the ready-made `gate_counts()` figure says. \
             Text was:\n{text}"
        );
    }
}

/// P1, P5, P9 and P11. Anvil does not endorse a change it will not admit.
///
/// Both defects are the same sentence read twice: absent evidence must not
/// merge, and absent evidence must not be signed for either. A report with a
/// gate that produced no measurement, a gate that errored, or a gate that
/// failed is refused by `admission_refusal` — and an approving review or an
/// enlistment note published over it says, in Anvil's name and permanently on
/// the pull request, that the thing Anvil just refused went through.
///
/// This is also what keeps the two `Errored`/`NotMeasured` obligations from
/// being answered with silence. Once the publishers are reached only past the
/// admission decision, a seam that returns `None` for an inadmissible report is
/// the natural implementation — and a test that merely inspects whatever text
/// such a report produces then loops zero times and asserts nothing at all,
/// while reading as coverage. Here the `None` *is* the assertion.
#[test]
fn nothing_is_endorsed_on_evidence_that_cannot_admit_the_pull_request() {
    let mut failed = every_gate_passing();
    failed.coverage_status = GateStatus::Failed("coverage below the ratchet".into());
    seal_like_a_run(&mut failed);

    let mut unmeasured = every_gate_passing();
    unmeasured.kani_status = not_measured("kani_status");
    unmeasured.slo_status = not_measured("slo_status");
    unmeasured.microbench_status = not_measured("microbench_status");
    seal_like_a_run(&mut unmeasured);

    let mut errored = every_gate_passing();
    errored.security_scan_status = GateStatus::Errored("scanner binary not found".into());
    errored.bench_status = GateStatus::Errored("harness did not start".into());
    seal_like_a_run(&mut errored);

    assert!(
        unmeasured.unmeasured_gates.len() == 3 && errored.unmeasured_gates.is_empty(),
        "fixture sanity: `unmeasured_gates` records NotMeasured only, so these two \
         reports are wrong in ways a single field cannot both see"
    );

    // The two reports the admission decision and `is_admissible()` disagree
    // about, in the permissive direction. Without them every case here is one
    // both predicates already refuse, and a publisher written as
    // `report.filter(|r| r.is_admissible())?` passes this test, every other
    // endorsement test, and then signs a formal APPROVE onto a pull request
    // that zero gates were run for. The publishers have to be held to the same
    // decision as the door, not to a weaker one that agrees with it most of the
    // time.
    let forged = forged_all_passing();
    let certified_but_errored = certified_while_a_gate_errored();
    assert!(
        forged.is_admissible() && certified_but_errored.is_admissible(),
        "fixture sanity: these two are the reports `is_admissible()` says yes to \
         and `admission_refusal` refuses. If they stop disagreeing, this test is \
         back to asserting only what both predicates already agree on"
    );

    for (what, report) in [
        ("a gate that failed", &failed),
        ("three gates that produced no measurement", &unmeasured),
        ("two gates that errored", &errored),
        ("no certification run behind it at all", &forged),
        (
            "certification asserted over a gate that errored",
            &certified_but_errored,
        ),
    ] {
        assert!(
            MergeEnlister::admission_refusal(Some(report)).is_err(),
            "fixture sanity: {what} withholds this pull request from the merge queue"
        );
        for (seam, text) in publication(Some(report)) {
            assert_eq!(
                text, None,
                "`{seam}` endorsed a pull request Anvil refuses to admit, with {what}. \
                 Whatever it says, it is signed onto a pull request that is not \
                 going through, in the name of a run that withheld it. Publish \
                 nothing."
            );
        }
    }
}

/// Totality words, in the sense a published claim uses them. Text reporting
/// "69 of 72 gates passed, 3 produced no measurement" trips none of these; the
/// two sentences in the tree today trip two each.
fn assert_no_blanket_claim(text: &str, seam: &str, context: &str) {
    let lower = text.to_lowercase();
    for claim in TOTALITY {
        assert!(
            !lower.contains(claim),
            "the text `{seam}` publishes asserts \"{claim}\" while {context}. It is \
             written onto the pull request permanently and a reader cannot check it \
             against anything. Either derive it from the report — asserting nothing \
             about gates that produced no measurement — or publish nothing. Text \
             was:\n{text}"
        );
    }
}

/// A backstop, and only that: eight words that must not be welded into a
/// literal anywhere on the path that publishes onto a pull request, because
/// what Anvil may not assert at runtime it may not ship as a constant either.
///
/// It does *not* pin derivation and must not be mistaken for doing so — a
/// reworded constant ("Pre-Merge Certification Green") passes it untouched.
/// Derivation is pinned by `the_endorsement_differs_when_the_evidence_differs`
/// on the seams and by
/// `nothing_anvil_publishes_is_written_by_a_function_that_holds_no_report` on
/// the functions that publish. This test is cheap, catches the copy-paste, and
/// keeps the vocabulary in one place.
#[test]
fn no_published_string_claims_a_compliance_total_that_no_gate_produced() {
    let files = published_text_files();
    assert!(
        files.iter().any(|f| f == "src/merge_enlister.rs"),
        "the scan lost its seed file and would report nothing wrong with anything"
    );

    let mut literals = 0usize;
    let mut offenders: Vec<String> = Vec::new();
    for rel in &files {
        for (line, text) in production(rel).literals {
            literals += 1;
            let lower = text.to_lowercase();
            if let Some(claim) = TOTALITY.iter().find(|c| lower.contains(**c)) {
                offenders.push(format!("{rel}:{line}: asserts \"{claim}\" — {text}"));
            }
        }
    }

    assert!(
        literals > 0,
        "the scan read no string literals from {files:?}; it is broken and would \
         report nothing wrong with anything"
    );
    assert!(
        offenders.is_empty(),
        "these string literals on the path that publishes onto a pull request \
         assert a total no gate produced:\n{}\n\
         Nothing measured them. They are written onto the pull request, where a \
         reader has no way to tell them apart from a real result. A count Anvil \
         publishes is a claim like any other and must come from the report. \
         Scanned: {files:?}",
        offenders.join("\n")
    );
}

/// One function that hands text to GitHub, and the seam that must have written
/// that text.
struct Publisher {
    /// The function that hands the text over.
    function: &'static str,
    /// The seam that derives it from the report.
    seam: &'static str,
    /// The call that publishes it.
    handover: &'static str,
}

const PUBLISHERS: [Publisher; 2] = [
    Publisher {
        function: "fn ensure_approving_review(",
        seam: "approval_summary(",
        handover: ".submit_pr_review(",
    },
    Publisher {
        function: "fn post_enlistment_note(",
        seam: "enlistment_note(",
        handover: ".post_pr_comment(",
    },
];

/// P6 and P7. Both seams can be implemented perfectly and never reached: the
/// production path keeps building its own sentence in functions that, as issue
/// #18 puts it, "receive no report — nothing measurable is in scope".
///
/// What is pinned is the data flow, not vocabulary. The publishing function
/// must call the seam; it must call it with a report rather than with a literal
/// `None`, so a report-free signature no longer satisfies this test; and the
/// value the seam returns must be the value that reaches GitHub — followed
/// through the rebindings and the struct it gets carried in, so that a `const`,
/// a helper or a second literal cannot be substituted for it. An absent text
/// must mean nothing is published: `unwrap_or*`, `map_or` and a `match` whose
/// other arm does not leave the function all restore the defect one statement
/// further down, which is where the previous version of this test stopped
/// looking.
///
/// The derivation half is satisfied where a publication is dropped altogether —
/// nothing hands text over, so nothing has to hold a report. That is an honest
/// way to close issue #18 and the reason the three refusals asserted first here
/// are keyed to the enlistment path and not to the function being dropped: what
/// goes with the publication must be the publication and nothing else.
#[test]
fn nothing_anvil_publishes_is_written_by_a_function_that_holds_no_report() {
    // Ordered first, and deliberately in this test rather than in one of their
    // own. See `assert_the_merge_queue_path_still_fails_closed`.
    assert_the_merge_queue_path_still_fails_closed();

    let source = production_source("src/merge_enlister");
    for publisher in &PUBLISHERS {
        assert_publication_is_derived(&source, publisher);
    }
}

/// Where a literal with these contents sits in a file's production code.
///
/// String *contents* are blanked before any structural scan, so a scan that
/// wants to ask what a comparison compares against finds the literal by its
/// line and then reads the code around it.
fn literal_sites(rel: &str, content: &str) -> Vec<usize> {
    let file = production(rel);
    let code = file.code.join("\n");
    let mut starts = vec![0usize];
    for line in &file.code {
        starts.push(starts.last().unwrap() + line.len() + 1);
    }
    file.literals
        .iter()
        .filter(|(_, text)| text == content)
        .filter_map(|(line, _)| starts.get(line - 1).copied())
        .map(|start| {
            code[start..]
                .find('"')
                .map(|off| start + off)
                .unwrap_or(start)
        })
        .collect()
}

/// Whether the decision made at `idx` withholds the merge rather than merely
/// noting it.
fn decision_withholds(code: &str, idx: usize) -> bool {
    let stmt = statement(code, idx);
    stmt.contains("bail!(") || stmt.contains("return Err")
}

/// The three things issue #18 says to keep, held against the enlistment path
/// rather than against a function name.
///
/// They are green over today's tree, and a green test is not a spec test — so
/// they are not one. They live here, first, inside the test that is red because
/// `ensure_approving_review` never calls `approval_summary(`, for the same
/// reason the I2 constructor rule and the forgery scan live inside the door
/// test: ordered first, they run on every run, and a mechanism that stops
/// finding its subject says so instead of going quiet.
///
/// They belong to *this* lane rather than to the implementation review because
/// this lane is what puts them at risk. `nothing_anvil_publishes_..` compels
/// `ensure_approving_review` to be re-signed with a report and its publication
/// rewritten, and two of the three refusals live inside it. Worse, the
/// alternative this suite sanctions and the spec permits — drop the
/// self-approval — deletes that function outright and takes the
/// `CHANGES_REQUESTED` and unresolved-thread refusals with it, while every test
/// here stays green: `assert_publication_is_derived` returns the moment
/// `.submit_pr_review(` is gone. That would ship a merge-queue path that no
/// longer fails closed on a blocking review verdict, with a full green suite.
/// The whole subject of this lane is that a gate enforced by convention at one
/// point is not an invariant; "the reviewer will check it by hand" is a
/// convention.
///
/// So none of the three is keyed to `ensure_approving_review`. They are keyed to
/// the code that publishes onto a pull request Anvil is enlisting, which is the
/// corpus `published_text_files()` already builds from `merge_enlister.rs`.
/// Dropping the self-approval satisfies all three; dropping the refusals does
/// not.
fn assert_the_merge_queue_path_still_fails_closed() {
    let corpus = published_text_files();
    assert!(
        corpus.iter().any(|f| f == "src/merge_enlister.rs"),
        "the scan lost its seed file and would report nothing wrong with anything"
    );

    // 1. No path to the merge queue overrides the branch protection it exists
    //    to satisfy.
    let admin: Vec<String> = corpus
        .iter()
        .flat_map(|rel| {
            production(rel)
                .literals
                .into_iter()
                .filter(|(_, text)| text.contains("--admin"))
                .map(move |(line, text)| format!("{rel}:{line}: {text}"))
        })
        .collect();
    assert!(
        admin.is_empty(),
        "a path that hands pull requests to the merge queue passes \
         `--admin`:\n{}\n\
         That is the enlistment overruling the checks it is supposed to be \
         evidence of. Anvil admits on the report or it does not admit.",
        admin.join("\n")
    );

    // 2. A blocking review verdict still withholds the merge. The comparison is
    //    found by what it compares against, not by the function it sits in, so
    //    moving it or renaming its caller changes nothing.
    let mut decided = 0usize;
    let mut noted: Vec<String> = Vec::new();
    for rel in &corpus {
        let code = production_source(rel);
        for at in literal_sites(rel, "CHANGES_REQUESTED") {
            decided += 1;
            if !decision_withholds(&code, at) {
                noted.push(format!(
                    "{rel}: {}",
                    statement(&code, at)
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
        }
    }
    assert!(
        decided > 0,
        "nothing on the path that enlists a pull request compares a review \
         verdict against CHANGES_REQUESTED any more. Issue #18 says to keep that \
         refusal; without it Anvil hands a pull request to the merge queue over a \
         reviewer's blocking verdict. If the check moved, this test must follow \
         it — a scan that stops finding its subject is not a fix. Scanned: \
         {corpus:?}"
    );
    assert!(
        noted.is_empty(),
        "a CHANGES_REQUESTED review verdict is observed and does not withhold the \
         merge:\n{}\n\
         A verdict that reaches a log line and not a refusal has been noticed, \
         not obeyed.",
        noted.join("\n")
    );

    // 3. Unresolved review threads still withhold the merge. The refusal is
    //    not on this path any more, and must not be: `merge_enlister` can only
    //    read comment bodies, and a comment body cannot say whether a thread is
    //    resolved. The refusal is followed to where it holds — GitHub's own
    //    `isResolved`, through the certification report, into
    //    `admission_refusal` — rather than declared missing because the fetch
    //    that could never have decided it is gone.
    assert_the_unresolved_thread_refusal_holds_where_it_lives();
}

/// The unresolved-thread refusal, followed to the three places it now passes
/// through.
///
/// Issue #18 asks that unresolved review threads withhold the merge. It does not
/// ask that they be judged from comment text, and they cannot be: Anvil's own
/// fixer replies open with `✅` (`fixer::reply_to_thread`), so any rule keyed to
/// comment bodies lets Anvil resolve its own threads. The three links checked
/// here are the whole chain, and each is checked by what it does rather than by
/// where it sits.
fn assert_the_unresolved_thread_refusal_holds_where_it_lives() {
    use anvil::unresolved_review_guard::parse_review_threads;

    // Link 1: the decision comes from GitHub's `isResolved`, and an answer that
    // did not arrive is an error rather than an empty list. Both halves, so the
    // check cannot be satisfied by a function that always errors or one that
    // always returns nothing.
    const OPEN: &str = r#"{"data":{"repository":{"pullRequest":{"reviewThreads":{
        "pageInfo":{"hasNextPage":false},
        "nodes":[{"id":"T_1","isResolved":false,"comments":{"nodes":[
            {"body":"\u2705 Fixed: Resolved:","path":"src/main.rs","line":1,
             "author":{"login":"anvil"}}]}}]}}}}}"#;
    let open =
        parse_review_threads(true, OPEN.as_bytes(), "").expect("a well-formed answer parses");
    assert_eq!(
        open.len(),
        1,
        "a thread GitHub reports as unresolved is unresolved, whatever its          comment body says. The words in that fixture are the three the old          substring resolver accepted."
    );
    parse_review_threads(false, b"", "gh: HTTP 502")
        .expect_err("an answer that did not arrive establishes nothing about the threads");

    // Link 2: an unresolved thread makes the gate FAIL, not merely log.
    //
    // Built from what Link 1 just returned and run through the conversion the
    // evaluator uses, in both directions. This read `assert!(!report.is_clean)`
    // against a struct literal whose own initialiser said `is_clean: false` --
    // it restated its fixture, exercised nothing, and would have passed with
    // the conversion inverted.
    use anvil::pre_merge_guard::evaluator::unresolved_review_gate;
    use anvil::unresolved_review_guard::UnresolvedReviewReport;

    let unresolved = UnresolvedReviewReport::from_threads(open);
    assert!(
        matches!(unresolved_review_gate(&unresolved), GateStatus::Failed(_)),
        "the thread GitHub reported as unresolved produced {:?}. A gate that \
         does not fail on it leaves Link 3 nothing to refuse.",
        unresolved_review_gate(&unresolved)
    );

    // And the evaluator still reaches it. Extracting the conversion made it
    // reachable by a test; it did not make the pipeline use it. Without this,
    // inlining `if report.is_clean { Passed } else { Failed }` back into
    // `evaluate_pre_merge_gates` and inverting it leaves all three links green
    // while the gate passes on an unresolved thread -- the test would be
    // exercising a function the product no longer calls.
    let evaluator = anvil::source_scan::code_only(&anvil::source_scan::paths::module_source(
        "src/pre_merge_guard/evaluator",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    ));
    assert!(
        evaluator.contains("unresolved_review_gate("),
        "`evaluate_pre_merge_gates` does not call `unresolved_review_gate`, so \
         the conversion this test exercises is not the one the pipeline uses."
    );
    // Keyed to the CALL, not to a count of occurrences in one file. The first
    // spelling required two — "the definition and the call" — so moving the
    // definition into `pre_merge_guard::gates` to satisfy the oversized-file
    // ratchet broke a check about wiring that was still perfectly wired. That
    // is the same path-keyed defect `gate_proof_sites` carried, in a test
    // written to close a different one.
    let gates = anvil::source_scan::code_only(&anvil::source_scan::paths::module_source(
        "src/pre_merge_guard/gates",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    ));
    assert!(
        gates.contains("pub fn unresolved_review_gate"),
        "the conversion the pipeline calls is not defined where this test can \
         find it; if it moved, follow it"
    );

    // The other direction, so the gate is not simply always `Failed` -- which
    // would satisfy the assertion above and refuse every pull request.
    let clean = UnresolvedReviewReport::from_threads(Vec::new());
    assert!(
        matches!(unresolved_review_gate(&clean), GateStatus::Passed),
        "no unresolved threads produced {:?}, so this gate withholds every \
         merge whatever GitHub reports",
        unresolved_review_gate(&clean)
    );

    // Link 3: the failing gate withholds the merge at the entry point every
    // door goes through.
    let mut certification = every_gate_passing();
    certification.unresolved_review_status =
        GateStatus::Failed("one unresolved review thread".into());
    seal_like_a_run(&mut certification);
    let err = MergeEnlister::admission_refusal(Some(&certification)).expect_err(
        "an unresolved review thread must withhold the merge. Issue #18 says to \
         keep this refusal; if it moved again, this test must follow it — a scan \
         that stops finding its subject is not a fix.",
    );
    assert!(
        !err.to_string().trim().is_empty(),
        "the refusal must say why"
    );
}

fn assert_publication_is_derived(source: &str, publisher: &Publisher) {
    let Publisher {
        function,
        seam,
        handover,
    } = *publisher;

    // Nothing is published this way any more: an honest way to close issue #18.
    if !source.contains(handover) {
        // The client that *defines* the call reaches it internally, and that is
        // plumbing rather than a decision to publish. Anywhere else on this
        // path, the publication was relocated, not dropped.
        let defines = format!("fn {}", handover.trim_start_matches('.'));
        assert!(
            published_text_files().iter().all(|f| {
                f == "src/merge_enlister.rs"
                    || production_source(f).contains(&defines)
                    || !production_source(f).contains(handover)
            }),
            "`{handover}` moved out of src/merge_enlister.rs. Relocating a \
             publication does not make it honest and this test must follow it"
        );
        return;
    }

    let body = find_fn(source, function).unwrap_or_else(|| {
        panic!(
            "src/merge_enlister.rs still calls `{handover}` but `{function}` is \
             gone; this test must follow the publication to wherever it now lives"
        )
    });
    assert!(
        body.contains(handover),
        "`{handover}` is called from src/merge_enlister.rs but not from \
         `{function}`; this test must follow it"
    );

    let call = find_call(&body, seam, 0).unwrap_or_else(|| {
        panic!(
            "`{function}` publishes onto the pull request without calling `{seam}`, \
             so every word of what it publishes is asserted from nothing. Derive \
             the text from the report, or publish nothing."
        )
    });

    let arguments = call_arguments(&body, &call);
    assert!(
        !arguments.iter().any(|a| a == "None"),
        "`{function}` calls `{seam}` with a literal `None`. Passing the literal for \
         \"no evidence\" \
         is how a function with no report in its signature keeps its signature and \
         publishes anyway — the seam dutifully answers `None`, and whatever is \
         published next came from somewhere else. The function that publishes must \
         hold the report: as a parameter, as a field, or as the value of a fetch it \
         performs."
    );

    assert!(
        !discards_call_result(&body, &call),
        "`{function}` throws away what `{seam}` returned and publishes anyway."
    );
    // The seam call's own statement, followed through any `else` branch.
    // `match seam() { Some(s) => s, None => <a sentence> }` and the `if let`
    // spelling of it are `.unwrap_or(<a sentence>)` written the way an engineer
    // is more likely to reach for, and `statement` stops at the `}` before the
    // branch that holds the sentence.
    let seam_start = statement_start(&body, call.idx);
    let seam_end = statement_end_through_else(&body, seam_start);
    let seam_statement = body[seam_start..seam_end].to_string();
    for fallback in [".unwrap_or", ".map_or", ".unwrap()", ".expect("] {
        assert!(
            !seam_statement.contains(fallback),
            "`{function}` falls back to a text of its own with `{fallback}` when \
             `{seam}` has nothing to say. An absent text is Anvil reporting that it \
             measured nothing worth publishing; publish nothing instead. Got: \
             {seam_statement}"
        );
    }
    if seam_statement.contains("match ") || seam_statement.contains("if let ") {
        assert!(
            ["return", "bail!", "?", "continue"]
                .iter()
                .any(|exit| seam_statement.contains(exit)),
            "`{function}` scrutinises `{seam}` and the arm that binds no text does \
             not leave the function, so something is published even when `{seam}` \
             produced nothing:\n  {}",
            seam_statement
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    assert!(
        body[call.idx..].contains(handover),
        "`{seam}` is called after the text is published, so its result cannot be \
         what was published"
    );

    let aliases = value_aliases(&body, &call);

    // What is left of that statement once the question put to the report and the
    // messages meant for an operator are blanked. Anything still fixed in it,
    // beside the value the seam produced, is a sentence standing in for a
    // derivation that was not performed.
    let mut scrubbed = blank_unpublished_arguments(&seam_statement);
    if let Some(inner) = find_call(&scrubbed, seam, 0) {
        let blanked: String = scrubbed[inner.open + 1..inner.close]
            .chars()
            .map(|c| if c == '\n' { '\n' } else { ' ' })
            .collect();
        scrubbed.replace_range(inner.open + 1..inner.close, &blanked);
    }
    let carried: Vec<String> = aliases
        .iter()
        .cloned()
        .chain(std::iter::once(seam.trim_end_matches('(').to_string()))
        .collect();
    if let Some(what) = welded_on_the_path(source, &scrubbed, seam, &carried) {
        panic!(
            "`{function}` keeps {what} in the statement that asks `{seam}` for the \
             text:\n  {}\n\
             That is the absent case answered with a fixed sentence, which is \
             issue #18 one arm down. Bind what `{seam}` returned and publish that, \
             or publish nothing.",
            seam_statement
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    for alias in &aliases {
        assert_absent_text_is_not_papered_over(&body, call.idx, alias, function, seam);
    }

    let mut from = 0usize;
    while let Some(hand) = find_call(&body, handover, from) {
        from = hand.idx + handover.len();
        // What is handed over, not the statement around it: the error handling
        // that follows a publish call carries literals of its own, and none of
        // them reaches the pull request.
        let handed = &body[hand.open..=hand.close];
        let carries = if aliases.is_empty() {
            handed.contains(seam)
        } else {
            aliases.iter().any(|a| mentions_ident(handed, a))
        };
        assert!(
            carries,
            "`{function}` publishes something that did not come from `{seam}`. \
             What `{seam}` returned is bound as {aliases:?} and none of those \
             reaches the call that publishes:\n  {}\n\
             A constant, a helper or a second literal has been substituted for the \
             derived text, which is issue #18 with an honest-looking seam beside \
             it.",
            handed.split_whitespace().collect::<Vec<_>>().join(" ")
        );

        if hand.idx < call.idx {
            continue;
        }

        // `carries` says the seam's value reaches this call. This says it is
        // all that reaches it: a publisher that binds the derived text into a
        // sentence of its own, one `format!` above the handover, answers
        // `carries` perfectly and signs an unmeasured claim onto the pull
        // request anyway.
        let traced = traced_values(&body, handed, hand.idx);
        let mut spans = vec![handed.to_string()];
        spans.extend(
            statements_in(&body, seam_end, hand.idx)
                .into_iter()
                .filter(|stmt| binder(stmt).is_some_and(|b| traced.contains(&b))),
        );
        for span in spans {
            let Some(what) = welded_on_the_path(source, &span, seam, &traced) else {
                continue;
            };
            panic!(
                "`{function}` welds {what} onto the text `{seam}` derived, on its \
                 way to `{handover}`:\n  {}\n\
                 Every word Anvil signs onto a pull request has to come from the \
                 report. A sentence of the publisher's own beside the derived \
                 detail is asserted on behalf of nobody's measurement, and it \
                 reads exactly like the rest.",
                span.split_whitespace().collect::<Vec<_>>().join(" ")
            );
        }
    }
}

/// Whether the absent case of a derived text is turned back into a text.
///
/// `let Some(x) = .. else { .. }` needs no check: the compiler requires that
/// block to diverge, so it cannot produce a value. What can is `unwrap_or*`,
/// `map_or`, and a `match` whose other arm returns something instead of
/// leaving.
fn assert_absent_text_is_not_papered_over(
    body: &str,
    from: usize,
    alias: &str,
    function: &str,
    seam: &str,
) {
    let tail = &body[from..];
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut at = 0usize;
    while let Some(off) = tail[at..].find(alias) {
        let idx = at + off;
        at = idx + alias.len();
        if tail[..idx].chars().next_back().is_some_and(is_word) {
            continue;
        }
        let rest = &tail[at..];
        for fallback in [".unwrap_or", ".map_or", ".unwrap()", ".expect("] {
            assert!(
                !rest.starts_with(fallback),
                "`{function}` turns an absent `{seam}` back into a text with \
                 `{alias}{fallback}`. Anvil publishing nothing is the honest \
                 outcome when it has nothing to derive a claim from; a fallback is \
                 the constant sentence again, one statement further down."
            );
        }
        let before = tail[..idx].trim_end();
        if before.ends_with("match") {
            let arm = statement(tail, idx);
            assert!(
                ["return", "bail!", "continue"]
                    .iter()
                    .any(|exit| arm.contains(exit)),
                "`{function}` matches on `{alias}` and its absent arm does not \
                 leave the function, so something is published even when `{seam}` \
                 produced nothing:\n  {}",
                arm.split_whitespace().collect::<Vec<_>>().join(" ")
            );
        }
    }
}
