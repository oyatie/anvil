//! Preserved historical Unix filesystem demonstrations; not portable controls.
//! The implementation validation does not execute these tests. Function bodies
//! remain unchanged from the reviewed baseline; their comments are historical.

#[cfg(unix)]
use super::common::docs_claims::{evaluate, md_files_under};

#[cfg(unix)]
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
