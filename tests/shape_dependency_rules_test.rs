//! The Dependency Rule as data, over edges read from real manifests: a face
//! may depend only on the faces its skeleton's matrix lists; another unit
//! may be reached only through its facade; a port trait is never declared
//! under an adapters face. Each seeded defect names its rule (G17) and ships
//! the edge to use instead.

use anvil::shape::adapters::{
    BuckLabelDeps, CargoManifestDeps, InMemoryTree, RustUseDeps, TsImportDeps,
};
use anvil::shape::core::{DepGraph, Fix, RuleId, ShapeSpec, SpecSource, measure, resolve};
use anvil::shape::ports::{DependencySource, discover_units};
use std::path::PathBuf;

fn spec(name: &str) -> ShapeSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/shape")
        .join(name);
    ShapeSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn registry() -> serde_json::Value {
    serde_json::json!({
        "capabilities": [ { "name": "iam" }, { "name": "billing" } ],
        "meta_directories": [], "faces": []
    })
}

fn graph_for(
    tree: &InMemoryTree,
    resolved: &anvil::shape::core::ResolvedSpec,
    sources: &[&dyn DependencySource],
) -> DepGraph {
    let units = discover_units(resolved, tree);
    let mut g = DepGraph::default();
    for s in sources {
        match s.edges(tree, resolved, &units) {
            Ok(e) => g.edges.extend(e),
            Err(e) => g.unavailable.push((s.profile(), e.to_string())),
        }
    }
    g
}

fn cargo(name: &str, deps: &[(&str, &str)]) -> String {
    let mut s = format!("[package]\nname = \"{name}\"\n\n[dependencies]\n");
    for (n, p) in deps {
        s.push_str(&format!("{n} = {{ path = \"{p}\" }}\n"));
    }
    s
}

#[test]
fn a_facade_crate_reaching_core_directly_is_denied_with_the_ports_seam_as_the_fix() {
    let tree = InMemoryTree::from_paths("fx", &[])
        .with_file("iam/core/domain/Cargo.toml", &cargo("oya-iam-domain", &[]))
        .with_file(
            "iam/ports/api/Cargo.toml",
            &cargo("oya-iam-api", &[("oya-iam-domain", "../../core/domain")]),
        )
        .with_file(
            "iam/adapters/pg/Cargo.toml",
            &cargo(
                "oya-iam-pg-adapter",
                &[
                    ("oya-iam-api", "../../ports/api"),
                    ("oya-iam-domain", "../../core/domain"),
                ],
            ),
        )
        .with_file(
            "iam/facade/app/Cargo.toml",
            &cargo(
                "oya-iam-app",
                &[
                    ("oya-iam-api", "../../ports/api"),
                    ("oya-iam-domain", "../../core/domain"),
                    ("oya-iam-pg-adapter", "../../adapters/pg"),
                ],
            ),
        );
    let resolved = resolve(&spec("oyatie.shape.json"), Some(&registry())).unwrap();
    let g = graph_for(&tree, &resolved, &[&CargoManifestDeps]);
    assert!(g.unavailable.is_empty(), "{:?}", g.unavailable);
    let report = measure(&resolved, &tree, "fx", SpecSource::Adopted, &g);
    let denied: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule == RuleId::new("face_edge_denied"))
        .collect();
    assert_eq!(
        denied.len(),
        1,
        "only facade->core is illegal here: {denied:?}"
    );
    assert_eq!(denied[0].key, "iam/facade/app->iam/core/domain");
    assert_eq!(
        denied[0].fix,
        Some(Fix::DependOnInstead {
            replace: "iam/core/domain/".into(),
            with: "iam/ports/".into()
        })
    );
}

#[test]
fn a_core_crate_depending_on_an_adapter_is_denied() {
    let tree = InMemoryTree::from_paths("fx", &[])
        .with_file(
            "iam/adapters/pg/Cargo.toml",
            &cargo("oya-iam-pg-adapter", &[]),
        )
        .with_file(
            "iam/core/domain/Cargo.toml",
            &cargo(
                "oya-iam-domain",
                &[("oya-iam-pg-adapter", "../../adapters/pg")],
            ),
        );
    let resolved = resolve(&spec("oyatie.shape.json"), Some(&registry())).unwrap();
    let g = graph_for(&tree, &resolved, &[&CargoManifestDeps]);
    let report = measure(&resolved, &tree, "fx", SpecSource::Adopted, &g);
    let keys: Vec<&str> = report
        .findings
        .iter()
        .filter(|f| f.rule == RuleId::new("face_edge_denied"))
        .map(|f| f.key.as_str())
        .collect();
    assert_eq!(keys, vec!["iam/core/domain->iam/adapters/pg"]);
}

#[test]
fn another_unit_is_reachable_only_through_its_facade() {
    let tree = InMemoryTree::from_paths("fx", &[])
        .with_file("iam/core/domain/Cargo.toml", &cargo("oya-iam-domain", &[]))
        .with_file("iam/facade/app/Cargo.toml", &cargo("oya-iam-app", &[]))
        .with_file(
            "billing/facade/app/Cargo.toml",
            &cargo(
                "oya-billing-app",
                &[
                    ("oya-iam-domain", "../../../iam/core/domain"),
                    ("oya-iam-app", "../../../iam/facade/app"),
                ],
            ),
        );
    let resolved = resolve(&spec("oyatie.shape.json"), Some(&registry())).unwrap();
    let g = graph_for(&tree, &resolved, &[&CargoManifestDeps]);
    let report = measure(&resolved, &tree, "fx", SpecSource::Adopted, &g);
    let cross: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule == RuleId::new("cross_unit_non_facade"))
        .collect();
    assert_eq!(cross.len(), 1, "{cross:?}");
    assert_eq!(cross[0].key, "billing/facade/app->iam/core/domain");
    assert_eq!(
        cross[0].fix,
        Some(Fix::DependOnInstead {
            replace: "iam/core/domain/".into(),
            with: "iam/facade/".into()
        })
    );
}

#[test]
fn buck_labels_are_edges_too() {
    let tree = InMemoryTree::from_paths("fx", &[])
        .with_file("iam/core/domain/BUCK", "rust_library(name = \"d\")\n")
        .with_file("iam/facade/app/BUCK", "rust_binary(\n  name = \"app\",\n  deps = [\"//iam/core/domain:d\", \"third-party//:serde\"],\n)\n");
    let resolved = resolve(&spec("oyatie.shape.json"), Some(&registry())).unwrap();
    let g = graph_for(&tree, &resolved, &[&BuckLabelDeps]);
    assert_eq!(g.edges.len(), 1, "{:?}", g.edges);
    let report = measure(&resolved, &tree, "fx", SpecSource::Adopted, &g);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule == RuleId::new("face_edge_denied")
                && f.key == "iam/facade/app->iam/core/domain")
    );
}

#[test]
fn module_tree_use_paths_are_edges_and_same_face_is_fine() {
    let tree = InMemoryTree::from_paths("fx", &[])
        .with_file(
            "src/shape/mod.rs",
            "pub mod core;\npub mod ports;\npub mod facade;\n",
        )
        .with_file("src/shape/core/a.rs", "use crate::shape::core::b::B;\n")
        .with_file("src/shape/core/b.rs", "use crate::shape::ports::P;\n")
        .with_file(
            "src/shape/facade/f.rs",
            "use crate::shape::ports::P;\nuse crate::ratchet::core::X;\n",
        )
        .with_file("src/ratchet/mod.rs", "pub mod core;\n")
        .with_file("src/ratchet/core/x.rs", "");
    let resolved = resolve(&spec("anvil.shape.json"), None).unwrap();
    let g = graph_for(&tree, &resolved, &[&RustUseDeps]);
    assert!(g.unavailable.is_empty(), "{:?}", g.unavailable);
    let report = measure(&resolved, &tree, "fx", SpecSource::Adopted, &g);
    let denied: Vec<&str> = report
        .findings
        .iter()
        .filter(|f| f.rule == RuleId::new("face_edge_denied"))
        .map(|f| f.key.as_str())
        .collect();
    assert_eq!(
        denied,
        vec!["src/shape/core/b.rs->src/shape/ports"],
        "core -> ports is the one illegal edge"
    );
    let cross: Vec<&str> = report
        .findings
        .iter()
        .filter(|f| f.rule == RuleId::new("cross_unit_non_facade"))
        .map(|f| f.key.as_str())
        .collect();
    assert_eq!(cross, vec!["src/shape/facade/f.rs->src/ratchet/core"]);
}

#[test]
fn a_port_trait_declared_under_adapters_is_reported() {
    let tree = InMemoryTree::from_paths("fx", &[])
        .with_file(
            "iam/adapters/pg/src/lib.rs",
            "pub trait TenantStore {}\npub struct PgTenantStore;\n",
        )
        .with_file("iam/ports/api/src/lib.rs", "pub trait TenantStore {}\n");
    let resolved = resolve(&spec("oyatie.shape.json"), Some(&registry())).unwrap();
    let report = measure(
        &resolved,
        &tree,
        "fx",
        SpecSource::Adopted,
        &DepGraph::default(),
    );
    let ports: Vec<&str> = report
        .findings
        .iter()
        .filter(|f| f.rule == RuleId::new("port_defined_in_adapter"))
        .map(|f| f.key.as_str())
        .collect();
    assert_eq!(ports, vec!["iam/adapters/pg/src/lib.rs:TenantStore"]);
}

#[test]
fn an_unreadable_profile_makes_the_dependency_rules_not_measured() {
    let tree = InMemoryTree::from_paths("fx", &[])
        .with_file("iam/core/domain/Cargo.toml", &cargo("oya-iam-domain", &[]));
    let resolved = resolve(&spec("oyatie.shape.json"), Some(&registry())).unwrap();
    let g = graph_for(&tree, &resolved, &[&CargoManifestDeps, &TsImportDeps]);
    assert_eq!(g.unavailable.len(), 1);
    let report = measure(&resolved, &tree, "fx", SpecSource::Adopted, &g);
    let nm: Vec<&str> = report
        .not_measured
        .iter()
        .map(|(r, _)| r.0.as_str())
        .collect();
    assert!(
        nm.contains(&"face_edge_denied") && nm.contains(&"cross_unit_non_facade"),
        "{nm:?}"
    );
}
