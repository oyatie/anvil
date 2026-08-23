//! Gate `kani_status` publishes a formal-verification claim over a lint that
//! only asks whether a comment exists.
//!
//! # The defect, restated from source
//!
//! `KaniGuard::lint_unsafe_safety_comments` (`src/kani_guard/mod.rs`) splits the
//! diff on `diff --git`, skips chunks whose text does not contain `.rs`, matches
//! added lines against `^\+\s*unsafe\s*(\{|fn|impl|trait)`, and asks whether
//! `//\s*SAFETY:` appears on the matched line or the five before it. That is the
//! whole computation. It then published:
//!
//! ```ignore
//! kani_proofs_passed: safety_proofs_valid,
//! ```
//!
//! and the sentence `"PASSED (N unsafe block(s) verified with valid `// SAFETY:`
//! proof clauses)"`, under the matrix row "🔬 Kani Formal Verification & Unsafe
//! Proofs / Mathematical memory safety & SAFETY: invariant proofs".
//!
//! No model checker runs. `proof_runner.rs` was deleted. Nothing in the gate has
//! ever invoked Kani, CBMC, Miri, Prusti or Creusot.
//!
//! # Why the check itself is kept
//!
//! The lint is legitimate and it fires on real code. It is a reimplementation of
//! `clippy::undocumented_unsafe_blocks`, which upstream files under the
//! `restriction` group -- an opt-in documentation policy -- and whose own
//! rationale is hedged to "may help in discovering unsoundness or bugs". The
//! Rust std dev guide states the same thing about `// SAFETY:` comments: they
//! exist so a human reviewer can check the argument. A comment being present
//! says nothing about whether it is true, whether it matches the block below it,
//! or whether the invariants hold.
//!
//! So the defect is the claim, not the check. These tests pin the check *harder*
//! than it was pinned before -- an undocumented `unsafe` block must still fail
//! the gate, with the count it measured in the sentence -- so that the honest
//! renaming cannot be satisfied by a gate that measures nothing and says so.
//!
//! # What is pinned, and why each half is needed
//!
//! 1. **Teeth.** An added `unsafe fn` with no `// SAFETY:` comment fails, with a
//!    non-empty violation naming it. Without this row, every naming assertion
//!    below is satisfied by deleting the gate.
//! 2. **The other direction.** A documented `unsafe` block passes, and the count
//!    it reports is the number of blocks it actually saw -- read as a numeric
//!    *token*, not a substring, and exercised at two so no incidental digit can
//!    supply it. Without this row, `all_documented = false` hardcoded passes
//!    row 1 forever and blocks every pull request.
//! 3. **No claim beyond the measurement.** No published sentence, no serialized
//!    field name, no public item name, and no matrix row may use the
//!    vocabulary of verification
//!    (`proof`, `verified`, `formal`, `mathematical`, `invariant`, `guarantee`)
//!    for a check that establishes only that a comment is present.
//! 4. **The disclosure.** The two passing sentences -- the paths on which a
//!    reader is deciding whether to trust a green gate -- must say that no model
//!    checker ran. A withdrawn claim that is merely silent is still read as the
//!    old claim, because the gate id is `kani_status` and cannot be renamed.
//! 5. **The registry.** `src/fidelity/registry.rs` is published output. Its
//!    `kani_status` entry must stay below `Measured` and must state that no
//!    bounded model checker is invoked, so the scorecard's low-fidelity
//!    disclosure keeps naming this gate on the green path.

use anvil::fidelity::{Fidelity, registry::AUDITED_GATES};
use anvil::git_manager::PrDiffContext;
use anvil::kani_guard::{KaniGuard, KaniGuardReport};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn diff(body: &str) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "main".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: body.to_string(),
        changed_files: vec!["src/raw.rs".to_string()],
        repo_working_dir: PathBuf::from("."),
        is_incremental: false,
        previous_head_sha: None,
    }
}

fn run(body: &str) -> KaniGuardReport {
    KaniGuard::new()
        .lint_unsafe_safety_comments(Path::new("."), &diff(body))
        .expect("the guard reads the diff")
}

/// A diff adding one `unsafe fn` with no `// SAFETY:` comment anywhere near it.
const UNDOCUMENTED: &str = "diff --git a/src/raw.rs b/src/raw.rs\n\
     @@ -1,0 +1,3 @@\n\
     +pub fn head(p: *const u8) -> u8 {\n\
     +    unsafe { *p }\n\
     +}\n\
     +unsafe fn deref(p: *const u8) -> u8 { *p }\n";

/// Two added `unsafe` items, each preceded by a `// SAFETY:` comment.
const DOCUMENTED: &str = "diff --git a/src/raw.rs b/src/raw.rs\n\
     @@ -1,0 +1,6 @@\n\
     +// SAFETY: the caller guarantees `p` is non-null and aligned.\n\
     +unsafe fn deref(p: *const u8) -> u8 { *p }\n\
     +\n\
     +// SAFETY: `Handle` owns its file descriptor exclusively.\n\
     +unsafe impl Send for Handle {}\n";

/// A Rust diff that adds no `unsafe` at all.
const CLEAN: &str = "diff --git a/src/lib.rs b/src/lib.rs\n\
     @@ -1,0 +1,1 @@\n\
     +pub fn add(a: i32, b: i32) -> i32 { a + b }\n";

/// The digits of `text`, as whole tokens. `summary.contains("1")` is satisfied
/// by the `1` in `Gate 17`; this is not.
fn numeric_tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Words that assert a machine-checked property. A gate that establishes only
/// "a comment is present" may not use any of them about its own result.
const VERIFICATION_VOCABULARY: &[&str] = &[
    "proof",
    "proofs",
    "proven",
    "prove",
    "verified",
    "verify",
    "verification",
    "verifier",
    "formal",
    "mathematic",
    "invariant",
    "guarantee",
];

fn overclaims(text: &str) -> Vec<&'static str> {
    let lower = text.to_lowercase();
    VERIFICATION_VOCABULARY
        .iter()
        .filter(|w| lower.contains(**w))
        .copied()
        .collect()
}

// ---------------------------------------------------------------------------
// 1. Teeth: the lint must still fire
// ---------------------------------------------------------------------------

/// Catches: the gate renamed into honesty by measuring nothing.
///
/// Every naming assertion in this file is satisfied by a guard that returns an
/// empty report on every diff. This row is the one that is not: an added
/// `unsafe fn` carrying no `// SAFETY:` comment must fail the gate, and the
/// violation must name the offending line rather than be a bare count.
#[test]
fn an_undocumented_unsafe_block_still_fails_the_gate() {
    let r = run(UNDOCUMENTED);

    assert!(
        !r.all_unsafe_blocks_documented,
        "an added `unsafe fn` with no `// SAFETY:` comment must fail the gate; \
         got: {}",
        r.summary
    );
    assert!(
        !r.violations.is_empty(),
        "the failing gate must publish what it found, not just a verdict: {}",
        r.summary
    );
    assert!(
        r.violations.iter().any(|v| v.contains("deref")),
        "the violation must name the line it is about, so an author can find \
         it; got {:?}",
        r.violations
    );
    assert!(
        r.violations.iter().any(|v| v.contains("SAFETY")),
        "the violation must say what is missing; got {:?}",
        r.violations
    );

    // The count it measured, as a token: two added lines open an `unsafe`
    // block here (`unsafe { *p }` on its own line, and `unsafe fn deref`).
    assert!(
        numeric_tokens(&r.summary).contains(&r.violations.len().to_string()),
        "the failing sentence must publish the number of undocumented blocks \
         it counted ({}); got: {}",
        r.violations.len(),
        r.summary
    );
}

// ---------------------------------------------------------------------------
// 2. The other direction: a documented block passes, with a real count
// ---------------------------------------------------------------------------

/// Catches: `all_unsafe_blocks_documented: false` shipped hardcoded -- row 1
/// green, every pull request in the repository blocked by gate 14 forever.
#[test]
fn documented_unsafe_blocks_pass_and_the_count_is_the_one_measured() {
    let r = run(DOCUMENTED);

    assert!(
        r.all_unsafe_blocks_documented,
        "two `unsafe` items each preceded by a `// SAFETY:` comment must pass: {} {:?}",
        r.summary, r.violations
    );
    assert!(r.violations.is_empty(), "got {:?}", r.violations);
    assert_eq!(
        r.unsafe_blocks_found, 2,
        "both an `unsafe fn` and an `unsafe impl` are added: {}",
        r.summary
    );
    assert_eq!(
        r.unsafe_blocks_with_safety_comment, 2,
        "both carry a `// SAFETY:` comment: {}",
        r.summary
    );
    assert!(
        numeric_tokens(&r.summary).contains(&"2".to_string()),
        "the passing sentence must publish the number it counted, so a reader \
         can tell a measured pass from an empty one; got: {}",
        r.summary
    );
}

/// Catches: a gate that counts an `unsafe` block it never looked at. The clean
/// path must report zero and must not accuse the diff of anything.
#[test]
fn a_diff_with_no_unsafe_reports_zero_and_accuses_nobody() {
    let r = run(CLEAN);

    assert!(r.all_unsafe_blocks_documented, "got: {}", r.summary);
    assert_eq!(r.unsafe_blocks_found, 0, "got: {}", r.summary);
    assert_eq!(r.unsafe_blocks_with_safety_comment, 0);
    assert!(r.violations.is_empty(), "got {:?}", r.violations);

    // Nothing-in-scope and something-measured are different facts and must not
    // share a sentence: a constant string satisfies every other assertion here.
    assert_ne!(
        r.summary,
        run(DOCUMENTED).summary,
        "the sentence for a diff with no `unsafe` must differ from the one for \
         a diff whose `unsafe` blocks were all documented"
    );
}

// ---------------------------------------------------------------------------
// 3. No claim beyond the measurement
// ---------------------------------------------------------------------------

/// Catches: the headline defect. A comment-presence lint publishing the
/// vocabulary of machine-checked verification.
///
/// Asserted on all three paths, because the claim was on all three: the clean
/// path said "safe Rust guarantees intact", the documented path said blocks were
/// "verified with valid `// SAFETY:` proof clauses", and the failing path
/// accused the author of lacking "formal `// SAFETY:` proof invariants".
#[test]
fn no_published_sentence_claims_a_verification_that_did_not_happen() {
    for (name, body) in [
        ("clean", CLEAN),
        ("documented", DOCUMENTED),
        ("undocumented", UNDOCUMENTED),
    ] {
        let r = run(body);
        let bad = overclaims(&r.summary);
        assert!(
            bad.is_empty(),
            "the {name} summary claims {bad:?} for a check that establishes \
             only that a comment is present:\n  {}",
            r.summary
        );
        for v in &r.violations {
            let bad = overclaims(v);
            assert!(bad.is_empty(), "a {name} violation claims {bad:?}:\n  {v}");
        }
    }
}

/// Catches: the claim moved from the sentence into the data. `kani_proofs_passed`
/// was the count of `// SAFETY:` comments under a name asserting that a model
/// checker discharged proof obligations. The report is `Serialize`, so the field
/// name is published output, not an internal detail.
#[test]
fn no_serialized_field_name_asserts_a_proof() {
    let value = serde_json::to_value(run(DOCUMENTED)).expect("the report serializes");
    let obj = value.as_object().expect("the report is a struct");

    for key in obj.keys() {
        let bad = overclaims(key);
        assert!(
            bad.is_empty(),
            "field `{key}` claims {bad:?}; the gate counts `// SAFETY:` \
             comments. Published keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
    }
    assert!(
        !obj.contains_key("kani_proofs_passed"),
        "no Kani proof is run, so no field may report how many passed"
    );

    // ...and the count is still published, under a name that says what it is.
    assert_eq!(
        obj.get("unsafe_blocks_with_safety_comment")
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "the count must survive the rename: {:?}",
        obj.keys().collect::<Vec<_>>()
    );
}

/// Catches: the claim moved out of the serialized keys and into the API. The
/// method that runs this lint was called `evaluate_unsafe_invariants` while
/// `invariant` sat on the list above, and the suite stayed green because
/// `no_serialized_field_name_asserts_a_proof` reads the JSON keys only. Item
/// names are published too -- rustdoc, call sites, every stack trace -- and the
/// exemption this gate claims covers the gate id and the module name, which
/// predate the correction, not names it is free to choose.
#[test]
fn no_public_item_name_asserts_a_proof() {
    let mut names: Vec<String> = Vec::new();
    for entry in std::fs::read_dir("src/kani_guard").expect("the module directory is readable") {
        let path = entry.expect("readable entry").path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("the module is readable");
        for line in src.lines() {
            let line = line.trim();
            if line.starts_with("//") {
                continue;
            }
            let Some(rest) = line.strip_prefix("pub ") else {
                continue;
            };
            // `pub fn foo`, `pub struct Foo`, ... and bare `pub foo: T` fields.
            let rest = [
                "fn ", "struct ", "enum ", "trait ", "type ", "const ", "mod ", "use ",
            ]
            .iter()
            .find_map(|kw| rest.strip_prefix(kw))
            .unwrap_or(rest);
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                names.push(name);
            }
        }
    }

    assert!(
        names.len() >= 8,
        "the scan found {} public names, so it is no longer reading the API \
         surface and would pass on anything: {names:?}",
        names.len()
    );
    for name in &names {
        let bad = overclaims(name);
        assert!(
            bad.is_empty(),
            "public item `{name}` claims {bad:?}; the gate establishes only \
             that a `// SAFETY:` comment is present. Scanned: {names:?}"
        );
    }
}

/// Catches: the sentence and the fields made honest while the row a reviewer
/// actually reads on a *passing* gate still says "Mathematical memory safety &
/// SAFETY: invariant proofs".
///
/// A passing gate produces no finding line on the scorecard, so this row and the
/// registry disclosure are the only things a reader sees about it.
#[test]
fn the_matrix_row_describes_a_comment_lint_not_a_model_checker() {
    let (label, detail) =
        anvil::pre_merge_guard::matrix::label_for("kani_status").expect("gate 14 has a matrix row");

    for (what, text) in [("label", label), ("detail", detail)] {
        let bad = overclaims(text);
        assert!(
            bad.is_empty(),
            "the matrix {what} claims {bad:?} for a `// SAFETY:` comment lint: {text}"
        );
    }
    assert!(
        detail.to_uppercase().contains("SAFETY"),
        "the row must say what is actually checked -- the presence of a \
         `// SAFETY:` comment: {detail}"
    );
    assert!(
        detail.to_lowercase().contains("comment"),
        "the row must name the thing measured as a comment, since that is all \
         it is: {detail}"
    );
}

// ---------------------------------------------------------------------------
// 4. The disclosure
// ---------------------------------------------------------------------------

/// Catches: the claim withdrawn by deletion. The gate id is `kani_status` and
/// cannot be renamed -- it is a published identifier -- so silence about the
/// model checker is read as the old claim. Both passing paths must say it.
///
/// Only the passing paths: a failing gate publishes a finding line the reader
/// already treats as a defect, and padding an accusation with a disclaimer
/// makes the accusation harder to read.
#[test]
fn a_passing_sentence_discloses_that_no_model_checker_ran() {
    for (name, body) in [("clean", CLEAN), ("documented", DOCUMENTED)] {
        let r = run(body);
        assert!(r.all_unsafe_blocks_documented);
        assert!(
            r.summary.to_lowercase().contains("no model checker"),
            "the {name} pass is published under the gate id `kani_status`, so \
             it must say that no model checker ran:\n  {}",
            r.summary
        );
    }
}

// ---------------------------------------------------------------------------
// 5. The registry
// ---------------------------------------------------------------------------

/// Catches: the gate declared honest in its own strings while the registry --
/// the thing the scorecard reads to decide which passing gates to disclose --
/// still claims more, or cites a file this repository deleted.
///
/// `src/publish/scorecard.rs` names every passing gate whose registry fidelity
/// is below `Measured`. If this entry were raised to `Measured`, gate 14 would
/// silently drop out of that disclosure and a green scorecard would say nothing
/// at all about it.
#[test]
fn the_registry_keeps_gate_14_below_measured_and_says_why() {
    let entry = AUDITED_GATES
        .iter()
        .find(|g| g.gate_id == "kani_status")
        .expect("gate 14 is audited in the fidelity registry");

    assert!(
        entry.fidelity < Fidelity::Measured,
        "a comment-presence lint is not a measurement of memory safety; \
         declared {:?}",
        entry.fidelity
    );
    assert!(
        !entry.gap.contains("proof_runner"),
        "`proof_runner.rs` was deleted from this repository; the gap may not \
         cite it: {}",
        entry.gap
    );
    let gap = entry.gap.to_lowercase();
    assert!(
        gap.contains("model checker"),
        "the gap must state that no bounded model checker is invoked: {}",
        entry.gap
    );
    assert!(
        gap.contains("safety:"),
        "the gap must name what the gate does instead -- look for a \
         `// SAFETY:` comment: {}",
        entry.gap
    );
}
