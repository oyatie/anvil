//! The first gate whose status is a harness verdict, and the join that
//! publishes it.
//!
//! `cleartext_transport_status` was `passed = findings == 0` over the diff
//! text. An empty diff produced zero findings and published `Passed`: examined
//! nothing, found nothing, reported clean -- the confusion `Evaluated` exists
//! to make unspellable, one type above where it was happening.
//!
//! What is checked here is the join, not the lint. `Withheld` must never reach
//! `Passed`, and a rule that could not run must block admission rather than
//! accuse the pull request.

use anvil::harness::corpus::Corpus;
use anvil::harness::gate_status::publish;
use anvil::harness::{Evaluated, Requires, Withheld, rules};
use anvil::pre_merge_guard::report::GateStatus;

const GATE: &str = "cleartext_transport_status";

fn verdict_over(paths: &[&str], diff: &str) -> Evaluated {
    let run = rules::registered().run(&Corpus::of_diff(paths, diff), &|_| true);
    run.per_rule
        .get(GATE)
        .expect("the harness inserts an entry for every registered rule")
        .clone()
}

/// The defect the conversion removes, stated as the property.
#[test]
fn a_change_that_adds_no_line_does_not_certify_this_gate() {
    let status = publish(GATE, &verdict_over(&["src/lib.rs"], ""));
    assert!(
        matches!(status, GateStatus::NotMeasured { .. }),
        "a diff with no added line was published as {status:?}. Zero findings \
         over zero lines is not a clean run, and the form this replaces \
         (`passed = findings == 0`) called it one."
    );
}

#[test]
fn an_added_cleartext_endpoint_fails_the_gate() {
    let status = publish(
        GATE,
        &verdict_over(
            &["src/client.rs"],
            "--- a/src/client.rs\n+++ b/src/client.rs\n\
             +const UP: &str = \"http://payments.internal/charge\";\n",
        ),
    );
    assert!(
        matches!(status, GateStatus::Failed(_)),
        "an added cleartext endpoint published {status:?}"
    );
}

#[test]
fn a_change_that_adds_no_cleartext_endpoint_passes() {
    let status = publish(
        GATE,
        &verdict_over(
            &["src/client.rs"],
            "--- a/src/client.rs\n+++ b/src/client.rs\n\
             +const UP: &str = \"https://payments.internal/charge\";\n",
        ),
    );
    assert_eq!(
        status,
        GateStatus::Passed,
        "a TLS endpoint must pass, or the gate refuses every change"
    );
}

/// Exhaustive over the reasons a rule can be withheld.
///
/// A count would let a new `Withheld` variant map to `Passed` unnoticed, which
/// is the only way I1 can be broken here.
#[test]
fn no_withheld_reason_whatsoever_publishes_a_pass() {
    for w in [
        Withheld::Undeclared,
        Withheld::InputsAbsent {
            needed: Requires::Changeset,
        },
        Withheld::FixtureFailed {
            detail: "the seeded defect stopped firing".to_string(),
        },
        Withheld::Unclassifiable {
            subjects: vec!["src/lib.rs".to_string()],
        },
    ] {
        let status = publish(GATE, &Evaluated::Withheld(w.clone()));
        match status {
            GateStatus::NotMeasured {
                ref gate_id,
                ref reason,
            } => {
                assert_eq!(gate_id, GATE, "the withholding must name its own gate");
                assert!(!reason.trim().is_empty(), "{w:?} withheld with no reason");
            }
            other => panic!("{w:?} published {other:?} instead of NotMeasured"),
        }
    }
}

/// An undeclared rule is withheld, not skipped.
///
/// `Harness::run` inserts an entry for every registered rule on every path, so
/// a rule the run did not declare is reported rather than absent from the map.
/// A missing key would read to the caller as a gate that was never registered.
#[test]
fn a_rule_the_run_did_not_declare_is_still_reported() {
    let run = rules::registered().run(&Corpus::of_diff(&["src/lib.rs"], "+x\n"), &|_| false);
    assert!(
        matches!(
            run.per_rule.get(GATE),
            Some(Evaluated::Withheld(Withheld::Undeclared))
        ),
        "an undeclared rule vanished from the run instead of being withheld"
    );
}
