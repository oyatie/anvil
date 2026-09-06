use super::*;
use syn::visit::Visit;

fn uncertainty(source: &str) -> bool {
    let mut symbols = Symbols::default();
    symbols.add_audited_derive_crate("async_trait", "async-trait");
    symbols.add_audited_derive_crate("tokio", "tokio");
    symbols.add_audited_derive_crate("tracing", "tracing");
    let mut syntax = Syntax::new(&[], &symbols);
    syntax.visit_file(&syn::parse_file(source).unwrap());
    !syntax.uncertainties.is_empty()
}

#[test]
fn async_trait_only_admits_unaffected_macro_token_trees() {
    assert!(!uncertainty(
        "#[async_trait::async_trait] impl Port for Adapter { async fn run(&self) { let _ = format!(\"{}\", 1); } }"
    ));
    assert!(uncertainty(
        "#[async_trait::async_trait] impl Port for Adapter { async fn run(&self) { let _ = format!(\"{:?}\", self); } }"
    ));
}

#[test]
fn tokio_function_macros_require_public_input_forms() {
    assert!(!uncertainty("fn f() { tokio::join!(one(), two()); }"));
    assert!(!uncertainty(
        "fn f() { tokio::select! { value = one() => { consume(value); }, else => {} } }"
    ));
    assert!(uncertainty("fn f() { tokio::join!(@internal one()); }"));
}

#[test]
fn nested_audited_invocations_keep_the_same_contract_and_input_traversal() {
    assert!(!uncertainty(
        "fn f() { tokio::select! { value = one() => { tracing::info!(\"{}\", value); } } }"
    ));
    assert!(uncertainty(
        "fn f() { tokio::select! { value = one() => { tracing::info!(\"{}\", unknown::value!()); } } }"
    ));
}

#[test]
fn try_join_accepts_only_the_audited_nonempty_public_forms() {
    assert!(!uncertainty("fn f() { tokio::try_join!(one(), two()); }"));
    assert!(!uncertainty(
        "fn f() { tokio::try_join!(biased; one(), two(),); }"
    ));
    assert!(uncertainty("fn f() { tokio::try_join!(); }"));
    assert!(uncertainty("fn f() { tokio::try_join!(@internal one()); }"));
    assert!(uncertainty(
        "fn f() { tokio::try_join!(unknown::future!()); }"
    ));
}

#[test]
fn public_function_contracts_follow_canonical_identity_through_both_routes() {
    for (entrypoint, alias, arguments) in [
        ("join", "combine", "one(), two()"),
        ("try_join", "collect_results", "biased; one(), two(),"),
        ("select", "join", "value = one() => { consume(value); }"),
    ] {
        for invocation in [
            format!("tokio::{entrypoint}! {{ {arguments} }}"),
            format!("{alias}! {{ {arguments} }}"),
        ] {
            for expression in [invocation.clone(), format!("tokio::join!({invocation})")] {
                let source =
                    format!("fn f() {{ use tokio::{entrypoint} as {alias}; {expression}; }}");
                assert!(!uncertainty(&source), "{source}");
            }
        }
    }
}

#[test]
fn resolved_function_identity_refuses_unknown_or_distinct_candidates() {
    let mut symbols = Symbols::default();
    symbols.add_audited_derive_crate("runtime", "tokio");
    symbols.add_audited_derive_crate("tokio", "tokio");
    let path = vec!["combine".to_owned()];
    let mut aliases = BTreeMap::from([(
        "combine".to_owned(),
        vec![vec!["runtime".to_owned(), "join".to_owned()]],
    )]);
    let resolve = |aliases: &BTreeMap<String, Vec<Vec<String>>>| {
        symbols.audited_function_macro(&path, &[], aliases, &BTreeMap::new())
    };
    assert_eq!(
        resolve(&aliases),
        Some(("tokio".to_owned(), "join".to_owned()))
    );
    aliases
        .get_mut("combine")
        .unwrap()
        .push(vec!["tokio".to_owned(), "join".to_owned()]);
    assert_eq!(
        resolve(&aliases),
        Some(("tokio".to_owned(), "join".to_owned()))
    );
    aliases
        .get_mut("combine")
        .unwrap()
        .push(vec!["tokio".to_owned(), "select".to_owned()]);
    assert_eq!(resolve(&aliases), None);
    aliases.insert(
        "combine".to_owned(),
        vec![vec!["unknown".to_owned(), "join".to_owned()]],
    );
    assert_eq!(resolve(&aliases), None);
    assert_eq!(resolve(&BTreeMap::new()), None);
}

#[test]
fn function_token_contract_requires_a_supported_package_and_entrypoint() {
    let expressions = "one(), two()".parse().unwrap();
    assert!(macro_contract::public_function_tokens(
        ("tokio", "try_join"),
        expressions
    ));
    assert!(!macro_contract::public_function_tokens(
        ("tokio", "try_join"),
        Default::default()
    ));
    assert!(!macro_contract::public_function_tokens(
        ("unknown", "join"),
        Default::default()
    ));
    assert!(!macro_contract::public_function_tokens(
        ("tokio", "unsupported"),
        Default::default()
    ));
}
