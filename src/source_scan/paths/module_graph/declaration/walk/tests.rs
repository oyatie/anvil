use super::*;
use crate::source_scan::paths::module_graph::roots::CrateRoot;

#[test]
fn matching_root_contexts_are_measured_separately_and_completeness_is_conjunctive() {
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
    assert!(
        roles_with_contexts(&canonical_repo, std::slice::from_ref(&root), &[context()])
            .unwrap()
            .complete
    );
    let unknown = CrateRoot {
        path: root.clone(),
        aliases: BTreeSet::new(),
        audited_derive_crates: BTreeMap::new(),
    };
    assert!(
        !roles_with_contexts(
            &canonical_repo,
            std::slice::from_ref(&root),
            &[context(), unknown]
        )
        .unwrap()
        .complete
    );
    assert!(
        !roles_with_contexts(&canonical_repo, &[root], &[])
            .unwrap()
            .complete
    );
}
