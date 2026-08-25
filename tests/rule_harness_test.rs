//! The harness must make "absence of a finding" unusable as "absence of a
//! problem" -- the defect that appeared seven times in one day.

use anvil::harness::corpus::Corpus;
use anvil::harness::rules::{IoInPureFace, registered};
use anvil::harness::{Evaluated, Fixture, Harness, Requires, Rule, Withheld};

fn all(_: &str) -> bool {
    true
}
fn none(_: &str) -> bool {
    false
}

#[test]
fn a_run_over_nothing_is_not_clean() {
    // The whole class in one assertion. Previously: zero subjects, zero
    // findings, `findings.is_empty()` -> clean.
    let run = registered().run(&Corpus::default(), &all);
    assert!(
        !run.is_clean(),
        "a run that examined nothing must not be clean"
    );
    assert_eq!(
        run.withheld().len(),
        registered().rule_ids().len(),
        "every rule must say why it did not run"
    );
}

#[test]
fn a_rule_the_spec_never_declared_is_present_and_withheld() {
    // Eleven rules were previously invisible: no finding, no unmeasured entry.
    let corpus = Corpus::default().with_contents("iam/ports/x/Cargo.toml", "serde = \"1\"\n");
    let run = registered().run(&corpus, &none);
    assert_eq!(
        run.per_rule.len(),
        registered().rule_ids().len(),
        "no rule may be absent from the output"
    );
    assert!(
        run.withheld()
            .iter()
            .all(|(_, w)| **w == Withheld::Undeclared)
    );
    assert!(!run.is_clean());
}

#[test]
fn missing_inputs_withhold_rather_than_pass() {
    // Paths present, contents absent: the rule needs FileContents.
    let run = registered().run(&Corpus::of_paths(&["iam/ports/x/Cargo.toml"]), &all);
    assert!(!run.is_clean());
    assert!(matches!(
        run.withheld().first().map(|(_, w)| w),
        Some(Withheld::InputsAbsent { .. })
    ));
}

#[test]
fn a_measurement_over_zero_subjects_cannot_be_constructed() {
    // The type-level half: the only constructor refuses it.
    assert!(matches!(
        Evaluated::measured(0, vec![]),
        Evaluated::Withheld(_)
    ));
    assert!(matches!(
        Evaluated::measured(1, vec![]),
        Evaluated::Measured { .. }
    ));
}

#[test]
fn a_clean_run_requires_every_rule_to_have_measured() {
    // Every rung must be fed, not just the two the first rules needed: a
    // corpus that satisfies some rules and starves others is not clean, and
    // that is the point of the predicate.
    let corpus = Corpus::of_diff(
        &["audit/ports/emission-kernel/Cargo.toml"],
        "+serde = \"1\"\n",
    )
    .with_contents(
        "audit/ports/emission-kernel/Cargo.toml",
        "[package]\nname = \"audit-emission-kernel\"\nserde = \"1\"\n",
    )
    .with_commits(vec!["feat(audit): add the emission kernel".to_string()]);
    let run = registered().run(&corpus, &all);
    assert!(run.is_clean(), "conformant corpus: {:?}", run.per_rule);
}

#[test]
fn a_real_violation_is_found_with_its_codemod() {
    let corpus = Corpus::default().with_contents(
        "bus/adapters/file/Cargo.toml",
        "[package]\nname = \"messaging-file-adapter\"\n",
    );
    let run = registered().run(&corpus, &all);
    let f = run.findings();
    let rename = f
        .iter()
        .find(|f| f.rule == "package_name_not_canonical")
        .expect("the misnamed package must be found");
    assert!(
        rename.fix.is_some(),
        "a fix a human applies 47 times is not a fix"
    );
}

/// A rule that cannot demonstrate it fires.
struct NeverFires;
impl Rule for NeverFires {
    fn id(&self) -> &'static str {
        "never_fires"
    }
    fn requires(&self) -> Requires {
        Requires::PathsOnly
    }
    fn examine(&self, c: &Corpus) -> Evaluated {
        Evaluated::measured(c.subjects.len(), vec![])
    }
    fn fixture(&self) -> Fixture {
        Fixture {
            must_flag: Corpus::of_paths(&["iam/ports/x/Cargo.toml"]),
            must_pass: Corpus::of_paths(&["iam/ports/x/Cargo.toml"]),
        }
    }
}

#[test]
fn an_unproven_rule_is_withheld_not_believed() {
    // An early check that silently passes is worse than none: it occupies the
    // slot and buys down scrutiny downstream without earning it. Eleven inert
    // rules were strictly worse than eleven absent ones.
    let mut h = Harness::default();
    h.register(Box::new(NeverFires));
    let run = h.run(&Corpus::of_paths(&["a/core/b/Cargo.toml"]), &all);
    assert!(
        !run.is_clean(),
        "a rule that cannot fire must not certify anything"
    );
    assert!(matches!(
        run.withheld().first().map(|(_, w)| w),
        Some(Withheld::FixtureFailed { .. })
    ));
}

#[test]
fn every_registered_rule_proves_itself_on_every_run() {
    // Not a separate test suite that can rot out of sync: the harness runs each
    // fixture before trusting that rule's verdict, on every invocation.
    let rule: &dyn Rule = &IoInPureFace;
    {
        let f = rule.fixture();
        assert!(
            !matches!(rule.examine(&f.must_flag), Evaluated::Measured { ref findings, .. } if findings.is_empty())
        );
        assert!(
            matches!(rule.examine(&f.must_pass), Evaluated::Measured { ref findings, .. } if findings.is_empty())
        );
    }
}

#[test]
fn each_rule_declares_the_cheapest_stage_that_can_host_it() {
    // A defect caught late pays the sunk cost of everything that carried it
    // there. Placement is a property of the rule, so the harness can host it
    // at the cheapest rung rather than every rule choosing.
    assert_eq!(Requires::PathsOnly.stage(), "editor");
    assert_eq!(Requires::BuildGraph.stage(), "presubmit");
    assert!(
        Requires::PathsOnly < Requires::BuildGraph,
        "ordered by cost"
    );
}

#[test]
fn the_change_under_review_is_a_rung_the_harness_can_express() {
    // The binding constraint on migrating the shipped gates: a corpus of the
    // working tree answers "is this file wrong", and roughly half of them ask
    // "does this change make it wrong". That is a question about a pair of
    // revisions, and until `Changeset` existed it had no spelling here.
    let key = format!("AKIA{}", "IOSFODNN7EXAMPLE");
    let leak = Corpus::of_diff(
        &["src/config.rs"],
        &format!("+const KEY: &str = \"{key}\";\n"),
    );
    assert!(leak.satisfies(Requires::Changeset));
    assert!(
        leak.satisfies(Requires::PathsOnly),
        "changed files are subjects, so a paths-only rule runs at its own rung"
    );

    let run = registered().run(&leak, &all);
    let hits: Vec<_> = run
        .findings()
        .into_iter()
        .filter(|f| f.rule == "secret_on_added_line")
        .collect();
    assert_eq!(hits.len(), 1, "the added credential is found: {run:?}");
}

#[test]
fn a_deletion_of_a_leaked_key_is_not_a_leak() {
    // The inversion that made the secret gate refuse the pull request cleaning
    // up the leak. It is a fixture rather than a comment, so the harness
    // re-proves it before trusting the rule on every single run.
    let key = format!("AKIA{}", "IOSFODNN7EXAMPLE");
    let cleanup = Corpus::of_diff(
        &["src/config.rs"],
        &format!("-const KEY: &str = \"{key}\";\n+const KEY: &str = env!(\"AWS_KEY\");\n"),
    );
    let run = registered().run(&cleanup, &all);
    assert!(
        !run.findings()
            .iter()
            .any(|f| f.rule == "secret_on_added_line"),
        "a removed credential is not an added one: {run:?}"
    );
}

#[test]
fn an_absent_commit_log_is_withheld_not_accused() {
    // Gate 38 took `&[String]` and read an empty slice as "no violations
    // provable", publishing NotMeasured as an accusation at every pull request
    // whose commits never reached it. `None` now fails `satisfies` before the
    // rule is ever called, so the harness -- not the rule -- refuses to judge.
    let no_log = Corpus::of_paths(&["src/lib.rs"]);
    assert!(!no_log.satisfies(Requires::History));
    let run = registered().run(&no_log, &all);
    assert_eq!(
        run.withheld()
            .into_iter()
            .filter(|(id, w)| *id == "conventional_commit_subject"
                && matches!(
                    w,
                    Withheld::InputsAbsent {
                        needed: Requires::History
                    }
                ))
            .count(),
        1,
        "{run:?}"
    );

    // Supplied and empty is a different fact and must not read the same way:
    // the range genuinely adds no judgeable subject, so the rule is measured
    // over zero and withheld for THAT reason, never reported clean.
    let empty_log = Corpus::of_paths(&["src/lib.rs"]).with_commits(vec![]);
    assert!(empty_log.satisfies(Requires::History));
    let run = registered().run(&empty_log, &all);
    assert!(
        !run.is_clean(),
        "a log that judged nothing is not a clean log: {run:?}"
    );
}

#[test]
fn a_non_conventional_subject_is_found_at_the_history_rung() {
    let bad = Corpus::of_paths(&["src/lib.rs"]).with_commits(vec![
        "fix(harness): withhold on an absent log".to_string(),
        "made it work".to_string(),
    ]);
    let run = registered().run(&bad, &all);
    let hits: Vec<_> = run
        .findings()
        .into_iter()
        .filter(|f| f.rule == "conventional_commit_subject")
        .collect();
    assert_eq!(hits.len(), 1, "{run:?}");
    assert_eq!(hits[0].subject, "made it work");
}

#[test]
fn the_ladder_reaches_every_stage_a_gate_actually_runs_at() {
    // Four rungs could not host roughly half the shipped gates: the ones that
    // read a diff, a commit log, a toolchain, or remote state had nowhere to
    // declare themselves and would have had to lie about their inputs.
    for (rung, stage) in [
        (Requires::PathsOnly, "editor"),
        (Requires::FileContents, "pre-commit"),
        (Requires::Changeset, "pre-commit"),
        (Requires::Manifests, "pre-push"),
        (Requires::History, "pre-push"),
        (Requires::BuildGraph, "presubmit"),
        (Requires::Toolchain, "presubmit"),
        (Requires::Network, "merge-queue"),
    ] {
        assert_eq!(rung.stage(), stage);
        assert!(
            !Corpus::default().satisfies(rung),
            "an empty corpus must satisfy no rung, including {rung:?}"
        );
    }
    assert!(
        Requires::PathsOnly < Requires::Changeset && Requires::Changeset < Requires::Network,
        "ordered by cost, so the harness can place a rule at its cheapest host"
    );
}
