//! The hook templates are exercised here, because nothing else exercises them.

use std::fs;
use std::path::Path;
use std::process::Command;

fn hook(name: &str) -> String {
    fs::read_to_string(Path::new("src/git_manager/hooks").join(name))
        .unwrap_or_else(|e| panic!("src/git_manager/hooks/{name} must be readable: {e}"))
}

fn run_hook_in(dir: &Path, name: &str, stdin: &str) -> (i32, String) {
    let out = Command::new("bash")
        .arg(fs::canonicalize(Path::new("src/git_manager/hooks").join(name)).expect("hook path"))
        .arg(stdin)
        .current_dir(dir)
        .output()
        .expect("hook runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr),
    )
}

#[test]
fn commit_msg_refuses_a_non_conventional_message_and_accepts_a_conventional_one() {
    let dir = tempfile::tempdir().expect("tempdir");
    let msg = dir.path().join("MSG");

    fs::write(&msg, "just some words\n").unwrap();
    let (code, out) = run_hook_in(dir.path(), "commit-msg", msg.to_str().unwrap());
    assert_eq!(code, 1, "a non-conventional message must be refused: {out}");

    fs::write(&msg, "fix(scope): a real summary\n").unwrap();
    let (code, out) = run_hook_in(dir.path(), "commit-msg", msg.to_str().unwrap());
    assert_eq!(code, 0, "a conventional message must pass: {out}");

    fs::write(&msg, "Merge branch 'main' into feature\n").unwrap();
    let (code, _) = run_hook_in(dir.path(), "commit-msg", msg.to_str().unwrap());
    assert_eq!(code, 0, "generated merge messages must not be refused");
}

#[test]
fn every_hook_honours_the_documented_escape_hatch() {
    for name in ["pre-commit", "pre-push", "commit-msg"] {
        assert!(
            hook(name).contains("ANVIL_SKIP_HOOKS"),
            "src/git_manager/hooks/{name} has no deliberate bypass"
        );
    }
}

#[test]
fn pre_push_fmts_the_file_list() {
    let src = hook("pre-push");
    assert!(
        src.contains("rustfmt --check"),
        "pre-push must rustfmt --check the changed *.rs list"
    );
    // pre-push *is* the clippy bar, alongside CI. #95 scoped clippy out on the
    // grounds that CI owns it. CI does own it -- and that is the problem: a
    // `-D warnings` failure that only CI catches costs a full round trip, and
    // an `async_fn_in_trait` lint reached the remote exactly that way once.
    //
    // The cost of keeping it here was measured rather than assumed: warm,
    // `cargo check --all-targets` is 2.29s and `cargo clippy --all-targets
    // -- -D warnings` is 3.83s. 1.5s on push against one CI round trip.
    assert!(
        src.contains("cargo clippy"),
        "pre-push must run clippy -D warnings; CI treats those as fatal"
    );
}

#[test]
fn pre_commit_names_every_agent_directory_that_has_been_found_tracked() {
    let src = hook("pre-commit");
    for dir in [
        "claude", "codex", "cursor", "grok", "agents", "beads", "omc", "omx",
    ] {
        assert!(src.contains(dir), "pre-commit does not mention '{dir}'");
    }
}
