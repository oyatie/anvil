//! Red tests for the `brand-absence` gate (plan §36.5, law §33 L1 / D33.2).
//!
//! # The defect these tests exist to catch
//!
//! Anvil's names and PR-visible strings carry *aspiration and category claims*
//! instead of describing what the code does. Verified in the tree at the time
//! of writing:
//!
//!   - `src/hyperscaler_consensus_guard/mod.rs` — the module actually detects
//!     unbounded `tokio` channels and `thread::sleep`; the name claims a
//!     five-vendor consensus.
//!   - `src/cloud_native_guard/mod.rs` — actually checks
//!     `PROPRIETARY_CLOUD_SDK_IN_CORE`, `HARDCODED_CLOUD_ENDPOINT`,
//!     `NON_RUST_SCRIPT_TOOLING`; the name states a category, and the category
//!     label is what let two unrelated checks be bundled under one module.
//!   - `src/ai_driver/stage_router.rs:60` — `StageModelRouter`
//!     actually returns a per-stage model fallback chain.
//!   - `src/hyperscaler_consensus_guard/mod.rs:195` —
//!     `"✅ UNANIMOUS APPROVAL (5/5 Hyperscalers Approved: AWS, GCP, Meta, Azure, OCI)"`
//!   - `src/monorepo_guard/disposition.rs:77` —
//!     `"Hyperscaler pattern mandates live Rust AST / Protobuf reflection"`
//!   - `src/monorepo_guard/mod.rs:53` — `"Running MonorepoGuard hyperscaler patterns on {}#{}..."`
//!   - `src/ai_driver/stage_router.rs:31` — `"7. 16-Lens Code Review & Hyperscaler Consensus"`
//!   - three log sites claim **70 gates** while the corpus is **68**:
//!     `src/webhook/pipelines/review.rs:23`, `src/pre_merge_guard/evaluator.rs:159`,
//!     `src/cli/server.rs:56`. The PR scorecard header
//!     (`src/pre_merge_guard/matrix.rs:98`) makes the same claim in the comment
//!     Anvil posts. Verified real count: 68 `GateStatus` fields on
//!     `PreMergeCertificationReport`, 68 entries in `all_statuses()`, and
//!     `report.rs`'s own test asserts `(68, 0)`.
//!
//! # Why prompting would not prevent this
//!
//! Every one of the above was written by a model that had been told to follow
//! hyperscaler engineering practice. The instruction *produced* the defect: an
//! aspiration in the prompt becomes an aspiration in the identifier, because a
//! grand-sounding name reads as evidence of having met the bar. The defect is
//! also invisible at review time — nothing in a diff distinguishes a truthful
//! name from a boastful one, and the boastful one looks *more* rigorous. A
//! count in a string is worse still: "70-gate" was true once, the corpus shrank
//! to 68, and no prompt can notice a number drifting away from the thing it
//! counts. Only a mechanical check comparing the claim to `gate_counts()` can.
//!
//! # Stage discipline
//!
//! These tests are written before the gate exists and are expected to fail to
//! compile (`E0432`: `anvil::brand_absence` is unresolved). Renames of the
//! offending modules are deliberately **out of scope** — plan §36.2/C1
//! sequences renaming after the retain/discard determination, because renaming
//! code that is about to be deleted is waste. This lane ships the *gate*, in
//! warn-only mode with the known violations recorded as an enumerated debt
//! ledger, so it lands green while every NEW violation becomes visible.

use anvil::brand_absence::{
    AllowlistedDebt, BrandAbsenceGate, BrandViolationKind, FORBIDDEN_STAMPS, KNOWN_VIOLATIONS,
};
use std::path::Path;

/// A hermetic allowlist used by the tests that need to exercise the ratchet
/// without depending on the exact contents of the production debt ledger.
static TEST_ALLOWLIST: &[AllowlistedDebt] = &[AllowlistedDebt {
    path: "src/legacy/known_offender.rs",
    stamp: "hyperscaler",
    occurrences: 1,
    debt_note: "test fixture: one recorded occurrence, ratchet must not allow a second",
}];

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

// ---------------------------------------------------------------------------
// 1. Names
// ---------------------------------------------------------------------------

/// Catches: a crate/module/type **name** that stamps an aspiration or a product
/// category onto itself instead of naming the check it performs — the class
/// that produced `hyperscaler_consensus_guard`, `cloud_native_guard`, and
/// `StageModelRouter`.
///
/// Prompting cannot prevent it: the aspiration arrives *through* the prompt.
/// Told to build to hyperscaler standard, a model names the artifact
/// "hyperscaler_*", and the name then reads as a claim the code has met that
/// standard. Nothing in review distinguishes the claim from the fact.
#[test]
fn flags_an_aspiration_stamp_in_a_module_or_type_name() {
    let gate = BrandAbsenceGate::with_allowlist(TEST_ALLOWLIST);

    let synthetic = r#"
pub struct EnterpriseThroughputOptimizer;

impl EnterpriseThroughputOptimizer {
    pub fn optimize(&self) -> usize { 0 }
}
"#;

    let report = gate.scan_source("src/synthetic/new_module.rs", synthetic);

    assert!(
        !report.new_violations.is_empty(),
        "gate must flag an aspiration stamp in a type name; got no violations. summary={}",
        report.summary
    );
    assert!(
        report
            .new_violations
            .iter()
            .any(|v| v.kind == BrandViolationKind::Name
                && v.stamp.to_lowercase().contains("enterprise")),
        "expected a Name violation carrying the 'enterprise' stamp, got {:?}",
        report.new_violations
    );
}

/// Catches: the same defect expressed as a module path rather than a type name
/// (`src/hyperscaler_consensus_guard/mod.rs`). A gate that only inspected type
/// declarations would miss the two worst real instances, both of which are
/// directory names.
///
/// Prompting cannot prevent it: directory names are chosen once, at scaffold
/// time, and are never re-read afterwards. There is no later moment at which a
/// reviewer is asked "does this folder name describe what is inside it?".
#[test]
fn flags_an_aspiration_stamp_in_a_module_path() {
    let gate = BrandAbsenceGate::with_allowlist(TEST_ALLOWLIST);

    let report = gate.scan_source(
        "src/hyperscale_throughput_guard/mod.rs",
        "pub struct ThroughputGuard;\n",
    );

    assert!(
        report
            .new_violations
            .iter()
            .any(|v| v.kind == BrandViolationKind::Name),
        "gate must flag an aspiration stamp carried by the module path itself, got {:?}",
        report.new_violations
    );
}

// ---------------------------------------------------------------------------
// 2. PR-visible display strings
// ---------------------------------------------------------------------------

/// Catches: an aspiration stamp inside a **display string or log line**, which
/// D33.2 covers explicitly *because those reach a pull request*. The real
/// instances are `"✅ UNANIMOUS APPROVAL (5/5 Hyperscalers Approved: ...)"` and
/// `"Hyperscaler pattern mandates live Rust AST / Protobuf reflection"` — a
/// vendor roll-call and an appeal to authority, posted onto someone's PR as if
/// they were findings.
///
/// Prompting cannot prevent it: a linter that only reads identifiers passes
/// this file, and a human reviewer reads the string as flavour text rather than
/// as an assertion. The string is the part that escapes the repository, so it
/// is exactly the part that must be checked mechanically.
#[test]
fn flags_an_aspiration_stamp_in_a_pr_visible_display_string() {
    let gate = BrandAbsenceGate::with_allowlist(TEST_ALLOWLIST);

    let synthetic = r#"
fn emit() {
    info!("UNANIMOUS APPROVAL (5/5 Hyperscalers Approved: AWS, GCP, Meta, Azure, OCI)");
}
"#;

    let report = gate.scan_source("src/synthetic/reporter.rs", synthetic);

    assert!(
        report
            .new_violations
            .iter()
            .any(|v| v.kind == BrandViolationKind::DisplayString),
        "gate must flag an aspiration stamp inside a PR-visible string, got {:?}",
        report.new_violations
    );
    assert!(
        report
            .new_violations
            .iter()
            .any(|v| v.snippet.contains("UNANIMOUS APPROVAL")),
        "violation must carry the offending snippet so the author can see what to change, got {:?}",
        report.new_violations
    );
}

/// Catches: a clean file being falsely accused. A gate that flags everything is
/// as useless as one that flags nothing, and a noisy warn-only gate is ignored
/// within a week — which would leave the real defect unguarded while appearing
/// to guard it (invariant I1: absent evidence is never a pass, but nor is it a
/// false accusation).
#[test]
fn does_not_flag_a_string_that_states_the_check_it_performs() {
    let gate = BrandAbsenceGate::with_allowlist(TEST_ALLOWLIST);

    let synthetic = r#"
fn emit() {
    info!("unbounded tokio channel detected; use a bounded channel with explicit capacity");
}
"#;

    let report = gate.scan_source("src/synthetic/constant_work.rs", synthetic);

    assert!(
        report.new_violations.is_empty(),
        "a string that names the check must not be flagged, got {:?}",
        report.new_violations
    );

    // Paired positive case, so this test cannot be satisfied by a gate that
    // simply never flags anything.
    let aspirational = r#"
fn emit() {
    info!("hyperscaler-grade backpressure certified");
}
"#;
    let dirty = gate.scan_source("src/synthetic/constant_work.rs", aspirational);
    assert_eq!(
        dirty.new_violations.len(),
        1,
        "the same file must flag the aspirational variant; a gate that flags neither is not \
         discriminating, it is inert. got {:?}",
        dirty.new_violations
    );
}

// ---------------------------------------------------------------------------
// 3. Gate-count claims
// ---------------------------------------------------------------------------

/// Catches: a hardcoded count in a PR-visible string that disagrees with the
/// real corpus size. Verified: the tree says "70-gate" in three log sites and
/// in the posted scorecard header, while `PreMergeCertificationReport` carries
/// 68 `GateStatus` fields and `all_statuses()` returns 68 entries.
///
/// Prompting cannot prevent it: the number was accurate when it was typed. It
/// became a lie through an unrelated edit elsewhere in the tree — a gate being
/// removed or merged — and no prompt, review, or memory catches a constant
/// drifting away from the thing it purports to count. Only a comparison
/// against the live count does.
#[test]
fn flags_a_gate_count_claim_that_disagrees_with_the_real_count() {
    let gate = BrandAbsenceGate::with_allowlist(TEST_ALLOWLIST);
    let real = gate.real_gate_count();

    assert_eq!(
        real,
        anvil::pre_merge_guard::report::TOTAL_GATES,
        "the corpus size must come from TOTAL_GATES, not a literal. This test hardcoded 68 \
         and went stale the moment two real gates were added -- the same way seven PR-visible \
         strings came to claim 70 against a corpus of 68"
    );

    // Derived so it is wrong BY CONSTRUCTION. This fixture used to hardcode
    // "70-Gate" to be wrong against a corpus of 68 -- then two real gates were
    // added, the corpus became 70, and the fixture silently became CORRECT.
    // The test then passed for the opposite of its stated reason, which is the
    // same class of decay it exists to catch.
    let wrong = real + 1;
    let synthetic = format!(
        "fn emit() {{\n    info!(\"Executing AI Code Review & {wrong}-Gate Pipeline for {{}}#{{}}\", repo, pr);\n}}\n"
    );

    let report = gate.scan_source("src/synthetic/pipeline.rs", &synthetic);

    assert!(
        report
            .new_violations
            .iter()
            .any(|v| v.kind == BrandViolationKind::GateCountClaim),
        "gate must flag a {wrong}-gate claim while the real count is {real}, got {:?}",
        report.new_violations
    );
    assert!(
        report
            .new_violations
            .iter()
            .any(|v| v.kind == BrandViolationKind::GateCountClaim
                && v.stamp == wrong.to_string()
                && v.snippet.contains(&format!("{wrong}-Gate"))),
        "the violation must name the claimed count ({wrong}) and carry the offending snippet, \
         got {:?}",
        report.new_violations
    );
    assert!(
        report.summary.contains(&real.to_string()),
        "the summary must state the real count ({real}) so the author knows what to write \
         instead; got: {}",
        report.summary
    );
}

/// Catches: a gate-count checker that flags *any* number near the word "gate",
/// which would fire on every honest string and force the whole gate to be
/// switched off. The truthful claim must pass.
#[test]
fn accepts_a_gate_count_claim_that_matches_the_real_count() {
    let gate = BrandAbsenceGate::with_allowlist(TEST_ALLOWLIST);
    let real = gate.real_gate_count();
    assert!(
        real > 0,
        "real_gate_count() returned 0; the gate has no source of truth to compare claims \
         against, so every count check below would be meaningless"
    );

    let synthetic = format!("info!(\"Evaluating {real} gates for {{}}#{{}}\", repo, pr);\n");

    let report = gate.scan_source("src/synthetic/honest.rs", &synthetic);

    assert!(
        report
            .new_violations
            .iter()
            .all(|v| v.kind != BrandViolationKind::GateCountClaim),
        "a claim that matches the real count of {real} must not be flagged, got {:?}",
        report.new_violations
    );

    // Paired positive case: off by one in either direction is still a false
    // claim, and this keeps the test from passing against an inert gate.
    for wrong in [real - 1, real + 1] {
        let bad = format!("info!(\"Evaluating {wrong} gates for {{}}#{{}}\", repo, pr);\n");
        let bad_report = gate.scan_source("src/synthetic/honest.rs", &bad);
        assert!(
            bad_report
                .new_violations
                .iter()
                .any(|v| v.kind == BrandViolationKind::GateCountClaim),
            "a claim of {wrong} gates against a real count of {real} must be flagged, got {:?}",
            bad_report.new_violations
        );
    }
}

// ---------------------------------------------------------------------------
// 4. The allowlist: debt ledger, not exemption
// ---------------------------------------------------------------------------

/// Catches: the gate blocking on pre-existing debt. Ship-condition for this
/// lane is warn-only with the known violations recorded, so the gate lands
/// green on a tree that currently contains dozens of them. If an allowlisted
/// violation were reported as new, the gate could not be merged, and a gate
/// that cannot be merged guards nothing.
#[test]
fn does_not_flag_an_allowlisted_known_violation() {
    let gate = BrandAbsenceGate::with_allowlist(TEST_ALLOWLIST);

    let report = gate.scan_source(
        "src/legacy/known_offender.rs",
        "info!(\"Running MonorepoGuard hyperscaler patterns on {}#{}...\", repo, pr);\n",
    );

    assert!(
        report.new_violations.is_empty(),
        "an allowlisted (path, stamp) at its recorded occurrence count must not be reported \
         as new, got {:?}",
        report.new_violations
    );
    assert_eq!(
        report.allowlisted_hits, 1,
        "the hit must still be counted as debt rather than silently dropped; summary={}",
        report.summary
    );
}

/// Catches: an allowlist that absorbs new violations. This is the failure mode
/// that turns a debt ledger into an exemption — once a file is listed, every
/// future violation in it disappears, and the dirtiest files become the least
/// guarded. The allowlist records an **occurrence count**, so the recorded
/// number is a ceiling: the (N+1)th occurrence is new and must be reported.
///
/// Prompting cannot prevent it: "don't add more hyperscaler strings" is exactly
/// the instruction that was already in force when all the existing ones were
/// written.
#[test]
fn allowlist_does_not_absorb_a_new_violation_in_an_already_listed_file() {
    let gate = BrandAbsenceGate::with_allowlist(TEST_ALLOWLIST);

    // TEST_ALLOWLIST records exactly one 'hyperscaler' occurrence for this path.
    let two_occurrences = "info!(\"hyperscaler patterns\");\ninfo!(\"hyperscaler consensus\");\n";

    let report = gate.scan_source("src/legacy/known_offender.rs", two_occurrences);

    assert_eq!(
        report.new_violations.len(),
        1,
        "the occurrence beyond the recorded ceiling of 1 must be reported as new; \
         an allowlist keyed only by path would report 0. got {:?}",
        report.new_violations
    );
}

/// Catches: an allowlist that is a pattern rather than an enumeration. A
/// wildcard, a directory prefix, or a regex entry lets unbounded future
/// violations in under a single line of diff, and no reviewer would read
/// `"src/*"` as the licence it is.
///
/// Also asserts every listed path exists and actually contains its stamp, so
/// the ledger cannot carry speculative or stale entries — an entry must be a
/// verified fact about the tree, not a reservation.
#[test]
fn allowlist_is_a_finite_enumerated_ledger_not_a_wildcard_exemption() {
    assert!(
        !KNOWN_VIOLATIONS.is_empty(),
        "the ledger must enumerate the known violations; an empty ledger on a tree that \
         demonstrably contains them means the scanner is not finding them"
    );

    for entry in KNOWN_VIOLATIONS {
        assert!(
            !entry.path.is_empty() && !entry.stamp.is_empty(),
            "every entry needs a concrete path and stamp, got {entry:?}"
        );
        for forbidden in ['*', '?', '[', '^', '$'] {
            assert!(
                !entry.path.contains(forbidden),
                "allowlist path '{}' contains pattern character '{forbidden}'; entries must be \
                 exact paths so the ledger stays finite",
                entry.path
            );
        }
        assert!(
            entry.occurrences >= 1,
            "entry {entry:?} records {} occurrences; a zero-count entry is an exemption \
             with no ceiling",
            entry.occurrences
        );
        assert!(
            !entry.debt_note.trim().is_empty(),
            "entry {entry:?} carries no debt note; the ledger records debt, and debt that \
             does not say what it is owed for is an exemption"
        );

        let full = repo_root().join(entry.path);
        assert!(
            full.exists(),
            "allowlisted path '{}' does not exist; a ledger entry must be a verified fact \
             about the tree",
            entry.path
        );
        let body = std::fs::read_to_string(&full)
            .unwrap_or_else(|e| panic!("cannot read allowlisted path '{}': {e}", entry.path));
        assert!(
            body.to_lowercase().contains(&entry.stamp.to_lowercase()),
            "allowlisted stamp '{}' does not appear in '{}'; the entry is stale and is now \
             covering nothing but future violations",
            entry.stamp,
            entry.path
        );
    }

    // No duplicate keys: two entries for the same (path, stamp) would make the
    // effective ceiling ambiguous and unauditable.
    let mut keys: Vec<(&str, &str)> = KNOWN_VIOLATIONS.iter().map(|e| (e.path, e.stamp)).collect();
    let before = keys.len();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(
        before,
        keys.len(),
        "the ledger contains duplicate (path, stamp) keys; the ceiling for a duplicated key \
         is ambiguous"
    );
}

/// Catches: debt recorded as an exemption rather than as a number. The
/// requirement is that the ledger's size is *stated*, so the count is a thing
/// that can be watched shrinking rather than a list nobody totals.
#[test]
fn report_states_the_allowlisted_debt_count() {
    let gate = BrandAbsenceGate::new();

    let expected_total: usize = KNOWN_VIOLATIONS.iter().map(|e| e.occurrences).sum();
    let report = gate.scan_source("src/synthetic/clean.rs", "pub fn ok() {}\n");

    assert_eq!(
        report.allowlisted_debt_total, expected_total,
        "the report must total the ledger, not report an unrelated constant"
    );
    assert!(
        report.summary.contains(&expected_total.to_string()),
        "the summary must state the debt count ({expected_total}); got: {}",
        report.summary
    );
}

/// Catches: the gate shipping as blocking and being reverted on first contact
/// with the existing tree. Warn-only means a new violation is *visible* but not
/// fatal — `is_blocking` stays false even when `new_violations` is non-empty.
#[test]
fn gate_ships_warn_only_so_it_lands_green_while_showing_new_violations() {
    let gate = BrandAbsenceGate::with_allowlist(TEST_ALLOWLIST);

    let report = gate.scan_source(
        "src/synthetic/new_module.rs",
        "info!(\"hyperscaler-grade enterprise pipeline\");\n",
    );

    assert!(
        !report.new_violations.is_empty(),
        "the new violation must be detected"
    );
    assert!(
        !report.is_blocking,
        "this lane ships warn-only; blocking on the existing 30+ known violations would \
         make the gate unmergeable"
    );
    assert!(
        report.summary.to_lowercase().contains("warn"),
        "the summary must say it is warn-only, so a green result is not misread as a clean \
         tree; got: {}",
        report.summary
    );
}

/// Catches: a stamp vocabulary that omits the terms actually found in the tree.
/// D33.2 names "hyperscaler", "cloud-native" and "enterprise" explicitly; a
/// gate that does not carry all three cannot flag the violations it was built
/// for.
#[test]
fn forbidden_stamp_vocabulary_covers_the_terms_named_in_the_law() {
    let vocab: Vec<String> = FORBIDDEN_STAMPS.iter().map(|s| s.to_lowercase()).collect();

    for required in ["hyperscale", "cloud", "enterprise"] {
        assert!(
            vocab.iter().any(|s| s.contains(required)),
            "FORBIDDEN_STAMPS must cover '{required}'; D33.2 names it. vocabulary = {vocab:?}"
        );
    }
}
