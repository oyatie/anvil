//! A supervisor must outlive the thing it supervises.
//!
//! The doc-parity probe told `agy --print-timeout` it had 120 seconds and put a
//! watchdog around it with a hardcoded budget of 30. The watchdog killed a
//! healthy call at 30 seconds and the gate published `Errored`, which blocks
//! merge-queue admission -- so pull requests in this repository were refused by
//! a supervisor that had truncated its own subject and then reported the
//! failure it caused.
//!
//! `agy_print_timeout_arg` already encodes the intended relationship: it
//! subtracts a margin so the inner deadline lands INSIDE the outer one, and its
//! own documentation says every agy spawn passes it "so the two deadlines agree
//! and the default never silently wins". This is that agreement, asserted.

use anvil::exec::{ExecClass, SupervisedTurn, agy_print_timeout_arg};
use std::time::Duration;

fn secs(arg: &str) -> u64 {
    arg.trim_end_matches('s')
        .parse()
        .unwrap_or_else(|_| panic!("agy timeout arg is `<n>s`, got {arg:?}"))
}

#[test]
fn the_inner_deadline_lands_inside_the_outer_one() {
    // The property, at every budget a call site might choose.
    for outer in [
        Duration::from_secs(30),
        Duration::from_secs(120),
        ExecClass::Model.timeout(),
    ] {
        let inner = secs(&agy_print_timeout_arg(outer));
        assert!(
            inner < outer.as_secs(),
            "an inner deadline of {inner}s does not fit inside an outer budget of {}s; the \
             tool would still be working when its supervisor kills it",
            outer.as_secs()
        );
    }
}

#[test]
fn one_value_yields_every_deadline_for_a_supervised_turn() {
    // Stronger than "the same constant is used twice", which is what this
    // asserted while the two were related only by convention. A supervised turn
    // has THREE deadlines -- the watchdog around the call, the process bound,
    // and the tool's own `--print-timeout` -- and they were three numbers in
    // three places with nothing relating them. `SupervisedTurn` is one value
    // that yields all three, so none can be tightened without the others.
    let turn = SupervisedTurn::bounded_at(Duration::from_secs(120));
    assert_eq!(turn.supervisor(), Duration::from_secs(120));
    let inner = secs(&turn.tool_arg());
    assert!(
        inner < turn.supervisor().as_secs(),
        "the tool's own deadline ({inner}s) does not fit inside its supervisor's \
         ({}s), so the supervisor would kill a turn that was still working",
        turn.supervisor().as_secs()
    );
}

#[test]
fn the_doc_parity_probe_takes_all_three_deadlines_from_that_one_value() {
    // Source-level, because the defect was numbers in separate places that
    // could not disagree within any single value a unit test could inspect.
    let src = anvil::source_scan::paths::module_source(
        "src/doc_guard",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    let code = anvil::source_scan::without_commentary(&src);

    assert!(
        code.contains("SupervisedTurn::bounded_at"),
        "the probe's budget is not a SupervisedTurn, so its deadlines can drift again"
    );
    // Two, not three, and the drop is the point. The tool's own deadline was
    // spelled here as a third `DOC_PARITY_PROBE.supervisor()` handed to
    // `agy_agent`. This site no longer picks a provider at all -- it names a
    // stage -- so that argument moved into `ai_driver::run_stage_within`, where
    // ONE capped value feeds both the process bound and the provider's own argv
    // deadline. The value is written once per consumer, and there is now one
    // fewer consumer here.
    //
    // Both remaining spellings are load-bearing: the watchdog's bound, and the
    // cap handed to the chain so a tier declaring a longer timeout cannot
    // outlive it.
    assert_eq!(
        code.matches("DOC_PARITY_PROBE.supervisor()").count(),
        2,
        "every deadline for this turn must come from the one value"
    );
    assert!(
        code.contains("run_stage_within("),
        "the probe must hand the chain its budget, or a tier's declared timeout \
         could outlive the watchdog that is supposed to bound it"
    );
    // That the chain really derives both deadlines from that cap is
    // `the_declared_chain_is_the_dispatched_chain_test::
    //  a_supplied_budget_bounds_the_whole_stage_not_each_attempt`,
    // asserted there rather than restated here. It was named
    // `..._caps_every_tier_and_reaches_the_provider` and this reference went
    // stale when that test was renamed -- CAPPING EVERY TIER was the defect,
    // not the property: one bound applied to each of five tiers is five times
    // the bound. A cross-reference by name is not checked by anything, which
    // is why the sentence below states the property rather than only pointing
    // at it: the chain must apply the supplied budget to the STAGE.
    assert!(
        !code.contains("from_secs(30)"),
        "a hardcoded supervisor budget is back"
    );
}
