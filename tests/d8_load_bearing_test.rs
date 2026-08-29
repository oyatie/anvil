//! D-8 admits a directory only if something loads it.
//!
//! The rule this covers is a predicate, not a denylist, and the assertion that
//! matters most is the negative one: prose about a directory must never keep it
//! alive. A first census of the repository this was written for counted 247
//! files "referencing" `specs/` by naive string match, and the only thing
//! referring to `IPs/` outside `IPs/` was a planning document describing the
//! plan to create them. Counting either as a load would have kept every doomed
//! tree alive on the strength of writing about deleting it.

use anvil::shape::adapters::in_memory_tree::InMemoryTree;
use anvil::shape::core::load_bearing::{LoadIndex, Standing};

fn dirs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn standing(tree: &InMemoryTree, dir: &str) -> Standing {
    let cands = dirs(&[dir]);
    LoadIndex::build(tree, &cands).standing(tree, dir)
}

#[test]
fn a_cargo_path_dependency_admits_the_directory() {
    let tree = InMemoryTree::from_paths("r", &["storage/core/blob/lib.rs", "Cargo.toml"])
        .with_file(
            "Cargo.toml",
            "[dependencies]\nblob = { path = \"storage/core/blob\" }\n",
        );
    let s = standing(&tree, "storage/core/blob/");
    assert!(matches!(s, Standing::Built { .. }), "got {s:?}");
    assert!(s.admits());
}

#[test]
fn a_buck_target_admits_the_directory() {
    let tree = InMemoryTree::from_paths("r", &["contracts/openapi/x.yaml", "gateway/BUCK"])
        .with_file(
            "gateway/BUCK",
            "export_file(src = \"contracts/openapi/x.yaml\")\n",
        );
    assert!(matches!(
        standing(&tree, "contracts/"),
        Standing::Built { .. }
    ));
}

#[test]
fn a_path_literal_in_rust_admits_the_directory() {
    let tree = InMemoryTree::from_paths("r", &["contracts/x.yaml", "gateway/src/load.rs"])
        .with_file(
            "gateway/src/load.rs",
            "fn load() { let _ = include_str!(\"contracts/x.yaml\"); }",
        );
    let s = standing(&tree, "contracts/");
    assert!(matches!(s, Standing::Loaded { .. }), "got {s:?}");
}

#[test]
fn a_path_named_only_in_a_rust_comment_does_not_admit_it() {
    let tree = InMemoryTree::from_paths("r", &["IPs/note.md", "gateway/src/load.rs"]).with_file(
        "gateway/src/load.rs",
        "// TODO: the old IPs/ drawer should be deleted\nfn load() {}",
    );
    let s = standing(&tree, "IPs/");
    assert!(
        !s.admits(),
        "a comment ABOUT deleting a directory must not keep it alive. Got {s:?}"
    );
}

#[test]
fn a_path_named_only_in_markdown_is_mentioned_not_loaded() {
    let tree = InMemoryTree::from_paths("r", &["IPs/note.md", "docs/plan.md"]).with_file(
        "docs/plan.md",
        "destination: owning capability IPs/ + beads\n",
    );
    let s = standing(&tree, "IPs/");
    assert_eq!(s, Standing::MentionedOnly { mentions: 1 });
    assert!(!s.admits());
}

#[test]
fn mentioned_only_is_distinct_from_orphan() {
    // A reader deciding whether to delete needs to know something still talks
    // about it -- collapsing these two would hide that.
    let tree = InMemoryTree::from_paths("r", &["IPs/note.md"]);
    assert_eq!(standing(&tree, "IPs/"), Standing::Orphan);
}

#[test]
fn owner_law_files_admit_a_directory_nothing_loads() {
    let tree = InMemoryTree::from_paths("r", &["billing/OWNERS", "billing/PLAN.md"]);
    let s = standing(&tree, "billing/");
    assert_eq!(s, Standing::OwnerLaw);
    assert!(s.admits());
}

#[test]
fn one_non_owner_law_file_removes_the_owner_law_standing() {
    let tree = InMemoryTree::from_paths("r", &["billing/OWNERS", "billing/scratch.md"]);
    assert!(!standing(&tree, "billing/").admits());
}

#[test]
fn a_directory_does_not_load_itself() {
    // Every file under IPs/ mentions IPs/. Counting those would make every
    // directory self-admitting, which is the failure mode that turns a
    // predicate back into a no-op.
    let tree = InMemoryTree::from_paths("r", &["IPs/a.md", "IPs/b.md"])
        .with_file("IPs/a.md", "see IPs/b.md")
        .with_file("IPs/b.md", "see IPs/a.md");
    assert_eq!(standing(&tree, "IPs/"), Standing::Orphan);
}

#[test]
fn the_trailing_slash_stops_a_prefix_collision() {
    let tree = InMemoryTree::from_paths("r", &["IPs/note.md", "gateway/src/x.rs"])
        .with_file("gateway/src/x.rs", "let p = \"IPsomething/file.yaml\";");
    assert!(
        !standing(&tree, "IPs/").admits(),
        "`IPsomething/` is a different directory"
    );
}

#[test]
fn every_standing_states_a_reason_a_person_can_act_on() {
    for s in [
        Standing::Built { edges: 2 },
        Standing::Loaded { sites: 1 },
        Standing::OwnerLaw,
        Standing::MentionedOnly { mentions: 3 },
        Standing::Orphan,
    ] {
        assert!(!s.reason().is_empty(), "{s:?} must explain itself");
    }
}

#[test]
fn a_directory_containing_a_crate_is_load_bearing_even_if_nothing_names_it() {
    // Workspace members are closed globs (D-39), so no file spells
    // `billing/facade/` literally, and the BUCK label that does spell it lives
    // inside the directory -- where the self-reference guard skips it. The
    // first run of this projection refused `billing/facade/` for exactly this
    // reason: a face full of real crates, condemned by the rule meant to
    // protect it.
    let tree =
        InMemoryTree::from_paths("r", &["billing/facade/invoicing/Cargo.toml", "Cargo.toml"])
            .with_file("Cargo.toml", "[workspace]\nmembers = [\"*/*/*\"]\n");
    let s = standing(&tree, "billing/facade/");
    assert!(
        s.admits(),
        "a directory holding a buildable crate is loaded by the glob that \
         reaches it. Got {s:?}"
    );
}

#[test]
fn containment_does_not_admit_a_directory_of_prose() {
    let tree = InMemoryTree::from_paths("r", &["billing/IPs/note.md", "billing/IPs/other.md"]);
    assert!(
        !standing(&tree, "billing/IPs/").admits(),
        "markdown is not a build target"
    );
}
