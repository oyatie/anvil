//! Every `agy` turn receives its provider deadline from the finite constructor.

use std::path::Path;

#[test]
fn finite_agy_constructor_always_derives_an_explicit_print_timeout() {
    let src = anvil::source_scan::paths::module_source(
        "src/exec/agent/provider",
        Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    let src = anvil::source_scan::without_test_modules(&src);
    let at = src
        .find("pub fn agy_agent(")
        .unwrap_or_else(|| panic!("the finite agy constructor moved; this ratchet must follow it"));
    let body: String = src[at..].chars().take(1_500).collect();
    assert!(body.contains("\"--print-timeout\""));
    assert!(
        body.contains("agy_print_timeout_arg(budget)"),
        "agy's deadline must be derived from the same supervised turn budget"
    );
    assert!(body.contains("\"--print\",\n        \"\""));
}

#[test]
fn production_agy_callers_only_request_the_finite_constructor() {
    let mut callers = Vec::new();
    let mut stack = vec![Path::new("src").to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap_or_default();
            let source = anvil::source_scan::without_test_modules(&source);
            let source = anvil::source_scan::without_commentary(&source);
            for _ in source.match_indices("agy_agent(") {
                callers.push(path.display().to_string());
            }
        }
    }
    assert!(
        callers.len() >= 6,
        "agy caller census lost its subject: {callers:?}"
    );
    assert!(
        callers.iter().all(|path| path != "src/exec/agent.rs"),
        "generic AgentCommand construction absorbed agy argv again"
    );
}
