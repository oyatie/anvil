//! Every `agy` turn receives its provider deadline from the finite constructor.

use std::path::Path;

#[test]
fn finite_agy_constructor_always_derives_an_explicit_print_timeout() {
    let src = anvil::source_scan::paths::module_source(
        "src/exec/agent/provider",
        Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    let src = anvil::source_scan::without_test_modules(&src);
    use quote::ToTokens;
    let file = syn::parse_file(&src).expect("provider syntax");
    let body = |name: &str| {
        file.items
            .iter()
            .find_map(|item| match item {
                syn::Item::Fn(function) if function.sig.ident == name => {
                    Some(function.block.to_token_stream().to_string())
                }
                _ => None,
            })
            .expect("finite provider function")
    };
    let expected: syn::Block = syn::parse_quote!({
        let args = agy_args(effort, budget, model)?;
        let mut cmd = super::command("agy", posture, Framing::AgyStreamJson)?;
        cmd.args(args);
        Ok(cmd)
    });
    assert_eq!(
        body("agy_agent"),
        expected.to_token_stream().to_string(),
        "the same budget-derived argv must reach the returned finite command"
    );
    let args: syn::Block = syn::parse_quote!({
        validate_effort(effort)?;
        if let Some(model) = model {
            validate_model_selector(model)?;
        }
        let timeout = crate::exec::agy_print_timeout_arg(budget);
        let mut args = vec![
            "--print".into(),
            "".into(),
            "--input-format".into(),
            "stream-json".into(),
            "--output-format".into(),
            "stream-json".into(),
            "--effort".into(),
            effort.into(),
            "--print-timeout".into(),
            timeout,
            "--dangerously-skip-permissions".into(),
        ];
        if let Some(model) = model {
            args.extend(["--model".into(), model.into()]);
        }
        Ok(args)
    });
    assert_eq!(
        body("agy_args"),
        args.to_token_stream().to_string(),
        "the timeout argument must occupy its exact slot in the returned argv"
    );
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
    // The census names the sites it expects rather than counting them. The
    // floor used to be `>= 6`, which encoded an architecture where five
    // production sites each named `agy_agent` directly and had no tier beneath
    // them. Routing those through `ai_driver::run_stage` removed all five, so
    // the floor began failing BECAUSE the thing it guarded had improved --
    // and the tempting fix, lowering 6 to 4, is how a check gets quieted into
    // meaning nothing. A count is a proxy for "the scan found its subject";
    // naming the subject is the thing itself.
    for expected in ["src/ai_driver/chain.rs", "src/ai_driver/router.rs"] {
        assert!(
            callers.iter().any(|path| path == expected),
            "agy caller census lost its subject: {expected} constructs an agy \
             command and the scan did not see it. Census: {callers:?}"
        );
    }
    assert!(
        callers.iter().all(|path| path != "src/exec/agent.rs"),
        "generic AgentCommand construction absorbed agy argv again"
    );
}
