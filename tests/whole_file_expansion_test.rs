use anvil::monorepo_guard::WholeFileExpansion;
use anvil::monorepo_guard::whole_file_expansion::FileChange;
use tempfile::tempdir;

#[test]
fn test_whole_file_expansion_catches_dark_code_violations() {
    let dir = tempdir().unwrap();
    let core_dir = dir.path().join("finance/core/src");
    std::fs::create_dir_all(&core_dir).unwrap();

    // Create a 350-line file that imports sqlx
    let mut file_content = "pub struct Account;\n".to_string();
    file_content.push_str("use sqlx::PgPool;\n");
    for i in 0..350 {
        file_content.push_str(&format!("fn method_{}() {{}}\n", i));
    }
    std::fs::write(core_dir.join("account.rs"), &file_content).unwrap();

    // A change that CREATES the file owns everything in it: every line is
    // added and the net growth is the whole file. Both findings are this
    // change's, which is what the gate is for.
    let change = FileChange {
        added: &file_content,
        net_lines: file_content.lines().count() as i64,
    };
    let violations =
        WholeFileExpansion::evaluate_whole_file(dir.path(), "finance/core/src/account.rs", &change);
    assert_eq!(violations.len(), 2);
    assert!(
        violations
            .iter()
            .any(|v| v.category == "OVERSIZED_WHOLE_FILE")
    );
    assert!(
        violations
            .iter()
            .any(|v| v.category == "CLEAN_ARCHITECTURE_CORE_IO_VIOLATION")
    );
}
