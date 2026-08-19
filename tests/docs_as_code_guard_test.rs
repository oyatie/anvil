use anvil::doc_guard::DocsAsCodeGuard;
use tempfile::tempdir;

#[tokio::test]
async fn test_docs_as_code_guard_enforces_rustdoc_comments() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();

    // 1. Undocumented struct fails
    std::fs::write(src.join("lib.rs"), "pub struct Undocumented;\n").unwrap();
    let guard = DocsAsCodeGuard::new();
    let r1 = guard
        .evaluate_docs_as_code(dir.path(), &["src/lib.rs".to_string()])
        .await
        .unwrap();
    assert!(!r1.is_compliant);
    assert_eq!(r1.missing_docstrings.len(), 1);

    // 2. Documented struct passes
    std::fs::write(
        src.join("lib.rs"),
        "/// Documented model\npub struct Documented;\n",
    )
    .unwrap();
    let r2 = guard
        .evaluate_docs_as_code(dir.path(), &["src/lib.rs".to_string()])
        .await
        .unwrap();
    assert!(r2.is_compliant);
}
