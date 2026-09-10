//! A gate that reads a file reads the commit the report names.
//!
//! The shared clone is never checked out at the head under review.
//! `ensure_repo_cloned` only fetches, `prepare_pr_diff` only fetches and diffs
//! by SHA, and the one thing that moves that working tree is the fixer. So a
//! gate reading a file from it read the base branch, or whichever pull request
//! the fixer last touched -- while the report carried a genuine provenance mark
//! and a subject naming this head, so `subject_refusal` admitted it.
//!
//! `SubjectRoot` answers which REPOSITORY a scanner was handed. `CertifiedTree`
//! answers which COMMIT, and it is the answer rather than the question: the
//! only constructor runs `git rev-parse HEAD` and compares.

use anvil::source_scan::paths::module_source;
use std::path::Path;

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The corpus builds the tree it measures, rather than being handed one.
///
/// Stronger than taking a `CertifiedTree` argument: no caller can pass the
/// shared clone because no caller passes a tree at all.
#[test]
fn the_gate_corpus_builds_the_tree_it_measures() {
    let src = module_source("src/webhook/pipelines/certify", repo());
    let body = src
        .split_once("pub async fn certify_pull_request")
        .expect("the corpus entry point exists")
        .1;
    let sig = body
        .split_once(") -> Result<")
        .expect("its signature closes")
        .0;
    assert!(
        !sig.contains("repo_dir: &Path"),
        "the corpus takes a bare path, so it can be handed a tree at an \
         unknown commit: {}",
        sig.trim()
    );

    let head = body.split_once("// 2.").expect("the gates follow").0;
    assert!(
        head.contains("certified_tree_at(") && head.contains("tree.as_path()"),
        "the corpus does not build a proven tree before its first gate runs, \
         so a filesystem-reading gate reads whatever the shared clone is on"
    );
}

/// Gates that read the tree out of `diff_ctx` get the certified one too.
///
/// Half the corpus takes `repo_dir` explicitly and half reads
/// `diff_ctx.repo_working_dir`, which `prepare_pr_diff` sets to the shared
/// clone. Rooting only the explicit half left `compliance_guard`,
/// `clean_architecture_guard`, `rust_language_policy` and the pre-merge
/// evaluator still reading whichever pull request the fixer last touched --
/// under a report signed for this head.
///
/// Constructed is not substituted. An earlier version of this test located the
/// struct literal and checked it came first; deleting the one line that binds
/// `diff_ctx` to it left the literal in place, sent every gate back to the
/// shared clone, and this test passed. So it asserts the SHADOW.
#[test]
fn the_context_handed_to_the_gates_is_rooted_at_the_certified_tree() {
    let src = module_source("src/webhook/pipelines/certify", repo());
    let body = src
        .split_once("pub async fn certify_pull_request")
        .expect("the corpus entry point exists")
        .1;

    let substitution = body.find("let diff_ctx = &certified_ctx;").expect(
        "the corpus builds a certified context but never binds `diff_ctx` to it, so \
             every gate keeps reading the shared clone the caller passed in",
    );
    assert!(
        body[..substitution].contains("repo_working_dir: tree.root()"),
        "`diff_ctx` is bound to a context that was not rooted at the certified tree"
    );
    // A gate CALL, not a numbered comment: the comment can stay put while the
    // call it labels moves above the substitution.
    let first_gate = body
        .find(".ensure_documentation_parity(")
        .expect("the first gate call follows the tree construction");
    assert!(
        substitution < first_gate,
        "`diff_ctx` is bound to the certified context only AFTER a gate has already \
         read it, so that gate measured the shared clone"
    );
}

/// There is one way to make one, and it measures rather than asserts.
#[test]
fn a_certified_tree_can_only_come_from_a_rev_parse() {
    let subject = module_source("src/git_manager/subject", repo());
    let ctor = subject
        .split_once("impl CertifiedTree {")
        .expect("CertifiedTree exists")
        .1;
    assert!(
        ctor.contains("pub(crate) fn proven"),
        "`CertifiedTree`'s constructor is not `pub(crate)`. A `pub` one lets a \
         caller assert the commit it hoped for, which is the assertion this \
         type exists to replace with a measurement."
    );

    let worktree = module_source("src/git_manager/worktree", repo());
    let verified = worktree
        .split_once("pub async fn verified_at")
        .expect("the constructor's only caller exists")
        .1
        .split_once("\n    }")
        .expect("it closes")
        .0;
    // The call, not its syntax. This pinned `.await?` and broke when the
    // failure path grew an explicit cleanup -- a check on the spelling of a
    // thing rather than the thing.
    assert!(
        verified.contains("self.verify_at(head_sha).await"),
        "`verified_at` hands out a `CertifiedTree` without running `verify_at`, \
         so the type carries a claim nobody checked"
    );

    // The fact this file existed to protect, and did not.
    //
    // Every assertion above passed for the whole life of the defect: the
    // corpus DID call `certified_tree_at`, `verified_at` DID run `verify_at`,
    // and the constructor WAS `pub(crate)`. What none of them asked is which
    // directory the proven path names. It named `repo_dir` -- the shared clone,
    // which `ensure_repo_cloned` only ever fetches into and the fixer moves --
    // so every filesystem-reading gate measured whichever pull request the
    // fixer last touched, under a report signed for this one.
    assert!(
        verified.contains("self.worktree_path"),
        "`verified_at` does not root the certified tree at the worktree. That \
         is the whole point of the type: the shared clone is never checked out \
         at the head under review, so a gate reading it reads another pull \
         request's tree."
    );
    assert!(
        !verified.contains("self.repo_dir.clone()"),
        "`verified_at` roots the certified tree at the shared clone. This is \
         the defect this file is named for."
    );
}

/// The proven path is only true while the worktree exists.
///
/// `certified_tree_at` returned a bare `CertifiedTree`, so the
/// `EphemeralWorktree` dropped at the end of that expression and `Drop` removed
/// the directory before the first gate ran. Compile-checked rather than
/// scanned: `CertifiedCheckout` owns the worktree, so the borrow checker keeps
/// it alive for as long as the path is usable, and a future edit that hands out
/// the tree alone stops compiling rather than stops being true.
#[test]
fn the_certified_path_outlives_the_gates_that_read_it() {
    fn _owns_its_worktree(checkout: &anvil::git_manager::CertifiedCheckout) -> &std::path::Path {
        checkout.as_path()
    }

    let worktree = module_source("src/git_manager/worktree", repo());
    let signature = worktree
        .split_once("pub async fn certified_tree_at")
        .expect("the entry point exists")
        .1
        .split_once('{')
        .expect("its body opens")
        .0;
    assert!(
        signature.contains("CertifiedCheckout"),
        "`certified_tree_at` hands back a tree without the worktree that makes \
         it true, so the directory is removed before a gate can read it: {}",
        signature.trim()
    );
}

/// Both certification paths reach it, and neither can skip it.
///
/// The enlistment path certifies immediately before a merge. Building the tree
/// inside `certify_pull_request` is what makes it unskippable from either.
#[test]
fn both_certification_paths_measure_a_proven_tree() {
    let certify = module_source("src/webhook/pipelines/certify", repo());
    let review = module_source("src/webhook/pipelines/review", repo());
    for (what, src) in [
        ("the enlistment path", &certify),
        ("the review path", &review),
    ] {
        assert!(
            src.contains("certify_pull_request("),
            "{what} does not call the corpus at all"
        );
    }
    assert!(
        certify.matches("certified_tree_at(").count() >= 1,
        "the corpus never asks for a proven tree"
    );
}

/// A tree that cannot be proven withholds the certification; it does not fall
/// back to the clone.
#[test]
fn an_unprovable_tree_withholds_rather_than_falling_back() {
    let src = module_source("src/git_manager/worktree", repo());
    let body = src
        .split_once("pub async fn certified_tree_at")
        .expect("the constructor exists")
        .1
        .split_once("\n    }")
        .expect("it closes")
        .0;
    assert!(
        !body.contains("ensure_repo_cloned"),
        "`certified_tree_at` reaches for the shared clone. A fallback here is \
         the defect: it produces a tree at an unknown commit and hands it to \
         the corpus as though it were the certified one."
    );
    // The teardown moved with the ownership: `verified_at` now takes `self`,
    // so the failure path lives there rather than in the caller. `Drop` would
    // also remove the worktree, but only through its synchronous fallback,
    // which blocks the runtime -- so the explicit async cleanup has to be
    // where the failure is now seen.
    let verified = module_source("src/git_manager/worktree", repo())
        .split_once("pub async fn verified_at")
        .expect("the constructor exists")
        .1
        .split_once("\n    }")
        .expect("it closes")
        .0
        .to_string();
    assert!(
        verified.contains("self.cleanup().await"),
        "a worktree that failed verification is left on disk for the synchronous \
         Drop fallback, which blocks the runtime"
    );
    assert!(
        verified.contains("return Err(error)"),
        "a worktree that failed verification is not reported"
    );
}
