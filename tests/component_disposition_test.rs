use anvil::monorepo_guard::{ComponentDisposition, ComponentDispositionClassifier};
use tempfile::tempdir;

#[test]
fn test_component_disposition_classes() {
    let dir = tempdir().unwrap();

    // 1. Retire: Unused tool
    let retire_path = dir.path().join("crates/unused-tool");
    std::fs::create_dir_all(&retire_path).unwrap();
    let r1 =
        ComponentDispositionClassifier::evaluate_component(dir.path(), "crates/unused-tool", 0);
    assert_eq!(r1.disposition, ComponentDisposition::Retire);

    // 2. Rewrite: YAML catalog
    let rewrite_path = dir.path().join("registry/catalog");
    std::fs::create_dir_all(&rewrite_path).unwrap();
    std::fs::write(rewrite_path.join("manifest.yaml"), "name: test\n").unwrap();
    let r2 = ComponentDispositionClassifier::evaluate_component(
        dir.path(),
        "registry/catalog/manifest.yaml",
        10,
    );
    assert_eq!(r2.disposition, ComponentDisposition::Rewrite);

    // 3. Move: Clean architecture domain crate
    let clean_path = dir.path().join("oya/billing/crates/domain");
    std::fs::create_dir_all(&clean_path).unwrap();
    std::fs::write(clean_path.join("lib.rs"), "pub struct Invoice;\n").unwrap();
    let r3 = ComponentDispositionClassifier::evaluate_component(
        dir.path(),
        "oya/billing/crates/domain",
        5,
    );
    assert_eq!(r3.disposition, ComponentDisposition::Move);

    // 4. Refactor: Monolithic I/O file
    let monolithic_path = dir.path().join("oya/billing/crates/monolith");
    std::fs::create_dir_all(&monolithic_path).unwrap();
    let mut big_content = "pub struct Big;\n".to_string();
    big_content.push_str("use sqlx::PgPool;\n");
    for i in 0..700 {
        big_content.push_str(&format!("fn query_{}() {{}}\n", i));
    }
    std::fs::write(monolithic_path.join("lib.rs"), big_content).unwrap();
    let r4 = ComponentDispositionClassifier::evaluate_component(
        dir.path(),
        "oya/billing/crates/monolith",
        3,
    );
    assert_eq!(r4.disposition, ComponentDisposition::Refactor);
}
