//! The reviewer's findings reach the lines they are about.
//!
//! `submit_pr_review_impl` hardcoded the diff to `""`. Every inline comment was
//! therefore unanchorable, every one was dropped, and the review was submitted
//! and reported as submitted with its findings flattened into the body. The one
//! reviewer this repository has posted zero inline comments on every pull
//! request it ever reviewed.
//!
//! Two halves are needed and only one of them is the plumbing. The diff must be
//! threaded from `PrDiffContext::diff_content`, and the door that has no diff
//! must refuse a review that carries comments — otherwise the next caller
//! bypasses validation the same way, by omission.

use anvil::github::GitHubClient;
use anvil::github::reviews::validate_comments_against_diff;
use anvil::reviewer::{InlineReviewComment, ReviewResponse};

const DIFF: &str = "\
diff --git a/src/main.rs b/src/main.rs
index 1111111..2222222 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
+    let x = compute().unwrap();
     println!(\"hi\");
 }
";

fn comment(path: &str, line: u64) -> InlineReviewComment {
    InlineReviewComment {
        path: path.to_string(),
        line,
        side: "RIGHT".to_string(),
        body: "this unwrap can panic".to_string(),
    }
}

/// The measurement that says the old behaviour was total: against an empty
/// diff, nothing is addressable, whatever the comment says.
#[test]
fn an_empty_diff_anchors_nothing() {
    let v = validate_comments_against_diff("", &[comment("src/main.rs", 2)]);
    assert!(
        v.kept.is_empty(),
        "with no diff there is nothing to anchor to, so every comment is \
         dropped -- which is what hardcoding the diff to \"\" did to all of them"
    );
    assert_eq!(v.dropped.len(), 1);
}

/// And the twin: given the diff the review was formed from, the same comment is
/// addressable. Both halves, so the check cannot be satisfied by a validator
/// that drops everything.
#[test]
fn a_comment_on_an_added_line_is_kept_when_the_diff_is_supplied() {
    let v = validate_comments_against_diff(DIFF, &[comment("src/main.rs", 2)]);
    assert_eq!(
        v.kept.len(),
        1,
        "the added line is addressable in this diff; dropped: {:?}",
        v.dropped
    );
}

/// The bypass by omission, refused. A caller who has findings and no diff gets
/// an error naming the door it should have used, rather than a successful
/// submission with the findings quietly gone.
#[tokio::test]
async fn the_diffless_door_refuses_a_review_that_carries_comments() {
    let client = GitHubClient::new();
    let review = ReviewResponse {
        summary: "one finding".to_string(),
        verdict: "COMMENT".to_string(),
        comments: vec![comment("src/main.rs", 2)],
    };

    let err = client
        .submit_pr_review("oyatie/anvil", 1, "deadbeef", &review)
        .await
        .expect_err("a review with comments and no diff must not be submitted");
    let msg = err.to_string();
    assert!(
        msg.contains("submit_pr_review_with_diff"),
        "the refusal must name the door that works: {msg}"
    );
    assert!(
        msg.contains("inline comment"),
        "and say what would have been lost: {msg}"
    );
}

/// The pipeline supplies the diff it reviewed. Keyed to the call rather than to
/// a line, and loud if the call is no longer there to find.
#[test]
fn the_review_pipeline_submits_with_the_diff_it_reviewed() {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/webhook/pipelines/review.rs"),
    )
    .expect("the review pipeline exists");

    let at = src.find("submit_pr_review").unwrap_or_else(|| {
        panic!(
            "the review pipeline no longer submits a review. If that moved, \
             this test must follow it -- a scan that stops finding its subject \
             is not a fix."
        )
    });
    let call: String = src[at..].chars().take(240).collect();
    assert!(
        call.starts_with("submit_pr_review_with_diff"),
        "the pipeline submits through the door that anchors nothing:\n  {}",
        call.lines().take(3).collect::<Vec<_>>().join(" ")
    );
    assert!(
        call.contains("diff_content"),
        "and it must hand over the diff the review was formed from, not some \
         other diff:\n  {}",
        call.lines().take(8).collect::<Vec<_>>().join(" ")
    );
}
