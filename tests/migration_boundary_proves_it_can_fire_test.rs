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
use anvil::pre_merge_guard::GateStatus;

/// A tree with one `Migrating` module and one `Superseded` module, where the
/// first depends on the second only if `edge` says so.
fn tree(tag: &str, edge: bool) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("anvil-boundary-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).expect("scratch");
    std::fs::write(
        root.join("src/lib.rs"),
        "mod account_pool;\nmod api_contract_guard;\nmod brand_absence;\n",
    )
    .expect("crate root");
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
    std::fs::write(
        root.join("src/api_contract_guard.rs"),
        "pub fn check() {}\n",
    )
    .expect("write");
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

#[test]
fn a_split_module_is_scanned_once_across_both_production_forms_and_not_its_tests() {
    let root = tree("split-module", false);
    let module_dir = root.join("src/brand_absence");
    std::fs::create_dir_all(&module_dir).expect("split module directory");
    std::fs::write(
        root.join("src/brand_absence.rs"),
        "mod tests;\n#[cfg(test)]\nmod fixtures;\npub fn f() {}\n",
    )
    .expect("module declarations");
    std::fs::write(
        module_dir.join("tests.rs"),
        "pub fn shipped() { let _ = crate::account_pool::thing(); }\n",
    )
    .expect("unconditional child");
    std::fs::write(
        module_dir.join("fixtures.rs"),
        "fn fixture() { let _ = crate::api_contract_guard::check(); }\n",
    )
    .expect("cfg-test child");

    let subject = SubjectRoot::asserted(root.clone(), Uncloned::TestFixture);
    let violations =
        anvil::migration::live_tree_violations(&subject).expect("the tree is readable");
    assert_eq!(
        violations.len(),
        1,
        "the unconditional tests.rs ships while the cfg-test fixture does not: {violations:?}"
    );
    assert_eq!(violations[0].from, "brand_absence");
    assert_eq!(violations[0].to, "account_pool/thing");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_tests_only_source_tree_is_not_a_clean_production_corpus() {
    let root = tempfile::tempdir().expect("source root");
    std::fs::create_dir_all(root.path().join("src/tests")).expect("tests directory");
    std::fs::write(
        root.path().join("src/tests/only_fixture.rs"),
        "#[test] fn fixture() {}\n",
    )
    .expect("fixture");
    let subject = SubjectRoot::asserted(root.path().to_path_buf(), Uncloned::TestFixture);

    let reason = anvil::migration::live_tree_violations(&subject)
        .expect_err("no crate root means no production corpus was measured");
    assert!(
        reason.contains("no production Rust modules"),
        "unexpected reason: {reason}"
    );
}

#[test]
fn a_declared_but_missing_source_is_not_a_clean_tree() {
    let root = tempfile::tempdir().expect("source root");
    std::fs::create_dir_all(root.path().join("src")).expect("source directory");
    std::fs::write(root.path().join("src/lib.rs"), "mod brand_absence;\n").expect("crate root");
    let subject = SubjectRoot::asserted(root.path().to_path_buf(), Uncloned::TestFixture);

    let reason = anvil::migration::live_tree_violations(&subject)
        .expect_err("a declaration without a readable source was not measured");
    assert!(reason.contains("no source"), "unexpected reason: {reason}");
}

#[test]
fn invalid_utf8_is_not_measured_by_the_guard() {
    let root = tempfile::tempdir().expect("source root");
    std::fs::create_dir_all(root.path().join("src")).expect("source directory");
    std::fs::write(root.path().join("src/lib.rs"), "mod brand_absence;\n").expect("crate root");
    std::fs::write(root.path().join("src/brand_absence.rs"), [0xff, 0xfe])
        .expect("invalid UTF-8 fixture");
    let subject = SubjectRoot::asserted(root.path().to_path_buf(), Uncloned::TestFixture);

    let status = anvil::pre_merge_guard::migration_boundary_gate_status(&subject);
    let GateStatus::NotMeasured { gate_id, reason } = status else {
        panic!("an unreadable production source must be NotMeasured, got {status:?}");
    };
    assert_eq!(gate_id, "migration_boundary_status");
    assert!(reason.contains("not UTF-8"), "unexpected reason: {reason}");
}

#[test]
fn a_literal_include_is_part_of_the_containing_migration_subject() {
    let root = tree("literal-include", false);
    std::fs::create_dir_all(root.join("src/brand_absence")).expect("include directory");
    std::fs::write(
        root.join("src/brand_absence.rs"),
        "include!(\"brand_absence/generated.rs\");\n",
    )
    .expect("include declaration");
    std::fs::write(
        root.join("src/brand_absence/generated.rs"),
        "pub fn generated() { crate::account_pool::thing(); }\n",
    )
    .expect("included production source");

    let subject = SubjectRoot::asserted(root.clone(), Uncloned::TestFixture);
    let violations = anvil::migration::live_tree_violations(&subject).expect("read include graph");
    assert_eq!(
        violations.len(),
        1,
        "included edge was omitted: {violations:?}"
    );
    assert_eq!(violations[0].from, "brand_absence");
    assert_eq!(violations[0].to, "account_pool/thing");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_literal_include_at_a_crate_root_contributes_declared_subjects() {
    let root = tree("root-include", false);
    std::fs::write(
        root.join("src/lib.rs"),
        "include!(\"root_modules.rs\");\nmod account_pool;\nmod api_contract_guard;\n",
    )
    .expect("crate root include");
    std::fs::write(root.join("src/root_modules.rs"), "mod brand_absence;\n")
        .expect("included module declaration");
    std::fs::write(
        root.join("src/brand_absence.rs"),
        "pub fn forbidden() { crate::account_pool::thing(); }\n",
    )
    .expect("included subject");

    let subject = SubjectRoot::asserted(root.clone(), Uncloned::TestFixture);
    let violations = anvil::migration::live_tree_violations(&subject).expect("scan root include");
    assert_eq!(
        violations.len(),
        1,
        "root include was omitted: {violations:?}"
    );
    assert_eq!(violations[0].from, "brand_absence");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_literal_include_that_is_missing_or_escapes_is_not_measured() {
    let root = tree("bad-includes", false);
    std::fs::write(
        root.join("src/brand_absence.rs"),
        "include!(\"brand_absence/missing.rs\");\n",
    )
    .expect("missing include");
    let subject = SubjectRoot::asserted(root.clone(), Uncloned::TestFixture);
    let missing = anvil::migration::live_tree_violations(&subject)
        .expect_err("missing source include is absent evidence");
    assert!(
        missing.contains("cannot resolve included source"),
        "{missing}"
    );

    let outside = tempfile::NamedTempFile::new().expect("outside source");
    std::fs::write(outside.path(), "pub fn outside() {}\n").expect("outside source bytes");
    std::fs::write(
        root.join("src/brand_absence.rs"),
        format!("include!({:?});\n", outside.path().to_string_lossy()),
    )
    .expect("escaping include");
    let escaped = anvil::migration::live_tree_violations(&subject)
        .expect_err("source include outside the repository is absent evidence");
    assert!(escaped.contains("escapes repository"), "{escaped}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn crate_paths_inside_reachable_macro_bodies_keep_two_segment_ledger_identity() {
    let root = tree("macro-edge", false);
    std::fs::write(
        root.join("src/brand_absence.rs"),
        r#"
            macro_rules! invoke { () => { crate::account_pool::thing() } }
            pub fn forbidden() { invoke!(); }
            const PROSE: &str = "crate::api_contract_guard::check";
        "#,
    )
    .expect("macro edge");
    let subject = SubjectRoot::asserted(root.clone(), Uncloned::TestFixture);
    let violations = anvil::migration::live_tree_violations(&subject).expect("scan macro tokens");
    assert_eq!(
        violations.len(),
        1,
        "macro edge census drifted: {violations:?}"
    );
    assert_eq!(violations[0].to, "account_pool/thing");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_dynamic_source_include_is_not_certified_as_clean() {
    let root = tree("dynamic-include", false);
    std::fs::write(
        root.join("src/brand_absence.rs"),
        "include!(concat!(\"brand_absence/\", \"generated.rs\"));\n",
    )
    .expect("dynamic include declaration");
    let subject = SubjectRoot::asserted(root.clone(), Uncloned::TestFixture);
    let reason = anvil::migration::live_tree_violations(&subject)
        .expect_err("an unresolved source include is absent evidence");
    assert!(
        reason.contains("dynamic source include"),
        "unexpected: {reason}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn an_ordinary_cargo_binary_root_cannot_hide_a_known_subject() {
    let root = tempfile::tempdir().expect("source root");
    std::fs::create_dir_all(root.path().join("src/bin")).expect("binary directory");
    std::fs::write(
        root.path().join("src/bin/tool.rs"),
        r#"
            mod account_pool { pub fn thing() {} }
            mod brand_absence {
                pub fn forbidden() { crate::account_pool::thing(); }
            }
            fn main() { brand_absence::forbidden(); }
        "#,
    )
    .expect("rustc-valid binary root");
    let subject = SubjectRoot::asserted(root.path().to_path_buf(), Uncloned::TestFixture);
    let violations = anvil::migration::live_tree_violations(&subject).expect("scan binary root");
    assert_eq!(
        violations.len(),
        1,
        "binary edge was omitted: {violations:?}"
    );
    assert_eq!(violations[0].from, "brand_absence");
    assert_eq!(violations[0].to, "account_pool/thing");
}
