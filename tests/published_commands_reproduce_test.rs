//! A number published beside a command must be what that command produces.
//!
//! This binary is the one that sees real corpus text: it reads `docs/plan/` and
//! passes what it finds to the evaluator. So it is scanned, along with the
//! evaluator module itself, by `no_source_here_reaches_a_process_or_writes`.
//!
//! The adversarial fixtures live in `docs_claims_hardening_test`, which builds
//! its own corpus in a tempdir and therefore writes files. It is deliberately
//! not scanned: it never sees `docs/plan/`. That is the boundary -- every file
//! that receives real corpus text is scanned; the file that fabricates a corpus
//! is not.

mod common;

use common::docs_claims::{check_corpus, evaluate, forbidden_hits, plan_docs, repo_root};
use std::path::Path;

#[test]
fn every_published_claim_produces_the_number_published_beside_it() {
    let root = repo_root().expect("the repository root must be readable");
    let docs = plan_docs().expect("docs/plan must be listable; an unreadable corpus is not a pass");
    let (claims, failures) = check_corpus(&root, &docs).unwrap_or_else(|e| panic!("{e}"));

    // A scan that examined nothing must not report a pass.
    assert!(
        claims > 0,
        "no `#=` assertions found under docs/plan/. A scan with an empty corpus \
         is not a pass."
    );

    assert!(
        failures.is_empty(),
        "{} of {} published claim(s) did not reproduce:\n\n{}",
        failures.len(),
        claims,
        failures.join("\n\n")
    );
}

#[test]
fn no_source_here_reaches_a_process_or_writes() {
    // A PROXY, deliberately labelled as one. What carries the safety property
    // is that the evaluator has one form -- `count '<regex>' in <glob>` -- with
    // no branch that takes a program name from the document at all. This scan
    // guards future edits to the two files that see corpus text; `include!`,
    // `#[path]` and anything else pulled in remain holes it names but cannot
    // close.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    for rel in [
        "tests/common/docs_claims.rs",
        "tests/published_commands_reproduce_test.rs",
    ] {
        let text = std::fs::read_to_string(manifest.join(rel)).unwrap_or_else(|e| {
            panic!("{rel} must be readable, or the scan measured nothing: {e}")
        });
        let hits = forbidden_hits(&text);
        assert!(
            hits.is_empty(),
            "{hits:?} appear in {rel}. Document text must never reach a process, \
             the network, or a write: `sort --compress-program`, `uniq IN OUT` \
             and `git grep --open-files-in-pager` all executed or wrote through \
             allowlists that looked airtight."
        );
    }
}

#[test]
fn a_malformed_claim_is_reported_rather_than_ignored() {
    // Everything that is not the one supported form fails loudly. Under the
    // spawning revisions, each of these was a command that ran.
    let root = repo_root().expect("repo root");
    for bad in [
        "rm -rf /tmp/x",
        "grep -c foo Cargo.toml",
        "echo touch /tmp/p | sort --compress-program=/bin/sh",
        "uniq Cargo.toml /tmp/written",
        "git config --global core.pager evil",
        "count missing-quotes in docs/plan/*.md",
        "count 'unterminated in docs/plan/*.md",
        "count 'ok' docs/plan/*.md",
        "count 'ok' in ",
        "count '[' in docs/plan/*.md",
    ] {
        assert!(
            evaluate(&root, bad).is_err(),
            "a malformed or hostile claim was accepted: {bad}"
        );
    }
}

#[test]
fn the_one_claim_form_measures_what_it_says() {
    // Against a known answer rather than trusted. Derived, not hardcoded: an
    // earlier revision pinned this to "4" and would have broken the moment a
    // test was added, for a reason unrelated to the thing under test.
    let root = repo_root().expect("repo root");
    let me = std::fs::read_to_string(file!()).expect("readable");
    let expect = me.lines().filter(|l| l.starts_with("#[test]")).count();
    let n = evaluate(
        &root,
        "count '^#\\[test\\]' in tests/published_commands_reproduce_test.rs",
    )
    .expect("well-formed");
    assert_eq!(
        n,
        expect.to_string(),
        "counted the wrong number of #[test] lines"
    );

    // A glob matching nothing is an error, not an empty-and-green zero.
    assert!(evaluate(&root, "count 'x' in docs/plan/zz-nonexistent-*.md").is_err());
}
