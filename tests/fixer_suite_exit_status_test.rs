//! Structural protection for the actual four suite-result dispatches.
//! These tests parse source; they do not execute contributor test suites.

use quote::ToTokens;
use syn::visit::Visit;

fn tokens(value: &impl ToTokens) -> String {
    value.to_token_stream().to_string()
}

fn refuses_unsuccessful_output(dispatch: &syn::ExprMatch) -> bool {
    let failures: Vec<_> = dispatch
        .arms
        .iter()
        .filter(|arm| tokens(&arm.pat) == "Ok (_)")
        .collect();
    if failures.len() != 1 || failures[0].guard.is_some() {
        return false;
    }
    let syn::Expr::Block(body) = &*failures[0].body else {
        return false;
    };
    body.block
        .stmts
        .last()
        .is_some_and(|statement| tokens(statement) == "return Ok (false) ;")
}

#[test]
fn every_actual_suite_dispatch_refuses_unsuccessful_output() {
    let source = anvil::source_scan::paths::module_source(
        "src/fixer/engine",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    let parsed = syn::parse_file(&source).expect("actual engine parses");
    let methods: Vec<_> = parsed
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Impl(implementation) if tokens(&implementation.self_ty) == "FixEngine" => {
                Some(&implementation.items)
            }
            _ => None,
        })
        .flatten()
        .filter_map(|item| match item {
            syn::ImplItem::Fn(method) if method.sig.ident == "run_test_verification_gate" => {
                Some(method)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        methods.len(),
        1,
        "the actual verification method must exist once"
    );
    #[derive(Default)]
    struct Dispatches(Vec<(String, bool)>);
    impl<'ast> Visit<'ast> for Dispatches {
        fn visit_expr_match(&mut self, dispatch: &'ast syn::ExprMatch) {
            self.0.push((
                tokens(&dispatch.expr),
                refuses_unsuccessful_output(dispatch),
            ));
            syn::visit::visit_expr_match(self, dispatch);
        }
    }
    let mut dispatches = Dispatches::default();
    dispatches.visit_block(&methods[0].block);
    dispatches.0.sort();
    assert_eq!(
        dispatches.0,
        ["check_out", "go_test", "npm_test", "test_out"].map(|name| (name.to_owned(), true))
    );
}

#[test]
fn refusal_predicate_rejects_fallthrough_and_success_returns() {
    for (body, expected) in [
        ("match result { Ok(_) => { return Ok(false); } }", true),
        ("match result { Ok(_) => {} }", false),
        ("match result { Ok(_) => { return Ok(true); } }", false),
        (
            "match result { Ok(_) => { if condition { return Ok(false); } } }",
            false,
        ),
        (
            "match result { Ok(_) if condition => { return Ok(false); } }",
            false,
        ),
        (
            "match result { Ok(out) => { if out.status.success() { return Ok(true); } } }",
            false,
        ),
    ] {
        let dispatch: syn::ExprMatch = syn::parse_str(body).expect("inert source parses");
        assert_eq!(refuses_unsuccessful_output(&dispatch), expected, "{body}");
    }
}
