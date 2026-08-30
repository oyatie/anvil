//! The review pipeline stamps `last_reviewed_head_sha` early and then skips
//! every later delivery for that SHA. So an exit taken *after* the stamp and
//! *before* the pipeline finishes does not retry the pull request -- it stops
//! it, permanently, until a new commit lands. Nothing tells the operator.
//!
//! Three such exits existed. The certification-failure arm rolled the stamp
//! back; the other two did not:
//!
//! * a bare `?` on the scorecard comment -- a rate-limited forge, after every
//!   gate had run and passed, stranded the pull request;
//! * the pause read before enlistment, added with the pause itself, which made
//!   a switch the operator believes is temporary into a one-way door.
//!
//! Fixing the two instances leaves the class open, so this is keyed to the
//! shape rather than to those two sites: after the stamp, no `?`, and every
//! exit -- `return`, `bail!`, `ensure!` -- rolls the stamp back.
//!
//! What it does NOT cover, stated rather than implied: the scan is bounded by
//! `execute_pr_review`'s own body, so post-stamp work moved into a helper
//! leaves its exits unseen. That boundary is real and this file cannot close
//! it; a reviewer moving fallible work out of this function is moving it out
//! of this guard.
//!
//! What `clear_reviewed_sha` itself does is measured next to it, by
//! `state::tests::clearing_the_reviewed_sha_allows_the_pr_to_be_retried`; this
//! file only asserts that the pipeline reaches it.

use anvil::source_scan::code_only;

/// `execute_pr_review`'s body, with commentary and string literals blanked out
/// so a scan cannot be satisfied by prose. Offsets are preserved by
/// `code_only`, so the returned string indexes the same as the file.
fn the_review_pipeline() -> String {
    let src = anvil::source_scan::paths::module_source(
        "src/webhook/pipelines/review",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    let code = code_only(&src);

    let start = code
        .find("pub async fn execute_pr_review")
        .expect("the review pipeline is still named execute_pr_review");
    // The next brace in the first column closes it: this file's items are all
    // top level, and `code_only` has already removed anything in a comment or
    // a literal that could look like one.
    let end = code[start..]
        .find("\n}")
        .map(|at| start + at)
        .expect("execute_pr_review is closed");

    // Blank the rest rather than slice it, so every offset below is a real
    // file offset and the failure messages can be read against the file.
    let mut body = " ".repeat(start);
    body.push_str(&code[start..end]);
    body
}

/// Where the stamp is set. Everything after this point is inside the window
/// where an exit strands the pull request.
fn the_stamp(body: &str) -> usize {
    body.find(".update_pr_state(")
        .expect("the pipeline still stamps the reviewed SHA; if that moved, this test must follow")
}

/// `?` is the exit that is easiest to write and hardest to see. After the
/// stamp it is never correct: the call it propagates from may fail for a
/// reason that has nothing to do with the pull request -- a rate limit, a
/// dropped connection -- and the cost of that is the pull request never being
/// looked at again.
#[test]
fn nothing_after_the_stamp_exits_through_a_question_mark() {
    let body = the_review_pipeline();
    let from = the_stamp(&body);

    // The stamp's own `?` is the one exception and it is not one: if
    // `update_pr_state` fails there is no stamp to roll back. Start after the
    // statement it belongs to.
    let stamp_stmt_end = body[from..]
        .find(";")
        .map(|at| from + at)
        .expect("the stamp is a statement");

    let strays: Vec<String> = body[stamp_stmt_end..]
        .match_indices('?')
        .map(|(at, _)| {
            let at = stamp_stmt_end + at;
            let line = body[..at].matches('\n').count() + 1;
            format!("src/webhook/pipelines/review.rs:{line}")
        })
        .collect();

    assert!(
        strays.is_empty(),
        "{} exit(s) after the reviewed-SHA stamp propagate with `?`: {}.\n\
         Each one strands the pull request -- the stamp is already set, and the \
         early-exit guard at the top of this function skips every later webhook \
         for this SHA, so the failure costs the whole review rather than a \
         retry. Handle the error and call `clear_reviewed_sha` before returning.",
        strays.len(),
        strays.join(", ")
    );
}

/// And the explicit exits roll it back. Walked in order, because a single
/// rollback anywhere in the window would otherwise vouch for every `return`
/// after it -- which is exactly how the pause's `return Ok(())` was written
/// next to a rollback that belonged to a different arm.
#[test]
fn every_return_after_the_stamp_rolls_it_back() {
    let body = the_review_pipeline();
    let mut checkpoint = the_stamp(&body);

    // Every way out, not just `return ` with a space after it. A bare
    // `return;` and the `bail!`/`ensure!` macros are exits that leave the stamp
    // set exactly as a `return Err(e)` does, and matching only `"return "`
    // meant a one-word edit restored the bug this file exists to prevent.
    let mut exits: Vec<usize> = Vec::new();
    for pat in ["return", "bail!", "ensure!"] {
        for (at, _) in body.match_indices(pat) {
            let before = body[..at].chars().last();
            if pat == "return"
                && (before.is_some_and(|c| c.is_alphanumeric() || c == '_')
                    || body[at + pat.len()..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_alphanumeric() || c == '_'))
            {
                continue; // part of a longer identifier
            }
            exits.push(at);
        }
    }
    exits.sort_unstable();

    let mut unrolled = Vec::new();
    for at in exits {
        if at < checkpoint {
            continue;
        }
        if !body[checkpoint..at].contains("clear_reviewed_sha") {
            let line = body[..at].matches('\n').count() + 1;
            let stmt: String = body[at..].chars().take(40).collect();
            unrolled.push(format!(
                "src/webhook/pipelines/review.rs:{line} `{}`",
                stmt.split(';').next().unwrap_or("").trim()
            ));
        }
        checkpoint = at;
    }

    assert!(
        unrolled.is_empty(),
        "{} exit(s) after the reviewed-SHA stamp leave it set: {}.\n\
         The pull request is not retried after them; it stops. Call \
         `state.state_mgr.clear_reviewed_sha(repo, pr_number).await` first, or \
         move the exit above the stamp.",
        unrolled.len(),
        unrolled.join(", ")
    );
}
