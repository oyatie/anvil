//! The ledger must be executable, not descriptive.
//!
//! A post-mortem nobody can run decays into prose, and prose that names checks
//! which do not exist reads as coverage. These are the properties that keep
//! `postmortem::FIX_CLASSES` a mechanism.

use anvil::postmortem::{
    CLASSES_ONLY_CAUGHT_AFTER_THE_FACT, FIX_CLASSES, Layer, Mechanism, Status,
    awaiting_early_remedy, missing_remedies, total_instances,
};
use std::collections::BTreeSet;
use std::path::Path;

/// `path.rs::symbol` -> does that symbol exist in that file?
fn names_something_real(named: &str) -> bool {
    let (file, symbol) = match named.split_once("::") {
        Some((f, s)) => (f, Some(s)),
        None => (named, None),
    };
    let p = Path::new(file);
    let Ok(src) = std::fs::read_to_string(p) else {
        return false;
    };
    match symbol {
        None => true,
        Some(sym) => src.lines().any(|l| {
            let t = l.trim_start();
            [
                "fn ", "struct ", "enum ", "const ", "static ", "trait ", "type ",
            ]
            .iter()
            .any(|kw| {
                t.starts_with(&format!("{kw}{sym}")) || t.starts_with(&format!("pub {kw}{sym}"))
            }) || t.starts_with(&format!("{sym} {{"))
                || t.starts_with(&format!("{sym}:"))
                || t.starts_with(&format!("pub {sym}:"))
        }),
    }
}

#[test]
fn every_live_remedy_names_something_that_exists() {
    // A remedy naming a check that is not there is worse than no remedy: it
    // reads as covered, and the next reader stops looking.
    let mut stale = Vec::new();
    for class in FIX_CLASSES {
        for remedy in class.remedies {
            if let Status::Live { named } = remedy.status
                && !names_something_real(named)
            {
                stale.push(format!("{}: `{named}`", class.id));
            }
        }
    }
    assert!(
        stale.is_empty(),
        "{} live remedy/remedies name something absent from the tree:\n  {}",
        stale.len(),
        stale.join("\n  ")
    );
}

#[test]
fn no_class_is_recorded_without_a_remedy_at_some_layer() {
    // A class with no remedy at all is a fix that taught nothing.
    for class in FIX_CLASSES {
        assert!(
            !class.remedies.is_empty(),
            "{} records no remedy; the fix was made and the class was not admitted",
            class.id
        );
    }
}

#[test]
fn every_class_states_its_first_principles_and_its_evidence() {
    // The two things that make an entry re-derivable rather than a label. The
    // length floors are crude on purpose: they refuse a placeholder, not a
    // short sentence.
    for class in FIX_CLASSES {
        assert!(
            class.first_principles.len() > 80,
            "{}: first_principles is a label, not an explanation of why the class occurs",
            class.id
        );
        assert!(
            class.evidence.len() > 60 && class.instances > 0,
            "{}: evidence must be specific enough to re-derive the instance count",
            class.id
        );
    }
}

#[test]
fn a_semantic_remedy_must_say_why_a_mechanism_cannot_decide_it() {
    // Deterministic work does not belong in the semantic layer. Judgement is
    // expensive and non-reproducible, so spending it needs a stated reason.
    for class in FIX_CLASSES {
        for remedy in class.remedies {
            if let Mechanism::Semantic { why_not_mechanical } = remedy.mechanism {
                assert!(
                    why_not_mechanical.len() > 40,
                    "{}: a semantic remedy must say why a mechanism could not decide it",
                    class.id
                );
            }
        }
    }
}

#[test]
fn classes_caught_only_after_the_fact_may_fall_but_never_rise() {
    // The number the doctrine is about. A class caught only in CI has been
    // observed, not prevented, and every observation is another wave of fixes.
    let stuck = awaiting_early_remedy();
    assert_eq!(
        stuck.len(),
        CLASSES_ONLY_CAUGHT_AFTER_THE_FACT,
        "{} class(es) are caught no earlier than CI; the ledger records \
         {CLASSES_ONLY_CAUGHT_AFTER_THE_FACT}.\n\
         If this ROSE, a class was admitted with only a CI remedy -- ask what would prevent it \
         instead.\n\
         If this FELL, lower the constant in the change that moved the class earlier.\n  {}",
        stuck.len(),
        stuck
            .iter()
            .map(|c| format!(
                "{} (earliest live remedy: {:?})",
                c.id,
                c.earliest_live_layer()
            ))
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

#[test]
fn a_missing_remedy_is_a_work_item_and_says_what_it_would_do() {
    // `Missing` is the ledger's backlog. An entry that does not say what the
    // remedy would do is not a work item, it is a placeholder.
    for (class, remedy) in missing_remedies() {
        assert!(
            remedy.what.len() > 50,
            "{}: a missing remedy at {:?} must say what it would do",
            class.id,
            remedy.layer
        );
    }
}

#[test]
fn no_class_id_is_recorded_twice() {
    let mut seen = BTreeSet::new();
    for class in FIX_CLASSES {
        assert!(
            seen.insert(class.id),
            "{} is recorded twice; one entry wins silently",
            class.id
        );
    }
}

#[test]
fn the_generating_class_is_prevented_and_not_merely_detected() {
    // Duplication generated most of the other classes: thirteen gates shared
    // two defects because thirteen places each parsed a diff. A class that
    // generates others earns prevention, not detection.
    let dup = FIX_CLASSES
        .iter()
        .find(|c| c.id == "n-copies-of-one-logic")
        .expect("the generating class is recorded");
    assert_eq!(
        dup.earliest_live_layer(),
        Some(Layer::Unspellable),
        "the class that generated the others is not prevented at the type level"
    );
    assert!(
        dup.instances >= total_instances() / 4,
        "the instance counts no longer show duplication as a leading class; re-derive them"
    );
}

#[test]
fn zero_after_the_fact_is_not_read_as_zero_work_left() {
    // The ratchet reaching zero is the moment a scoreboard becomes misleading.
    // No class is caught only in CI, and that is not the same as every class
    // being fully covered: `missing_remedies` is the backlog, and it is not
    // empty. Asserted so nobody deletes the backlog because the headline number
    // looks finished.
    assert_eq!(
        awaiting_early_remedy().len(),
        CLASSES_ONLY_CAUGHT_AFTER_THE_FACT
    );
    assert!(
        !missing_remedies().is_empty(),
        "the ledger claims no outstanding remedies at all. If that is true, say so \
         deliberately by deleting this test; if it is not, a work item was dropped"
    );
    for (class, remedy) in missing_remedies() {
        assert!(
            !matches!(remedy.layer, Layer::Ci),
            "{}: a remedy still to be built should target something earlier than CI",
            class.id
        );
    }
}

/// The ledger must reach the artifact a person reads.
///
/// It was complete, ratcheted, and unreachable from production: it ran only
/// from a pre-push test, so the mechanism that makes "CI is debt" measurable
/// was not part of the running system. A caller alone is not enough — the
/// output has to arrive somewhere, or this is a stage that runs and says
/// nothing.
#[test]
fn the_prevention_debt_reaches_the_scorecard() {
    let line = anvil::postmortem::prevention_debt_line();
    assert!(
        line.contains("Prevention ledger"),
        "the published line does not identify itself: {line}"
    );
    for expected in [
        "class(es)",
        "instance(s)",
        "no earlier than CI",
        "review prose",
    ] {
        assert!(
            line.contains(expected),
            "the line drops `{expected}`, so a reader cannot tell which kind \
             of debt this is: {line}"
        );
    }
    // The scorecard is where it is published; assert the caller is there
    // rather than trusting that it is.
    let scorecard = anvil::source_scan::paths::module_source(
        "src/publish/scorecard",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    assert!(
        anvil::source_scan::code_only(&scorecard).contains("prevention_debt_line"),
        "nothing in the scorecard calls the ledger, so it is written, tested \
         and published nowhere"
    );
}

/// A class defended only by prose will recur, because the next instance needs
/// a reviewer to notice again. Measured: one class recurred three times in a
/// single day of this repository's history.
#[test]
fn classes_defended_only_by_prose_are_identifiable() {
    let prose_only = anvil::postmortem::defended_only_by_prose();
    for class in &prose_only {
        assert!(
            class
                .remedies
                .iter()
                .any(|r| matches!(r.mechanism, anvil::postmortem::Mechanism::Semantic { .. })),
            "`{}` is reported as prose-defended and names no semantic remedy",
            class.id
        );
    }
    // Not vacuous in the other direction: a class with a live mechanical
    // remedy must not be listed.
    assert!(
        anvil::postmortem::FIX_CLASSES.len() > prose_only.len(),
        "every class is reported as prose-defended, which would mean the \
         mechanical remedies this ledger records do not exist"
    );
}
