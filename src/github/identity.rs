//! Which account Anvil publishes as, and which accounts it answers.
//!
//! Anvil submits its review through `gh`, so its own inline comments are
//! delivered straight back to the `pull_request_review_comment` door -- and
//! that door spawns the fixer: a clone, a model turn, and a push to the
//! contributor's branch, which can post more comments.
//!
//! The filter standing between that and an unbounded loop reads the identity
//! GitHub already sends. Every comment payload carries `user.id`, a stable
//! numeric actor id, and `user.type`, one of "User", "Bot" or "Organization".
//! A login that happens to contain the letters "bot" is a proxy for that type,
//! and a proxy answers a different question: `abbott` is a person and
//! `dependabot[bot]` is not, and no substring separates them.
//!
//! # This module does not close issue #171
//!
//! Anvil authenticates as `jason931225`, the same human account that reviews
//! it. `me != author.login` is therefore false for the reviewer's own comments
//! as well as for Anvil's, so the fixer still never runs on them. No predicate
//! over one shared login can separate the two. Only a GitHub App installation
//! gives Anvil its own principal, and creating one is the operator's action,
//! not a code change. What lands here is the typed half: the account type is
//! read from the payload, and so is the id the App will make exact.

use crate::exec::{ExecClass, run_bounded};

/// A GitHub actor, as the payload names it.
///
/// `id` is the stable numeric actor id: it survives a rename, which a login
/// does not, and it is what becomes an exact test once Anvil has a principal
/// of its own. `kind` is the payload's `type` field.
///
/// Both are optional because GitHub does not promise either on every payload
/// shape, and a delivery that omits one must still parse. Dropping the whole
/// comment over a missing field loses the comment as well as the field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub login: String,
    pub id: Option<u64>,
    pub kind: Option<String>,
}

/// The `type` of an account a person holds. GitHub's other values are `Bot`
/// and `Organization`; neither leaves comments a fixer should answer.
pub const HUMAN_ACTOR_TYPE: &str = "User";

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
    let mut cmd = crate::exec::gh();
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

/// The decision, with both identities supplied. Pure, so it is testable.
///
/// Two terms, and every unknown refuses:
///
/// - `me != author.login`: Anvil does not answer itself. `me` of `None` -- the
///   account could not be established -- is not answerable, because an
///   unanswerable identity is exactly when the loop would start.
/// - The author's `kind` must be exactly [`HUMAN_ACTOR_TYPE`]. Stated as an
///   equality rather than `!= "Bot"` so that a MISSING type refuses too. An
///   absent type must never read as "not a bot" (invariant I1), and neither
///   `Organization` nor any type GitHub adds later reads as one either.
///
/// An `author` of `None` -- a payload carrying no user at all -- is not
/// answerable for the same reason. A missed fix is recoverable; a push loop to
/// somebody else's branch is not.
pub fn answerable_by(me: Option<&str>, author: Option<&Actor>) -> bool {
    match (me, author) {
        (Some(me), Some(author)) => {
            me != author.login && author.kind.as_deref() == Some(HUMAN_ACTOR_TYPE)
        }
        _ => false,
    }
}

/// Whether a comment by `author` is one Anvil answers.
///
/// See [`answerable_by`] for the decision and for what each unknown does.
pub async fn answerable(author: Option<&Actor>) -> bool {
    answerable_by(authenticated_login().await.as_deref(), author)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(login: &str, kind: Option<&str>) -> Actor {
        Actor {
            login: login.to_string(),
            id: Some(1),
            kind: kind.map(str::to_string),
        }
    }

    #[test]
    fn a_bot_typed_author_is_not_answered() {
        let bot = actor("dependabot[bot]", Some("Bot"));
        assert!(
            !answerable_by(Some("anvil"), Some(&bot)),
            "a Bot-typed actor is answerable, so the fixer clones, runs a model \
             turn and pushes in response to a machine's comment -- which can \
             post another"
        );
    }

    #[test]
    fn a_human_whose_login_contains_bot_is_answered() {
        let human = actor("abbott", Some("User"));
        assert!(
            answerable_by(Some("anvil"), Some(&human)),
            "a User-typed actor whose login merely contains \"bot\" is refused. \
             That is the substring answering a question about identity, and it \
             drops every review comment `abbott` leaves"
        );
    }

    #[test]
    fn an_author_with_no_type_is_not_answered() {
        let untyped = actor("abbott", None);
        assert!(
            !answerable_by(Some("anvil"), Some(&untyped)),
            "an actor carrying no type is answered. Absent evidence read as a \
             pass is invariant I1's failing direction, and here it admits every \
             bot on a payload shape that omits the field"
        );
    }

    #[test]
    fn an_organization_is_not_answered() {
        let org = actor("oyatie", Some("Organization"));
        assert!(!answerable_by(Some("anvil"), Some(&org)));
    }

    #[test]
    fn anvil_does_not_answer_itself() {
        let me = actor("anvil", Some("User"));
        assert!(!answerable_by(Some("anvil"), Some(&me)));
    }

    #[test]
    fn an_unestablished_identity_answers_nothing() {
        let human = actor("abbott", Some("User"));
        assert!(
            !answerable_by(None, Some(&human)),
            "with its own account unknown, Anvil cannot tell its comments from \
             anyone's, and answering is what starts the loop"
        );
    }

    #[test]
    fn an_absent_author_is_not_answered() {
        assert!(!answerable_by(Some("anvil"), None));
    }
}
