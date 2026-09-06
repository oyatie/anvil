use super::process_scan_with_context;

fn events(source: &str) -> Vec<String> {
    let (sites, spawns, controls) = process_scan_with_context(source, None, None);
    assert!(spawns.is_empty(), "unexpected safe spawn: {spawns:?}");
    assert!(controls.is_empty(), "unexpected lint control: {controls:?}");
    let mut events = sites
        .into_iter()
        .map(|site| site.method)
        .collect::<Vec<_>>();
    events.sort();
    events
}

#[test]
fn qualified_existing_attribute_identity_is_not_a_new_execution_site() {
    let source = "#[async_trait::async_trait] trait Checkout { async fn observe(&self); } \
        struct Host; #[async_trait::async_trait] impl Checkout for Host { async fn observe(&self) {} }";
    let found = events(source);
    assert!(
        found.is_empty(),
        "qualified existing identity rejected: {found:?}"
    );
}

#[test]
fn approved_import_and_qualified_forms_keep_body_inspection() {
    for (prefix, attribute) in [
        ("use async_trait::async_trait;", "async_trait"),
        ("", "async_trait::async_trait"),
        ("", "tracing::instrument"),
    ] {
        let source =
            format!("{prefix} #[{attribute}] fn run(mut command: Process) {{ command.spawn(); }}");
        let found = events(&source);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("method:"), "{found:?}");
        assert!(found[0].ends_with(":spawn"), "{found:?}");
    }
}

#[test]
fn unapproved_qualified_paths_do_not_borrow_a_trusted_last_segment() {
    for attribute in [
        "other::async_trait",
        "other::instrument",
        "async_trait::other",
        "nested::async_trait::async_trait",
    ] {
        assert_eq!(
            events(&format!("#[{attribute}] fn run() {{}}")),
            [format!("unapproved-attribute:{attribute}")]
        );
    }
}

#[test]
fn qualified_syntax_does_not_clear_existing_shadow_refusals() {
    for source in [
        "mod async_trait {} #[async_trait::async_trait] fn run() {}",
        "use other as async_trait; #[async_trait::async_trait] fn run() {}",
        "#[cfg(windows)] use other as async_trait; #[async_trait::async_trait] fn run() {}",
        "extern crate other as async_trait; #[async_trait::async_trait] fn run() {}",
    ] {
        let found = events(source);
        assert!(!found.is_empty(), "shadow escaped: {source}");
        assert!(
            found
                .iter()
                .any(|event| event.starts_with("shadowed-trusted-path-root")
                    || event == "untrusted-extern-crate-binding"),
            "{found:?}"
        );
    }
}

#[test]
fn cfg_attr_checks_all_retained_payloads_without_platform_selection() {
    for cfg in ["windows", "not(windows)"] {
        assert!(
            events(&format!(
                "#[cfg_attr({cfg}, async_trait::async_trait)] fn run() {{}}"
            ))
            .is_empty()
        );
        assert_eq!(
            events(&format!(
                "#[cfg_attr({cfg}, other::async_trait)] fn run() {{}}"
            )),
            ["unapproved-attribute:other::async_trait"]
        );
    }
    assert!(events("#[cfg(test)] #[other::async_trait] fn fixture() {}").is_empty());
}
