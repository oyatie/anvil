//! Gates must not report a pass they have no evidence for.

use anvil::pre_merge_guard::GateStatus;

/// `evaluate_quarantine_lifecycle` set `let passed = true;` as a literal, so
/// `flake_quarantine_status` was `Passed` on every pull request ever certified.
/// Underneath, "quarantine" was a substring match for "flaky" against the
/// changed *file paths* (the parameter is named `modified_tests`), and nothing
/// was ever isolated because there is no quarantine lane to isolate into.
///
/// Anvil retains no test-run history, so it cannot know which tests are flaky.
/// That is the honest answer, and it is not a pass.
#[test]
fn flake_quarantine_reports_no_history_rather_than_a_clean_lane() {
    let report = anvil::flake_quarantine::FlakeQuarantineLifecycle::new()
        .evaluate_quarantine_lifecycle(&["tests/flaky_network_test.rs".to_string()]);

    assert_eq!(
        report.status.unmeasured_gate_id(),
        Some("flake_quarantine_status"),
        "no run history exists, so flakiness is unknown"
    );
    assert!(
        !report.passed,
        "a lane nothing was isolated into is not a clean one"
    );
    assert!(
        !matches!(report.status, GateStatus::Passed),
        "the gate must not stamp a green it cannot justify"
    );
}

/// `is_optimized` was a literal `true`, so `predictive_test_status` passed
/// regardless of what the DAG selection returned -- and when package discovery
/// found nothing, the selector invented a package named "anvil" to select.
#[test]
fn predictive_selection_is_measured_not_asserted() {
    use anvil::predictive_test_selector::PredictiveTestSelector;

    // No workspace to discover under an empty directory: nothing to prune, so
    // nothing to claim.
    let dir = tempfile::tempdir().expect("tempdir");
    let report = PredictiveTestSelector::new()
        .evaluate_test_selection(dir.path(), &diff_ctx(&["src/lib.rs".to_string()]))
        .expect("evaluates");

    assert_eq!(
        report.status.unmeasured_gate_id(),
        Some("predictive_test_status"),
        "no workspace was discovered, so no pruning was measured"
    );
    assert!(!report.is_optimized);
}

fn diff_ctx(changed: &[String]) -> anvil::git_manager::PrDiffContext {
    anvil::git_manager::PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "main".to_string(),
        base_sha: "a".to_string(),
        head_sha: "b".to_string(),
        diff_content: String::new(),
        changed_files: changed.to_vec(),
        repo_working_dir: std::path::PathBuf::from("."),
        is_incremental: false,
        previous_head_sha: None,
    }
}

/// The measured path must also be measured. Without this, restoring
/// `let is_optimized = true;` kills nothing: the empty-workspace test above
/// covers only the NotMeasured branch, so a constant verdict on the branch
/// that actually runs would go unnoticed. That mutant survived until this
/// test existed.
///
/// A change that touches every package prunes nothing, and pruning nothing is
/// not an optimised selection.
#[test]
fn a_selection_that_prunes_nothing_is_not_reported_as_optimised() {
    use anvil::predictive_test_selector::PredictiveTestSelector;

    let dir = tempfile::tempdir().expect("tempdir");
    // A real single-package workspace: the change touches it, so nothing is
    // spared and there is no pruning to claim.
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"solo\"\nversion = \"0.1.0\"\n",
    )
    .expect("manifest");
    std::fs::create_dir_all(dir.path().join("src")).expect("src");
    std::fs::write(dir.path().join("src/lib.rs"), "pub fn f() {}\n").expect("lib");

    let report = PredictiveTestSelector::new()
        // Selection matches a changed path against the package name, so this
        // path affects the only package there is.
        .evaluate_test_selection(dir.path(), &diff_ctx(&["solo/src/lib.rs".to_string()]))
        .expect("evaluates");

    assert_eq!(
        report.skipped_packages_count, 0,
        "the only package was affected, so nothing was spared"
    );
    assert!(
        !report.is_optimized,
        "pruning nothing is not an optimised selection; this is the assertion \
         a constant `true` would violate"
    );
}

// ---------------------------------------------------------------------------
// rust_skills_status: a rule count nothing counted
// ---------------------------------------------------------------------------
//
// `rules_evaluated_count: 380` was a literal, written twice in
// `rust_language_policy/mod.rs`, and `categories_evaluated` was a hand-written
// list of eight categories carrying their own invented per-category counts
// ("API Design (46 rules)"). The engine underneath implements seven regexes,
// four of which can block. Nothing in the process has ever held 380 rules.
//
// The number was not merely unearned, it was wrong: the corpus it names --
// `jason931225/rust-skills` -- publishes a `rules-434` badge over 434 files in
// `rules/`, and its own changelog records the count passing through 380 in
// mid-August 2026 on the way there. A literal transcribed from a moving corpus
// is stale from the day after it is written, which is the general argument for
// counting the ruleset you loaded rather than the one you remember.
//
// The second half is the early return: a diff with no `.rs` file published
// `is_idiomatic: true`, that same `rules_evaluated_count: 380`, the category
// "All 27 Categories (Zero Rust files in PR)" and the sentence "rust-skills
// quality check passed" -- 380 rules reported as evaluated over zero files.

use anvil::rust_language_policy::{RustLanguagePolicy, RustSkillsMeasurement, engine};

fn rust_diff_ctx(changed: &[&str], diff: &str) -> anvil::git_manager::PrDiffContext {
    let mut ctx = diff_ctx(&changed.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    ctx.diff_content = diff.to_string();
    ctx
}

/// Catches: the early return publishing a corpus it did not evaluate.
///
/// A documentation-only pull request is the ordinary case here, and the gate
/// answered it with a rule count and the word "passed".
#[test]
fn a_diff_with_no_rust_file_publishes_no_rule_count_and_no_compliance_claim() {
    let report = RustLanguagePolicy::new()
        .evaluate_rust_quality(
            std::path::Path::new("."),
            &rust_diff_ctx(&["README.md"], "+++ b/README.md\n+ a documentation line\n"),
        )
        .expect("evaluates");

    assert_eq!(
        report.measurement,
        RustSkillsMeasurement::NothingToMeasure,
        "no `.rs` file changed, so no rule was evaluated"
    );
    assert_eq!(
        report.rules_evaluated_count, 0,
        "a run that evaluated nothing must publish a count of nothing, not the \
         size of a corpus it never loaded"
    );
    assert!(
        report.categories_evaluated.is_empty(),
        "no category was evaluated either; got {:?}",
        report.categories_evaluated
    );

    let summary = report.summary.to_lowercase();
    for claim in ["380", "compliant", "passed", "verified"] {
        assert!(
            !summary.contains(claim),
            "the summary of a run that evaluated nothing claims `{claim}`: {:?}",
            report.summary
        );
    }
}

/// Catches: the count restored as a literal somewhere else, or drifting from
/// the rules the engine actually runs.
///
/// The published count must be the length of the ruleset the scan iterates,
/// which is the ESLint/semgrep convention -- report the resolved ruleset, never
/// a remembered total.
#[test]
fn the_published_rule_count_is_the_ruleset_the_engine_actually_evaluates() {
    let report = RustLanguagePolicy::new()
        .evaluate_rust_quality(
            std::path::Path::new("."),
            &rust_diff_ctx(
                &["src/handler.rs"],
                "+++ b/src/handler.rs\n+ pub fn ok() -> u32 { 2 }\n",
            ),
        )
        .expect("evaluates");

    assert_eq!(
        report.rules_evaluated_count,
        engine::RULES.len(),
        "the count published on the scorecard must be the ruleset that ran"
    );
    assert!(
        matches!(
            report.measurement,
            RustSkillsMeasurement::Evaluated { rust_files: 1 }
        ),
        "one `.rs` file was in scope; got {:?}",
        report.measurement
    );

    let mut categories: Vec<&str> = engine::RULES.iter().map(|r| r.category).collect();
    categories.sort_unstable();
    categories.dedup();
    assert_eq!(
        report.categories_evaluated.len(),
        categories.len(),
        "the categories published must be the categories the ruleset covers, \
         not a hand-written list with its own invented per-category totals"
    );
}

/// Catches: `RULES` and the scan drifting apart -- a rule deleted from the scan
/// while its entry stays in the table, or a rule added to the scan and never
/// counted. The count is only honest while the table is the scan's inventory.
///
/// A mechanism over source text, because nothing in the compiler relates a
/// `rule_id` string constructed in one place to a table in another.
#[test]
fn every_rule_the_engine_reports_is_in_the_table_the_count_comes_from() {
    let src = std::fs::read_to_string("src/rust_language_policy/engine.rs")
        .expect("engine.rs must exist");

    let mut emitted: Vec<String> = Vec::new();
    for (idx, _) in src.match_indices("rule_id: \"") {
        let rest = &src[idx + "rule_id: \"".len()..];
        let Some(close) = rest.find('"') else {
            continue;
        };
        emitted.push(rest[..close].to_string());
    }
    emitted.sort();
    emitted.dedup();
    assert!(
        !emitted.is_empty(),
        "no `rule_id` literal was found in engine.rs, so this check scanned nothing"
    );

    let mut tabled: Vec<String> = engine::RULES.iter().map(|r| r.id.to_string()).collect();
    tabled.sort();
    tabled.dedup();

    assert_eq!(
        emitted, tabled,
        "the rules the engine emits findings for and the table `rules_evaluated_count` \
         is derived from must be the same set"
    );
}

/// Catches: the literal surviving in the text a human reads. The struct can be
/// honest while the scorecard row, the pipeline comment and the README still
/// advertise a corpus of 380.
#[test]
fn nothing_the_rust_skills_gate_publishes_still_advertises_the_unevaluated_corpus() {
    const PUBLISHERS: &[&str] = &[
        "src/rust_language_policy/mod.rs",
        "src/rust_language_policy/engine.rs",
        "src/pre_merge_guard/matrix.rs",
        "src/pre_merge_guard/evaluator.rs",
        "src/webhook/pipelines/certify.rs",
        "README.md",
    ];

    // Code only. A comment recording that the literal used to be there is not a
    // publication; a `380` still reachable by `rustc` is. Same distinction
    // `fidelity_registry_citations_test::code_only` draws, for the same reason.
    let code_only = |line: &str| -> String {
        match line.find("//") {
            Some(at) => line[..at].to_string(),
            None => line.to_string(),
        }
    };

    let mut offenders: Vec<String> = Vec::new();
    for rel in PUBLISHERS {
        let body = std::fs::read_to_string(rel).unwrap_or_else(|e| panic!("read {rel}: {e}"));
        for (n, line) in body.lines().enumerate() {
            let code = if rel.ends_with(".rs") {
                code_only(line)
            } else {
                line.to_string()
            };
            if code.contains("380") {
                offenders.push(format!("{rel}:{}: {}", n + 1, line.trim()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "these lines still publish a 380-rule corpus that nothing here loads or \
         evaluates:\n  {}",
        offenders.join("\n  ")
    );
}
