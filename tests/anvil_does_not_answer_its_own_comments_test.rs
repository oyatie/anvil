//! Anvil's own inline comments must not reopen the door that answers them.
//!
//! Each comment Anvil posts is delivered back as
//! `pull_request_review_comment`/`created`, and that door spawns
//! `fixer::resolve_and_fix`: a clone, a model turn, and a push to the
//! contributor's branch -- which can post more comments. The filter standing
//! between that and an unbounded loop is `github::identity::answerable`.
//!
//! It decides on the identity GitHub sends, not on the spelling of a login.
//! A substring over the login answers a different question: `abbott` is a
//! person and `dependabot[bot]` is not, and no substring separates them. The
//! wrong answer here is a loop that pushes, not a refusal that is merely
//! missed, so both directions are asserted.

use anvil::github::identity::{Actor, answerable_by};
use anvil::source_scan::{paths::module_source, without_commentary};

fn repo_root() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn door() -> String {
    let raw = module_source("src/webhook/webhook_handlers", repo_root());
    // `without_commentary`, not `code_only`: the door is located by its event
    // string, and `code_only` blanks literals. Commentary is what must go --
    // the prose beside this door names `identity`.
    let code = without_commentary(&raw);
    let at = code
        .find("pull_request_review_comment")
        .expect("the comment door still exists; if it moved, this test must follow it");
    let spawn = code[at..]
        .find("tokio::spawn(")
        .map(|s| at + s)
        .expect("the door still detaches the fixer");
    code[at..spawn].to_string()
}

fn actor(login: &str, kind: Option<&str>) -> Actor {
    Actor {
        login: login.to_string(),
        id: Some(7),
        kind: kind.map(str::to_string),
    }
}

/// The door asks. A door that decides authorship itself is a second copy of
/// the rule, and the copies drift.
#[test]
fn the_comment_door_asks_whether_the_comment_is_one_anvil_answers() {
    assert!(
        door().contains("answerable("),
        "the comment door does not ask whether the comment is one Anvil \
         answers, so it cannot know which comments are its own. Every comment \
         it posts then spawns a clone, a model turn and a push that can post \
         more."
    );
}

/// The decision is an equality against the account Anvil publishes as.
#[test]
fn anvil_does_not_answer_the_account_it_publishes_as() {
    assert!(
        !answerable_by(Some("anvil"), Some(&actor("anvil", Some("User")))),
        "`answerable_by` answers a comment authored by the very account Anvil \
         publishes as. That is the loop."
    );
    assert!(answerable_by(
        Some("anvil"),
        Some(&actor("someone-else", Some("User")))
    ));
}

/// A machine's comment is refused by its type, and a person's is not refused
/// by the letters in their login. The substring got exactly this pair wrong.
#[test]
fn the_decision_reads_the_type_and_not_the_spelling_of_the_login() {
    assert!(
        !answerable_by(Some("anvil"), Some(&actor("dependabot[bot]", Some("Bot")))),
        "a Bot-typed actor is answered, so a machine's comment spawns a clone, \
         a model turn and a push that can post another"
    );
    assert!(
        answerable_by(Some("anvil"), Some(&actor("abbott", Some("User")))),
        "a person whose login merely contains \"bot\" is refused, so every \
         review comment they leave is dropped"
    );
}

/// And every unknown refuses. An identity that could not be established, an
/// actor with no type, and a payload with no actor at all are all the
/// fail-closed direction: a missed fix is recoverable, a push loop is not.
#[test]
fn an_unknown_identity_does_not_run_the_fixer() {
    let human = actor("abbott", Some("User"));
    assert!(
        !answerable_by(None, Some(&human)),
        "`answerable_by` answers when Anvil's own login could not be \
         established. Treating that as 'not mine' is what turns a rate-limited \
         API call into an unbounded push loop."
    );
    assert!(
        !answerable_by(Some("anvil"), Some(&actor("abbott", None))),
        "an actor carrying no type is answered. A missing type must never read \
         as 'not a bot' -- absent evidence read as a pass is invariant I1's \
         failing direction."
    );
    assert!(
        !answerable_by(Some("anvil"), None),
        "a comment with no user at all is answered"
    );
}

/// The substring is gone from the module, not merely unused by it. A second
/// term reintroduced beside the typed one would refuse `abbott` again.
#[test]
fn no_substring_over_the_login_survives_in_the_decision() {
    let decision = without_commentary(&module_source("src/github/identity", repo_root()));
    for proxy in ["contains(\"bot\")", "contains(\"antigravity\")"] {
        assert!(
            !decision.contains(proxy),
            "`{proxy}` is back in `github::identity`. A substring over a login \
             is a proxy for the account type GitHub already sends, and it is \
             wrong in both directions: it admits a bot whose login hides it, \
             and it drops every person whose name contains the letters."
        );
    }
}
