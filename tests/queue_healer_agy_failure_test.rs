//! A timed-out model must not count as a successful heal.
//!
//! `QueueHealer` asks `agy` to resolve merge conflicts and fix broken code by
//! editing the workspace directly, then runs a test gate, then **commits and
//! pushes the healed branch**. The prompt's return value is discarded — the
//! edits are the product, not the text.
//!
//! Observed live, four times in eight hours:
//!
//! ```text
//! ERROR anvil::queue_healer: agy returned non-zero status in QueueHealer: exit status: 1
//! WARN  anvil::queue_healer: agy stderr: Error: timeout waiting for response
//! ```
//!
//! The handler bailed only when stdout was *entirely empty*. Any byte of
//! partial output from a process that died mid-edit was treated as success, so
//! a half-applied conflict resolution could reach the commit-and-push step. For
//! an agent that edits files there is no partial success: a truncated edit
//! session is strictly worse than no session, because it leaves the workspace
//! in a state nobody chose.

use anvil::exec::interpret_agy_outcome;

#[test]
fn a_timed_out_run_that_printed_something_is_still_a_failure() {
    let err = interpret_agy_outcome(
        false,
        "Resolving conflict in src/lib.rs...\n",
        "Error: timeout waiting for response",
    )
    .expect_err("a non-zero exit must fail the heal even with partial output");
    let msg = err.to_string();
    assert!(
        msg.contains("timeout") || msg.contains("non-zero") || msg.contains("exit"),
        "the error must say why the heal failed, so the operator is not left guessing: {msg}"
    );
}

#[test]
fn an_empty_failed_run_is_a_failure_too() {
    assert!(
        interpret_agy_outcome(false, "", "boom").is_err(),
        "this case was already handled; it must stay handled"
    );
}

#[test]
fn a_successful_run_returns_its_output() {
    let out = interpret_agy_outcome(true, "done\n", "").expect("success must pass through");
    assert_eq!(out, "done\n");
}

#[test]
fn a_successful_run_with_empty_output_is_not_an_error() {
    // agy edits files; it is not required to print anything. Treating silence
    // as failure would make every clean heal look broken.
    assert!(
        interpret_agy_outcome(true, "", "").is_ok(),
        "a clean run that printed nothing still edited the workspace"
    );
}

/// The "bail only when stdout is empty" idiom must not come back.
///
/// It appeared independently in two places — `queue_healer` and
/// `fixer/engine` — which is what makes it a class rather than a slip. Both
/// spawn a model that edits the workspace and both then commit; both treated
/// any byte of output from a dead process as a completed edit.
///
/// Prompting cannot prevent the third instance. The shape is small, reads as
/// defensive, and looks like it is *adding* a guard rather than removing one.
#[test]
fn no_source_treats_partial_output_from_a_failed_process_as_success() {
    let mut offenders = Vec::new();
    let mut stack = vec![std::path::PathBuf::from("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if !p.extension().is_some_and(|e| e == "rs") {
                continue;
            }
            let body = std::fs::read_to_string(&p).unwrap_or_default();
            let code: String = body
                .lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");

            // The shape: a failed-status branch whose bail is itself guarded by
            // an emptiness check, so a non-empty stdout falls through.
            // Only the FAILED branch. `Ok(o) if o.status.success() => { if !stdout.is_empty()`
            // is the correct shape -- it proceeds only on success and then checks the
            // output is usable -- and matching bare `status.success()` flagged it.
            for (i, _) in code.match_indices("!output.status.success()") {
                let window = &code[i..code.len().min(i + 420)];
                if window.contains("trim().is_empty()") && window.contains("bail!") {
                    offenders.push(p.display().to_string());
                    break;
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these treat partial output from a failed process as success: {offenders:?}\n\
         A non-zero exit is a failure regardless of what was printed. Route the decision \
         through exec::interpret_agy_outcome instead."
    );
}
