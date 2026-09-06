//! Adversarial spellings for the live migration dependency graph.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn tree(tag: &str, body: &str) -> tempfile::TempDir {
    let root = tempfile::Builder::new()
        .prefix(&format!("anvil-migration-spelling-{tag}-"))
        .tempdir()
        .expect("fixture root");
    let src = root.path().join("src");
    fs::create_dir_all(&src).expect("source directory");
    fs::write(
        src.join("lib.rs"),
        "mod account_pool;\nmod api_contract_guard;\nmod brand_absence;\n",
    )
    .expect("crate root");
    fs::write(src.join("brand_absence.rs"), body).expect("migrating source");
    fs::write(src.join("account_pool.rs"), "pub fn thing() {}\n").expect("dependency source");
    fs::write(src.join("api_contract_guard.rs"), "pub fn check() {}\n").expect("allowed source");
    root
}

fn brand_dependencies(root: &Path) -> BTreeSet<String> {
    anvil::source_scan::paths::production_module_dependencies(root)
        .expect("measure dependency graph")
        .remove("brand_absence")
        .expect("migrating subject")
}

#[test]
fn direct_grouped_renamed_and_glob_use_trees_are_edges_once() {
    for (tag, body, expected) in [
        (
            "direct",
            "use crate::account_pool::thing; pub fn f() { thing(); }\n",
            "account_pool/thing",
        ),
        (
            "grouped",
            "use crate::account_pool::{thing}; pub fn f() { thing(); }\n",
            "account_pool/thing",
        ),
        (
            "renamed",
            "use crate::account_pool::{thing as invoke}; pub fn f() { invoke(); }\n",
            "account_pool/thing",
        ),
        (
            "glob",
            "use crate::account_pool::*; pub fn f() { thing(); }\n",
            "account_pool",
        ),
    ] {
        let root = tree(tag, body);
        assert_eq!(
            brand_dependencies(root.path()),
            BTreeSet::from([expected.to_owned()]),
            "UseTree spelling {tag} was omitted or counted twice"
        );
    }
}

#[test]
fn block_local_imports_resolve_without_leaking_across_sibling_scopes() {
    let root = tree(
        "local-imports",
        r#"
            pub fn migrating() {
                use crate::account_pool::thing as invoke;
                invoke();
            }
            pub fn allowed() {
                use crate::api_contract_guard::check as invoke;
                invoke();
            }
        "#,
    );
    assert_eq!(
        brand_dependencies(root.path()),
        BTreeSet::from([
            "account_pool/thing".to_owned(),
            "api_contract_guard/check".to_owned(),
        ]),
        "block imports must resolve in their own lexical scope"
    );
}

#[test]
fn relative_and_crate_alias_paths_resolve_logically() {
    let relative = tree("super", "pub fn f() { super::account_pool::thing(); }\n");
    assert_eq!(
        brand_dependencies(relative.path()),
        BTreeSet::from(["account_pool/thing".to_owned()])
    );

    let aliased = tree(
        "crate-alias",
        r#"
            pub fn f() { root::account_pool::thing(); }
            use crate as root;
        "#,
    );
    assert_eq!(
        brand_dependencies(aliased.path()),
        BTreeSet::from(["account_pool/thing".to_owned()]),
        "a crate alias must resolve independent of declaration order"
    );
}

#[test]
fn namespaced_builtin_includes_are_scanned_with_literal_policy() {
    for (tag, spelling) in [("std", "std::include!"), ("core", "::core::include!")] {
        let root = tree(
            tag,
            &format!("{spelling}(\"brand_absence/generated.rs\");\n"),
        );
        let generated = root.path().join("src/brand_absence/generated.rs");
        fs::create_dir_all(generated.parent().unwrap()).expect("include directory");
        fs::write(
            generated,
            "pub fn generated() { crate::account_pool::thing(); }\n",
        )
        .expect("included source");
        assert_eq!(
            brand_dependencies(root.path()),
            BTreeSet::from(["account_pool/thing".to_owned()]),
            "{spelling} source was omitted"
        );
    }
}

#[test]
fn namespaced_dynamic_or_missing_includes_fail_closed() {
    let dynamic = tree(
        "dynamic-std",
        "std::include!(concat!(\"brand_absence/\", \"generated.rs\"));\n",
    );
    let reason = anvil::source_scan::paths::production_module_dependencies(dynamic.path())
        .expect_err("dynamic std::include must not certify absence");
    assert!(reason.contains("dynamic source include"), "{reason}");

    let missing = tree("missing-core", "core::include!(\"missing.rs\");\n");
    let reason = anvil::source_scan::paths::production_module_dependencies(missing.path())
        .expect_err("missing core::include must not certify absence");
    assert!(
        reason.contains("cannot resolve included source"),
        "{reason}"
    );
}

#[test]
fn cfg_predicates_are_classified_by_non_test_satisfiability() {
    let root = tree(
        "cfg-logic",
        r#"
            #[cfg(all(test, unix))]
            pub fn fixture() { crate::account_pool::thing(); }
            #[cfg(any(test, unix))]
            pub fn potentially_ships() { crate::api_contract_guard::check(); }
        "#,
    );
    assert_eq!(
        brand_dependencies(root.path()),
        BTreeSet::from(["api_contract_guard/check".to_owned()]),
        "all(test, ..) must be excluded while any(test, ..) remains production"
    );
}

#[test]
fn crate_reexports_and_extern_self_aliases_resolve_across_module_scopes() {
    for (tag, root_alias) in [
        ("reexport", "pub(crate) use crate::account_pool as pool;"),
        ("extern-self", "extern crate self as pool;"),
    ] {
        let root = tree(tag, "pub fn f() { crate::pool::account_pool::thing(); }\n");
        let call = if tag == "reexport" {
            "pub fn f() { crate::pool::thing(); }\n"
        } else {
            "pub fn f() { crate::pool::account_pool::thing(); }\n"
        };
        fs::write(
            root.path().join("src/lib.rs"),
            format!(
                "mod account_pool;\nmod api_contract_guard;\n{root_alias}\nmod brand_absence;\n"
            ),
        )
        .expect("crate alias root");
        fs::write(root.path().join("src/brand_absence.rs"), call).expect("aliased caller");
        assert_eq!(
            brand_dependencies(root.path()),
            BTreeSet::from(["account_pool/thing".to_owned()]),
            "scope-qualified alias {tag} was omitted"
        );
    }
}

#[test]
fn nested_reexport_aliases_resolve_from_crate_qualified_paths() {
    let root = tree(
        "nested-reexport",
        "pub fn f() { crate::aliases::pool::thing(); }\n",
    );
    fs::write(
        root.path().join("src/lib.rs"),
        r#"
            mod account_pool;
            mod api_contract_guard;
            mod aliases { pub(crate) use crate::account_pool as pool; }
            mod brand_absence;
        "#,
    )
    .expect("nested alias root");
    assert_eq!(
        brand_dependencies(root.path()),
        BTreeSet::from(["account_pool/thing".to_owned()])
    );
}

#[test]
fn local_globs_reexport_parent_symbols_without_hiding_edges() {
    let root = tree(
        "parent-glob",
        r#"
            pub(crate) use crate::account_pool as pool;
            mod child {
                use super::*;
                pub fn f() { pool::thing(); }
            }
        "#,
    );
    assert_eq!(
        brand_dependencies(root.path()),
        BTreeSet::from(["account_pool".to_owned()])
    );
}

#[test]
fn cfg_test_subtrees_below_items_do_not_create_production_edges() {
    let root = tree(
        "deep-cfg",
        r#"
            struct Runner;
            impl Runner {
                #[cfg(test)] fn fixture() { crate::account_pool::thing(); }
                fn ships() { crate::api_contract_guard::check(); }
            }
            trait Behavior {
                #[cfg(test)] fn fixture() { crate::account_pool::thing(); }
            }
            struct Fields {
                #[cfg(test)] fixture: crate::account_pool::Fixture,
            }
            enum Variants {
                #[cfg(test)] Fixture(crate::account_pool::Fixture),
                Shipping,
            }
            #[test]
            fn harness_only() { crate::account_pool::thing(); }
            pub fn body(value: bool) {
                #[cfg(test)]
                let _fixture = crate::account_pool::thing();
                #[cfg(test)]
                { crate::account_pool::thing(); }
                match value {
                    #[cfg(test)] true => crate::account_pool::thing(),
                    false => (),
                }
            }
        "#,
    );
    assert_eq!(
        brand_dependencies(root.path()),
        BTreeSet::from(["api_contract_guard/check".to_owned()])
    );
}

#[test]
fn renamed_include_inherits_caller_aliases_and_module_lookup_context() {
    let root = tree(
        "renamed-include",
        r#"
            extern crate std as platform;
            use platform::include as load;
            use crate::account_pool as pool;
            load!("generated/part.rs");
        "#,
    );
    fs::create_dir_all(root.path().join("src/generated")).expect("include directory");
    fs::write(
        root.path().join("src/generated/part.rs"),
        "pub fn generated() { pool::thing(); }\nmod nested;\n",
    )
    .expect("included source");
    fs::write(
        root.path().join("src/generated/nested.rs"),
        "pub fn nested() { crate::account_pool::thing(); }\n",
    )
    .expect("physical include-relative module");
    fs::create_dir_all(root.path().join("src/brand_absence")).expect("decoy directory");
    fs::write(
        root.path().join("src/brand_absence/nested.rs"),
        "pub fn nested() { crate::api_contract_guard::check(); }\n",
    )
    .expect("caller-logical decoy module");
    assert_eq!(
        brand_dependencies(root.path()),
        BTreeSet::from(["account_pool".to_owned(), "account_pool/thing".to_owned(),])
    );
}

#[test]
fn macro_generated_crate_paths_fail_closed_when_the_segment_is_unknown() {
    let root = tree(
        "macro-metavariable",
        r#"
            macro_rules! invoke { ($module:ident) => { crate::$module::thing() } }
            pub fn f() { invoke!(account_pool); }
        "#,
    );
    let reason = anvil::source_scan::paths::production_module_dependencies(root.path())
        .expect_err("a macro-generated dependency cannot certify a clean graph");
    assert!(reason.contains("metavariable"), "{reason}");
}

#[test]
fn an_unused_private_macro_body_is_not_an_executed_dependency() {
    let root = tree(
        "unused-macro",
        r#"
            macro_rules! unused { () => { crate::account_pool::thing() } }
            pub fn ships() { crate::api_contract_guard::check(); }
        "#,
    );
    assert_eq!(
        brand_dependencies(root.path()),
        BTreeSet::from(["api_contract_guard/check".to_owned()])
    );
}

#[test]
fn an_include_alias_with_unmeasurable_shadowing_fails_closed() {
    let root = tree(
        "shadowed-include",
        r#"
            use std::include as load;
            macro_rules! load { ($path:literal) => {} }
            load!("not-source.rs");
        "#,
    );
    let reason = anvil::source_scan::paths::production_module_dependencies(root.path())
        .expect_err("ambiguous include provenance cannot certify absence");
    assert!(reason.contains("include provenance"), "{reason}");

    let block = tree(
        "block-shadowed-include",
        r#"
            pub fn f() {
                use std::include as load;
                macro_rules! load { ($path:literal) => {} }
                load!("not-source.rs");
            }
        "#,
    );
    let reason = anvil::source_scan::paths::production_module_dependencies(block.path())
        .expect_err("a block-local macro can shadow an include alias");
    assert!(reason.contains("include provenance"), "{reason}");

    let ancestor = tree(
        "ancestor-shadowed-include",
        "include!(\"not-source.rs\");\n",
    );
    fs::write(
        ancestor.path().join("src/lib.rs"),
        "macro_rules! include { ($path:literal) => {} }\nmod account_pool;\nmod api_contract_guard;\nmod brand_absence;\n",
    )
    .expect("ancestor macro");
    let reason = anvil::source_scan::paths::production_module_dependencies(ancestor.path())
        .expect_err("an ancestor macro can shadow the prelude include");
    assert!(reason.contains("include provenance"), "{reason}");

    let namespace = tree(
        "namespace-shadowed-include",
        "mod std {}\nstd::include!(\"not-source.rs\");\n",
    );
    let reason = anvil::source_scan::paths::production_module_dependencies(namespace.path())
        .expect_err("a local std namespace can shadow std::include");
    assert!(reason.contains("include provenance"), "{reason}");
}

#[test]
fn ordinary_macro_metavariables_cannot_synthesize_unmeasured_source() {
    for (tag, body) in [
        (
            "forward-item",
            "macro_rules! inject { ($item:item) => { $item } }\ninject!(mod hidden;);\n",
        ),
        (
            "forward-expression",
            "macro_rules! inject { ($expr:expr) => { $expr } }\npub fn f(){ let _: () = inject!(include!(\"hidden.inc\")); }\n",
        ),
    ] {
        let root = tree(tag, body);
        fs::write(
            root.path().join("src/brand_absence/hidden.rs"),
            "pub fn f(){ crate::account_pool::thing(); }\n",
        )
        .ok();
        fs::write(
            root.path().join("src/hidden.inc"),
            "{ crate::account_pool::thing(); }\n",
        )
        .expect("include body");
        let reason = anvil::source_scan::paths::production_module_dependencies(root.path())
            .expect_err("forwarded syntax must withhold an exact dependency claim");
        assert!(reason.contains("metavariable"), "{tag}: {reason}");
    }
}

#[test]
fn a_shared_physical_module_is_scanned_in_each_crate_symbol_universe() {
    let root = tempfile::tempdir().expect("workspace");
    fs::create_dir_all(root.path().join("crates/a/src")).unwrap();
    fs::create_dir_all(root.path().join("crates/b/src")).unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers=['crates/a','crates/b']\nresolver='3'\n",
    )
    .unwrap();
    for package in ["a", "b"] {
        fs::write(
            root.path().join(format!("crates/{package}/Cargo.toml")),
            format!("[package]\nname='{package}'\nversion='0.0.0'\nedition='2024'\n"),
        )
        .unwrap();
    }
    fs::write(
        root.path().join("shared.rs"),
        "pub fn shared(){ crate::Alias::thing(); }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("crates/a/src/lib.rs"),
        "mod safe { pub fn thing(){} }\npub use crate::safe as Alias;\n#[path=\"../../../shared.rs\"] mod shared;\n",
    )
    .unwrap();
    fs::write(
        root.path().join("crates/b/src/lib.rs"),
        "mod account_pool { pub fn thing(){} }\npub use crate::account_pool as Alias;\n#[path=\"../../../shared.rs\"] mod shared;\n",
    )
    .unwrap();

    let edges = anvil::source_scan::paths::production_module_dependencies(root.path())
        .expect("both crate roots are measured");
    assert!(
        edges
            .get("shared")
            .is_some_and(|deps| deps.contains("account_pool/thing")),
        "second crate alias universe was skipped: {edges:?}"
    );
}

#[test]
fn macro_use_hoists_a_child_macro_into_the_parent_textual_scope() {
    let root = tree("macro-use", "call_bad!();\n");
    fs::write(
        root.path().join("src/lib.rs"),
        "mod account_pool;\nmod api_contract_guard;\n#[macro_use] mod macros;\nmod brand_absence;\n",
    )
    .unwrap();
    fs::write(
        root.path().join("src/macros.rs"),
        "macro_rules! call_bad { () => { pub fn generated(){ crate::account_pool::thing(); } } }\n",
    )
    .unwrap();
    assert_eq!(
        brand_dependencies(root.path()),
        BTreeSet::from(["account_pool/thing".to_owned()])
    );
}

#[test]
fn expression_position_include_is_parsed_as_an_expression() {
    let root = tree(
        "expression-include",
        "pub fn value() -> usize { include!(\"value.inc\") }\n",
    );
    fs::write(
        root.path().join("src/value.inc"),
        "{ crate::account_pool::thing(); 3usize }\n",
    )
    .expect("included expression");
    assert_eq!(
        brand_dependencies(root.path()),
        BTreeSet::from(["account_pool/thing".to_owned()])
    );
}

#[test]
fn nested_expression_includes_inherit_block_local_aliases() {
    let root = tree(
        "block-include-context",
        r#"
            pub fn value() -> usize {
                use crate::account_pool as chosen;
                use std::include as load;
                macro_rules! touch { () => { crate::api_contract_guard::check() } }
                load!("outer.inc")
            }
        "#,
    );
    fs::write(root.path().join("src/outer.inc"), "load!(\"inner.inc\")\n")
        .expect("outer included expression");
    fs::write(
        root.path().join("src/inner.inc"),
        "{ chosen::thing(); touch!(); 3usize }\n",
    )
    .expect("nested included expression");
    assert_eq!(
        brand_dependencies(root.path()),
        BTreeSet::from([
            "account_pool".to_owned(),
            "account_pool/thing".to_owned(),
            "api_contract_guard/check".to_owned(),
        ]),
        "textual include expansion lost the caller's lexical aliases or macros"
    );
}

#[test]
fn function_local_inline_modules_are_part_of_the_dependency_graph() {
    let root = tree(
        "function-inline-module",
        r#"
            pub fn f() {
                mod local {
                    pub fn thing() { crate::account_pool::thing(); }
                }
                local::thing();
            }
        "#,
    );
    assert!(
        brand_dependencies(root.path())
            .iter()
            .any(|dependency| dependency.starts_with("account_pool")),
        "a module declared in a function was omitted"
    );
}

#[test]
fn path_modules_declared_in_blocks_are_part_of_the_dependency_graph() {
    let root = tree(
        "block-path-module",
        r#"
            pub fn f() {
                #[path = "block_child.rs"]
                mod child;
                child::thing();
            }
        "#,
    );
    fs::write(
        root.path().join("src/block_child.rs"),
        "pub fn thing() { crate::account_pool::thing(); }\n",
    )
    .expect("block module");
    assert!(
        brand_dependencies(root.path())
            .iter()
            .any(|dependency| dependency.starts_with("account_pool")),
        "a #[path] module declared in a block was omitted"
    );
}

#[test]
fn block_local_extern_crate_self_aliases_keep_dependency_provenance() {
    let root = tree(
        "block-extern-self",
        "pub fn f() { extern crate self as app; app::account_pool::thing(); }\n",
    );
    assert!(
        brand_dependencies(root.path())
            .iter()
            .any(|dependency| dependency.starts_with("account_pool")),
        "a block-local extern-crate alias hid the dependency"
    );
}

#[test]
fn reachable_macro_generated_includes_are_traversed() {
    let root = tree(
        "macro-include",
        r#"
            macro_rules! generated { () => { include!("hidden.inc"); } }
            pub fn f() { generated!(); }
        "#,
    );
    fs::write(
        root.path().join("src/hidden.inc"),
        "crate::account_pool::thing()\n",
    )
    .expect("included expression");
    assert!(
        brand_dependencies(root.path())
            .iter()
            .any(|dependency| dependency.starts_with("account_pool")),
        "a reachable macro-generated include was omitted"
    );
}

#[test]
fn transitively_reachable_macro_generated_includes_are_traversed() {
    let root = tree(
        "nested-macro-include",
        r#"
            macro_rules! inner { () => { include!("hidden.inc"); } }
            macro_rules! outer { () => { inner!(); } }
            pub fn f() { outer!(); }
        "#,
    );
    fs::write(
        root.path().join("src/hidden.inc"),
        "crate::account_pool::thing()\n",
    )
    .expect("included expression");
    assert!(
        brand_dependencies(root.path())
            .iter()
            .any(|dependency| dependency.starts_with("account_pool")),
        "a transitively reachable macro-generated include was omitted"
    );
}

#[test]
fn macro_export_definitions_are_visible_at_the_crate_root() {
    let root = tree("macro-export-root", "pub fn f() { crate::call_bad!(); }\n");
    fs::write(
        root.path().join("src/lib.rs"),
        "mod account_pool; mod macro_owner; mod brand_absence;\n",
    )
    .expect("crate root");
    fs::write(
        root.path().join("src/macro_owner.rs"),
        "#[macro_export] macro_rules! call_bad { () => { crate::account_pool::thing(); } }\n",
    )
    .expect("exported macro");
    assert!(
        brand_dependencies(root.path())
            .iter()
            .any(|dependency| dependency.starts_with("account_pool")),
        "a nested #[macro_export] definition was not indexed at crate root"
    );
}

#[test]
fn token_forwarding_macro_cannot_hide_a_builtin_include() {
    let root = tree(
        "forwarded-include",
        r#"
            macro_rules! load { ($m:ident) => { $m!("hidden.inc"); } }
            pub fn f() { load!(include); }
        "#,
    );
    fs::write(
        root.path().join("src/hidden.inc"),
        "crate::account_pool::thing()\n",
    )
    .expect("included expression");
    let measured = anvil::source_scan::paths::production_module_dependencies(root.path());
    assert!(
        measured
            .as_ref()
            .is_err_and(|reason| reason.contains("macro"))
            || measured
                .as_ref()
                .ok()
                .and_then(|graph| graph.get("brand_absence"))
                .is_some_and(|dependencies| dependencies
                    .iter()
                    .any(|dependency| dependency.starts_with("account_pool"))),
        "forwarding include as a metavariable produced a false-clean graph: {measured:?}"
    );
}

#[test]
fn repeated_token_forwarding_cannot_hide_a_path_module() {
    let root = tree(
        "forwarded-module",
        r#"
            macro_rules! emit { ($($t:tt)*) => { $($t)* } }
            emit!(#[path = "hidden.rs"] mod hidden;);
            pub fn f() { hidden::thing(); }
        "#,
    );
    fs::write(
        root.path().join("src/hidden.rs"),
        "pub fn thing() { crate::account_pool::thing(); }\n",
    )
    .expect("forwarded module");
    let measured = anvil::source_scan::paths::production_module_dependencies(root.path());
    assert!(
        measured
            .as_ref()
            .is_err_and(|reason| reason.contains("macro"))
            || measured
                .as_ref()
                .ok()
                .and_then(|graph| graph.get("brand_absence"))
                .is_some_and(|dependencies| dependencies
                    .iter()
                    .any(|dependency| dependency.starts_with("account_pool"))),
        "forwarded module tokens produced a false-clean graph: {measured:?}"
    );
}

#[test]
fn included_path_attributes_use_the_physical_include_directory() {
    let root = tree("include-path-context", "include!(\"generated/part.rs\");\n");
    fs::create_dir_all(root.path().join("src/generated")).expect("included source directory");
    fs::write(
        root.path().join("src/generated/part.rs"),
        "#[path = \"child.rs\"] mod child; pub fn f() { child::thing(); }\n",
    )
    .expect("included items");
    fs::write(
        root.path().join("src/generated/child.rs"),
        "pub fn thing() { crate::account_pool::thing(); }\n",
    )
    .expect("real path module");
    fs::write(root.path().join("src/child.rs"), "pub fn thing() {}\n")
        .expect("caller-directory decoy");
    assert!(
        brand_dependencies(root.path())
            .iter()
            .any(|dependency| dependency.starts_with("account_pool")),
        "an included #[path] followed the caller decoy"
    );
}

#[test]
fn root_item_includes_resolve_conventional_modules_from_the_included_directory() {
    let root = tree("root-include-context", "");
    fs::write(
        root.path().join("src/lib.rs"),
        "mod account_pool; mod api_contract_guard; include!(\"generated/part.rs\");\n",
    )
    .expect("crate root");
    fs::create_dir_all(root.path().join("src/generated")).expect("included source directory");
    fs::write(
        root.path().join("src/generated/part.rs"),
        "mod brand_absence;\n",
    )
    .expect("included items");
    fs::write(
        root.path().join("src/generated/brand_absence.rs"),
        "pub fn thing() { crate::account_pool::thing(); }\n",
    )
    .expect("real conventional module");
    fs::write(
        root.path().join("src/brand_absence.rs"),
        "pub fn harmless_decoy() {}\n",
    )
    .expect("caller-directory decoy");

    assert_eq!(
        anvil::source_scan::paths::production_module_dependencies(root.path())
            .expect("dependency graph")["brand_absence"],
        BTreeSet::from(["account_pool/thing".to_owned()]),
        "a root include used its caller's module directory instead of its physical directory"
    );
}

#[test]
fn mutually_exclusive_recursive_includes_fail_closed_without_recursing_forever() {
    let root = tree("include-cycle", "#[cfg(unix)] include!(\"cycle.inc\");\n");
    fs::write(
        root.path().join("src/cycle.inc"),
        r#"
            macro_rules! local { () => {} }
            #[cfg(windows)] include!("cycle.inc");
        "#,
    )
    .expect("cyclic included source");
    let error = anvil::source_scan::paths::production_module_dependencies(root.path())
        .expect_err("a conservatively followed include cycle must fail closed");
    assert!(error.contains("include cycle"), "{error}");
}

#[test]
fn mutually_exclusive_path_module_cycles_fail_closed_by_physical_file() {
    let root = tree(
        "path-module-cycle",
        "#[cfg(unix)] #[path = \"a.rs\"] mod a;\n",
    );
    fs::write(
        root.path().join("src/a.rs"),
        "#[cfg(windows)] #[path = \"b.rs\"] mod b;\n",
    )
    .expect("first cycle source");
    fs::write(
        root.path().join("src/b.rs"),
        "#[cfg(unix)] #[path = \"a.rs\"] mod a;\n",
    )
    .expect("second cycle source");
    let started = std::time::Instant::now();
    let error = anvil::source_scan::paths::production_module_dependencies(root.path())
        .expect_err("a cfg-conservative module cycle must fail closed");
    assert!(error.contains("source cycle"), "{error}");
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn cfg_local_shadow_does_not_erase_outer_crate_alias_in_other_configuration() {
    let root = tree(
        "cfg-shadowed-outer-alias",
        r#"
            extern crate self as app;
            pub fn f() {
                #[cfg(windows)]
                extern crate missing as app;
                app::account_pool::thing();
            }
        "#,
    );
    assert!(
        brand_dependencies(root.path())
            .iter()
            .any(|dependency| dependency.starts_with("account_pool")),
        "a cfg-inactive local shadow erased outer crate-alias provenance"
    );
}

#[test]
fn extern_prelude_names_can_be_shadowed_by_measured_local_modules() {
    let root = tree(
        "local-tokio",
        "use crate::tokio; pub fn f() { tokio::marker(); }\n",
    );
    fs::write(
        root.path().join("src/lib.rs"),
        "mod account_pool; mod api_contract_guard; mod tokio; mod brand_absence;\n",
    )
    .expect("crate root");
    fs::write(root.path().join("src/tokio.rs"), "pub fn marker() {}\n").expect("local module");

    let dependencies = brand_dependencies(root.path());
    assert!(
        dependencies.contains("tokio/marker"),
        "an extern-prelude spelling overrode a declared local binding: {dependencies:?}"
    );
}

#[test]
fn crate_root_block_modules_are_not_dropped_from_the_dependency_graph() {
    for (tag, declaration) in [
        (
            "root-block-path",
            "fn boot() { #[path = \"brand_absence.rs\"] mod brand_absence; brand_absence::run(); }\n",
        ),
        (
            "root-block-inline",
            "fn boot() { mod brand_absence { pub fn run() { crate::account_pool::thing(); } } brand_absence::run(); }\n",
        ),
    ] {
        let root = tempfile::Builder::new().prefix(tag).tempdir().unwrap();
        fs::create_dir(root.path().join("src")).unwrap();
        fs::write(
            root.path().join("src/lib.rs"),
            format!("mod account_pool; mod api_contract_guard; {declaration}"),
        )
        .unwrap();
        fs::write(
            root.path().join("src/account_pool.rs"),
            "pub fn thing() {}\n",
        )
        .unwrap();
        fs::write(
            root.path().join("src/api_contract_guard.rs"),
            "pub fn check() {}\n",
        )
        .unwrap();
        if tag == "root-block-path" {
            fs::write(
                root.path().join("src/brand_absence.rs"),
                "pub fn run() { crate::account_pool::thing(); }\n",
            )
            .unwrap();
        }
        let graph = anvil::source_scan::paths::production_module_dependencies(root.path())
            .expect("measure root block module");
        assert!(
            graph
                .get("brand_absence")
                .is_some_and(|deps| deps.iter().any(|dep| dep.starts_with("account_pool"))),
            "root block module was omitted for {tag}: {graph:?}"
        );
    }
}

#[test]
fn cfg_alternate_alias_cannot_erase_a_possible_local_module() {
    let root = tree(
        "cfg-module-alias",
        "pub fn f() { crate::account_pool::thing(); }\n",
    );
    fs::write(
        root.path().join("src/lib.rs"),
        "#[cfg(unix)] mod account_pool;\n#[cfg(windows)] use crate::api_contract_guard as account_pool;\nmod api_contract_guard; mod brand_absence;\n",
    )
    .unwrap();
    let dependencies = brand_dependencies(root.path());
    assert!(
        dependencies
            .iter()
            .any(|dep| dep.starts_with("account_pool")),
        "a cfg-alternate alias erased possible module provenance: {dependencies:?}"
    );
}

#[test]
fn value_namespace_module_named_include_does_not_shadow_the_builtin_macro() {
    let root = tree("include-namespace", "include!(\"hidden.inc\");\n");
    fs::write(
        root.path().join("src/lib.rs"),
        "mod include; mod account_pool; mod api_contract_guard; mod brand_absence;\n",
    )
    .unwrap();
    fs::write(root.path().join("src/include.rs"), "pub fn value() {}\n").unwrap();
    fs::write(
        root.path().join("src/hidden.inc"),
        "pub fn hidden() { crate::account_pool::thing(); }\n",
    )
    .unwrap();
    assert!(
        brand_dependencies(root.path())
            .iter()
            .any(|dependency| dependency.starts_with("account_pool"))
    );
}

#[test]
fn glob_imported_macro_named_include_is_not_assumed_to_be_builtin() {
    let root = tree(
        "glob-macro-include",
        "use crate::macros::*; include!(\"definitely-not-a-file.rs\");\n",
    );
    fs::write(
        root.path().join("src/lib.rs"),
        "mod macros; mod account_pool; mod api_contract_guard; mod brand_absence;\n",
    )
    .unwrap();
    fs::write(
        root.path().join("src/macros.rs"),
        "macro_rules! include { ($path:literal) => {} } pub(crate) use include;\n",
    )
    .unwrap();
    let error = anvil::source_scan::paths::production_module_dependencies(root.path())
        .expect_err("glob macro provenance must be expanded or withheld");
    assert!(error.contains("include provenance"), "{error}");
}

#[test]
fn data_macro_arguments_cannot_hide_a_block_local_module() {
    let root = tree(
        "format-module",
        r#"
            #[cfg(windows)]
            pub fn f() {
                let _ = format!("{}", {
                    #[path = "../tests/shipping.rs"] mod shipping;
                    shipping::run()
                });
            }
        "#,
    );
    fs::create_dir_all(root.path().join("tests")).unwrap();
    fs::write(
        root.path().join("tests/shipping.rs"),
        "pub fn run() { crate::account_pool::thing(); }\n",
    )
    .unwrap();
    let measured = anvil::source_scan::paths::production_module_dependencies(root.path());
    assert!(
        measured
            .as_ref()
            .is_err_and(|error| error.contains("module"))
            || measured
                .as_ref()
                .ok()
                .and_then(|graph| graph.get("brand_absence"))
                .is_some_and(|deps| deps.iter().any(|dep| dep.starts_with("account_pool"))),
        "a module inside safe-macro arguments was certified absent: {measured:?}"
    );
}
