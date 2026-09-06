use super::strip;

#[test]
fn strings_and_comments_cannot_close_a_test_module_early() {
    let source = r#"
#[cfg(test)]
mod tests {
    const CLOSE: &str = "}";
    // }
    /* { } */
}
pub fn production_after() {}
"#;
    let stripped = strip(source).expect("valid Rust");
    assert!(!stripped.contains("const CLOSE"));
    assert!(stripped.contains("pub fn production_after"));
    assert_eq!(stripped.lines().count(), source.lines().count());
    assert_eq!(stripped.len(), source.len());
}

#[test]
fn lexer_shaped_bytes_cannot_desynchronize_ast_spans() {
    let source = r####"
pub const BEFORE: &str = "🦀 { #[cfg(test)] mod fake; }";
#[cfg(test)]
mod fixtures {
    const RAW: &str = r###" } \" #[cfg(not(test))] { "###;
    const BYTE: u8 = b'}';
    fn lifetime<'a>(value: &'a str) -> &'a str { value }
    /* nested-looking { // } */
}
pub const AFTER: char = '}';
"####;
    let stripped = strip(source).expect("token-aware Rust stripping");
    assert!(stripped.contains("pub const BEFORE"));
    assert!(stripped.contains("#[cfg(test)] mod fake"));
    assert!(stripped.contains("pub const AFTER"));
    assert!(!stripped.contains("const RAW"));
    assert!(!stripped.contains("fn lifetime"));
    assert_eq!(stripped.len(), source.len());
    assert_eq!(stripped.lines().count(), source.lines().count());
}

#[test]
fn nested_inline_test_module_is_removed_without_erasing_its_parent() {
    let source = r#"
mod production {
    pub fn before() {}
    #[cfg ( test )] pub(in crate) mod arbitrary_name { pub fn fixture() {} }
    pub fn after() {}
}
"#;
    let stripped = strip(source).expect("nested inline module");
    assert!(stripped.contains("mod production"));
    assert!(stripped.contains("pub fn before"));
    assert!(stripped.contains("pub fn after"));
    assert!(!stripped.contains("arbitrary_name"));
    assert!(!stripped.contains("fixture"));
}

#[test]
fn spaced_cfg_visibility_and_same_line_modules_are_removed() {
    let source = "#[allow(dead_code)] #[cfg( test )] pub(crate) mod fixtures { fn x() {} } pub fn ships() {}\n";
    let stripped = strip(source).expect("valid Rust");
    assert!(!stripped.contains("fixtures"));
    assert!(stripped.contains("pub fn ships"));
}

#[test]
fn an_external_test_declaration_does_not_consume_following_production() {
    let source = "#[cfg( test )] pub(super) mod fixtures; pub fn ships() {}\n";
    let stripped = strip(source).expect("valid Rust");
    assert!(!stripped.contains("fixtures"));
    assert!(stripped.contains("pub fn ships"));
}

#[test]
fn malformed_rust_is_not_a_test_free_corpus() {
    assert!(strip("#[cfg(test)] mod fixtures {").is_err());
}

#[test]
fn compound_cfg_strips_only_modules_impossible_without_test() {
    let source = r#"
#[cfg(all(test, unix))]
mod fixture { const TEST_ONLY: bool = true; }
#[cfg(not(not(test)))]
mod also_fixture { const ALSO_TEST_ONLY: bool = true; }
#[cfg(any(test, unix))]
mod potentially_shipping { const MAY_SHIP: bool = true; }
#[cfg_attr(not(test), cfg(test))]
mod impossible_in_production { const CFG_ATTR_TEST_ONLY: bool = true; }
"#;
    let stripped = strip(source).expect("valid compound cfg source");
    assert!(!stripped.contains("TEST_ONLY"));
    assert!(!stripped.contains("ALSO_TEST_ONLY"));
    assert!(!stripped.contains("CFG_ATTR_TEST_ONLY"));
    assert!(stripped.contains("potentially_shipping"));
    assert!(stripped.contains("MAY_SHIP"));
    assert_eq!(stripped.len(), source.len());
    assert_eq!(stripped.lines().count(), source.lines().count());
}
