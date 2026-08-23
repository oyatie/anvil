//! The ABI gate must compare public function signatures, not count `pub fn`
//! substrings in the whole diff.
//!
//! DEFECT UNDER TEST
//! -----------------
//! `SignatureScanner::scan_abi_diff` decided with one whole-diff predicate:
//!
//! ```text
//! diff.contains("-pub fn ") && !diff.contains("+pub fn ")
//! ```
//!
//! Three structural consequences, none reachable by tuning the predicate:
//!
//! 1. **Removal is masked by any addition.** Delete `pub fn legacy_api` and add
//!    `pub fn anything_else` in the same pull request and the right-hand term is
//!    false, so nothing is reported.
//! 2. **A signature change can never be detected.** Editing a signature emits a
//!    `-pub fn` line *and* a `+pub fn` line, so the predicate is false by
//!    construction — for the one change class the gate is named after.
//! 3. **A `pub fn` inside a string literal counts.** `contains` is unanchored,
//!    so a diff line `+ let diff = "-pub fn legacy_api()"` — the shape of this
//!    repository's own fixtures — satisfies the left term and clears no right
//!    term, and the gate reports a removal that never happened.
//!
//! WHY PROMPTING WOULD NOT PREVENT THIS
//! ------------------------------------
//! The predicate is not a lapse of care; it is the wrong *shape*. It reduces a
//! set-difference question ("which public names left, and which changed") to two
//! booleans over one string, and no instruction to "be more careful with the
//! predicate" recovers the per-name comparison that was never there. Only a
//! test that removes one name while adding another can tell the two apart.
//!
//! WHAT THIS FILE PINS
//! -------------------
//! Both directions. The detecting cases above, and — equally load-bearing — the
//! refactor shapes that must NOT fire, because a per-name matcher that reports
//! every moved or reflowed function blocks every pull request and gets turned
//! off, which is the same outcome as not detecting anything.

use anvil::git_manager::PrDiffContext;
use anvil::pre_merge_guard::report::GateStatus;
use anvil::semantic_abi_ratchet::{SemanticAbiRatchet, SemanticAbiReport};
use std::path::{Path, PathBuf};

/// One `diff --git` chunk for `path`, with the body lines given verbatim.
fn chunk(path: &str, body: &str) -> String {
    format!(
        "diff --git a/{path} b/{path}\nindex 1111111..2222222 100644\n--- a/{path}\n+++ b/{path}\n@@ -1,4 +1,4 @@\n{body}\n"
    )
}

fn certify(chunks: &[String]) -> SemanticAbiReport {
    let diff_content = chunks.concat();
    let ctx = PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 4242,
        base_branch: "main".to_string(),
        base_sha: "base".to_string(),
        head_sha: "head".to_string(),
        previous_head_sha: None,
        is_incremental: false,
        repo_working_dir: PathBuf::from("."),
        changed_files: Vec::new(),
        diff_content,
    };
    SemanticAbiRatchet::new()
        .evaluate_abi_stability(Path::new("."), &ctx)
        .expect("the ratchet reads a diff and returns a report")
}

// ---------------------------------------------------------------------------
// Failure mode 2: a signature change was undetectable by construction.
// ---------------------------------------------------------------------------

#[test]
fn changing_the_signature_of_a_public_function_is_detected() {
    let report = certify(&[chunk(
        "src/api.rs",
        "-pub fn parse(input: &str) -> u32 {\n+pub fn parse(input: &str, strict: bool) -> u32 {\n     todo!()\n }",
    )]);

    assert!(
        !report.is_abi_stable,
        "a public function whose parameter list gained an argument is a breaking \
         change; the gate reported it stable. {}",
        report.summary
    );
    assert!(
        report
            .breaking_findings
            .iter()
            .any(|f| f.symbol_name == "parse"),
        "the finding must name the function that changed, not the phrase \
         'public function': {:?}",
        report.breaking_findings
    );
}

#[test]
fn changing_only_the_return_type_of_a_public_function_is_detected() {
    let report = certify(&[chunk(
        "src/api.rs",
        "-pub fn version() -> u32 {\n+pub fn version() -> u64 {\n     0\n }",
    )]);
    assert!(
        !report.is_abi_stable,
        "widening a public return type is a breaking change: {}",
        report.summary
    );
}

// ---------------------------------------------------------------------------
// Failure mode 1: a removal was masked by any unrelated addition.
// ---------------------------------------------------------------------------

#[test]
fn a_removal_is_not_masked_by_an_unrelated_addition_elsewhere() {
    let report = certify(&[
        chunk(
            "src/legacy.rs",
            "-pub fn legacy_api() -> u32 {\n-    42\n-}",
        ),
        chunk(
            "src/fresh.rs",
            "+pub fn brand_new_helper() -> u32 {\n+    7\n+}",
        ),
    ]);

    assert!(
        !report.is_abi_stable,
        "`legacy_api` left the public surface; adding an unrelated \
         `brand_new_helper` does not put it back. {}",
        report.summary
    );
    assert!(
        report
            .breaking_findings
            .iter()
            .any(|f| f.symbol_name == "legacy_api"),
        "the removed name must be reported: {:?}",
        report.breaking_findings
    );
}

#[test]
fn renaming_a_public_function_is_reported_as_a_removal() {
    let report = certify(&[chunk(
        "src/api.rs",
        "-pub fn old_name(x: u32) -> u32 {\n+pub fn new_name(x: u32) -> u32 {\n     x\n }",
    )]);
    assert!(
        !report.is_abi_stable,
        "a rename removes the old path from the public surface: {}",
        report.summary
    );
}

// ---------------------------------------------------------------------------
// Failure mode 3: `contains` is unanchored, so quoted code counted.
// ---------------------------------------------------------------------------

#[test]
fn a_pub_fn_quoted_inside_a_string_literal_is_not_a_signature() {
    // The shape of this repository's own scanner fixtures: diff lines whose
    // *content* quotes a signature. Nothing is declared or removed here, and the
    // deleted line is the one that matters -- an unanchored match reads it as a
    // public function leaving the surface and blocks the pull request.
    let report = certify(&[chunk(
        "src/scanner_test.rs",
        "-    let diff = \"pub fn legacy_api() -> u32 {\";\n+    let diff = \"-pub fn legacy_api() -> u32 {\";\n+    assert!(diff.is_empty());",
    )]);
    assert!(
        report.is_abi_stable,
        "a signature inside a string literal is data, not a declaration; the gate \
         accused a diff that removes nothing. {} {:?}",
        report.summary, report.breaking_findings
    );
}

// ---------------------------------------------------------------------------
// The other direction: shapes that must NOT fire.
//
// A matcher that reports every refactor blocks every pull request, which ends
// the same way as reporting nothing.
// ---------------------------------------------------------------------------

#[test]
fn adding_a_public_function_does_not_fire() {
    let report = certify(&[chunk(
        "src/api.rs",
        "+pub fn brand_new() -> u32 {\n+    1\n+}",
    )]);
    assert!(
        report.is_abi_stable,
        "an additive change is not breaking: {}",
        report.summary
    );
}

#[test]
fn reflowing_a_signature_across_several_lines_does_not_fire() {
    // rustfmt splitting a long signature is the single commonest way a
    // `-pub fn`/`+pub fn` pair appears with nothing behind it.
    let report = certify(&[chunk(
        "src/api.rs",
        "-pub fn wide(alpha: Alpha, beta: Beta, gamma: Gamma) -> Delta {\n\
         +pub fn wide(\n\
         +    alpha: Alpha,\n\
         +    beta: Beta,\n\
         +    gamma: Gamma,\n\
         +) -> Delta {\n     todo!()\n }",
    )]);
    assert!(
        report.is_abi_stable,
        "reformatting is not an API change; a signature the gate cannot read on \
         one line must not be reported as changed. {} {:?}",
        report.summary, report.breaking_findings
    );
}

#[test]
fn whitespace_only_edits_to_a_signature_do_not_fire() {
    let report = certify(&[chunk(
        "src/api.rs",
        "-pub fn spaced(a:u32,b:u32) -> u32 {\n+pub fn spaced(a: u32, b: u32) -> u32 {\n     a\n }",
    )]);
    assert!(
        report.is_abi_stable,
        "spacing is not signature: {} {:?}",
        report.summary, report.breaking_findings
    );
}

#[test]
fn moving_a_public_function_between_files_does_not_fire() {
    let report = certify(&[
        chunk(
            "src/old_home.rs",
            "-pub fn relocated(x: u32) -> u32 {\n-    x\n-}",
        ),
        chunk(
            "src/new_home.rs",
            "+pub fn relocated(x: u32) -> u32 {\n+    x\n+}",
        ),
    ]);
    assert!(
        report.is_abi_stable,
        "the name and signature both survive the move; the gate resolves no \
         module paths and must not invent a break it did not measure. {} {:?}",
        report.summary, report.breaking_findings
    );
}

#[test]
fn a_name_declared_more_than_once_on_each_side_is_not_paired() {
    // `new` occurs on dozens of impls. Pairing one removal with an unrelated
    // addition of the same name would report a signature change between two
    // functions that have nothing to do with each other.
    let report = certify(&[
        chunk(
            "src/a.rs",
            "-pub fn new(cfg: Config) -> Self {\n+pub fn new(cfg: Config, clock: Clock) -> Self {\n     todo!()\n }",
        ),
        chunk(
            "src/b.rs",
            "-pub fn new(seed: u64) -> Self {\n+pub fn new(seed: u64) -> Self {\n     todo!()\n }",
        ),
    ]);
    assert!(
        report.is_abi_stable,
        "two removals and two additions of `new` cannot be paired from diff text; \
         the gate must decline rather than guess. {} {:?}",
        report.summary, report.breaking_findings
    );
}

#[test]
fn a_removal_under_the_tests_directory_is_not_public_api() {
    let report = certify(&[chunk(
        "tests/helpers_test.rs",
        "-pub fn make_ctx() -> Ctx {\n-    Ctx\n-}",
    )]);
    assert!(
        report.is_abi_stable,
        "`tests/` is not a published library surface: {} {:?}",
        report.summary, report.breaking_findings
    );
}

#[test]
fn a_diff_touching_no_rust_at_all_does_not_fire() {
    let report = certify(&[chunk(
        "README.md",
        "-pub fn documented_example() -> u32 {\n+Nothing here is code.",
    )]);
    assert!(
        report.is_abi_stable,
        "prose naming a signature is not a declaration: {} {:?}",
        report.summary, report.breaking_findings
    );
}

#[test]
fn a_private_function_is_outside_the_public_surface() {
    let report = certify(&[chunk(
        "src/api.rs",
        "-fn internal(x: u32) -> u32 {\n-    x\n-}",
    )]);
    assert!(
        report.is_abi_stable,
        "a private function is not public API: {} {:?}",
        report.summary, report.breaking_findings
    );
}

// ---------------------------------------------------------------------------
// The claim the gate cannot deliver, said as `NotMeasured` rather than as a
// pass.
//
// The gate's title claimed "struct memory layout stability". Nothing in it
// computed a layout, and nothing reading a unified diff can: a `repr(Rust)`
// type has no guaranteed layout to be stable, and the `#[repr(C)]` family needs
// rustc or DWARF out of a compiled artifact. So the half-claim is withdrawn --
// and where it would have decided the answer, the gate says it did not measure.
// ---------------------------------------------------------------------------

#[test]
fn a_diff_that_changes_a_repr_attribute_is_not_measured_rather_than_passed() {
    let report = certify(&[chunk(
        "src/wire.rs",
        "-#[repr(C)]\n+#[repr(C, packed)]\n pub struct Header {\n     tag: u8,\n }",
    )]);

    match &report.status {
        GateStatus::NotMeasured { gate_id, reason } => {
            assert_eq!(gate_id, "semantic_abi_status");
            assert!(
                reason.contains("layout is not computed"),
                "the reason must say what was not measured: {reason}"
            );
        }
        other => panic!(
            "packing a repr(C) struct changes its layout, and this gate computes no layout; \
             reporting anything but NotMeasured publishes silence as evidence. Got {other:?}: {}",
            report.summary
        ),
    }
}

#[test]
fn an_ordinary_clean_diff_still_passes_rather_than_going_unmeasured() {
    // The symmetric error: an unmeasured status on every pull request blocks the
    // merge queue through `unmeasured_gates` and is no more honest.
    let report = certify(&[chunk(
        "src/api.rs",
        "+pub fn added(x: u32) -> u32 {\n+    x\n+}",
    )]);
    assert!(
        matches!(report.status, GateStatus::Passed),
        "{:?} / {}",
        report.status,
        report.summary
    );
}

#[test]
fn a_breaking_change_publishes_a_failing_status_and_names_the_kind() {
    let removal = certify(&[chunk("src/a.rs", "-pub fn dropped() {}")]);
    assert!(matches!(removal.status, GateStatus::Failed(_)));
    assert_eq!(removal.breaking_findings[0].change_kind, "REMOVAL");

    let changed = certify(&[chunk(
        "src/a.rs",
        "-pub fn kept(a: u32) {}\n+pub fn kept(a: u64) {}",
    )]);
    assert!(matches!(changed.status, GateStatus::Failed(_)));
    assert_eq!(
        changed.breaking_findings[0].change_kind, "SIGNATURE_CHANGE",
        "a removal and a signature change are different repairs and must not share a label"
    );
}

#[test]
fn a_layout_change_never_overrides_a_signature_break() {
    let report = certify(&[chunk(
        "src/wire.rs",
        "-#[repr(C)]\n+#[repr(C, packed)]\n-pub fn dropped() {}",
    )]);
    assert!(
        matches!(report.status, GateStatus::Failed(_)),
        "an unmeasured layout must not launder a measured break: {:?}",
        report.status
    );
}
