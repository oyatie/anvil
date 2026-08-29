//! A fix begins with the class, not the patch.
//!
//! Root cause, then existing machinery, then the layer a remedy belongs at.
//! Patching is what happens after those three have answers, and only when no
//! class matches — otherwise the instance closes and the class stays open.

use anvil::fixer::triage::{Disposition, classify, triage};
use anvil::postmortem::FIX_CLASSES;

fn a_recorded_class() -> &'static anvil::postmortem::FixClass {
    FIX_CLASSES.first().expect("the ledger is not empty")
}

#[test]
fn a_finding_matching_no_class_is_unclassified_and_is_the_only_patchable_case() {
    let d = triage("the button is the wrong shade of blue");
    assert!(matches!(d, Disposition::Unclassified { .. }));
    assert!(d.patch_is_the_work());
}

#[test]
fn a_classified_finding_is_never_answered_with_a_patch() {
    let c = a_recorded_class();
    let d = triage(c.what);
    assert!(
        !d.patch_is_the_work(),
        "a known class needs its remedy run, not a hand-edit beside it: {d:?}"
    );
}

#[test]
fn classification_needs_more_than_one_shared_word() {
    // One word in common is coincidence. Inference over names already produced
    // a false match in `gate_proof`, where a gate was claimed proven by an
    // unrelated test.
    assert!(classify("the").is_none());
    assert!(classify("a report").is_none());
}

#[test]
fn a_class_whose_remedy_is_named_but_unbuilt_routes_to_building_it() {
    let unbuilt = FIX_CLASSES.iter().find(|c| {
        c.remedies
            .iter()
            .all(|r| matches!(r.status, anvil::postmortem::Status::Missing))
            && !c.remedies.is_empty()
    });
    let Some(c) = unbuilt else {
        return; // every class is defended; nothing to assert
    };
    assert!(
        matches!(triage(c.what), Disposition::RemedyMissing { .. }),
        "a decision already taken and not carried out is the work"
    );
}

#[test]
fn every_disposition_explains_what_to_do_next() {
    let d = triage("the button is the wrong shade of blue");
    let e = d.explain();
    assert!(e.len() > 40, "{d:?} explains nothing");
    assert!(
        !e.contains("  "),
        "wrapped literal leaked indentation: {e:?}"
    );
}
