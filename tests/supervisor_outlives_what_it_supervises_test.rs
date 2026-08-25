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

use anvil::exec::{ExecClass, agy_print_timeout_arg};
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
fn the_doc_parity_probe_supervises_its_own_deadline_and_not_a_smaller_one() {
    // Source-level, because the defect was two numbers in one function that
    // could not disagree in any single value a unit test could inspect. The
    // supervisor budget and the `--print-timeout` argument are now the same
    // constant, and that is what this pins.
    let src = std::fs::read_to_string("src/doc_guard/mod.rs").expect("doc_guard source");

    let supervised = src
        .split("run_with_watchdog(")
        .nth(1)
        .expect("the probe is supervised");
    // Everything up to the closure body is the watchdog's own arguments.
    let args = supervised
        .split("move ||")
        .next()
        .expect("the operation follows the budget");

    assert!(
        args.contains("DOC_PARITY_PROBE_TIMEOUT"),
        "the watchdog budget is not the probe's own deadline. It was a \
         hardcoded 30s while the probe handed agy 120s, so the supervisor \
         killed the call it was supposed to be watching:\n{args}"
    );
    assert!(
        !args.contains("from_secs(30)"),
        "the hardcoded 30s budget is back:\n{args}"
    );
    assert!(
        src.contains("agy_print_timeout_arg(DOC_PARITY_PROBE_TIMEOUT)"),
        "the probe must hand agy the same constant the supervisor is given, or \
         the two deadlines can drift apart again"
    );
}
