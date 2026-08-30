//! The migration boundary gate demonstrates both halves.
//!
//! The rule: a `Migrating` component must not depend on a `Superseded` one.
//! Migrating code anchored to code that is going away has to be rewritten twice,
//! and the second rewrite is the one nobody schedules. Depending on `Rewired` is
//! allowed — its port survives absorption and only the adapter behind it is
//! swapped.
//!
//! Both fixtures build a real source tree and run the gate over it, rather than
//! calling `check_edge` directly. The rule and the tree walk are different
//! things, and a proof of the first says nothing about the second: the walk
//! strips comments, resolves `crate::a::b` to a two-segment name, and dedupes
//! before it asks the rule anything.

use anvil::git_manager::{SubjectRoot, Uncloned};

/// A tree with one `Migrating` module and one `Superseded` module, where the
/// first depends on the second only if `edge` says so.
fn tree(tag: &str, edge: bool) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("anvil-boundary-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).expect("scratch");
    // `brand_absence` is Migrating; `account_pool` is Superseded. Both names are
    // read from the registry, so this fixture follows the ledger rather than
    // asserting a classification of its own.
    let body = if edge {
        "pub fn f() { let _ = crate::account_pool::thing(); }\n"
    } else {
        "pub fn f() {}\n"
    };
    std::fs::write(root.join("src/brand_absence.rs"), body).expect("write");
    std::fs::write(root.join("src/account_pool.rs"), "pub fn thing() {}\n").expect("write");
    root
}

#[test]
fn migration_boundary_fires_when_migrating_code_depends_on_superseded_code() {
    let root = tree("red", true);
    let subject = SubjectRoot::asserted(root.clone(), Uncloned::TestFixture);
    let violations =
        anvil::migration::live_tree_violations(&subject).expect("the tree is readable");
    assert!(
        !violations.is_empty(),
        "a Migrating module imports a Superseded one and the gate saw nothing. \
         That edge anchors migrating code to code that is going away."
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn migration_boundary_spares_the_same_tree_without_the_edge() {
    let root = tree("green", false);
    let subject = SubjectRoot::asserted(root.clone(), Uncloned::TestFixture);
    let violations =
        anvil::migration::live_tree_violations(&subject).expect("the tree is readable");
    assert!(
        violations.is_empty(),
        "the same two modules with no dependency between them is the conformant \
         case; flagging it accuses a tree that carries no forbidden edge: {violations:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// And a comment cannot fabricate an edge, which the walk strips on purpose.
#[test]
fn a_forbidden_edge_written_in_a_comment_is_not_an_edge() {
    let root = tree("comment", false);
    std::fs::write(
        root.join("src/brand_absence.rs"),
        "// once called crate::account_pool::thing()\npub fn f() {}\n",
    )
    .expect("write");
    let subject = SubjectRoot::asserted(root.clone(), Uncloned::TestFixture);
    let violations =
        anvil::migration::live_tree_violations(&subject).expect("the tree is readable");
    assert!(
        violations.is_empty(),
        "prose describing a dependency is not a dependency: {violations:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}
