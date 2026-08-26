//! Self-conformance tests for `CleanArchitectureGuard`.
//!
//! DEFECT UNDER TEST
//! -----------------
//! `src/clean_architecture_guard.rs` enforces the Core -> Ports -> Adapters ->
//! Facade dependency direction on *other people's* repositories only. Its single
//! public entrypoint is `evaluate_architecture(&PrDiffContext)`, which consumes
//! `diff_ctx.repo` / `diff_ctx.diff_content` from an inbound pull request, and its
//! own unit-test fixtures name `oyatie/oyatie` and `oyatie/console`. There is no
//! way to point the guard at a directory on disk, and nothing anywhere in the tree
//! points it at Anvil's own `src/`. Anvil ships an architectural assertion about
//! everyone except itself.
//!
//! WHAT THESE TESTS ASSERT
//! -----------------------
//! Not that Anvil is clean -- it is not, and these tests must never be "fixed" by
//! contorting the guard until Anvil passes. They assert that the guard is
//! *applied* to Anvil and that the result it produces is *honest*:
//!   1. the guard can be invoked against a source tree on disk;
//!   2. running it over Anvil's own tree does not yield a fabricated clean pass;
//!   3. the self-check is wired into production code, not only into a test;
//!   4. the report type can say "not measured" when there is no layering to read.
//!
//! When this test was written Anvil had 106 flat `pub mod` declarations and zero
//! directories named core / ports / adapters / facade, so the only honest
//! self-check result was `NotMeasured`. Since the Shape Program landed
//! `src/shape/{core,facade}` the guard classifies a handful of files and the
//! honest result is a *count*: N layered files measured, M files not. A clean
//! verdict must state both numbers; it must never describe the whole tree as
//! "verified" or "100% intact" on the strength of the few files it could read.
//!
//! WHY PROMPTING WOULD NOT PREVENT THIS
//! ------------------------------------
//! "Also run the guard on ourselves" is an instruction that evaporates the moment
//! it leaves the context window; nothing in the type system, the build, or CI
//! notices that a guard's only callers pass foreign repositories. The failure mode
//! is silent and self-flattering: the code compiles, the unit tests are green, the
//! dashboard shows a guard named "CleanArchitectureGuard", and the asymmetry is
//! visible only if someone goes looking for the call sites. A test is the only
//! artifact that keeps looking after the prompt is gone.

use std::fs;
use std::path::{Path, PathBuf};

use anvil::clean_architecture_guard::CleanArchitectureGuard;
use anvil::git_manager::PrDiffContext;

/// Crate root == the Anvil tree under test.
fn anvil_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn anvil_src() -> PathBuf {
    anvil_root().join("src")
}

/// Every `.rs` file under `dir`, recursively, sorted for determinism.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries = match fs::read_dir(&d) {
            Ok(e) => e,
            Err(e) => panic!("cannot read {}: {e}", d.display()),
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Source lines with whole-line `//` comments removed, so that a doc comment
/// mentioning an identifier cannot satisfy a structural assertion about code.
fn non_comment_source(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    text.lines()
        .filter(|l| {
            let t = l.trim_start();
            !(t.starts_with("//") || t.starts_with("/*") || t.starts_with('*'))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The guard's own layer-classification convention, restated here so the test can
/// independently count how many of Anvil's files the guard is even capable of
/// examining. Mirrors `is_core_file` / `is_ports_file` in the guard.
fn is_layered_path(rel: &str) -> bool {
    rel.contains("/core/")
        || rel.contains("/domain/")
        || rel.contains("/ports/")
        || rel.contains("/application/")
        || rel.ends_with("/core.rs")
        || rel.ends_with("/domain.rs")
        || rel.ends_with("/ports.rs")
        || rel.ends_with("/application.rs")
}

/// Synthesizes a unified-diff view of Anvil's own source tree, in the exact shape
/// `evaluate_architecture` already parses (`+++ b/<path>` headers, `+` lines), so
/// that the guard's *existing* entrypoint can be aimed at Anvil with no production
/// change. This is the "run the guard on ourselves" experiment.
fn anvil_tree_as_diff() -> (PrDiffContext, Vec<String>) {
    let root = anvil_root();
    let files = rust_files(&anvil_src());
    let mut diff = String::new();
    let mut rels = Vec::new();
    for f in &files {
        let rel = f
            .strip_prefix(&root)
            .unwrap_or(f)
            .to_string_lossy()
            .to_string();
        diff.push_str("+++ b/");
        diff.push_str(&rel);
        diff.push('\n');
        let body = fs::read_to_string(f).unwrap_or_default();
        for line in body.lines() {
            diff.push('+');
            diff.push_str(line);
            diff.push('\n');
        }
        rels.push(rel);
    }
    (
        PrDiffContext {
            repo: "anvil".to_string(),
            pr_number: 0,
            base_branch: "main".to_string(),
            base_sha: "self".to_string(),
            head_sha: "self".to_string(),
            is_incremental: false,
            previous_head_sha: None,
            diff_content: diff,
            changed_files: rels.clone(),
            repo_working_dir: root,
        },
        rels,
    )
}

/// Catches: the guard has no entrypoint that can be aimed at a source tree on
/// disk. Every public method takes a `PrDiffContext` carrying someone else's pull
/// request, so "run it on ourselves" is not merely un-done, it is un-expressible.
///
/// A self-check cannot exist until the guard can read a directory, so this is the
/// structural precondition for tests 2-4. It requires two things of
/// `src/clean_architecture_guard.rs`, both outside comments: a function signature
/// that accepts a filesystem path, and an actual filesystem read.
///
/// Prompting would not prevent this: the missing capability is invisible at the
/// call site -- `evaluate_architecture(&diff_ctx)` reads as a complete API, and
/// nothing about it announces that the only reachable inputs are foreign repos.
#[test]
fn guard_exposes_an_entrypoint_that_scans_a_source_tree() {
    let guard_src = anvil_src().join("clean_architecture_guard.rs");
    let code = non_comment_source(&guard_src);

    let takes_a_path = code
        .lines()
        .any(|l| l.contains("fn ") && (l.contains("Path") || l.contains("PathBuf")));
    let reads_the_filesystem =
        code.contains("read_dir") || code.contains("read_to_string") || code.contains("WalkDir");

    assert!(
        takes_a_path && reads_the_filesystem,
        "CleanArchitectureGuard cannot be run against a source tree.\n  \
         fn taking a Path/PathBuf: {takes_a_path}\n  \
         reads the filesystem (read_dir/read_to_string/WalkDir): {reads_the_filesystem}\n  \
         Only entrypoint today is evaluate_architecture(&PrDiffContext), which can \
         only be fed another repository's pull request. A guard that is structurally \
         incapable of examining its own tree is an assertion about other people."
    );
}

/// Catches the core defect: aim the guard at Anvil's own `src/` and it announces
/// "Hexagonal Clean Architecture verified ... 100% intact" while having examined
/// exactly zero files. Anvil has no core/ports/adapters/facade layering at all, so
/// every one of its ~241 Rust files falls through the classifier untouched, and
/// `violations.is_empty()` -- the guard's sole definition of `is_clean` -- is
/// vacuously true. Absent evidence is being reported as a pass.
///
/// The assertion is deliberately NOT "Anvil is clean" and NOT "Anvil has N
/// violations". It is: when the run measured nothing, the report must say it
/// measured nothing. That matches the `GateStatus::NotMeasured` invariant this
/// repo already applies to coverage (`src/coverage_guard.rs`: "absent evidence is
/// never a pass") and must not be satisfied by tuning the guard until Anvil looks
/// compliant.
///
/// Prompting would not prevent this: `is_clean: violations.is_empty()` is a
/// perfectly ordinary line of code, and the fabricated pass only becomes visible
/// when the guard is pointed at a tree with no layers -- a case no PR-shaped
/// fixture ever produces.
#[test]
fn guard_run_over_anvils_own_tree_does_not_fabricate_a_clean_pass() {
    let (ctx, rels) = anvil_tree_as_diff();
    let layered = rels.iter().filter(|r| is_layered_path(r)).count();
    let total = rels.len();

    let report = CleanArchitectureGuard::new()
        .evaluate_architecture(&ctx)
        .expect("guard evaluates Anvil's own tree");

    let s = report.summary.to_lowercase();
    let claims_verified =
        report.is_clean && (s.contains("verified") || s.contains("intact") || s.contains("100%"));
    let admits_unmeasured = s.contains("not measured")
        || s.contains("notmeasured")
        || s.contains("nothing to measure")
        || s.contains("no layer")
        || s.contains("unmeasured");

    assert!(
        !claims_verified,
        "Guard claims Anvil's architecture is verified while measuring nothing.\n  \
         rust files fed to the guard : {total}\n  \
         files it could classify     : {layered}\n  \
         is_clean                    : {}\n  \
         violations                  : {}\n  \
         summary                     : {}\n  \
         Anvil has zero core/ports/adapters/facade directories; a clean verdict here \
         is absent evidence reported as a pass.",
        report.is_clean,
        report.violations.len(),
        report.summary
    );

    if layered == 0 {
        assert!(
            admits_unmeasured,
            "Guard measured 0 of {total} Anvil files and did not say so.\n  \
             summary: {}\n  \
             With no layering present the honest result is an explicit \
             NotMeasured-style report naming that absence.",
            report.summary
        );
    } else {
        // A measured run must carry its own denominator: how many files it
        // classified and how many it could not. A count the reader can check
        // is the difference between a measurement and a slogan.
        assert!(
            report.measurement.is_measured(),
            "{layered} layered file(s) exist but the report says it measured nothing"
        );
        // The guard also classifies facade/adapter paths this heuristic does
        // not count, so its number may exceed ours but never fall short.
        let classified = report.measurement.files_classified();
        assert!(
            classified >= layered,
            "guard classified {classified} file(s) but {layered} layered path(s) exist"
        );
        assert!(
            s.contains(&format!("{classified} layered file")),
            "summary must state the measured count ({classified}): {}",
            report.summary
        );
        assert!(
            s.contains("not measured"),
            "summary must state that the unlayered remainder was not measured: {}",
            report.summary
        );
    }
}

/// Catches: even once a self-check exists, it could live only inside a test file,
/// where its finding is never recorded, never surfaced on a gate, and never seen
/// by anyone who does not run `cargo test`. The defect is Anvil holding others to
/// a standard it does not report on itself; a green test in a file nobody reads
/// does not close that gap.
///
/// Requires production code under `src/` (outside the guard module itself, and
/// outside `#[cfg(test)]`-only test files) to reference a self-conformance
/// entrypoint. Any reasonable name satisfies the regex-free match below.
///
/// Prompting would not prevent this: "add a self-check" is naturally satisfied by
/// the cheapest artifact that makes the sentence true -- a unit test -- and the
/// difference between a recorded finding and a private one is invisible in review.
#[test]
fn self_conformance_check_is_wired_into_production_code() {
    let needles = [
        "self_conformance",
        "self_check",
        "evaluate_self",
        "evaluate_source_tree",
        "evaluate_tree",
        "self_architecture",
    ];
    let mut hits: Vec<String> = Vec::new();

    for f in rust_files(&anvil_src()) {
        let rel = f
            .strip_prefix(anvil_root())
            .unwrap_or(&f)
            .to_string_lossy()
            .to_string();
        let code = non_comment_source(&f);
        let mentions_guard = code.contains("CleanArchitectureGuard")
            || code.contains("clean_architecture_guard")
            || rel.ends_with("clean_architecture_guard.rs");
        if !mentions_guard {
            continue;
        }
        if needles.iter().any(|n| code.contains(n)) {
            hits.push(rel);
        }
    }

    assert!(
        !hits.is_empty(),
        "No production source file under src/ invokes a clean-architecture \
         self-conformance check.\n  \
         Files referencing CleanArchitectureGuard today: src/main.rs, \
         src/webhook/mod.rs, src/webhook/pipelines/review.rs, \
         src/pre_merge_guard/evaluator.rs, src/cli/handlers.rs -- every one of them \
         feeds it another repository's PR diff.\n  \
         Searched for any of: {needles:?}"
    );
}

/// Catches: `CleanArchitectureReport` is a two-state type -- `is_clean: bool` plus
/// a summary string -- so it is *incapable* of expressing "I found no layering to
/// check". Pointed at a tree with no core/ports/adapters/facade structure, the
/// only outcome the type permits is `true`, i.e. a pass. The honest third state
/// must exist in the type, not just in prose.
///
/// This repo already has the vocabulary: `GateStatus::NotMeasured { gate_id,
/// reason }` in `src/pre_merge_guard/report.rs`, documented there as "absent
/// evidence is never a pass". The clean-architecture report does not use it.
/// Whole-line comments are stripped before matching, so a doc comment promising
/// NotMeasured cannot satisfy this test.
///
/// Prompting would not prevent this: a boolean report field is the default shape
/// anyone writes, and its inability to represent "unknown" is a silent property of
/// the type -- it produces no warning, no error, and a plausible-looking green.
#[test]
fn guard_can_report_not_measured_when_no_layering_exists() {
    let guard_src = anvil_src().join("clean_architecture_guard.rs");
    let code = non_comment_source(&guard_src);

    let has_third_state = code.contains("NotMeasured")
        || code.contains("not_measured")
        || code.contains("NothingToMeasure");

    assert!(
        has_third_state,
        "CleanArchitectureReport cannot express 'no layering found'.\n  \
         Its state today is `is_clean: bool` + `violations` + `summary`, so a tree \
         with nothing to measure can only be reported as clean.\n  \
         Adopt the repo's existing NotMeasured vocabulary (src/pre_merge_guard/report.rs) \
         so Anvil's own result is recorded as unmeasured rather than passed."
    );
}

/// A real, present bypass: `git_manager` has no layer of its own, and reaches
/// straight into another module's `adapters`. Only `facade` is importable from
/// outside a unit -- that is the whole point of the four faces, and it is the
/// edge that closes anvil's `change_delivery -> git_manager -> shape` cycle.
///
/// The guard classifies a file by ITS OWN path, so an unclassified importer is
/// invisible to it and this edge is reported as clean.
#[test]
fn unclassified_importer_reaching_into_a_units_adapters_is_a_violation() {
    let r = CleanArchitectureGuard::new().self_conformance().unwrap();
    let bypass = r.violations.iter().any(|v| {
        v.file_path.contains("git_manager") && v.snippet.contains("change_delivery::adapters")
    });
    assert!(
        bypass,
        "git_manager/mod.rs imports crate::change_delivery::adapters::git_vcs::LANE_LEASE_FILE, \
         which reaches past that unit's facade. Guard reported {} violation(s).",
        r.violations.len()
    );
}

/// The ratchet. Exact: a bypass that disappears must be noticed as much as one
/// that appears, because a count that drops for an unknown reason is a count
/// nobody is reading.
#[test]
fn facade_bypasses_match_the_recorded_count() {
    let r = CleanArchitectureGuard::new().self_conformance().unwrap();
    let bypasses = r
        .violations
        .iter()
        .filter(|v| v.description.contains("reaches past"))
        .count();
    assert_eq!(
        bypasses,
        anvil::clean_architecture_guard::FACADE_BYPASSES_IN_ANVIL,
        "cross-unit facade bypasses moved. Offenders:\n{}",
        r.violations
            .iter()
            .filter(|v| v.description.contains("reaches past"))
            .map(|v| format!("  {} -> {}", v.file_path, v.target_layer))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The other half of a proof: the rule must SPARE a conformant subject. A check
/// that only ever fires is indistinguishable from one that always fires.
///
/// A unit reaching into its OWN interior is the normal, correct case -- it is
/// how a facade uses its adapters -- and `shape/facade` does exactly that.
#[test]
fn a_unit_reaching_into_its_own_interior_is_spared() {
    let r = CleanArchitectureGuard::new().self_conformance().unwrap();
    let self_reach = r
        .violations
        .iter()
        .find(|v| v.file_path.starts_with("src/shape/") && v.target_layer.starts_with("shape::"));
    assert!(
        self_reach.is_none(),
        "a unit was flagged for using its own faces: {self_reach:?}"
    );
    // And the rule is not inert: it is finding real cross-unit edges.
    assert!(
        r.violations
            .iter()
            .any(|v| v.description.contains("reaches past")),
        "rule fired on nothing at all, so sparing proves nothing"
    );
}
