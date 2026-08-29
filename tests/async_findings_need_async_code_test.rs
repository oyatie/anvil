//! Two rules that accused synchronous code of starving an executor.
//!
//! `async-spawn-blocking` and `async-no-lock-await` are named for what they do
//! to a Tokio executor: starving its workers, deadlocking across an `.await`.
//! Neither is true of the same call in a synchronous function, where blocking
//! is simply how the code works — and neither rule established that the line it
//! flagged was in async code at all. Every `std::fs::read` in a plain `fn` was
//! a HIGH-severity finding against correct code.
//!
//! That is the symmetric violation of I1. A gate must not report what it did
//! not measure, and a fabricated accusation is as much a failure to measure as
//! a missed defect.

use anvil::git_manager::PrDiffContext;
use anvil::rust_language_policy::RustQualityEngine;

fn diff(body: &str) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        previous_head_sha: None,
        repo_working_dir: std::path::PathBuf::from("."),
        diff_content: body.to_string(),
        changed_files: vec!["src/service.rs".to_string()],
        is_incremental: false,
    }
}

fn rule_ids(body: &str) -> Vec<String> {
    RustQualityEngine::new()
        .scan_diff(&diff(body))
        .expect("the scan runs")
        .into_iter()
        .map(|f| f.rule_id)
        .collect()
}

/// The false accusation, as the shipped rule made it.
#[test]
fn a_blocking_call_in_a_synchronous_function_is_not_a_finding() {
    let found = rule_ids(
        "+++ b/src/service.rs\n\
         @@ -1,3 +1,4 @@ fn load_config(path: &str) -> Vec<u8> {\n\
         +    std::fs::read(path).unwrap_or_default()\n",
    );
    assert!(
        !found.iter().any(|id| id == "async-spawn-blocking"),
        "`std::fs::read` in a plain `fn` is how synchronous code reads a file. \
         Got: {found:?}"
    );
}

/// The must-flag twin: the same call, in async code, is the defect.
#[test]
fn a_blocking_call_inside_an_async_function_is_a_finding() {
    let found = rule_ids(
        "+++ b/src/service.rs\n\
         @@ -1,3 +1,4 @@ async fn load_config(path: &str) -> Vec<u8> {\n\
         +    std::fs::read(path).unwrap_or_default()\n",
    );
    assert!(
        found.iter().any(|id| id == "async-spawn-blocking"),
        "the hunk header names an async function, so this starves a worker. \
         Got: {found:?}"
    );
}

/// The declaration can arrive in the added lines rather than the header.
#[test]
fn an_async_declaration_in_the_diff_itself_is_enough() {
    let found = rule_ids(
        "+++ b/src/service.rs\n\
         @@ -1,3 +1,5 @@\n\
         +async fn tick() {\n\
         +    std::thread::sleep(std::time::Duration::from_secs(1));\n\
         +}\n",
    );
    assert!(
        found.iter().any(|id| id == "async-spawn-blocking"),
        "the diff declares the async function it then blocks inside. Got: {found:?}"
    );
}

/// And a synchronous function opened after an async one closes the scope.
#[test]
fn a_synchronous_function_after_an_async_one_is_not_async() {
    let found = rule_ids(
        "+++ b/src/service.rs\n\
         @@ -1,3 +1,8 @@\n\
         +async fn tick() {}\n\
         +\n\
         +fn load(path: &str) -> Vec<u8> {\n\
         +    std::fs::read(path).unwrap_or_default()\n\
         +}\n",
    );
    assert!(
        !found.iter().any(|id| id == "async-spawn-blocking"),
        "`fn load` is not inside `async fn tick`. Got: {found:?}"
    );
}

/// The lock rule could hardly fire: it matched the literal path
/// `std::sync::Mutex::lock`, which is not how anyone calls it.
#[test]
fn the_std_lock_spelling_is_what_the_lock_rule_matches() {
    let found = rule_ids(
        "+++ b/src/service.rs\n\
         @@ -1,3 +1,4 @@ async fn handle(state: &State) {\n\
         +    let guard = state.inner.lock().unwrap();\n",
    );
    assert!(
        found.iter().any(|id| id == "async-no-lock-await"),
        "`.lock().unwrap()` is the std spelling -- tokio's returns a future and \
         is awaited -- so this is a synchronous mutex in async code. Got: {found:?}"
    );
}

/// The must-spare twin for the lock rule: tokio's own mutex, awaited.
#[test]
fn an_awaited_lock_is_not_a_synchronous_lock() {
    let found = rule_ids(
        "+++ b/src/service.rs\n\
         @@ -1,3 +1,4 @@ async fn handle(state: &State) {\n\
         +    let guard = state.inner.lock().await;\n",
    );
    assert!(
        !found.iter().any(|id| id == "async-no-lock-await"),
        "`tokio::sync::Mutex::lock` returns a future; awaiting it is the \
         remedy this rule asks for. Got: {found:?}"
    );
}

/// Nothing here weakens the rules that do not depend on async context.
#[test]
fn the_other_rules_are_untouched_by_the_async_gate() {
    let found = rule_ids(
        "+++ b/src/service.rs\n\
         @@ -1,3 +1,4 @@ fn describe() -> String {\n\
         +    format!(\"a constant\")\n",
    );
    assert!(
        found.iter().any(|id| id == "mem-avoid-format"),
        "`format!` on a literal is wrong in any context. Got: {found:?}"
    );
}
