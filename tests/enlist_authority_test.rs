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
//! come back refused. That is behaviour, not a scan, and it holds whatever
//! shape the guard is written in. What remains for source scans is the part no
//! in-process call can reach: which callers exist, what they hand over, and
//! what the two publishing functions weld into the text they sign.
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
//!     -> `a_fully_measured_and_certified_report_admits_the_pull_request`.
//! P5. The entry point is fixed and the callers keep passing `None`, so nothing
//!     ever merges again; or a caller manufactures the report it hands over,
//!     which is the same defect wearing a report's clothes.
//!     -> `every_door_hands_the_merge_queue_evidence_a_certification_run_produced`.
//! P6. The blanket claim is deleted from `ensure_approving_review` and reappears
//!     a few lines down in the enlistment note, or moves into a `const`, a
//!     helper, or a sibling file. The struct is honest, the published comment
//!     is not — and the comment is what a human reads.
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
//!     -> `an_endorsement_asserts_no_total_when_a_gate_errored`, and
//!        `a_report_that_certifies_while_a_gate_errored_is_still_refused`.
//! P10. The text is derived from the ready-made `gate_counts()`, which scores
//!     `NotMeasured` as acceptable and so reports "72 of 72 gates passed" for a
//!     report where three gates measured nothing. Honest-looking, asserted on
//!     behalf of nobody's measurement, and it trips no ban on totality wording.
//!     -> the two `asserts_no_total` tests carry a positive obligation as well
//!        as the ban: a publication made over an unmeasured or errored gate must
//!        name that gate or say how many there were.
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
//! is bound to `_`, wrapped in `drop(`, or its result — everything after the
//! call's closing parenthesis — is `.ok()`d or `unwrap_or`'d. A `.ok()` inside
//! the argument list belongs to an argument, not to the call, and
//! `Self::admission_refusal(self.report(..).await.ok().as_ref())?` is a guarded
//! door: converting a fetch failure into `None` and refusing on it is exactly
//! what the spec asks for.
//!
//! # Scaffolding this suite depends on
//!
//! Three signatures with `todo!()` bodies in `src/merge_enlister.rs` —
//! `admission_refusal`, `approval_summary`, `enlistment_note` — and one
//! parameter added to `enlist_into_merge_queue`, with every caller passing
//! `None` so the tree compiles. No body holds logic.
//!
//! # Not pinned here, and why
//!
//! `ensure_approving_review` already fails closed on `CHANGES_REQUESTED` and on
//! unresolved review threads, and the crate uses no `--admin` anywhere. Those
//! are green today, so a guard for them would be a green test in a lane whose
//! rule is that every spec test starts red. If self-approval is dropped by
//! deleting `ensure_approving_review`, those two bails go with it and nothing
//! here would notice.

use anvil::github::GitHubClient;
use anvil::merge_enlister::MergeEnlister;
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
const TOTALITY: [&str; 8] = [
    "100%",
    "all automated",
    "all gates",
    "all checks",
    "all safety",
    "every gate",
    "fully compliant",
    "fully green",
];

/// The merge strategy the enlistment note is written about. Held constant
/// wherever two notes are compared, so any difference between them comes from
/// the report and not from this.
const STRATEGY: &str = "Squash & Merge";

// -------------------------------------------------------------------------
// Report fixtures
// -------------------------------------------------------------------------

/// A report in which every gate in the corpus reports `Passed`.
///
/// Built by round-tripping `PreMergeCertificationReport::unmeasured` through
/// serde rather than by naming seventy-two fields, so it stays correct when the
/// corpus grows. There is deliberately no "all passed" constructor in
/// production (invariant I2) and this fixture is not one — it is test data, and
/// `every_door_hands_the_merge_queue_evidence_a_certification_run_produced`
/// exists to keep it from becoming production data.
fn every_gate_passing() -> PreMergeCertificationReport {
    let base = PreMergeCertificationReport::unmeasured("fixture baseline");
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
    report.seal();
    assert!(
        report.is_admissible(),
        "fixture sanity: every gate passing must be admissible"
    );
    report
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
    report.seal();
    report
}

/// A report that asserts certification while a gate errored. Deliberately not
/// sealed: `is_admissible()` alone says yes to it.
fn certified_while_a_gate_errored() -> PreMergeCertificationReport {
    let mut report = every_gate_passing();
    report.slo_status = GateStatus::Errored("prometheus probe timed out".into());
    report.is_certified_ready = true;
    report.recompute_unmeasured();
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
fn production(rel: &str) -> Production {
    let path = repo_path(rel);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
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
    prefix.ends_with("drop(") || prefix.contains("_ =")
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

/// The identifier a statement binds, in the spellings a value that may be
/// absent is bound with: `let x =`, `let mut x =`, `let Some(x) = .. else`,
/// `if let Some(x) =`.
fn binder(statement: &str) -> Option<String> {
    let mut rest = statement.trim_start();
    for prefix in ["if ", "while "] {
        if let Some(stripped) = rest.strip_prefix(prefix) {
            rest = stripped.trim_start();
        }
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
fn value_aliases(body: &str, idx: usize) -> Vec<String> {
    let mut aliases: Vec<String> = Vec::new();
    if let Some(name) = binder(&body[statement_start(body, idx)..statement_end(body, idx)]) {
        aliases.push(name);
    }
    if aliases.is_empty() {
        return aliases;
    }
    let mut from = statement_end(body, idx);
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
    "src/cli/handlers.rs",
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
    report.seal();

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
    assert!(
        !err.to_string().trim().is_empty(),
        "the refusal must say why"
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

    let enlister = MergeEnlister::new(Arc::new(GitHubClient::new()));
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
    }
}

/// P5. The entry point can only refuse on what it is given. A door that hands
/// it `None` for ever has closed the queue rather than guarded it; a door that
/// hands it a report the door itself wrote has reopened issue #17 behind a
/// well-typed argument.
///
/// The second half is the one with no live offender today, and it is here
/// because the wrong implementation is four lines: run `cargo check` in the
/// healer, round-trip `PreMergeCertificationReport::unmeasured` through serde
/// setting every `*_status` to `Passed`, `seal()`, hand that over. Every
/// admission test in this file passes on it, because the report is a forgery
/// and the predicate is asked politely. Gate statuses are assigned by the
/// certification run and by nothing else.
#[test]
fn every_door_hands_the_merge_queue_evidence_a_certification_run_produced() {
    let doors = merge_queue_doors();
    assert!(
        !doors.is_empty(),
        "this scan found no call to `enlist_into_merge_queue`. Either the merge \
         queue entry point was renamed and this test must follow it, or the scan \
         is broken — a mechanism that cannot find its subject reports nothing \
         wrong with anything"
    );

    let empty_handed: Vec<String> = doors
        .iter()
        .filter(|d| {
            let args = d.arguments();
            args.len() < 3 || args.iter().any(|a| a == "None")
        })
        .map(|d| format!("{} — {}", d.at(), d.statement()))
        .collect();
    assert!(
        empty_handed.is_empty(),
        "these paths hand a pull request to the merge queue with no evidence for \
         it:\n{}\n\
         The entry point refuses what it is handed nothing for, so this is \
         fail-closed and safe — and it is also a door that can never admit \
         anything again. Obtain the certification report on this path and hand it \
         over, or delete the door.",
        empty_handed.join("\n")
    );

    let forged: Vec<String> = rust_sources_under("src")
        .into_iter()
        .filter(|rel| !rel.starts_with("src/pre_merge_guard/"))
        .flat_map(|rel| {
            let code = production_source(&rel);
            let mut hits = Vec::new();
            for (needle, what) in [
                ("PreMergeCertificationReport {", "builds a report by hand"),
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
        "these production files outside src/pre_merge_guard/ produce a \
         certification report rather than receiving one:\n{}\n\
         A report a caller wrote is not evidence, it is the caller's opinion in \
         the shape of evidence, and `admission_refusal` cannot tell the \
         difference. Gate statuses come from the certification run.",
        forged.join("\n")
    );
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

/// P3, one layer out. The refusal not being dropped at the call site is a third
/// of the problem: `manual_enlist_handler` spawns the enlistment into a
/// detached task and answers `202 ACCEPTED` with `success: true` before the
/// task has done anything. Logging the refusal inside that task satisfies the
/// discard scan and still tells the operator — or the automation calling the
/// API — that an enlistment happened when it was refused.
///
/// Either the handler waits for the outcome and answers with it, or it stops
/// asserting an outcome it does not have. Vacuously satisfied if the handler is
/// deleted or if it no longer detaches the work.
#[test]
fn the_enlist_api_does_not_answer_success_for_an_enlistment_it_has_not_performed() {
    const HANDLER: &str = "fn manual_enlist_handler(";
    let source = production_source("src/webhook/manual_handlers.rs");
    let Some(body) = find_fn(&source, HANDLER) else {
        return;
    };
    let Some(spawn) = body.find("tokio::spawn(") else {
        return;
    };
    let spawned = &body[spawn..statement_end(&body, spawn)];
    if !spawned.contains("enlist_into_merge_queue(") {
        return;
    }

    let after = &body[statement_end(&body, spawn)..];
    assert!(
        !after.contains("success: true"),
        "`manual_enlist_handler` detaches the enlistment and then answers \
         `success: true` regardless of what it does:\n{}\n\
         The requester is told the pull request was enlisted when it may have \
         been refused, and a refusal that reaches only a log line inside a \
         detached task has not been surfaced to anybody who asked. Wait for the \
         outcome and answer with it, or answer something that is true of a job \
         that has not run yet.",
        after
            .trim()
            .lines()
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" ")
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
#[test]
fn the_endorsement_differs_when_the_evidence_differs() {
    let clean = every_gate_passing();

    let mut ragged = every_gate_passing();
    ragged.kani_status = not_measured("kani_status");
    ragged.coverage_status = GateStatus::Failed("coverage below the ratchet".into());
    ragged.seal();
    assert_publications_differ(
        &clean,
        &ragged,
        "the same text was published for a pull request whose gates all passed and \
         for one with a failed gate and a gate that produced no measurement. A \
         claim identical across both is derived from neither",
    );

    let mut warned = every_gate_passing();
    warned.bench_status = GateStatus::Warning("throughput regressed within tolerance".into());
    warned.seal();
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
}

/// P8. The only test that reads the text Anvil publishes onto a pull request it
/// actually admits. A warning is not a pass, so nothing written over one may
/// sweep the corpus into a total.
#[test]
fn an_endorsement_asserts_no_total_when_a_gate_only_warned() {
    let mut report = every_gate_passing();
    report.bench_status = GateStatus::Warning("throughput regressed within tolerance".into());
    report.seal();
    assert!(
        report.is_admissible(),
        "fixture sanity: this pull request is admitted, so whatever Anvil publishes \
         onto it is published onto a merge that really happens"
    );

    for (seam, text) in published_texts(&report) {
        assert_no_blanket_claim(&text, seam, "bench_status reported a warning, not a pass");
    }
}

/// P6 and P10. A gate reporting `NotMeasured` made no claim in either
/// direction. Text that sweeps it into a total — "all gates", "100%" — asserts
/// on its behalf something nobody measured; and text built from `gate_counts()`,
/// which scores `NotMeasured` as acceptable, does the same thing in arithmetic
/// instead of adjectives.
///
/// So the ban on totality wording is paired with a positive obligation. That
/// pins no wording: naming the gates, or saying how many produced nothing, both
/// satisfy it. Publishing the ready-made passed-count and stopping does not.
#[test]
fn an_endorsement_asserts_no_total_when_a_gate_was_not_measured() {
    let mut report = every_gate_passing();
    report.kani_status = not_measured("kani_status");
    report.slo_status = not_measured("slo_status");
    report.microbench_status = not_measured("microbench_status");
    report.seal();

    assert_eq!(
        report.unmeasured_gates.len(),
        3,
        "fixture sanity: three gates measured nothing"
    );
    assert_eq!(
        report.gate_counts().0,
        TOTAL_GATES,
        "fixture sanity: gate_counts() scores NotMeasured as acceptable, so its \
         ready-made figure here is the whole corpus — that is the number a \
         publication must not publish as what passed"
    );

    for (seam, text) in published_texts(&report) {
        assert_no_blanket_claim(&text, seam, "three gates produced no measurement");
        assert!(
            report
                .unmeasured_gates
                .iter()
                .any(|gate| text.contains(gate.as_str()))
                || mentions_number(&text, report.unmeasured_gates.len()),
            "the text `{seam}` publishes says nothing about the {} gates that \
             produced no measurement: it names none of {:?} and states no count. A \
             reader is told how many gates passed and cannot discover that part of \
             the evidence is missing. Text was:\n{text}",
            report.unmeasured_gates.len(),
            report.unmeasured_gates
        );
    }
}

/// P9 and P10. `unmeasured_gates` tracks `NotMeasured` only. Text derived from
/// that field alone still describes an `Errored` gate — configured, attempted,
/// no result — as one of the gates that passed.
#[test]
fn an_endorsement_asserts_no_total_when_a_gate_errored() {
    let mut report = every_gate_passing();
    report.security_scan_status = GateStatus::Errored("scanner binary not found".into());
    report.bench_status = GateStatus::Errored("harness did not start".into());
    report.seal();

    assert!(
        report.unmeasured_gates.is_empty(),
        "fixture sanity: `unmeasured_gates` records NotMeasured only, so text \
         derived from that field alone sees nothing wrong with this report"
    );

    for (seam, text) in published_texts(&report) {
        assert_no_blanket_claim(&text, seam, "two gates errored");
        assert!(
            text.contains("security_scan_status")
                || text.contains("bench_status")
                || mentions_number(&text, 2),
            "the text `{seam}` publishes says nothing about the two gates that \
             errored: it names neither and states no count. Text was:\n{text}"
        );
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
/// Vacuously satisfied where a publication is dropped altogether — nothing
/// hands text over, so nothing has to hold a report.
#[test]
fn nothing_anvil_publishes_is_written_by_a_function_that_holds_no_report() {
    let source = production_source("src/merge_enlister.rs");
    for publisher in &PUBLISHERS {
        assert_publication_is_derived(&source, publisher);
    }
}

fn assert_publication_is_derived(source: &str, publisher: &Publisher) {
    let Publisher {
        function,
        seam,
        handover,
    } = *publisher;

    // Nothing is published this way any more: an honest way to close issue #18.
    if !source.contains(handover) {
        assert!(
            published_text_files()
                .iter()
                .all(|f| f == "src/merge_enlister.rs" || !production_source(f).contains(handover)),
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
    let call_statement = statement(&body, call.idx);
    for fallback in [".unwrap_or", ".map_or", ".unwrap()", ".expect("] {
        assert!(
            !call_statement.contains(fallback),
            "`{function}` falls back to a text of its own with `{fallback}` when \
             `{seam}` has nothing to say. An absent text is Anvil reporting that it \
             measured nothing worth publishing; publish nothing instead. Got: \
             {call_statement}"
        );
    }

    assert!(
        body[call.idx..].contains(handover),
        "`{seam}` is called after the text is published, so its result cannot be \
         what was published"
    );

    let aliases = value_aliases(&body, call.idx);
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
        assert!(
            !handed.contains('"'),
            "`{function}` hands a string literal straight to `{handover}`:\n  {}\n\
             String contents are blanked before this scan, so a surviving quote is \
             a fixed sentence reaching the pull request whatever `{seam}` returned.",
            handed.split_whitespace().collect::<Vec<_>>().join(" ")
        );
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
