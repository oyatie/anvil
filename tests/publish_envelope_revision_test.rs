//! Everything anvil publishes states which revision it judged.
//!
//! The envelope is the deterministic half of a published artifact: the marker,
//! the action, the separator and the revision are mechanical, and only the
//! finding text is written by a model. The revision was the missing piece --
//! a review comment with no anchor cannot be told apart from one describing a
//! head that a force-push has replaced, and the reader has no way to know
//! which they are looking at.
//!
//! `Judged` is an enum rather than an `Option<&str>` so a call site must
//! DECIDE. `NotRevisionScoped` has to be written down, which makes it a claim
//! someone made rather than a default someone fell into.

use anvil::publish::{AnvilAction, Judged, body, is_signed, issue};

const SHA: &str = "6746459418c2f0d1a7b3e5c9d2f4a6b8c0e1d3f5";

#[test]
fn a_revision_scoped_body_carries_the_short_sha() {
    let b = body(AnvilAction::Reviewed, "a finding", Judged::Rev(SHA.into()));
    assert!(b.as_str().contains("674645941"), "no anchor in:\n{b}");
    assert!(
        !b.as_str().contains(SHA),
        "the full forty characters are noise; twelve is what GitHub shows"
    );
}

#[test]
fn the_anchor_sits_with_the_signature_not_in_the_content() {
    let b = body(AnvilAction::Certified, "content", Judged::Rev(SHA.into()));
    let (before, after) = b.as_str().split_once("---").expect("separator");
    assert!(!before.contains("674645941"), "anchor leaked into content");
    assert!(
        after.contains("674645941"),
        "anchor must close the artifact"
    );
}

#[test]
fn a_body_that_is_not_about_a_commit_carries_no_anchor() {
    let b = body(
        AnvilAction::Healed,
        "the queue was stuck",
        Judged::NotRevisionScoped,
    );
    assert!(
        !b.as_str().contains('`'),
        "no anchor should be rendered:\n{b}"
    );
}

#[test]
fn the_envelope_is_still_signed_and_marked_either_way() {
    for j in [Judged::Rev(SHA.into()), Judged::NotRevisionScoped] {
        let b = body(AnvilAction::Reviewed, "x", j);
        assert!(is_signed(b.as_str()), "signature lost:\n{b}");
        assert!(b.as_str().starts_with("<!--"), "marker must lead:\n{b}");
    }
}

#[test]
fn an_issue_is_anchored_the_same_way_as_a_comment() {
    let i = issue(
        AnvilAction::Triaged,
        "trunk red",
        "ci",
        "cargo test failed",
        None,
        Judged::Rev(SHA.into()),
    );
    assert!(i.body.contains("674645941"), "{}", i.body);
    assert!(i.title.starts_with("[anvil] "), "{}", i.title);
}

#[test]
fn a_short_or_ragged_sha_does_not_panic_or_overrun() {
    // Whatever a caller hands over is rendered without indexing past its end:
    // a panic in the publisher would lose the finding entirely.
    for s in ["", "abc", "  6746459418c2  "] {
        let b = body(AnvilAction::Fixed, "x", Judged::Rev(s.to_string()));
        assert!(is_signed(b.as_str()));
    }
}
