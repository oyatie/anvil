//! Pure identity records and parsed text only; no filesystem or runtime fixture.

use super::*;

fn identity(package: &str, target: &str) -> CrateIdentity {
    CrateIdentity {
        manifest: Some(PathBuf::from(format!("{package}/Cargo.toml"))),
        root: PathBuf::from(format!("{package}/{target}.rs")),
        kind: if target == "lib" { "library" } else { "binary" },
    }
}

fn evidence(identity: CrateIdentity) -> TargetEvidence {
    TargetEvidence {
        root: OwnershipRoot {
            identity,
            aliases: BTreeMap::new(),
        },
        files: BTreeSet::from([PathBuf::from("shared-source.rs")]),
        complete: true,
        self_aliases: BTreeSet::new(),
        uncertain_aliases: BTreeSet::new(),
    }
}

fn relation(targets: Vec<TargetEvidence>, name: &str) -> RootRelation {
    ArchitectureOwnership {
        repo: PathBuf::new(),
        targets,
    }
    .relation_at(Path::new("shared-source.rs"), name)
}

#[test]
fn cross_package_and_same_package_distinct_targets_are_not_self() {
    for destination in [identity("right", "lib"), identity("left", "bin")] {
        let mut source = evidence(identity("left", "lib"));
        source
            .root
            .aliases
            .insert("named".into(), BTreeSet::from([destination]));
        assert_eq!(relation(vec![source], "named"), RootRelation::OtherCrate);
    }
}

#[test]
fn actual_identity_not_alias_spelling_controls_exemption() {
    for name in ["custom_library", "renamed_dependency"] {
        let mut source = evidence(identity("left", "lib"));
        source
            .root
            .aliases
            .insert(name.into(), BTreeSet::from([identity("right", "lib")]));
        assert_eq!(relation(vec![source], name), RootRelation::OtherCrate);
    }
    let mut source = evidence(identity("left", "lib"));
    source.root.aliases.insert(
        "same".into(),
        BTreeSet::from([source.root.identity.clone()]),
    );
    assert_eq!(relation(vec![source], "same"), RootRelation::SameCrate);
}

#[test]
fn named_self_alias_is_supported_from_actual_parsed_evidence() {
    let syntax = syn::parse_file("extern crate self as named_self;").unwrap();
    let (known, uncertain) = self_aliases(&syntax);
    let mut source = evidence(identity("left", "lib"));
    source.self_aliases = known;
    source.uncertain_aliases = uncertain;
    source.root.aliases.insert(
        "named_self".into(),
        BTreeSet::from([identity("right", "lib")]),
    );
    assert_eq!(
        relation(vec![source], "named_self"),
        RootRelation::SameCrate
    );
}

#[test]
fn conditional_self_alias_is_not_unconditional_identity() {
    let syntax =
        syn::parse_file("#[cfg(feature = \"maybe\")] extern crate self as named_self;").unwrap();
    let (known, uncertain) = self_aliases(&syntax);
    assert!(known.is_empty());
    let mut source = evidence(identity("left", "lib"));
    source.uncertain_aliases = uncertain;
    assert_eq!(relation(vec![source], "named_self"), RootRelation::Unknown);
}

#[test]
fn ambiguous_alias_and_incomplete_graph_are_not_clean_identity() {
    let mut ambiguous = evidence(identity("left", "lib"));
    ambiguous.root.aliases.insert(
        "named".into(),
        BTreeSet::from([identity("right", "lib"), identity("third", "lib")]),
    );
    assert_eq!(relation(vec![ambiguous], "named"), RootRelation::Unknown);
    let mut incomplete = evidence(identity("left", "lib"));
    incomplete.complete = false;
    incomplete.self_aliases.insert("named".into());
    assert_eq!(relation(vec![incomplete], "named"), RootRelation::Unknown);
    let mut incomplete_other = evidence(identity("left", "lib"));
    incomplete_other.complete = false;
    incomplete_other
        .root
        .aliases
        .insert("named".into(), BTreeSet::from([identity("right", "lib")]));
    assert_eq!(
        relation(vec![incomplete_other], "named"),
        RootRelation::Unknown
    );
}

#[test]
fn known_forbidden_context_survives_other_unknown_owners() {
    let mut known = evidence(identity("left", "lib"));
    known
        .root
        .aliases
        .insert("named".into(), BTreeSet::from([identity("right", "lib")]));
    let mut unknown = evidence(identity("left", "bin"));
    unknown.complete = false;
    assert_eq!(
        relation(vec![unknown, known], "named"),
        RootRelation::OtherCrate
    );
}

#[test]
fn foreign_and_absent_ownership_are_distinct() {
    assert_eq!(
        relation(vec![evidence(identity("left", "lib"))], "foreign"),
        RootRelation::Foreign
    );
    assert_eq!(relation(Vec::new(), "foreign"), RootRelation::Unknown);
}

#[test]
fn incomplete_roles_never_invent_ownership_of_other_files() {
    use super::super::declaration::{RoleMap, Roles, exact_role_evidence};
    for complete in [false, true] {
        let (files, actual_complete) = exact_role_evidence(RoleMap {
            roles: BTreeMap::from([
                (
                    PathBuf::from("production.rs"),
                    Roles {
                        production: true,
                        test: false,
                    },
                ),
                (
                    PathBuf::from("test-only.rs"),
                    Roles {
                        production: false,
                        test: true,
                    },
                ),
            ]),
            complete,
        });
        assert_eq!(actual_complete, complete);
        assert_eq!(files, BTreeSet::from([PathBuf::from("production.rs")]));
    }
}
