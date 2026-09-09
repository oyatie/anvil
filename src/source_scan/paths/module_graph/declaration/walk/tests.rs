use super::*;
use crate::source_scan::paths::module_graph::roots::CrateRoot;

#[test]
fn a_root_context_that_cannot_measure_a_derive_names_the_file_and_the_reason() {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path().join("lib.rs");
    std::fs::write(
        &root,
        "#[derive(serde::Serialize)] struct Record; #[cfg(test)] mod fixture;",
    )
    .unwrap();
    std::fs::write(repo.path().join("fixture.rs"), "pub struct Fixture;").unwrap();
    let canonical_repo = std::fs::canonicalize(repo.path()).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let context = || CrateRoot {
        path: root.clone(),
        aliases: BTreeSet::new(),
        audited_derive_crates: [("serde".to_owned(), "serde".to_owned())].into(),
    };
    // The audited derive is measurable, so the walk yields roles.
    assert!(
        roles_with_contexts(&canonical_repo, std::slice::from_ref(&root), &[context()])
            .unwrap()
            .roles
            .contains_key(&root)
    );

    // The same root under a context that cannot account for the derive is not
    // a quieter answer: it is an error naming the file and why.
    let unaudited = CrateRoot {
        path: root.clone(),
        aliases: BTreeSet::new(),
        audited_derive_crates: BTreeMap::new(),
    };
    for contexts in [vec![context(), unaudited], Vec::new()] {
        let error = roles_with_contexts(&canonical_repo, std::slice::from_ref(&root), &contexts)
            .expect_err("an unmeasurable derive must not be answered with a role map");
        assert!(
            error.contains("lib.rs") && error.contains("cannot be measured"),
            "the error must name the file and the reason, got: {error}"
        );
    }
}
