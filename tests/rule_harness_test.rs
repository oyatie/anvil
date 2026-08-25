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
        2,
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
        2,
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
    let corpus = Corpus::default().with_contents(
        "audit/ports/emission-kernel/Cargo.toml",
        "[package]\nname = \"audit-emission-kernel\"\nserde = \"1\"\n",
    );
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
