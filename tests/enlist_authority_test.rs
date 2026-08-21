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
//! architecture, and safety gates have passed with 100% compliance". That
//! sentence is a string literal in a function that receives no report. It is
//! written into the permanent review record of the pull request, so a reader
//! cannot tell a genuinely certified PR from one enlisted with zero gates run.
//!
//! # Premortem
//!
//! Assume both fixes shipped and then failed. The ways they can have failed,
//! each turned into a test below:
//!
//! P1. The check is added to `enlist_into_merge_queue` but reads
//!     `is_certified_ready` instead of `is_admissible`, so a report that
//!     certifies while three gates produced no measurement still merges — the
//!     exact distinction the two predicates exist to draw.
//!     -> `a_certified_report_with_an_unmeasured_gate_is_refused_and_the_gate_is_named`.
//! P2. A caller that cannot obtain a report treats "no report" as "nothing
//!     objected" and enlists. Absent configuration is not permission.
//!     -> `evidence_that_was_never_obtained_does_not_admit_a_pull_request`.
//! P3. The refusal is a silent `return Ok(())`, or the caller throws it away.
//!     Nothing merges and nobody can say why; the operator concludes the daemon
//!     is wedged and disables it.
//!     -> every refusal test asserts a reason;
//!        `no_door_into_the_merge_queue_is_left_unchecked` requires the refusal
//!        to divert control flow rather than merely be mentioned, and
//!        `no_path_drops_a_merge_queue_refusal_on_the_floor` bans discarding the
//!        outcome by shape — `let _`, a bare `_ =`, `.ok()`, `.unwrap_or*`,
//!        `drop(`.
//! P4. Over-correction: the precondition refuses everything, including a
//!     genuinely certified, fully measured pull request. I1 cuts both ways —
//!     absent evidence is not a pass and present evidence is not an accusation.
//!     -> `a_fully_measured_and_certified_report_admits_the_pull_request`.
//! P5. One or two of the three ungated doors are fixed and the third is not,
//!     which is how the defect arose in the first place.
//!     -> `no_door_into_the_merge_queue_is_left_unchecked`, a mechanism over
//!        source (I22) rather than a reviewer's memory. It accepts either
//!        design: one check at the entry point, or a check at every caller.
//! P6. The blanket claim is deleted from `ensure_approving_review` and reappears
//!     a few lines down in the enlistment note, or moves into a `const`, a
//!     helper, or a sibling file. The struct is honest, the published comment
//!     is not — and the comment is what a human reads.
//!     -> `no_published_string_claims_a_compliance_total_that_no_gate_produced`
//!        scans every source file that defines or feeds what Anvil publishes,
//!        not one function.
//! P7. `approval_summary` is implemented correctly and never called: the
//!     production path keeps writing its own sentence with no report in scope.
//!     -> `the_approving_review_is_not_written_by_a_function_that_holds_no_report`.
//! P8. The claim is reworded rather than derived — a different literal, equally
//!     unmeasured, identical for every pull request in the fleet.
//!     -> `the_endorsement_differs_when_the_evidence_differs`, which pins
//!        derivation without pinning one wording, on a pair of reports that are
//!        *both* admissible so an endorsement is actually published for each;
//!        and `an_endorsement_asserts_no_total_when_a_gate_only_warned`, the one
//!        test that reads what Anvil signs onto a pull request it admits.
//! P9. The text is derived from `unmeasured_gates` only, so a gate that
//!     `Errored` — configured, attempted, no result — is still described as
//!     having passed.
//!     -> `an_endorsement_asserts_no_total_when_a_gate_errored`.
//! P10. The text is derived from the ready-made `gate_counts()`, which scores
//!     `NotMeasured` as acceptable and so reports "72 of 72 gates passed" for a
//!     report where three gates measured nothing. Honest-looking, asserted on
//!     behalf of nobody's measurement, and it trips no ban on totality wording.
//!     -> the two `asserts_no_total` tests carry a positive obligation as well
//!        as the ban: an endorsement published over an unmeasured or errored
//!        gate must name that gate or say how many there were.
//!
//! # What the source scans will and will not accept
//!
//! The four mechanisms below read production source, because these paths shell
//! out to `gh` and take an `AppState` of ninety `Arc` fields: the wiring
//! between a decision and a door cannot be exercised in-process without a
//! network. What they read is code only — comment text *and the contents of
//! string literals* are blanked before any structural scan, so neither a
//! comment documenting the convention nor a `warn!` reminding callers of it can
//! answer a question about whether a decision is taken.
//!
//! A door counts as guarded when it routes through `admission_refusal` **and**
//! the refusal has a consequence: propagated with `?`, `bail!`ed, returned
//! from, `continue`d past, or wrapped around the enlistment itself. A mention
//! that gates nothing is not a guard, and neither is
//! `if refusal.is_err() { return Ok(()) }`.
//!
//! `is_admissible()` is deliberately *not* accepted as the door's guard, even
//! though the review pipeline uses it today:
//! `a_report_that_certifies_while_a_gate_errored_is_still_refused` establishes
//! that it says yes to a report this lane refuses, so a door written
//! `if r.is_admissible() { enlist }` would be scored as guarded while admitting
//! precisely that report. The seam this lane defines is `admission_refusal`,
//! and it is what the doors must consult.
//!
//! # Scaffolding this suite depends on
//!
//! Two signatures with `todo!()` bodies in `src/merge_enlister.rs`:
//! `MergeEnlister::admission_refusal` and `MergeEnlister::approval_summary`.
//! They exist so the invariant can be stated before anything implements it.
//! Neither prescribes where the decision is wired: `no_door_into_the_merge_queue_is_left_unchecked`
//! deliberately accepts the entry point deciding once — directly, or in a
//! helper whose failure it propagates — or every caller deciding for itself,
//! and returning `None` from `approval_summary` for every report, which is to
//! say dropping self-approval altogether, is a valid implementation.
//!
//! # Not pinned here, and why
//!
//! `ensure_approving_review` already fails closed on `CHANGES_REQUESTED` and on
//! unresolved review threads, and the crate uses no `--admin` anywhere. Those
//! are green today, so a guard for them would be a green test in a lane whose
//! rule is that every spec test starts red. They are called out in the handoff
//! instead: if self-approval is dropped by deleting `ensure_approving_review`,
//! those two bails go with it and nothing here would notice.

use anvil::merge_enlister::MergeEnlister;
use anvil::pre_merge_guard::report::{GateStatus, PreMergeCertificationReport, TOTAL_GATES};
use std::fs;
use std::path::{Path, PathBuf};

/// The seam every door into the merge queue must route through.
const SEAM: &str = "admission_refusal";

/// Totality words, in the sense a published claim uses them.
///
/// Shared by `assert_no_blanket_claim` and by the source scan: what Anvil may
/// not assert about a report at runtime is what it may not weld into a literal
/// either. An honest derived body — "69 of 72 gates passed, 3 produced no
/// measurement" — contains none of them.
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

// -------------------------------------------------------------------------
// Report fixtures
// -------------------------------------------------------------------------

/// A report in which every gate in the corpus reports `Passed`.
///
/// Built by round-tripping `PreMergeCertificationReport::unmeasured` through
/// serde rather than by naming seventy-two fields, so it stays correct when the
/// corpus grows. There is deliberately no "all passed" constructor in
/// production (invariant I2) and this fixture is not one — it is test data.
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
    /// One entry per line, with comment text and the *contents* of every string
    /// literal replaced by spaces. Line numbers still line up with the file,
    /// and the quote characters are kept, so a surviving `"` marks a literal.
    ///
    /// A token found here is code. Neither a comment explaining an invariant
    /// nor a `warn!` reminding callers of one can satisfy a scan for it.
    code: Vec<String>,
    /// Every string literal, as (1-based line, contents) — what a scan for a
    /// published claim reads.
    literals: Vec<(usize, String)>,
}

/// Reads production source, dropping everything from `#[cfg(test)]` onwards so
/// a call made by a module's own unit tests cannot answer a question about
/// production.
fn production(rel: &str) -> Production {
    let path = repo_path(rel);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let text = match text.find("#[cfg(test)]") {
        Some(i) => text[..i].to_string(),
        None => text,
    };
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

/// The decision a token at `idx` sits in: back to the previous `;`, `{` or `}`,
/// and forward either to the `;` ending the statement or, when the token opens
/// a block, past that block's closing `}`.
///
/// The forward half is the point. What becomes of a value is written after it:
/// `...enlist_into_merge_queue(..).await.ok();` throws a refusal away *after*
/// the call, and `if refusal.is_err() { return Ok(()) }` swallows one inside a
/// block. A window that stops at the token sees neither.
fn decision_span(text: &str, idx: usize) -> String {
    let start = text[..idx]
        .rfind([';', '{', '}'])
        .map(|i| i + 1)
        .unwrap_or(0);
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
                depth -= 1;
                if opened && depth <= 0 {
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
    let mut end = i.min(text.len());
    while end < text.len() && !text.is_char_boundary(end) {
        end += 1;
    }
    text[start..end].to_string()
}

/// Whether a span throws away the `Result` it is about, in any of the spellings
/// Rust offers for it.
fn discards_result(span: &str) -> bool {
    ["_ =", ".ok()", ".unwrap_or", "drop("]
        .iter()
        .any(|d| span.contains(d))
}

/// Whether a refusal in this span can actually stop anything.
///
/// Propagated, bailed, returned from, `continue`d past, or wrapped around the
/// enlistment itself. `return Ok` is excluded by name: that is the silent
/// no-op — the pull request is withheld and the caller is told it was admitted.
fn refusal_has_teeth(span: &str) -> bool {
    if discards_result(span) {
        return false;
    }
    span.contains('?')
        || span.contains("bail!")
        || span.contains("continue")
        || (span.contains("return") && !span.contains("return Ok"))
        || span.contains("enlist_into_merge_queue(")
}

/// Whether a slab of production code takes an admissibility decision it cannot
/// walk past. See the module doc for why `is_admissible` is not accepted here.
fn takes_an_admissibility_decision(code: &str) -> bool {
    let mut from = 0usize;
    while let Some(off) = code[from..].find(SEAM) {
        let idx = from + off;
        from = idx + SEAM.len();
        if refusal_has_teeth(&decision_span(code, idx)) {
            return true;
        }
    }
    false
}

/// The body of one method in a rustfmt-formatted `impl`, from its signature to
/// the closing `    }`.
fn find_method_body(source: &str, signature_fragment: &str) -> Option<String> {
    let start = source.find(signature_fragment)?;
    let rest = &source[start..];
    let end = rest.find("\n    }").map(|i| i + 6).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

fn method_body(source: &str, signature_fragment: &str) -> String {
    find_method_body(source, signature_fragment)
        .unwrap_or_else(|| panic!("no method matching `{signature_fragment}` in source"))
}

/// Methods this body calls and whose failure it propagates — where a guard
/// extracted into a helper would live.
fn propagated_callees(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for prefix in ["self.", "Self::"] {
        let mut from = 0usize;
        while let Some(off) = body[from..].find(prefix) {
            let idx = from + off;
            from = idx + prefix.len();
            let rest = &body[from..];
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if name.is_empty() || !rest[name.len()..].starts_with('(') {
                continue;
            }
            if refusal_has_teeth(&decision_span(body, idx)) {
                out.push(name);
            }
        }
    }
    out
}

/// Whether `enlist_into_merge_queue` itself takes the decision — directly, or
/// in a helper whose failure it propagates. Extracting a guard into a private
/// helper is a normal thing to do and must not read as an unguarded door.
fn entry_point_is_gated(source: &str) -> bool {
    let body = method_body(source, "fn enlist_into_merge_queue(");
    takes_an_admissibility_decision(&body)
        || propagated_callees(&body).iter().any(|callee| {
            find_method_body(source, &format!("fn {callee}("))
                .is_some_and(|helper| takes_an_admissibility_decision(&helper))
        })
}

/// One place in production source where a pull request is handed to the merge
/// queue.
struct MergeQueueDoor {
    file: String,
    line: usize,
    /// The fifty lines of production source immediately preceding the call.
    /// A guard placed further away than that is not a guard a reader can see.
    approach: String,
    /// What becomes of the `Result`: the call's own statement, read forward to
    /// the `;` that ends it.
    span: String,
}

/// Every call to `enlist_into_merge_queue`, excluding its own definition and
/// any mention of it inside a comment or a string literal.
fn merge_queue_doors() -> Vec<MergeQueueDoor> {
    const NEEDLE: &str = "enlist_into_merge_queue(";
    let mut doors = Vec::new();
    for rel in rust_sources_under("src") {
        let lines = production_lines(&rel);
        let text = lines.join("\n");
        let mut from = 0usize;
        while let Some(offset) = text[from..].find(NEEDLE) {
            let idx = from + offset;
            from = idx + NEEDLE.len();
            // A call, not the declaration: the identifier is reached through
            // `.`, ignoring the whitespace and newlines rustfmt puts in a
            // method chain.
            let preceding = text[..idx].trim_end();
            if !preceding.ends_with('.') {
                continue;
            }
            let line = text[..idx].matches('\n').count();
            let start = line.saturating_sub(50);
            doors.push(MergeQueueDoor {
                file: rel.clone(),
                line: line + 1,
                approach: lines[start..=line].join("\n"),
                span: decision_span(&text, idx),
            });
        }
    }
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
    let mut report = every_gate_passing();
    report.kani_status = not_measured("kani_status");
    report.seal();

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
///
/// This is also why the source scans require a door to route through
/// `admission_refusal` rather than accepting `is_admissible()` at the door.
#[test]
fn a_report_that_certifies_while_a_gate_errored_is_still_refused() {
    let mut report = every_gate_passing();
    report.slo_status = GateStatus::Errored("prometheus probe timed out".into());
    // Not sealed: this is a report that asserts certification it has not
    // earned, which is precisely the input a precondition exists to catch.
    report.is_certified_ready = true;
    report.recompute_unmeasured();

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

/// P5, P3. The defect is that the rule holds at one door of four. Either the
/// door itself takes the decision — one precondition inside
/// `enlist_into_merge_queue`, or in a helper whose failure it propagates — or
/// every caller does. Both are accepted; neither being true is the bug.
///
/// A decision, not a mention: see the module doc. String literals and comments
/// are blanked before this reads anything, and the refusal must divert control
/// flow rather than merely appear near the door.
#[test]
fn no_door_into_the_merge_queue_is_left_unchecked() {
    let doors = merge_queue_doors();
    assert!(
        !doors.is_empty(),
        "this scan found no call to `enlist_into_merge_queue`. Either the merge \
         queue entry point was renamed and this test must follow it, or the scan \
         is broken — a mechanism that cannot find its subject reports nothing \
         wrong with anything"
    );

    if entry_point_is_gated(&production_source("src/merge_enlister.rs")) {
        return;
    }

    let unchecked: Vec<String> = doors
        .iter()
        .filter(|d| !takes_an_admissibility_decision(&d.approach))
        .map(|d| format!("{}:{}", d.file, d.line))
        .collect();

    assert!(
        unchecked.is_empty(),
        "these paths hand a pull request to the merge queue without taking an \
         admissibility decision: {:?}\n\
         The entry point does not take one either, so nothing does. Invariant I1 — \
         absent evidence must never merge — is stated in \
         src/webhook/pipelines/review.rs and enforced only there; a rule held at \
         one door of {} is a convention, not an invariant. Fix it at the entry \
         point or at every caller, but not at some of them. A refusal that is \
         mentioned, logged or discarded rather than acted on does not count.",
        unchecked,
        doors.len()
    );
}

/// P3. `POST /api/enlist` binds the enlistment to `_` inside a detached task
/// and answers `202 ACCEPTED` regardless, so a refusal has nowhere to go: no
/// log, no response, no record. A refusal nobody can observe is indistinguishable
/// from an enlistment that happened.
///
/// Banned by shape rather than by spelling: `let _`, a bare `_ =`, `.ok()`,
/// `.unwrap_or*` and `drop(` throw the same `Result` away, and the window runs
/// forward to the end of the statement so the ones written after the call are
/// in scope.
#[test]
fn no_path_drops_a_merge_queue_refusal_on_the_floor() {
    let doors = merge_queue_doors();
    assert!(
        !doors.is_empty(),
        "this scan found no call to `enlist_into_merge_queue`; see \
         `no_door_into_the_merge_queue_is_left_unchecked`"
    );

    let discarded: Vec<String> = doors
        .iter()
        .filter(|d| discards_result(&d.span))
        .map(|d| {
            format!(
                "{}:{} — {}",
                d.file,
                d.line,
                d.span.split_whitespace().collect::<Vec<_>>().join(" ")
            )
        })
        .collect();

    assert!(
        discarded.is_empty(),
        "these paths discard the outcome of merge queue enlistment:\n{}\n\
         The refusal must be observable — surfaced to the caller or at minimum \
         logged. Thrown away it is a silent no-op, and the operator cannot tell \
         a withheld pull request from an admitted one.",
        discarded.join("\n")
    );
}

// =========================================================================
// Issue #18 — Anvil endorses nothing it did not measure
// =========================================================================

/// The honest answer when there is no report is to sign nothing. Today the
/// function that signs receives no report and signs anyway.
#[test]
fn nothing_is_endorsed_when_nothing_was_measured() {
    assert_eq!(
        MergeEnlister::approval_summary(None),
        None,
        "with no certification report there is nothing to derive a claim from, so \
         Anvil must publish no approving review at all"
    );
}

/// Publishing nothing at all is always honest, so two `None`s assert nothing.
/// Anything else must differ: an endorsement present on one report and absent
/// on the other has already discriminated.
fn assert_endorsements_differ(
    a: &PreMergeCertificationReport,
    b: &PreMergeCertificationReport,
    what: &str,
) {
    let on_a = MergeEnlister::approval_summary(Some(a));
    let on_b = MergeEnlister::approval_summary(Some(b));
    if on_a.is_none() && on_b.is_none() {
        return;
    }
    assert_ne!(on_a, on_b, "{what}");
}

/// P8. The defect is not the wording, it is that the wording is a constant: the
/// same sentence is signed onto every pull request in the fleet whatever its
/// gates did. Two reports that differ must not produce one endorsement.
///
/// The second pair is the one that bites. Both reports are admissible, so an
/// implementation that endorses only admissible pull requests publishes a
/// sentence for each and has to make them differ — which a constant cannot do.
/// A pair with an inadmissible side lets that implementation answer `None` and
/// assert nothing.
#[test]
fn the_endorsement_differs_when_the_evidence_differs() {
    let clean = every_gate_passing();

    let mut ragged = every_gate_passing();
    ragged.kani_status = not_measured("kani_status");
    ragged.coverage_status = GateStatus::Failed("coverage below the ratchet".into());
    ragged.seal();
    assert_endorsements_differ(
        &clean,
        &ragged,
        "the same endorsement was produced for a pull request whose gates all \
         passed and for one with a failed gate and a gate that produced no \
         measurement. A claim identical across both is derived from neither",
    );

    let mut warned = every_gate_passing();
    warned.bench_status = GateStatus::Warning("throughput regressed within tolerance".into());
    warned.seal();
    assert!(
        warned.is_admissible(),
        "fixture sanity: a Warning is acceptable and measured, so this report is \
         still admissible — whatever the clean report is endorsed with, this one \
         is endorsed with too"
    );
    assert_endorsements_differ(
        &clean,
        &warned,
        "the same endorsement was produced for a pull request with a clean bench \
         gate and for one whose bench gate reported a warning. Both are \
         admissible, so both are signed — with one constant sentence, which is \
         issue #18 restored verbatim",
    );
}

/// P8. The only test that reads the text Anvil publishes onto a pull request it
/// actually admits. A warning is not a pass, so an endorsement written over one
/// must not sweep the corpus into a total.
#[test]
fn an_endorsement_asserts_no_total_when_a_gate_only_warned() {
    let mut report = every_gate_passing();
    report.bench_status = GateStatus::Warning("throughput regressed within tolerance".into());
    report.seal();
    assert!(
        report.is_admissible(),
        "fixture sanity: this pull request is admitted, so whatever Anvil signs \
         onto it is signed onto a merge that really happens"
    );

    if let Some(text) = MergeEnlister::approval_summary(Some(&report)) {
        assert_no_blanket_claim(&text, "bench_status reported a warning, not a pass");
    }
}

/// P6 and P10. A gate reporting `NotMeasured` made no claim in either
/// direction. An endorsement that sweeps it into a total — "all gates", "100%"
/// — asserts on its behalf something nobody measured; and one built from
/// `gate_counts()`, which scores `NotMeasured` as acceptable, does the same
/// thing in arithmetic instead of adjectives.
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
         ready-made figure here is the whole corpus — that is the number an \
         endorsement must not publish as what passed"
    );

    let Some(text) = MergeEnlister::approval_summary(Some(&report)) else {
        return;
    };
    assert_no_blanket_claim(&text, "three gates produced no measurement");
    assert!(
        report
            .unmeasured_gates
            .iter()
            .any(|gate| text.contains(gate.as_str()))
            || mentions_number(&text, report.unmeasured_gates.len()),
        "the approving review Anvil signs says nothing about the {} gates that \
         produced no measurement: it names none of {:?} and states no count. A \
         reader is told how many gates passed and cannot discover that part of \
         the evidence is missing. Body was:\n{text}",
        report.unmeasured_gates.len(),
        report.unmeasured_gates
    );
}

/// P9 and P10. `unmeasured_gates` tracks `NotMeasured` only. An endorsement
/// derived from that field alone still describes an `Errored` gate —
/// configured, attempted, no result — as one of the gates that passed.
#[test]
fn an_endorsement_asserts_no_total_when_a_gate_errored() {
    let mut report = every_gate_passing();
    report.security_scan_status = GateStatus::Errored("scanner binary not found".into());
    report.bench_status = GateStatus::Errored("harness did not start".into());
    report.seal();

    assert!(
        report.unmeasured_gates.is_empty(),
        "fixture sanity: `unmeasured_gates` records NotMeasured only, so a body \
         derived from that field alone sees nothing wrong with this report"
    );

    let Some(text) = MergeEnlister::approval_summary(Some(&report)) else {
        return;
    };
    assert_no_blanket_claim(&text, "two gates errored");
    assert!(
        text.contains("security_scan_status")
            || text.contains("bench_status")
            || mentions_number(&text, 2),
        "the approving review Anvil signs says nothing about the two gates that \
         errored: it names neither and states no count. Body was:\n{text}"
    );
}

/// Totality words, in the sense the published approval uses them. A body that
/// reports "69 of 72 gates passed, 3 produced no measurement" trips none of
/// these; the sentence in the tree today trips two.
fn assert_no_blanket_claim(text: &str, context: &str) {
    let lower = text.to_lowercase();
    for claim in TOTALITY {
        assert!(
            !lower.contains(claim),
            "the approving review Anvil signs asserts \"{claim}\" while {context}. \
             The review record is permanent and a reader cannot check it against \
             anything. Either derive the sentence from the report — asserting \
             nothing about gates that produced no measurement — or publish no \
             approval. Body was:\n{text}"
        );
    }
}

/// P6, at source level. Two literals live in `merge_enlister.rs` today: one in
/// the approval body, one in the enlistment note posted immediately after it.
/// Deleting the first and leaving the second fixes nothing, and neither does
/// moving either into a `const`, a helper or a sibling file.
///
/// It reads the same `TOTALITY` vocabulary `assert_no_blanket_claim` uses, so a
/// sentence that would be refused at runtime cannot be welded into a literal
/// instead. Rewording is not a fix either: what is banned is a *fixed* sentence
/// asserting a total, whatever words it picks. A derived body —
/// `format!("{n} of {TOTAL_GATES} gates passed, {m} produced no measurement")` —
/// contains none of them.
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

/// P7. `approval_summary` can be implemented perfectly and never reached: the
/// production path keeps building its own sentence in a function that, as issue
/// #18 puts it, "receives no report — nothing measurable is in scope".
///
/// The link is what is pinned, not vocabulary: the submitting method must call
/// `approval_summary`, must not paper over an absent summary with a fallback,
/// and must not hand a string literal to the review it submits. Vacuously
/// satisfied if the self-approval is dropped — nothing submits a review, so
/// nothing has to hold a report.
#[test]
fn the_approving_review_is_not_written_by_a_function_that_holds_no_report() {
    const SUBMIT: &str = ".submit_pr_review(";
    let files = published_text_files();
    let submitting: Vec<&String> = files
        .iter()
        .filter(|f| production_source(f).contains(SUBMIT))
        .collect();
    if submitting.is_empty() {
        return;
    }

    let source = production_source("src/merge_enlister.rs");
    assert!(
        source.contains(SUBMIT),
        "an approving review is submitted from {submitting:?} rather than from \
         src/merge_enlister.rs; relocating it does not make it honest and this \
         test must follow it"
    );

    let body = method_body(&source, "fn ensure_approving_review(");
    assert!(
        body.contains(SUBMIT),
        "the approving review is submitted from somewhere other than \
         `ensure_approving_review`; this test must follow it"
    );

    let derived = body.find("approval_summary(").unwrap_or_else(|| {
        panic!(
            "`ensure_approving_review` submits a formal GitHub APPROVE without \
             calling `approval_summary`, so every word of the body it signs is \
             asserted from nothing. Derive the text from the report, or stop \
             self-approving."
        )
    });
    let span = decision_span(&body, derived);
    assert!(
        !discards_result(&span) && !span.contains("unwrap_or"),
        "an absent summary is Anvil saying it measured nothing worth signing. \
         Falling back to a sentence of its own restores the defect one line \
         further down; publish no review instead. Got: {span}"
    );
    assert!(
        body[derived..].contains(SUBMIT),
        "`approval_summary` is called after the review is submitted, so its \
         result cannot be what was signed"
    );

    // String-literal contents are blanked above, so a surviving `"` on the line
    // that hands the body over is a fixed sentence reaching the review record.
    let literal_body: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|l| (l.starts_with("summary:") || l.contains(SUBMIT)) && l.contains('"'))
        .collect();
    assert!(
        literal_body.is_empty(),
        "a string literal is handed straight to the review Anvil submits: {literal_body:?}\n\
         Whatever `approval_summary` returns, this is what gets signed."
    );
}
