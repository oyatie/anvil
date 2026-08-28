//! What to do about a finding, decided before anything is written.
//!
//! # Why a fix is not the first question
//!
//! The fix loop today is: review comment in, patched line out. That closes the
//! instance and leaves the class, so the same defect arrives again under a
//! different line number and is patched again. Anvil's own history has one
//! class recorded four times and another three times within a single day.
//!
//! The order that stops it is: what CLASS is this, does a remedy already
//! EXIST, and if not, at which LAYER should one live. Patching is what happens
//! after those three have answers, and only sometimes.
//!
//! # This builds nothing new
//!
//! `postmortem::FIX_CLASSES` already records every known class, every remedy,
//! the layer each sits at and whether it is `Live` or `Missing`. `Layer`
//! already is the prevention ladder. `harness::Rule` + `Fixture` already is
//! what a new mechanical remedy looks like. None of it was reachable from the
//! fixer, which is the whole defect: the ledger was written, and the component
//! that decides what to do about a finding never opened it.

use crate::postmortem::{FIX_CLASSES, FixClass, Layer, Mechanism, Status};

/// What the fixer should do, in the order the doctrine requires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disposition {
    /// A live remedy already refuses this class. Apply it; do not write a new
    /// one. This is the branch that stops N+1 mechanisms accumulating for one
    /// class, and it is the most common correct answer once a ledger exists.
    RemedyExists {
        class: &'static str,
        remedy: &'static str,
        layer: Layer,
    },
    /// The class is known and its remedy was named but never built. The work
    /// is to BUILD THE REMEDY, not to patch this instance -- the decision was
    /// already taken and left undone.
    RemedyMissing {
        class: &'static str,
        remedy: &'static str,
        layer: Layer,
    },
    /// Known, and defended only by review prose. Every future instance depends
    /// on a reviewer noticing again, so the work is a mechanical remedy at the
    /// earliest layer that can decide.
    OnlyProse {
        class: &'static str,
        why_not_mechanical: &'static str,
    },
    /// No class matches. Record the class FIRST: a finding that enters no
    /// ledger will be found again, and the second sighting will not know about
    /// the first.
    Unclassified { subject: String },
}

impl Disposition {
    /// Whether writing a patch is the right next action.
    ///
    /// False for every classified branch. That is the point: a known class
    /// with a live remedy needs the remedy run, not a hand-edit beside it.
    pub fn patch_is_the_work(&self) -> bool {
        matches!(self, Disposition::Unclassified { .. })
    }

    pub fn explain(&self) -> String {
        match self {
            Disposition::RemedyExists {
                class,
                remedy,
                layer,
            } => format!(
                "`{class}` is already refused at {layer:?} by: {remedy}. Apply that \
                 rather than writing a second mechanism for one class."
            ),
            Disposition::RemedyMissing {
                class,
                remedy,
                layer,
            } => format!(
                "`{class}` has a remedy named at {layer:?} and NOT built: {remedy}. \
                 The work is building it; patching this instance leaves the class open."
            ),
            Disposition::OnlyProse {
                class,
                why_not_mechanical,
            } => format!(
                "`{class}` is defended only by review prose ({why_not_mechanical}). \
                 Every future instance needs a reviewer to notice again."
            ),
            Disposition::Unclassified { subject } => format!(
                "no recorded class matches `{subject}`. Record the class before \
                 fixing it: an unledgered finding is found again by someone who \
                 does not know it was found before."
            ),
        }
    }
}

/// Which recorded class a finding belongs to.
///
/// Matched on the class's own vocabulary rather than on free text: each class
/// carries `what` and `first_principles` written in the terms a reviewer uses,
/// and a finding that shares distinctive words with one of them is that class.
/// Deliberately conservative -- a wrong class is worse than none, because it
/// sends the work at a remedy that does not refuse this defect.
pub fn classify(finding: &str) -> Option<&'static FixClass> {
    let hay = finding.to_lowercase();
    FIX_CLASSES
        .iter()
        .filter_map(|c| {
            let score = distinctive_words(c.what)
                .filter(|w| hay.contains(*w))
                .count();
            (score >= 2).then_some((score, c))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, c)| c)
}

/// Words from a class description worth matching on.
///
/// Short and common words are dropped: matching on "the" or "a" would make
/// every finding every class, which is how a fuzzy matcher becomes a random
/// one. The same reasoning is recorded in `gate_proof`, where inference over
/// test names claimed a gate was proven by an unrelated test.
fn distinctive_words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 5)
}

/// The doctrine, applied: class first, existing machinery second, layer third.
pub fn triage(finding: &str) -> Disposition {
    let Some(class) = classify(finding) else {
        return Disposition::Unclassified {
            subject: finding.chars().take(80).collect(),
        };
    };

    // Existing machinery first. A live remedy means this is a dispatch
    // problem, not a design problem.
    if let Some(r) = class
        .remedies
        .iter()
        .filter(|r| matches!(r.status, Status::Live { .. }))
        .min_by_key(|r| r.layer)
    {
        // A class defended only by prose stays open to whether the next
        // reviewer notices, so it is reported rather than counted as covered.
        let only_prose = class
            .remedies
            .iter()
            .filter(|r| matches!(r.status, Status::Live { .. }))
            .all(|r| matches!(r.mechanism, Mechanism::Semantic { .. }));
        if let (true, Mechanism::Semantic { why_not_mechanical }) = (only_prose, r.mechanism) {
            return Disposition::OnlyProse {
                class: class.id,
                why_not_mechanical,
            };
        }
        return Disposition::RemedyExists {
            class: class.id,
            remedy: r.what,
            layer: r.layer,
        };
    }

    // Known class, nothing built. The earliest named layer is where it belongs.
    if let Some(r) = class.remedies.iter().min_by_key(|r| r.layer) {
        return Disposition::RemedyMissing {
            class: class.id,
            remedy: r.what,
            layer: r.layer,
        };
    }

    Disposition::Unclassified {
        subject: class.id.to_string(),
    }
}
