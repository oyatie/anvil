//! Every property the docs-claim gate was hardened with, exercised against a
//! corpus built to break it.
//!
//! Before this file, all of them were prose. A review deleted each in turn --
//! neutered `gate`, killed fence tracking, killed the empty-expectation guard,
//! restored the `unwrap_or_default()` that broke I1 -- and the suite stayed
//! green all four times, because `docs/plan/` contains no symlink, no absolute
//! path, no empty `#=`, no unfenced marker and no unreadable file. A defense
//! nothing exercises is a defense the next edit removes for free.
//!
//! This binary fabricates its corpus, so it writes files, and is therefore not
//! covered by the forbidden-needle scan. It never reads `docs/plan/`.

mod common;

use common::docs_claims::{check_claim, claims_in, evaluate, forbidden_hits, md_files_under};
use std::io::Write;

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
fn containment_is_exercised_rather_than_asserted() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().canonicalize().expect("canonical root");
    std::fs::create_dir_all(root.join("docs/plan")).expect("mkdir");
    std::fs::write(root.join("docs/plan/real.md"), "a\nb\n").expect("write");

    // The out-of-tree target is `/etc/hosts`, not a second tempdir: on macOS a
    // tempdir is named `.tmpXXXX`, so an absolute path into one is refused by
    // the dot rule before containment is ever consulted. The first draft of
    // this test did exactly that -- an assertion naming `gate` while measuring
    // the rule above it, which is the defect this file exists to stop.
    let outside = std::path::Path::new("/etc/hosts");
    assert!(
        outside.is_file(),
        "this test needs an out-of-tree regular file with no dot component"
    );

    // An absolute path out of the tree, with no dot component, so only
    // `canonicalize().starts_with(root)` can refuse it -- neutering `gate`
    // turns this red.
    let e = evaluate(&root, "count '^127' in /etc/hosts")
        .expect_err("an absolute path out of the corpus must be refused");
    assert!(e.contains("not a readable path"), "wrong refusal: {e}");

    // A symlink out of the tree is refused, not followed -- in the glob path
    // and in the corpus walk, which are two different code paths that got this
    // wrong at different times.
    #[cfg(unix)]
    {
        let link = root.join("docs/plan/link.md");
        std::os::unix::fs::symlink(outside, &link).expect("symlink");
        let e = evaluate(&root, "count '^127' in docs/plan/link.md")
            .expect_err("a symlink out of the corpus must be refused");
        assert!(
            e.contains("not a readable path"),
            "a symlink must refuse indistinguishably from absent, or it is an \
             existence oracle: {e}"
        );
        assert!(
            md_files_under(&root, &root.join("docs/plan")).is_err(),
            "the corpus walk followed a symlink"
        );
        std::fs::remove_file(&link).expect("unlink");
    }

    // A dot component. `.git/config` is inside the repository, so containment
    // alone admits it -- and it holds the checkout token, which a failing
    // claim's reported count can binary-search out of a public CI log.
    std::fs::create_dir_all(root.join(".git")).expect("mkdir .git");
    std::fs::write(root.join(".git/config"), "token = s3cret\n").expect("write");
    let e = evaluate(&root, "count 's3cret' in .git/config")
        .expect_err("a dot component must be refused");
    assert!(e.contains("beginning with `.`"), "wrong refusal: {e}");

    // `..` is caught by the same rule, before containment runs.
    assert!(evaluate(&root, "count '^127' in ../etc/hosts").is_err());
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
