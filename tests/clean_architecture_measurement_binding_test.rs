//! Read-only source contracts for the actual production conversion and seam.

use std::path::Path;

use anvil::source_scan::paths::module_source;

#[test]
fn certification_uses_the_report_status_conversion() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let evaluator = module_source("src/pre_merge_guard/evaluator", root);
    assert!(evaluator.contains("clean_arch_report.gate_status()"));
    let report = module_source("src/clean_architecture_guard/report", root);
    assert!(report.contains("pub fn gate_status(&self) -> GateStatus"));
    assert!(report.contains("self.measurement.not_measured_reason()"));
}

#[test]
fn both_real_entrypoints_reach_the_tested_diff_core() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let owner = module_source("src/clean_architecture_guard/mod", root);
    assert_eq!(owner.matches("analyze::analyze_unified_diff(").count(), 2);
    let analyzer = module_source("src/clean_architecture_guard/analyze", root);
    assert!(analyzer.contains("TestSourceClassifier::new(repo_root)"));
    assert!(analyzer.contains("analyze_with_inputs("));
    assert!(analyzer.contains("ArchitectureOwnership::new(repo_root)"));
    assert!(analyzer.contains("ownership.relation(file, root)"));
    assert!(analyzer.contains("classify_test(Path::new(&current_file))?"));
}

#[test]
fn ownership_uses_exact_roles_and_retains_alias_destinations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ownership = module_source("src/source_scan/paths/module_graph/ownership", root);
    assert!(ownership.contains("exact_production_roles(&repo, &root.identity.root)?"));
    assert!(ownership.contains("self_aliases(&syntax)"));
    let roots = module_source("src/source_scan/paths/module_graph/roots/mod", root);
    assert!(roots.contains("local_alias_targets(target.kind, &packages)?"));
    let compact_roots: String = roots
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert!(compact_roots.contains("libraries.get(directory)"));
    let aliases = module_source(
        "src/source_scan/paths/module_graph/roots/workspace/aliases",
        root,
    );
    let compact_aliases: String = aliases
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert!(compact_aliases.contains("self.local_alias_targets(target_kind,packages)?"));
    let declaration = module_source("src/source_scan/paths/module_graph/declaration/mod", root);
    assert!(declaration.contains("Ok(exact_role_evidence(measured))"));
    assert!(declaration.contains("return all_contained_files(repo_root)"));
}
