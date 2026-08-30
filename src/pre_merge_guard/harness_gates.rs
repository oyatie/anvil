//! Gates whose verdict comes from the rule harness.
//!
//! Its own module for two reasons. `evaluator.rs` is 584 lines past the file
//! budget, and the plan converts roughly fourteen more gates after this one --
//! each arriving as a `Corpus`, a `Harness::run` and a `publish`, which is a
//! shape worth having one home for rather than fifteen copies inside a
//! function that already assembles seventy-two statuses.

use super::report::GateStatus;
use crate::git_manager::PrDiffContext;

/// The harness rule that decides `cleartext_transport_status`.
///
/// The first gate whose verdict comes from the rule harness rather than from a
/// boolean on a report. `Harness::run` proves the rule against its own seeded
/// fixture before it will trust the verdict, and `Evaluated::measured` refuses
/// a measurement over zero subjects -- so a change adding no line no longer
/// certifies this gate green, which the form it replaces did.
const CLEARTEXT_RULE: &str = "cleartext_transport_status";

/// The CWE-319 lint's verdict, as a gate status.
///
/// `Harness::run` proves the rule against its own seeded fixture before it will
/// trust the verdict, and `Evaluated::measured` refuses a measurement over zero
/// subjects -- so a change adding no line withholds rather than certifying,
/// which the `passed = findings == 0` form it replaces did not.
pub fn cleartext_transport(diff_ctx: &PrDiffContext) -> GateStatus {
    let corpus = crate::harness::corpus::Corpus::of_changeset(diff_ctx.clone());
    let run = crate::harness::rules::registered().run(&corpus, &|_| true);
    match run.per_rule.get(CLEARTEXT_RULE) {
        Some(evaluated) => crate::harness::gate_status::publish(CLEARTEXT_RULE, evaluated),
        // `Harness::run` reports every registered rule, so this is
        // unreachable; withheld anyway, since a rule that vanished did
        // not run clean.
        None => GateStatus::NotMeasured {
            gate_id: CLEARTEXT_RULE.to_string(),
            reason: "the rule is not registered in the harness".to_string(),
        },
    }
}
