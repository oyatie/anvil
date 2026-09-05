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

/// An intermediate symlink is followed by `canonicalize`, so containment alone
/// admits `src/g/config` when `src/g -> ../.git`. The dot rule has to run on the
/// resolved path, not on the glob the caller wrote.
#[cfg(unix)]
#[test]
fn a_link_into_dot_git_does_not_launder_the_dot_rule() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().canonicalize().expect("canonical root");
    std::fs::create_dir_all(root.join("docs/plan")).expect("mkdir");
    std::fs::create_dir_all(root.join(".git/hooks")).expect("mkdir .git");
    std::fs::write(root.join(".git/config"), "token = s3cret\n").expect("write");
    std::fs::write(root.join(".git/hooks/pre-push"), "s3cret\n").expect("write");
    std::os::unix::fs::symlink("../.git", root.join("docs/g")).expect("symlink");

    for glob in [
        "docs/g/config",
        "docs/g/conf*",
        "docs/g/hooks/pre-push",
        "docs/g/hooks/*",
    ] {
        let r = evaluate(&root, &format!("count 's3cret' in {glob}"));
        assert!(
            r.is_err(),
            "`{glob}` leaked a count through an intermediate symlink: {r:?}"
        );
    }
}

/// The scan covers every file compiled into the binaries that see corpus text.
///
/// `tests/common/mod.rs` is the evaluator's parent module, compiled into both
/// binaries and named nowhere: a verbatim spawn there ran with 8/8 green. The
/// boundary was a sentence in a header rather than a list the scan reads.
#[test]
fn the_scanned_set_covers_the_whole_compiled_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    for rel in [
        "tests/common/mod.rs",
        "tests/common/docs_claims.rs",
        "tests/published_commands_reproduce_test.rs",
    ] {
        let text = std::fs::read_to_string(manifest.join(rel))
            .unwrap_or_else(|e| panic!("{rel} must be readable: {e}"));
        assert!(
            forbidden_hits(&text).is_empty(),
            "{rel} is compiled into a binary that reads the corpus and must be \
             scanned: {:?}",
            forbidden_hits(&text)
        );
    }
    // `mod` is the ordinary way to pull a file in, and the disclaimer named only
    // `include!` and `#[path]`. A submodule of the evaluator would be unscanned.
    let dir = manifest.join("tests/common");
    for entry in std::fs::read_dir(&dir).expect("tests/common must be listable") {
        let path = entry.expect("entry").path();
        if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path).expect("readable");
            assert!(
                forbidden_hits(&text).is_empty(),
                "{} is under tests/common/ and unscanned",
                path.display()
            );
        }
    }
}

/// The walk root is gated, not just the entries under it.
///
/// `md_files_under` gates every entry it lists, and the previous fixture put a
/// symlink *inside* `docs/plan`, which the per-entry gate catches. Nothing
/// covered `docs/plan` ITSELF being a link -- so deleting the walk-root gate
/// left the suite 12/12 green, and `docs/plan -> /etc` would be `read_dir`'d
/// with an entry name reaching the failure message. That is the exact defect
/// the gate's own comment records, with no fixture behind it until now.
#[cfg(unix)]
#[test]
fn the_corpus_walk_gates_its_own_root() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().canonicalize().expect("canonical root");
    let away = tempfile::tempdir().expect("away");
    std::fs::write(away.path().join("leak.md"), "x\n").expect("write");
    std::fs::create_dir_all(root.join("docs")).expect("mkdir");
    std::os::unix::fs::symlink(away.path(), root.join("docs/plan")).expect("symlink");

    let e = md_files_under(&root, &root.join("docs/plan"))
        .expect_err("a corpus root that is a symlink out of the tree must be refused");
    assert!(
        !e.contains("leak.md"),
        "the refusal named an entry read from outside the repository: {e}"
    );
}

/// A symlink inside the corpus is refused even when it points at a file that is
/// also inside the corpus.
///
/// Containment and the dot rule already refuse every link that leaves the tree,
/// so with only those fixtures the symlink arm could be deleted with the suite
/// green -- an unmeasured branch, which by this repository's own reckoning is
/// not defence in depth but dead code. This is the one case only that arm can
/// decide, and it is a policy the module states: nothing in the corpus may be a
/// link, because git carries symlinks in a pull request.
#[cfg(unix)]
#[test]
fn a_symlink_inside_the_corpus_is_refused_even_pointing_inside_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().canonicalize().expect("canonical root");
    std::fs::create_dir_all(root.join("docs/plan")).expect("mkdir");
    std::fs::write(root.join("docs/plan/real.md"), "a\n").expect("write");
    std::os::unix::fs::symlink("real.md", root.join("docs/plan/alias.md")).expect("symlink");

    let e = evaluate(&root, "count 'a' in docs/plan/alias.md")
        .expect_err("a link inside the corpus must be refused, not followed");
    assert!(e.contains("not a readable path"), "wrong refusal: {e}");
    assert!(
        md_files_under(&root, &root.join("docs/plan")).is_err(),
        "the walk followed a link that stays inside the corpus"
    );
}
