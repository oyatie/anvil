//! Docs-claim contracts and preserved historical hardening fixtures.
//!
//! Historical comments below describe earlier revisions, not fresh verification.
//! Portable inert/data controls live in `docs_claims_contract`; filesystem
//! demonstrations remain separately named and Unix-gated in `docs_claims_legacy`.
//! This binary fabricates test corpora and is outside the lexical source proxy.
//! Its inventory assertion reads helper source, not the planning corpus.

mod common;
mod docs_claims_contract;
mod docs_claims_legacy;

use common::docs_claims::{check_claim, claims_in, evaluate, forbidden_hits, proxy_sources};
use std::io::Write;
use std::path::Path;

#[test]
fn the_forms_the_scan_catches_are_asserted_not_described() {
    // The header used to *describe* which forms defeat the scan. One of those
    // descriptions was false for two revisions -- including the commit that
    // claimed to fix it -- because nothing executed it. These execute.
    //
    // Snippets are assembled at runtime for the same reason the needles are:
    // written as literals they would trip the scan over the module.
    let (s, pr, cmd, f, opt, inc) = ("std", "process", "Command", "File", "options", "include");
    let cases: Vec<(String, bool, &str)> = vec![
        (
            format!("use {s}::{{{pr} as p}};"),
            true,
            "an alias behind a brace was the live defeat",
        ),
        (
            format!("use {s}::{{{pr}::{{{cmd} as C}}}};"),
            true,
            "nested braces with an alias",
        ),
        (
            format!("{s} :: {pr} :: {cmd}"),
            true,
            "spacing, normalised away",
        ),
        (
            format!("{f}::{opt}().write(true).open(p)"),
            true,
            "a write path that names no option type at all",
        ),
        (format!("{inc}!(\"elsewhere.rs\")"), true, "another file"),
        (
            "let total = text.lines().filter(|l| re.is_match(l)).count();".to_string(),
            false,
            "the evaluator's own hot line must stay green, or the scan is merely \
             always-true",
        ),
    ];
    for (snippet, should_trip, why) in cases {
        let hits = forbidden_hits(&snippet);
        assert_eq!(
            !hits.is_empty(),
            should_trip,
            "`{snippet}` -- {why}. Scan returned {hits:?}"
        );
    }
}

#[test]
fn absent_evidence_is_never_a_pass() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().canonicalize().expect("canonical root");
    std::fs::create_dir_all(root.join("docs/plan")).expect("mkdir");

    // Invalid UTF-8 must be an error, never a corpus of zero claims. Restoring
    // `unwrap_or_default()` turns this red.
    let bad = root.join("docs/plan/invalid.md");
    let mut fh = std::fs::File::create(&bad).expect("create");
    fh.write_all(&[0x23, 0x3d, 0xff, 0xfe])
        .expect("write bytes");
    drop(fh);
    assert!(
        claims_in(&bad).is_err(),
        "invalid UTF-8 read as an empty claim list; absent evidence is never a pass"
    );
}

#[test]
fn a_marker_the_scan_cannot_evaluate_is_refused_not_skipped() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().canonicalize().expect("canonical root");
    std::fs::create_dir_all(root.join("docs/plan")).expect("mkdir");
    std::fs::write(root.join("docs/plan/real.md"), "a\nb\n").expect("write");
    let doc = root.join("docs/plan/marks.md");

    // An unfenced marker, then a correctly fenced one. Both are collected, and
    // the fence toggle survives the line that carries a marker -- testing the
    // marker first and `continue`ing inverted fence state for the rest of the
    // file.
    std::fs::write(
        &doc,
        "count 'a' in docs/plan/real.md  #= 1\n\n```\ncount 'a' in docs/plan/real.md  #= 1\n```\n",
    )
    .expect("write");
    let claims = claims_in(&doc).expect("readable");
    assert_eq!(claims.len(), 2, "a marker was dropped");
    assert!(
        !claims[0].fenced,
        "an unfenced marker was reported as fenced"
    );
    assert!(
        claims[1].fenced,
        "the fence toggle was swallowed by the line carrying a marker"
    );

    // ...and the unfenced one is REFUSED, not merely recorded. `fenced: true ||`
    // turns this red.
    let why = check_claim(&root, &claims[0]).expect("an unfenced marker must be refused");
    assert!(
        why.contains("outside a fenced block"),
        "wrong refusal: {why}"
    );
    assert!(
        check_claim(&root, &claims[1]).is_none(),
        "a correct fenced claim was refused"
    );

    // An empty expectation asserts nothing and must be refused rather than read
    // as agreement. `if false &&` turns this red.
    std::fs::write(&doc, "```\ncount 'a' in docs/plan/real.md  #=\n```\n").expect("write");
    let claims = claims_in(&doc).expect("readable");
    assert_eq!(claims.len(), 1);
    let why = check_claim(&root, &claims[0]).expect("an empty expectation must be refused");
    assert!(why.contains("no expected value"), "wrong refusal: {why}");
}

/// One fixture per verdict `check_claim` can return.
///
/// Round 7 deleted the arm that compares the published number with the measured
/// one -- the gate's entire purpose -- and the suite stayed 8/8 green. So did
/// deleting `Err(_) => None`, which makes a malformed or refused claim pass
/// silently. Seven properties were removable that way, and the cause was one
/// thing: the arms of `check_claim` had no fixtures, only the two the previous
/// round happened to name.
///
/// A table over the verdicts rather than a seed per property, so the arm added
/// next is covered by the shape of this test rather than by remembering to add
/// an eighth.
#[test]
fn every_verdict_check_claim_can_return_has_a_fixture() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().canonicalize().expect("canonical root");
    std::fs::create_dir_all(root.join("docs/plan")).expect("mkdir");
    std::fs::write(root.join("docs/plan/real.md"), "a\nb\n").expect("write");
    let doc = root.join("docs/plan/case.md");

    // (corpus text, the substring the verdict must contain; None == reproduces)
    let cases: &[(&str, Option<&str>)] = &[
        ("```\ncount 'a' in docs/plan/real.md  #= 1\n```\n", None),
        (
            "```\ncount 'a' in docs/plan/real.md  #= 99\n```\n",
            Some("but the claim measures"),
        ),
        (
            "```\ncount 'a' in docs/plan/absent-*.md  #= 1\n```\n",
            Some("matched no files"),
        ),
        ("```\nnot a claim at all  #= 1\n```\n", Some("not a claim")),
        (
            "count 'a' in docs/plan/real.md  #= 1\n",
            Some("outside a fenced block"),
        ),
        (
            "```\ncount 'a' in docs/plan/real.md  #=\n```\n",
            Some("no expected value"),
        ),
    ];

    for (text, want) in cases {
        std::fs::write(&doc, text).expect("write");
        let claims = claims_in(&doc).expect("readable");
        assert_eq!(
            claims.len(),
            1,
            "fixture must hold exactly one claim: {text:?}"
        );
        let got = check_claim(&root, &claims[0]);
        match want {
            None => assert!(
                got.is_none(),
                "a claim that reproduces was reported as a failure: {got:?}"
            ),
            Some(needle) => {
                let why = got.unwrap_or_else(|| {
                    panic!("{text:?} must not reproduce, and did: expected {needle:?}")
                });
                assert!(
                    why.contains(needle),
                    "wrong verdict for {text:?}: expected {needle:?}, got {why:?}"
                );
            }
        }
    }
}

/// `evaluate` reads the files a glob matched, and that read is propagated too.
///
/// `claims_in`'s read was ratcheted; `evaluate`'s -- one function over, same
/// defect -- was not, so `unwrap_or_default()` there turned an unreadable target
/// into a count of zero.
#[test]
fn an_unreadable_target_is_an_error_not_a_count_of_zero() {
    use std::io::Write;
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().canonicalize().expect("canonical root");
    std::fs::create_dir_all(root.join("docs/plan")).expect("mkdir");
    let bad = root.join("docs/plan/bad.md");
    let mut fh = std::fs::File::create(&bad).expect("create");
    fh.write_all(&[0x61, 0xff, 0xfe]).expect("write bytes");
    drop(fh);
    let e = evaluate(&root, "count 'a' in docs/plan/bad.md")
        .expect_err("an unreadable target must be an error");
    assert!(e.contains("cannot be read"), "wrong error: {e}");
}

/// The declared helper subtree and explicit consumer share one lexical proxy.
/// This does not resolve external module mappings or generated source.
#[test]
fn source_proxy_covers_the_declared_helper_tree_and_consumer() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    for path in proxy_sources(manifest).expect("declared proxy inventory must be readable") {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} must be readable: {e}", path.display()));
        assert!(
            forbidden_hits(&text).is_empty(),
            "{} is in the declared source proxy: {:?}",
            path.display(),
            forbidden_hits(&text),
        );
    }
}
