//! Anvil's own inline comments must not reopen the door that answers them.
//!
//! This branch is what makes the question live. Before it,
//! `submit_pr_review_impl` passed an empty diff to
//! `validate_comments_against_diff`, so every proposed inline comment was
//! dropped and Anvil posted none. The `pull_request_review_comment` door's
//! self-authorship test was never asked anything it could get wrong.
//!
//! Now the comments are real. Each one is delivered back as
//! `pull_request_review_comment`/`created`, and that door spawns
//! `fixer::resolve_and_fix`: a clone, a model turn, and a push to the
//! contributor's branch -- which can post more comments. The filter standing
//! between that and an unbounded loop was
//! `author.contains("bot") || author.contains("antigravity")`, and Anvil
//! publishes as the account `gh` is authenticated as, whose login carries
//! neither marker.
//!
//! A substring is not an identity. That is the defect this tree deleted from
//! the thread resolver, and it is worse here: the wrong answer is a loop that
//! pushes, not a refusal that is merely missed.

use anvil::source_scan::without_commentary;

fn door() -> String {
    let raw = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/webhook/webhook_handlers.rs"),
    )
    .expect("the webhook handlers exist");
    // `without_commentary`, not `code_only`: the door is located by its event
    // string, and `code_only` blanks literals. Commentary is what must go --
    // the prose beside this door names `authenticated_login`.
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

/// The decision is an equality against the account Anvil publishes as.
#[test]
fn the_comment_door_decides_authorship_by_identity() {
    let d = door();
    assert!(
        d.contains("answerable("),
        "the comment door does not ask whether the comment is one Anvil \
         answers, so it cannot know which comments are its own. Every comment \
         it posts then spawns a clone, a model turn and a push that can post \
         more."
    );

    let decision = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/github/identity.rs"),
    )
    .expect("the identity module exists");
    let decision = without_commentary(&decision);
    assert!(
        decision.contains("authenticated_login()") && decision.contains("me != author"),
        "`answerable` does not compare the author against the account Anvil \
         publishes as, by equality"
    );
}

/// And it fails closed. An unknown identity must not run the fixer: a missed
/// fix is recoverable, a loop that pushes to somebody's branch is not.
#[test]
fn an_unestablished_identity_does_not_run_the_fixer() {
    let decision = without_commentary(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/github/identity.rs"),
        )
        .expect("the identity module exists"),
    );
    assert!(
        decision.contains("None => false"),
        "`answerable` does not handle the case where Anvil's own login could \
         not be established. Treating that as 'not mine' is what turns a \
         rate-limited API call into an unbounded push loop."
    );
}
