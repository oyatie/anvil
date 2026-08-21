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
//! P3. The refusal is a silent `return Ok(())`. Nothing merges and nobody can
//!     say why; the operator concludes the daemon is wedged and disables it.
//!     -> every refusal test asserts a reason, and
//!        `no_path_drops_a_merge_queue_refusal_on_the_floor` bans discarding it.
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
//!        scans every source file on the enlistment path, not one function.
//! P7. `approval_summary` is implemented correctly and never called: the
//!     production path keeps writing its own sentence with no report in scope.
//!     -> `the_approving_review_is_not_written_by_a_function_that_holds_no_report`.
//! P8. The claim is reworded rather than derived — a different literal, equally
//!     unmeasured, identical for every pull request in the fleet.
//!     -> `the_endorsement_differs_when_the_evidence_differs`, which pins
//!        derivation without pinning one wording.
//! P9. The text is derived from `unmeasured_gates` only, so a gate that
//!     `Errored` — configured, attempted, no result — is still described as
//!     having passed.
//!     -> `an_endorsement_asserts_no_total_when_a_gate_errored`.
//!
//! # Scaffolding this suite depends on
//!
//! Two signatures with `todo!()` bodies in `src/merge_enlister.rs`:
//! `MergeEnlister::admission_refusal` and `MergeEnlister::approval_summary`.
//! They exist so the invariant can be stated before anything implements it.
//! Neither prescribes where the decision is wired: `no_door_into_the_merge_queue_is_left_unchecked`
//! deliberately accepts the entry point checking once or every caller checking
//! for itself, and returning `None` from `approval_summary` for every report —
//! that is, dropping self-approval altogether — is a valid implementation.
//!
//! # Not pinned here, and why
//!
//! `ensure_approving_review` already fails closed on `CHANGES_REQUESTED` and on
//! unresolved review threads, and the crate uses no `--admin` anywhere. Those
//! are green today, so a guard for them would be a green test in a lane whose
//! rule is that every spec test starts red. They are called out in the handoff
//! instead.

use anvil::merge_enlister::MergeEnlister;
use anvil::pre_merge_guard::report::{GateStatus, PreMergeCertificationReport, TOTAL_GATES};
use std::fs;
use std::path::{Path, PathBuf};

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

// -------------------------------------------------------------------------
// Source-scanning helpers
//
// These paths shell out to `gh` and take an `AppState` with ~90 Arc fields, so
// the wiring between the decision and the door cannot be exercised in-process
// without a network. It is pinned as a mechanism over source text instead —
// the idiom already used by `tests/no_autonomous_destructive_actions_test.rs`
// and `tests/api_auth_and_prompt_delimiting_test.rs`.
// -------------------------------------------------------------------------

fn repo_path(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

/// Production source only, with comment lines blanked in place.
///
/// Everything from `#[cfg(test)]` onwards is dropped so a call made by a
/// module's own unit tests cannot answer a question about production. Comment
/// lines are replaced by empty lines rather than removed so line numbers still
/// line up with the file — a comment explaining an invariant must not be able
/// to satisfy a scan for it.
fn production_lines(rel: &str) -> Vec<String> {
    let path = repo_path(rel);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let text = match text.find("#[cfg(test)]") {
        Some(i) => text[..i].to_string(),
        None => text,
    };
    text.lines()
        .map(|l| {
            if l.trim_start().starts_with("//") {
                String::new()
            } else {
                l.to_string()
            }
        })
        .collect()
}

fn production_source(rel: &str) -> String {
    production_lines(rel).join("\n")
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

/// One place in production source where a pull request is handed to the merge
/// queue.
struct MergeQueueDoor {
    file: String,
    line: usize,
    /// The fifty lines of production source immediately preceding the call.
    /// A guard placed further away than that is not a guard a reader can see.
    approach: String,
    /// The statement the call sits in: back to the previous `;`, `{` or `}`.
    statement: String,
}

/// Every call to `enlist_into_merge_queue`, excluding its own definition and
/// any mention of it inside a string literal.
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
            // A call, not the declaration and not prose: the identifier is
            // reached through `.`, ignoring the whitespace and newlines
            // rustfmt puts in a method chain.
            let preceding = text[..idx].trim_end();
            if !preceding.ends_with('.') {
                continue;
            }
            let line = text[..idx].matches('\n').count();
            let start = line.saturating_sub(50);
            let approach = lines[start..=line].join("\n");
            let stmt_start = text[..idx]
                .rfind([';', '{', '}'])
                .map(|i| i + 1)
                .unwrap_or(0);
            doors.push(MergeQueueDoor {
                file: rel.clone(),
                line: line + 1,
                approach,
                statement: text[stmt_start..idx].to_string(),
            });
        }
    }
    doors
}

/// Whether a slab of source consults admissibility at all.
///
/// Both names are accepted so neither design is forced: `is_admissible` is the
/// predicate the review pipeline already uses, `admission_refusal` is the seam
/// this lane scaffolds.
fn consults_admissibility(code: &str) -> bool {
    code.contains("is_admissible") || code.contains("admission_refusal")
}

/// The body of one method in a rustfmt-formatted `impl`, from its signature to
/// the closing `    }`.
fn method_body(source: &str, signature_fragment: &str) -> String {
    let start = source
        .find(signature_fragment)
        .unwrap_or_else(|| panic!("no method matching `{signature_fragment}` in source"));
    let rest = &source[start..];
    let end = rest.find("\n    }").map(|i| i + 6).unwrap_or(rest.len());
    rest[..end].to_string()
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

/// P5. The defect is that the rule holds at one door of four. Either the door
/// itself checks — one precondition inside `enlist_into_merge_queue` — or every
/// caller does. Both are accepted; neither being true is the bug.
#[test]
fn no_door_into_the_merge_queue_is_left_unchecked() {
    let enlister = production_source("src/merge_enlister.rs");
    let entry_point = method_body(&enlister, "fn enlist_into_merge_queue(");
    if consults_admissibility(&entry_point) {
        return;
    }

    let doors = merge_queue_doors();
    assert!(
        !doors.is_empty(),
        "this scan found no call to `enlist_into_merge_queue`. Either the merge \
         queue entry point was renamed and this test must follow it, or the scan \
         is broken — a mechanism that cannot find its subject reports nothing \
         wrong with anything"
    );

    let unchecked: Vec<String> = doors
        .iter()
        .filter(|d| !consults_admissibility(&d.approach))
        .map(|d| format!("{}:{}", d.file, d.line))
        .collect();

    assert!(
        unchecked.is_empty(),
        "these paths hand a pull request to the merge queue without consulting \
         admissibility: {:?}\n\
         The entry point does not check either, so nothing does. Invariant I1 — \
         absent evidence must never merge — is stated in \
         src/webhook/pipelines/review.rs and enforced only there; a rule held at \
         one door of {} is a convention, not an invariant. Fix it at the entry \
         point or at every caller, but not at some of them.",
        unchecked,
        doors.len()
    );
}

/// P3. `POST /api/enlist` binds the enlistment to `_` inside a detached task
/// and answers `202 ACCEPTED` regardless, so a refusal has nowhere to go: no
/// log, no response, no record. A refusal nobody can observe is indistinguishable
/// from an enlistment that happened.
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
        .filter(|d| d.statement.contains("let _"))
        .map(|d| format!("{}:{}", d.file, d.line))
        .collect();

    assert!(
        discarded.is_empty(),
        "these paths discard the outcome of merge queue enlistment: {:?}\n\
         The refusal must be observable — surfaced to the caller or at minimum \
         logged. Bound to `_` it is a silent no-op, and the operator cannot tell \
         a withheld pull request from an admitted one.",
        discarded
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

/// P8. The defect is not the wording, it is that the wording is a constant: the
/// same sentence is signed onto every pull request in the fleet whatever its
/// gates did. Two reports that differ must not produce one endorsement.
///
/// Publishing nothing is always honest, so both `None` outcomes are accepted —
/// dropping self-approval entirely is a valid fix.
#[test]
fn the_endorsement_differs_when_the_evidence_differs() {
    let clean = every_gate_passing();

    let mut ragged = every_gate_passing();
    ragged.kani_status = not_measured("kani_status");
    ragged.coverage_status = GateStatus::Failed("coverage below the ratchet".into());
    ragged.seal();

    match (
        MergeEnlister::approval_summary(Some(&clean)),
        MergeEnlister::approval_summary(Some(&ragged)),
    ) {
        (Some(on_clean), Some(on_ragged)) => assert_ne!(
            on_clean, on_ragged,
            "the same sentence was signed onto a pull request whose gates all passed \
             and onto one with a failed gate and a gate that produced no \
             measurement. A claim identical across both is derived from neither"
        ),
        // Endorsing nothing, on either or both, asserts nothing.
        _ => {}
    }
}

/// P6. A gate reporting `NotMeasured` made no claim in either direction. An
/// endorsement that sweeps it into a total — "all gates", "100%" — asserts on
/// its behalf something nobody measured.
///
/// This pins the absence of a blanket claim, not any particular wording: an
/// honest body naming the counts it really has passes.
#[test]
fn an_endorsement_asserts_no_total_when_a_gate_was_not_measured() {
    let mut report = every_gate_passing();
    report.kani_status = not_measured("kani_status");
    report.seal();

    if let Some(text) = MergeEnlister::approval_summary(Some(&report)) {
        assert_no_blanket_claim(&text, "kani_status produced no measurement");
    }
}

/// P9. `unmeasured_gates` tracks `NotMeasured` only. An endorsement derived
/// from that field alone still describes an `Errored` gate — configured,
/// attempted, no result — as one of the gates that passed.
#[test]
fn an_endorsement_asserts_no_total_when_a_gate_errored() {
    let mut report = every_gate_passing();
    report.security_scan_status = GateStatus::Errored("scanner binary not found".into());
    report.seal();

    if let Some(text) = MergeEnlister::approval_summary(Some(&report)) {
        assert_no_blanket_claim(&text, "security_scan_status errored");
    }
}

/// Totality words, in the sense the published approval uses them. A body that
/// reports "71 of 72 gates passed, 1 produced no measurement" trips none of
/// these; the sentence in the tree today trips two.
fn assert_no_blanket_claim(text: &str, context: &str) {
    const TOTALITY: [&str; 7] = [
        "100%",
        "all automated",
        "all gates",
        "all checks",
        "all safety",
        "every gate",
        "fully compliant",
    ];
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
/// Scoped to source on the enlistment path rather than one function, so a move
/// is still caught, and rather than all of `src`, so a gate reporting its own
/// measured finding ("100% compliant across 40 rules") is not swept up — this
/// lane owns the merge-queue claims, not every percentage in the crate.
#[test]
fn no_published_string_claims_a_compliance_total_that_no_gate_produced() {
    // A percentage on its own is not a claim about the corpus -- "100% parity",
    // "100% in sync" describe one gate's own finding. A percentage welded to a
    // compliance verdict is.
    const VERDICT_WORDS: [&str; 3] = ["compliance", "green", "have passed"];

    let mut offenders: Vec<String> = Vec::new();
    let mut scanned = 0usize;
    for rel in rust_sources_under("src") {
        let lines = production_lines(&rel);
        if !lines.iter().any(|l| l.contains("enlist")) {
            continue;
        }
        scanned += 1;
        for (i, line) in lines.iter().enumerate() {
            let lower = line.to_lowercase();
            if lower.contains("100%") && VERDICT_WORDS.iter().any(|w| lower.contains(w)) {
                offenders.push(format!("{}:{}: {}", rel, i + 1, line.trim()));
            }
        }
    }

    assert!(
        scanned > 0,
        "no production source on the enlistment path was found; the scan is broken \
         and would report nothing wrong with anything"
    );
    assert!(
        offenders.is_empty(),
        "these lines on the enlistment path publish a total compliance verdict as a \
         string literal:\n{}\n\
         Nothing measured them. They are written onto the pull request, where a \
         reader has no way to tell them apart from a real result. A count Anvil \
         publishes is a claim like any other and must come from the report.",
        offenders.join("\n")
    );
}

/// P7. `approval_summary` can be implemented perfectly and never reached: the
/// production path keeps building its own sentence in a function that, as issue
/// #18 puts it, "receives no report — nothing measurable is in scope".
///
/// Vacuously satisfied if the self-approval is dropped: no submission, nothing
/// to hold a report.
#[test]
fn the_approving_review_is_not_written_by_a_function_that_holds_no_report() {
    let source = production_source("src/merge_enlister.rs");
    if !source.contains("submit_pr_review(") {
        return;
    }

    let body = method_body(&source, "fn ensure_approving_review(");
    assert!(
        body.contains("submit_pr_review("),
        "the approving review is submitted from somewhere other than \
         `ensure_approving_review`; this test must follow it"
    );

    let lower = body.to_lowercase();
    let holds_evidence = lower.contains("approval_summary")
        || lower.contains("premergecertificationreport")
        || lower.contains("report");
    assert!(
        holds_evidence,
        "`ensure_approving_review` submits a formal GitHub APPROVE with no \
         certification report in scope, so every word of the body it signs is \
         asserted from nothing. Pass the report in and derive the text, or stop \
         self-approving."
    );
}
