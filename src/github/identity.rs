//! Which account Anvil publishes as.
//!
//! Anvil submits its review through `gh`, so its own inline comments are
//! delivered straight back to the `pull_request_review_comment` door -- and
//! that door spawns the fixer: a clone, a model turn, and a push to the
//! contributor's branch, which can post more comments.
//!
//! The filter standing between that and an unbounded loop was
//! `author.contains("bot") || author.contains("antigravity")`. Anvil's login
//! carries neither marker. It had never been wrong because it had never been
//! asked: until the reviewer was given the diff its comments are anchored in,
//! every proposed comment was dropped and there were none to answer.
//!
//! A substring is not an identity. This tree deleted that same defect from the
//! thread resolver, and here the wrong answer is worse -- a loop that pushes
//! to somebody else's branch rather than a refusal that is merely missed.

use crate::exec::{ExecClass, run_bounded};
use tokio::process::Command;

/// The login, or `None` if it could not be established.
///
/// Cached for the life of the process: the answer cannot change under it.
/// `None` is not "not mine" -- callers must fail closed on it, because an
/// unanswerable identity is exactly when the loop would start.
pub async fn authenticated_login() -> Option<String> {
    static LOGIN: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    if let Some(cached) = LOGIN.get() {
        return cached.clone();
    }
    let mut cmd = Command::new("gh");
    cmd.args(["api", "user", "--jq", ".login"]);
    let answer = match run_bounded(cmd, ExecClass::Api, "gh api user").await {
        Ok(out) if out.status.success() => {
            let login = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if login.is_empty() { None } else { Some(login) }
        }
        _ => None,
    };
    let _ = LOGIN.set(answer.clone());
    answer
}

/// Whether a comment by `author` is one Anvil answers.
///
/// False for Anvil's own, false for bots, and false when the login could not
/// be established -- that last one is the fail-closed direction, because an
/// unanswerable identity is exactly when the loop would start. A missed fix is
/// recoverable; a push loop is not.
pub async fn answerable(author: &str) -> bool {
    match authenticated_login().await {
        None => false,
        Some(me) => me != author && !author.contains("bot") && !author.contains("antigravity"),
    }
}
