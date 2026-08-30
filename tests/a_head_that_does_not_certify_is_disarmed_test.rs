//! An arming outlives the head it was granted for, unless something takes it.
//!
//! `enlist_into_merge_queue` passes `--match-head-commit`, which GitHub
//! validates once, at the moment auto-merge is enabled. The merge happens
//! later, whenever the required checks go green. A contributor with write
//! access who pushes after that moves the head, and GitHub does not disable
//! auto-merge for it -- so the commit that eventually merges can be one no
//! report ever measured.
//!
//! The review pipeline re-certifies every head it sees. Until this change, what
//! it did with an inadmissible one was `warn!`.

use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Production Rust for a module, whether it is a file or a directory.
///
/// Given `src/merge_enlister`, this reads `merge_enlister.rs` if that is what
/// exists and every `.rs` under `merge_enlister/` if it is a directory instead.
/// Keyed to the module rather than to a path: splitting a file to satisfy the
/// oversized-file ratchet is a thing this repository does routinely, and a
/// check that a split silently blinds is the defect these very tests describe.
/// It bit this file within an hour of being written.
///
/// Delegated rather than spelled here: a copy that walks only the top level of
/// a directory misses the module whose own split has a split inside it, and a
/// scan missing part of its subject reports nothing wrong with that part.
fn production(module: &str) -> String {
    anvil::source_scan::paths::module_source(module, &repo())
}

/// The call exists and asks the forge for the right thing.
#[test]
fn the_enlister_can_take_an_arming_away() {
    let src = production("src/merge_enlister");
    assert!(
        src.contains("fn disarm_auto_merge"),
        "`MergeEnlister` cannot disarm, so an arming outlives the head it was \
         granted for"
    );
    assert!(
        src.contains("\"--disable-auto\""),
        "`disarm_auto_merge` does not pass `--disable-auto`, so whatever it \
         asks the forge, it is not this"
    );
}

/// Every path that is not `Enlist` must reach it.
///
/// Keyed to the dispatch rather than to a line number, and asserted as the
/// NEGATION -- `!matches!(phase, ... Enlist)` -- because a per-arm call is what
/// rots: a new `NextPhase` variant added later gets an arm and no disarm, and
/// nothing here would notice.
#[test]
fn every_declining_path_disarms_rather_than_one_of_them() {
    let disarm = production("src/merge_enlister");
    let guard = disarm
        .split_once("pub async fn unless_enlisting")
        .expect("the rule must live with the disarm, not at the call site")
        .1;
    assert!(
        guard.contains("matches!")
            && guard.contains("NextPhase::Enlist")
            && guard.contains("return None"),
        "`unless_enlisting` does not decide on `Enlist` alone. Written as the \
         negation, a new `NextPhase` variant disarms by default and has to be \
         argued out; written per-arm, it is forgotten."
    );

    let review = production("src/webhook/pipelines/review");
    let between = review
        .split_once("let phase = crate::webhook::next_phase::next_phase(&situation);")
        .expect("the review pipeline must decide a phase")
        .1
        .split_once("match phase {")
        .expect("the dispatch")
        .0;
    assert!(
        between.contains("unless_enlisting"),
        "nothing disarms between deciding the phase and acting on it, so an \
         arming from an earlier head survives this run's refusal"
    );
}

/// `disarm_auto_merge` must not be `Result`, so a caller cannot `?` on it.
///
/// `gh` exits non-zero when there is nothing to disable, which is the ordinary
/// case -- most pull requests were never armed. A caller writing `?` would
/// abandon the rest of a rejection because a pull request had nothing armed.
#[test]
fn disarming_cannot_abort_the_refusal_that_called_it() {
    let src = production("src/merge_enlister");
    let sig = src
        .split_once("pub async fn disarm_auto_merge")
        .expect("the method exists")
        .1
        .split_once('{')
        .expect("a body")
        .0;
    assert!(
        sig.contains("-> Disarmed"),
        "`disarm_auto_merge` returns something other than `Disarmed`: {}",
        sig.trim()
    );
    assert!(
        !sig.contains("Result"),
        "`disarm_auto_merge` returns a `Result`, so a caller can `?` on it and \
         abandon a refusal because there was nothing armed to take away"
    );
}

/// An unreachable forge is not "nothing was armed".
#[test]
fn the_outcome_keeps_absent_evidence_apart_from_a_measurement() {
    let src = production("src/merge_enlister");
    let decl = src
        .split_once("pub enum Disarmed {")
        .expect("the outcome type exists")
        .1
        .split_once("\n}")
        .expect("it closes")
        .0;
    for variant in ["WasArmed", "NothingArmed", "Unknown"] {
        assert!(
            decl.contains(variant),
            "`Disarmed` has no `{variant}`. Collapsing an unreachable forge \
             into \"nothing was armed\" reports a pull request as safe on \
             evidence nobody has."
        );
    }
}
