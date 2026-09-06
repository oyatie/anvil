//! Source bindings for evidence that authorizes promotion-adjacent operations.
//! These tests read source only; no forge, command fixture, or daemon runs.

fn source(module: &str) -> String {
    // `/mod` selects the declared owner only, excluding descendants and test fixtures.
    anvil::source_scan::paths::module_source(
        module,
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    )
}

#[test]
fn formal_submission_consumes_a_recorded_receipt_not_comment_success() {
    let reviews = source("src/github/reviews/mod");
    let formal = reviews
        .split("pub async fn submit_pr_review_impl(")
        .nth(1)
        .expect("formal implementation")
        .split("pub async fn submit_pr_review_with_diff(")
        .next()
        .unwrap();
    assert!(
        formal.contains("require_recorded("),
        "formal path needs a recorded receipt"
    );
    assert!(
        reviews.contains("RecordedReview::from_response("),
        "receipt must read the actual POST response"
    );
    assert!(reviews.contains("&output.stdout"));
    assert!(reviews.contains("ReviewPublication::SummaryCommentOnly"));
}

#[test]
fn approval_requires_the_recorded_state_and_exact_head() {
    let enlister = source("src/merge_enlister/mod");
    let approval = enlister
        .split("pub async fn ensure_approving_review(")
        .nth(1)
        .expect("approval owner")
        .split("pub async fn reconcile_pr_title_and_scope(")
        .next()
        .unwrap();
    assert!(approval.contains(".require_approved_for(&meta.head_ref_oid)"));
    assert!(approval.contains(".submit_pr_review("));
}

#[test]
fn disarm_classifies_completed_failures_and_reports_the_outcome() {
    let disarm = source("src/merge_enlister/disarm/mod");
    assert!(disarm.contains("Disarmed::from_completion("));
    assert!(!disarm.contains("Disarmed::NothingArmed"));
    let caller = source("src/webhook/pipelines/review/mod");
    let between = caller
        .split("let phase = crate::webhook::next_phase::next_phase(&situation);")
        .nth(1)
        .expect("phase decision")
        .split("let mut enlisted = false;")
        .next()
        .unwrap();
    assert!(between.contains("if let Some(outcome)"));
    assert!(between.contains("outcome.report(repo, pr_number)"));
}

#[test]
fn baseline_acquisition_is_fallible_before_reseed_and_write() {
    let shape = source("src/cli/handlers/shape/mod");
    let baseline = shape
        .split("ShapeAction::Baseline {")
        .nth(1)
        .expect("baseline command")
        .split("ShapeAction::Plan {")
        .next()
        .unwrap();
    assert!(
        !baseline.contains(".ok()"),
        "read and parse failures must remain failures"
    );
    let load = baseline
        .find("baseline_inputs::load(")
        .expect("real loader call");
    let reseed = baseline.find("reseed_from_commit(").unwrap();
    let write = baseline.find("tokio::fs::write(").unwrap();
    assert!(load < reseed && reseed < write);
    assert!(baseline[..reseed].contains(".await?"));
}
