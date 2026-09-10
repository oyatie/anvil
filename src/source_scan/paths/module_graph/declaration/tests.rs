//! What the declaration graph does when it cannot measure a file.
//!
//! Absent evidence is never a pass. An unmeasurable graph may not answer with
//! an empty test set, with every contained file called production, or with
//! "not a test" for a source it never reached: each is a verdict drawn from no
//! evidence. It names the file and the reason instead.

use super::*;

/// A crate root whose production syntax cannot be measured, and the file the
/// walk would have to reach through it.
fn unmeasurable_root(repo: &Path) -> PathBuf {
    let root = repo.join("lib.rs");
    fs::write(
        &root,
        "#[derive(unaudited::Trait)] pub struct Record; #[cfg(test)] mod fixture;",
    )
    .unwrap();
    fs::write(repo.join("fixture.rs"), "pub struct Fixture;").unwrap();
    fs::canonicalize(root).unwrap()
}

fn measurable_root(repo: &Path) -> PathBuf {
    let root = repo.join("lib.rs");
    fs::write(&root, "pub struct Record; #[cfg(test)] mod fixture;").unwrap();
    fs::write(repo.join("fixture.rs"), "pub struct Fixture;").unwrap();
    fs::canonicalize(root).unwrap()
}

fn names_the_file_and_the_reason(error: &str) {
    assert!(
        error.contains("lib.rs") && error.contains("cannot be measured"),
        "the error must name the file and the reason, got: {error}"
    );
}

#[test]
fn an_unmeasurable_graph_is_not_an_empty_test_set() {
    let repo = tempfile::tempdir().unwrap();
    let root = unmeasurable_root(repo.path());
    let canonical_repo = fs::canonicalize(repo.path()).unwrap();

    // The seed applies: the same fixture without the unaudited derive is
    // measured, and proves the declared child is test-only.
    let measurable = tempfile::tempdir().unwrap();
    let good_root = measurable_root(measurable.path());
    let good_repo = fs::canonicalize(measurable.path()).unwrap();
    let measured =
        declared_test_module_files_from_roots(&good_repo, std::slice::from_ref(&good_root))
            .expect("a measurable root yields its declared test sources");
    assert!(measured.contains(&fs::canonicalize(good_repo.join("fixture.rs")).unwrap()));

    // An empty set is indistinguishable from "this repository declares no test
    // modules", which is why it may not stand in for "not measured".
    let error = declared_test_module_files_from_roots(&canonical_repo, &[root])
        .expect_err("an unmeasurable graph must not be reported as an empty test set");
    names_the_file_and_the_reason(&error);
}

#[test]
fn an_unmeasurable_graph_does_not_call_every_contained_file_production() {
    let repo = tempfile::tempdir().unwrap();
    let root = unmeasurable_root(repo.path());
    let canonical_repo = fs::canonicalize(repo.path()).unwrap();
    let error = declared_production_module_files_from_roots(&canonical_repo, &[root])
        .expect_err("an unmeasurable graph must not be widened to every contained file");
    names_the_file_and_the_reason(&error);
}

#[test]
fn an_unmeasurable_graph_is_not_a_role_map_that_omits_the_test_role() {
    let repo = tempfile::tempdir().unwrap();
    let root = unmeasurable_root(repo.path());
    let canonical_repo = fs::canonicalize(repo.path()).unwrap();
    // Every classifier is built from this map, so a map that omits a role is
    // a classifier that calls a test source production. `new` takes this with
    // `?`, so an unmeasurable graph leaves no classifier to ask.
    let error = module_roles_from_roots(&canonical_repo, &[root])
        .expect_err("an unmeasurable graph must not be answered with a role map");
    names_the_file_and_the_reason(&error);
}

#[test]
fn a_test_harness_attribute_is_not_unmeasured_production_syntax() {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path().join("lib.rs");
    fs::write(
        &root,
        "pub struct Record;\n#[cfg(test)]\nmod tests {\n    #[tokio::test]\n    async fn measured() {}\n}\n",
    )
    .unwrap();
    let canonical_repo = fs::canonicalize(repo.path()).unwrap();
    let root = fs::canonicalize(root).unwrap();
    // Nothing `#[tokio::test]` expands can ship, so its provenance is not the
    // production-syntax question. The same attribute on a shipped item is.
    module_roles_from_roots(&canonical_repo, std::slice::from_ref(&root))
        .expect("a test-harness attribute inside a test module is measurable");

    fs::write(&root, "#[tokio::test]\nasync fn shipped() {}\n").unwrap();
    let error = module_roles_from_roots(&canonical_repo, &[root])
        .expect_err("the same attribute on a shipped item has no audited provenance");
    names_the_file_and_the_reason(&error);
}
