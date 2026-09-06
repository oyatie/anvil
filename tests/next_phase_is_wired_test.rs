//! `next_phase` decides what anvil does after a verdict. It existed, fully
//! written and tested, with no caller anywhere in `src/`.
//!
//! So the approve arm enlisted unconditionally and the reject arm did nothing
//! at all: `execute_pr_fix` was reachable only from the CLI and one manual HTTP
//! handler, and a pull request anvil asked to change sat until a person
//! noticed. Its own module doc says so.
//!
//! A unit test over a pure function cannot catch that -- the function was
//! correct the whole time. What was missing is that anything ran it.

use std::fs;
use std::path::Path;

fn production_sources() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![Path::new("src").to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let text = fs::read_to_string(&p).unwrap_or_default();
                out.push((p.to_string_lossy().to_string(), text));
            }
        }
    }
    out
}

#[test]
fn the_decision_has_a_caller_in_production_code() {
    let callers: Vec<&str> = production_sources()
        .iter()
        .filter(|(path, text)| {
            !path.contains("next_phase.rs")
                && anvil::source_scan::code_only(text).contains("next_phase(")
        })
        .map(|(p, _)| p.clone().leak() as &str)
        .collect();
    assert!(
        !callers.is_empty(),
        "`next_phase` decides what happens after a verdict and nothing calls \
         it, so the verdict reaches no decision. A pull request anvil asks to \
         change stops there."
    );
}

#[test]
fn the_review_pipeline_runs_the_fixer_on_its_own_verdict() {
    let src = anvil::source_scan::paths::module_source(
        "src/webhook/pipelines/review",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    let code = anvil::source_scan::code_only(&src);
    assert!(
        code.contains("execute_pr_fix"),
        "the review pipeline reaches no fixer, so REQUEST_CHANGES chains into \
         nothing and the loop is open at its first arrow"
    );
    assert!(
        code.contains("record_auto_fix_attempt"),
        "the fixer is reachable and the attempt is not counted, which is an \
         unbounded rewrite loop: each run pushes a new head, so `once per \
         head` alone does not stop it"
    );
}

/// The bound must survive a restart, or it is not a bound.
#[test]
fn the_attempt_count_is_persisted_not_only_held_in_memory() {
    let src = anvil::source_scan::paths::module_source(
        "src/state",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    let code = anvil::source_scan::code_only(&src);
    let body = code
        .split("pub async fn record_auto_fix_attempt")
        .nth(1)
        .expect("recorder exists");
    let body = &body[..body.find("\n    pub ").unwrap_or(body.len())];
    assert!(
        body.contains("append_wal") && body.contains("atomic_checkpoint"),
        "the attempt count is written to memory only. It resets on restart, \
         so the daemon resumes rewriting a pull request it had already given \
         up on three times."
    );
}
