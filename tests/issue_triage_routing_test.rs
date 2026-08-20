//! Issue-fate routing: the chain must stay cheap, diverse, and bounded.
//!
//! These assert the properties that make the stage correct, not the specific
//! model names -- a model can be swapped, but if the swap makes triage
//! expensive, slow, or single-vendor, that is a regression these catch.

use anvil::ai_driver::stage_router::{
    AgenticStage, EnterpriseAgenticPipelineRouter as StageRouter,
};

#[test]
fn issue_triage_has_a_routing_chain_at_all() {
    let chain = StageRouter::get_stage_fallback_chain(AgenticStage::IssueTriage);
    assert!(
        !chain.tiers.is_empty(),
        "issue fate decisions must route somewhere; an empty chain means triage \
         silently does nothing"
    );
}

#[test]
fn issue_triage_never_pays_for_deep_reasoning() {
    let chain = StageRouter::get_stage_fallback_chain(AgenticStage::IssueTriage);
    for tier in &chain.tiers {
        assert_eq!(
            tier.reasoning_effort, "low",
            "issue fate is classification, not repair -- tier {:?} asks for '{}' effort \
             and would spend reasoning budget per issue at volume",
            tier.specific_model, tier.reasoning_effort
        );
    }
}

#[test]
fn issue_triage_stays_bounded_enough_to_be_cheap() {
    let chain = StageRouter::get_stage_fallback_chain(AgenticStage::IssueTriage);
    for tier in &chain.tiers {
        assert!(
            tier.print_timeout_secs <= 90,
            "a triage call allowed {}s has stopped being cheap; it should fall through \
             to the next tier rather than block the sweep",
            tier.print_timeout_secs
        );
    }
}

#[test]
fn issue_triage_survives_one_provider_going_down() {
    let chain = StageRouter::get_stage_fallback_chain(AgenticStage::IssueTriage);
    let mut providers: Vec<String> = chain
        .tiers
        .iter()
        .map(|t| format!("{:?}", t.provider))
        .collect();
    providers.sort();
    providers.dedup();
    assert!(
        providers.len() >= 2,
        "every tier resolves to the same provider, so one outage or one exhausted \
         quota takes out the entire chain"
    );
}
