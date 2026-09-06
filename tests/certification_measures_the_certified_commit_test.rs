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
    assert!(
        verified.contains("self.verify_at(head_sha).await?"),
        "`verified_at` hands out a `CertifiedTree` without running `verify_at`, \
         so the type carries a claim nobody checked"
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
    assert!(
        body.contains("Err(e)") && body.contains("cleanup"),
        "a worktree that failed verification is neither reported nor cleaned up"
    );
}
