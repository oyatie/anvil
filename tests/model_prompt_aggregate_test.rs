//! Aggregate model-prompt budget regressions.
//!
//! Individual contributor channels are bounded independently. These checks
//! prove that repetition cannot defeat those caps and that overflow rejects
//! the whole prompt rather than publishing a truncated task or schema.

use anvil::fixer::engine::build_apply_prompt;
use anvil::fixer::evaluator::{
    ItemEvaluation, ReviewFeedbackItem, build_feedback_evaluation_prompt,
};
use anvil::model_prompt::{MAX_MODEL_PROMPT_BYTES, ModelPrompt, ModelPromptPurpose};

fn feedback(index: usize) -> ReviewFeedbackItem {
    ReviewFeedbackItem {
        comment_id: Some(index as u64),
        file_path: Some(format!("src/{}.rs", "p".repeat(4_000))),
        line: Some(index as u64),
        body: "review-body".repeat(1_000),
        author: "outside-author".repeat(200),
    }
}

fn evaluation(index: usize) -> ItemEvaluation {
    ItemEvaluation {
        item_index: index,
        is_valid: true,
        rationale: "valid".into(),
        files_to_edit: vec!["src/lib.rs".into()],
        proposed_fix: Some("proposed-fix".repeat(1_000)),
    }
}

#[test]
fn empty_prompt_is_rejected() {
    let error = match ModelPrompt::builder().finish() {
        Ok(_) => panic!("an empty prompt reached a model sink"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("must not be empty"));
}

#[test]
fn metadata_only_prompt_is_rejected_without_a_trusted_terminal_task() {
    let mut builder = ModelPrompt::builder();
    builder.push_u64(7);
    let error = match builder.finish() {
        Ok(_) => panic!("metadata-only prompt reached a model sink"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("trusted terminal task"));
}

#[test]
fn exact_aggregate_boundary_includes_the_terminal_task() {
    let mut baseline = ModelPrompt::builder();
    baseline.push_u64(7);
    let baseline = baseline
        .finish_for(ModelPromptPurpose::SubscriptionProbe)
        .expect("probe task fits");
    let terminal_bytes = baseline.len() - 1;
    let data_bytes = MAX_MODEL_PROMPT_BYTES - terminal_bytes;

    let mut exact = ModelPrompt::builder();
    for _ in 0..data_bytes {
        exact.push_u64(7);
    }
    let prompt = exact
        .finish_for(ModelPromptPurpose::SubscriptionProbe)
        .expect("exact byte ceiling including terminal task is valid");
    assert_eq!(prompt.len(), MAX_MODEL_PROMPT_BYTES);

    let mut over = ModelPrompt::builder();
    for _ in 0..=data_bytes {
        over.push_u64(7);
    }
    let error = match over.finish_for(ModelPromptPurpose::SubscriptionProbe) {
        Ok(_) => panic!("an over-bound prompt was accepted"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("aggregate rendered-byte ceiling")
    );
}

#[test]
fn repeated_evaluator_feedback_fails_closed_at_the_aggregate_bound() {
    let one = feedback(0);
    let prompt =
        build_feedback_evaluation_prompt("oyatie/console", &[one]).expect("one feedback item fits");
    assert!(prompt.len() <= MAX_MODEL_PROMPT_BYTES);

    let many: Vec<_> = (0..100).map(feedback).collect();
    let error = match build_feedback_evaluation_prompt("oyatie/console", &many) {
        Ok(_) => panic!("repeated feedback bypassed the aggregate prompt cap"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("aggregate rendered-byte ceiling")
    );
}

#[test]
fn repeated_fix_items_cannot_truncate_the_final_apply_task() {
    let one = vec![(feedback(0), evaluation(0))];
    let prompt = build_apply_prompt("oyatie/console", &one).expect("one fix item fits");
    assert!(prompt.len() <= MAX_MODEL_PROMPT_BYTES);

    let many: Vec<_> = (0..100)
        .map(|index| (feedback(index), evaluation(index)))
        .collect();
    let error = match build_apply_prompt("oyatie/console", &many) {
        Ok(_) => panic!("repeated fix items yielded a partial apply prompt"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("aggregate rendered-byte ceiling")
    );
}
