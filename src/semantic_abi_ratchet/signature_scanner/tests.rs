use super::*;

fn scan(diff: &str) -> AbiScan {
    SignatureScanner::new().scan_abi_diff(diff)
}

fn chunk(path: &str, body: &str) -> String {
    format!("diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1 +1 @@\n{body}\n")
}

#[test]
fn test_detects_removed_public_function() {
    let findings = scan(&chunk(
        "src/api.rs",
        "-pub fn legacy_api() -> u32 {\n-    42\n-}",
    ))
    .findings;
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].symbol_name, "legacy_api");
    assert_eq!(findings[0].change_kind, "REMOVAL");
}

#[test]
fn test_passes_added_public_function() {
    assert!(
        scan(&chunk(
            "src/api.rs",
            "+pub fn new_api() -> u32 {\n+    42\n+}"
        ))
        .findings
        .is_empty()
    );
}

#[test]
fn a_deleted_file_still_names_the_functions_it_took_with_it() {
    let diff = "diff --git a/src/gone.rs b/src/gone.rs\n--- a/src/gone.rs\n+++ /dev/null\n@@ -1,3 +0,0 @@\n-pub fn vanished() -> u32 { 0 }\n";
    let findings = scan(diff).findings;
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].file_path, "src/gone.rs");
}

#[test]
fn a_repr_attribute_is_recorded_as_a_layout_the_gate_cannot_compute() {
    let scan = scan(&chunk("src/wire.rs", "-#[repr(C)]\n+#[repr(C, packed)]"));
    assert_eq!(scan.layout_files, vec!["src/wire.rs".to_string()]);
    assert!(scan.findings.is_empty());
}

#[test]
fn restricted_visibility_is_not_a_published_surface() {
    assert!(
        scan(&chunk("src/api.rs", "-pub(crate) fn internal() {}"))
            .findings
            .is_empty()
    );
    // ...and narrowing a published function to it is a removal.
    let findings = scan(&chunk(
        "src/api.rs",
        "-pub fn open() {}\n+pub(crate) fn open() {}",
    ))
    .findings;
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].change_kind, "REMOVAL");
}

#[test]
fn normalized_ignores_spacing_and_the_block_opener() {
    assert_eq!(
        normalized("pub  fn  f(a: u32) -> u32 {"),
        normalized("pub fn f(a:u32)->u32;")
    );
    assert_eq!(
        normalized("pub fn f("),
        None,
        "an unclosed parameter list is not a signature"
    );
}
