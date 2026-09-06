use super::{merge_process_scans, process_scan_with_context};

#[test]
fn mixed_task_and_process_alias_is_not_classified_as_safe() {
    let source = r#"
        #[cfg(windows)]
        use std::process::Command as Runner;
        #[cfg(not(windows))]
        use tokio as Runner;
        fn run() { Runner::spawn(task); }
    "#;
    let (sites, spawns, _) = process_scan_with_context(source, None, None);
    assert!(
        sites
            .iter()
            .any(|site| site.method == "associated-call:spawn"),
        "a retained process interpretation must be charged before a safe alternative: {sites:?}"
    );
    assert!(spawns.is_empty(), "mixed authority became safe: {spawns:?}");
}

fn events(source: &str) -> (Vec<String>, Vec<String>) {
    let (sites, spawns, _) = process_scan_with_context(source, None, None);
    let mut sites = sites
        .into_iter()
        .map(|site| site.method)
        .collect::<Vec<_>>();
    let mut spawns = spawns.into_iter().map(|site| site.path).collect::<Vec<_>>();
    sites.sort();
    spawns.sort();
    (sites, spawns)
}

#[test]
fn both_orders_and_block_bindings_retain_raw_spawn_multiplicity() {
    let process = "#[cfg(windows)] use std::process::Command as Runner;";
    let task = "#[cfg(not(windows))] use tokio as Runner;";
    for declarations in [format!("{process} {task}"), format!("{task} {process}")] {
        for source in [
            format!("{declarations} fn run() {{ Runner::spawn(task); Runner::spawn(task); }}"),
            format!("fn run() {{ {declarations} Runner::spawn(task); Runner::spawn(task); }}"),
        ] {
            let (sites, spawns) = events(&source);
            assert_eq!(sites, ["associated-call:spawn", "associated-call:spawn"]);
            assert!(spawns.is_empty());
        }
    }
}

#[test]
fn bare_imported_function_alias_keeps_its_process_target() {
    let (sites, spawns) = events(
        r#"
        #[cfg(windows)] use std::process::Command::spawn as launch;
        #[cfg(not(windows))] use tokio::spawn as launch;
        fn run() { launch(task); }
    "#,
    );
    assert_eq!(sites, ["associated-call:spawn"]);
    assert!(spawns.is_empty());
}

#[test]
fn safe_alternatives_do_not_erase_missing_local_or_cyclic_targets() {
    for other in [
        "use missing::runtime as Runner;",
        "use self::local::Runner;",
        "use super::missing as Runner;",
        "use self::Cycle as Runner;",
    ] {
        let source = format!(
            r#"
            mod local {{ pub struct Runner; }}
            type Cycle = Cycle;
            #[cfg(windows)] {other}
            #[cfg(not(windows))] use tokio as Runner;
            fn run() {{ Runner::spawn(task); }}
        "#
        );
        let (sites, spawns) = events(&source);
        assert_eq!(sites, ["ambiguous-associated-spawn"], "{other}");
        assert!(spawns.is_empty());
    }
}

#[test]
fn literal_safe_spelling_cannot_restore_an_unknown_or_local_target() {
    for declaration in [
        "use missing::runtime as tokio;",
        "mod harmless {} use self::harmless as tokio;",
    ] {
        let (sites, spawns) = events(&format!("{declaration} fn run() {{ tokio::spawn(task); }}"));
        assert!(
            sites
                .iter()
                .any(|site| site == "ambiguous-associated-spawn")
        );
        assert!(spawns.is_empty());
    }
}

#[test]
fn distinct_safe_alternatives_each_preserve_call_multiplicity() {
    let (sites, spawns) = events(
        r#"
        #[cfg(windows)] use tokio as Runner;
        #[cfg(not(windows))] use std::thread as Runner;
        fn run() { Runner::spawn(task); Runner::spawn(task); }
    "#,
    );
    assert!(sites.is_empty());
    assert_eq!(
        spawns,
        [
            "std::thread::spawn",
            "std::thread::spawn",
            "tokio::spawn",
            "tokio::spawn"
        ]
    );
}

#[test]
fn equivalent_safe_aliases_are_not_double_counted() {
    let (sites, spawns) = events(
        r#"
        #[cfg(windows)] use tokio as Runner;
        #[cfg(not(windows))] use tokio::task as Runner;
        fn run() { Runner::spawn(task); Runner::spawn(task); }
    "#,
    );
    assert!(sites.is_empty());
    assert_eq!(spawns, ["tokio::spawn", "tokio::spawn"]);
}

#[test]
fn stored_mixed_function_alias_remains_a_rejected_reference() {
    let (sites, spawns) = events(
        r#"
        #[cfg(windows)] use std::process::Command::spawn as launch;
        #[cfg(not(windows))] use tokio::spawn as launch;
        fn run() { let stored = launch; }
    "#,
    );
    assert_eq!(sites, ["associated-reference:spawn"]);
    assert!(spawns.is_empty());
}

#[test]
fn raw_candidate_still_wins_beside_an_unresolved_alternative() {
    let (sites, spawns) = events(
        r#"
        #[cfg(windows)] use std::process::Command as Runner;
        #[cfg(not(windows))] use missing::runtime as Runner;
        fn run() { Runner::spawn(task); }
    "#,
    );
    assert_eq!(sites, ["associated-call:spawn"]);
    assert!(spawns.is_empty());
}

#[test]
fn root_context_merge_keeps_maximum_not_sum_multiplicity() {
    let one = "fn run() { tokio::spawn(task); }";
    let two = "fn run() { tokio::spawn(task); tokio::spawn(task); }";
    let (sites, spawns, _) = merge_process_scans(vec![
        process_scan_with_context(one, None, None),
        process_scan_with_context(two, None, None),
    ]);
    assert!(sites.is_empty());
    assert_eq!(spawns.len(), 2);
    assert!(spawns.iter().all(|site| site.path == "tokio::spawn"));
}

#[test]
fn unrelated_calls_and_other_process_methods_keep_their_policy() {
    let (sites, spawns) = events(
        r#"
        mod other { pub struct Runner; }
        fn run() { other::Runner::spawn(task); unknown_function(); }
    "#,
    );
    assert!(sites.is_empty());
    assert!(spawns.is_empty());
    let (sites, spawns) = events(
        r#"
        fn run(command: &mut std::process::Command) {
            std::process::Command::output(command);
            command.status();
        }
    "#,
    );
    assert_eq!(sites, ["associated-call:output", "method:command:status"]);
    assert!(spawns.is_empty());
}
