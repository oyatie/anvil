//! `occupancy`: admit this pull request's path-set, or refuse it.
//!
//! Two hops combine iff their write-sets are disjoint. The verdict is
//! `change_delivery::core::shard::admit_spawn` — this binary is only the
//! shell that hands it path lists and prints what it said. There is no
//! second copy of the rule here to drift from the first.
//!
//! Overlap is resolved as a queue, not as a standoff. Two pull requests
//! that both touch one file are both refused if each is compared against
//! the other, so neither can ever land and the pair has to be broken by
//! closing one — which is what draining this trunk cost twice. Each hop
//! is therefore compared only against the pull requests *ahead of it*:
//! lower number, opened earlier. The lowest number in any overlapping set
//! is compared against nothing and lands; the next one lands behind it.
//! The rule is a total order, so it cannot cycle.
//!
//! Inputs are newline lists produced by the workflow from the forge REST
//! API, never a prompt and never a model. The binary itself makes no
//! network call, so a forge failure is caught by the collecting step's
//! `set -euo pipefail` and never reaches this process as an empty set.
//!
//! Statuses are `pre_merge_guard::report::GateStatus`, and the exit code
//! is 0 only for `Passed` or an audited override `Warning`:
//!
//! - `Failed` — occupancy measured an overlap, a second hub hop, or a hub
//!   without the applicable base freshness. A defect this gate found.
//! - `Errored` — the gate was configured and had a data source but could
//!   not produce a measurement: an unreadable list, a malformed line, a
//!   malformed or missing freshness evidence. Invariant I1:
//!   absent evidence is never a pass.
//! - `NotMeasured` is never emitted. It is acceptable by construction
//!   (`GateStatus::is_acceptable`), so reporting it here would turn a
//!   forge that did not answer into "no overlap" — the exact false green
//!   this check exists to prevent.

#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::disallowed_methods))]

use anvil::change_delivery::facade::occupancy::{
    Hop, SpawnRefused, admit_in_queue_with_freshness, anvil_hubs,
};
use anvil::pre_merge_guard::report::GateStatus;

mod freshness;
mod inputs;
use inputs::{OVERRIDE_LABEL, collect, owner_number};
use std::collections::BTreeSet;
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let status = run(&env::args().skip(1).collect::<Vec<_>>());
    println!("{} occupancy: {}", status.badge(), describe(&status));
    if admits(&status) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn describe(status: &GateStatus) -> String {
    match status {
        GateStatus::Passed => "path-set is disjoint from every open hop on this trunk".to_owned(),
        GateStatus::Failed(reason) | GateStatus::Errored(reason) => reason.clone(),
        other => format!("{other:?}"),
    }
}

/// Every input error becomes `Errored`, so a missing or unparseable list
/// cannot be read as an empty path-set.
fn run(args: &[String]) -> GateStatus {
    evaluate(collect(args))
}

fn evaluate(input: Result<inputs::Inputs, String>) -> GateStatus {
    match input {
        Ok(i) => {
            println!("occupancy: {}", i.freshness.diagnostic());
            verdict(
                &i.this,
                i.this_pr,
                &i.in_flight,
                &i.freshness,
                i.override_label,
            )
        }
        Err(reason) => GateStatus::Errored(reason),
    }
}

fn verdict(
    this: &BTreeSet<String>,
    this_pr: u64,
    in_flight: &[(String, BTreeSet<String>)],
    freshness: &freshness::FreshnessProof,
    override_label: bool,
) -> GateStatus {
    let open: Vec<Hop> = match in_flight
        .iter()
        .map(|(owner, write)| {
            owner_number(owner).map(|position| Hop {
                position,
                write: write.clone(),
            })
        })
        .collect::<Result<_, _>>()
    {
        Ok(open) => open,
        Err(reason) => return GateStatus::Errored(reason),
    };
    let ahead: Vec<(String, BTreeSet<String>)> = in_flight
        .iter()
        .filter(|(owner, _)| owner_number(owner).is_ok_and(|n| n < this_pr))
        .cloned()
        .collect();

    let hubs = anvil_hubs();
    match admit_in_queue_with_freshness(this, this_pr, &hubs, &open, freshness.kind()) {
        Ok(_) => GateStatus::Passed,
        Err(SpawnRefused::Overlap { path }) => held(
            format!(
                "`{path}` is already occupied by {}, which is ahead of #{this_pr} in the \
                 queue; two hops combine only when their write-sets are disjoint. Land \
                 behind it or rebase onto it.",
                owner_of(&path, &ahead)
            ),
            override_label,
        ),
        Err(SpawnRefused::HubAlreadyInFlight) => held(
            format!(
                "a hub file is already in flight in {}, which is ahead of #{this_pr}; \
                 hubs are N=1",
                hub_holder(&hubs, &ahead)
            ),
            override_label,
        ),
        // Not overridable. The other two refusals order hops that were each
        // measured; this one says the measurement was taken against a
        // combination the queue will not build, so admitting it would publish a
        // verdict about a tree that does not exist.
        Err(SpawnRefused::HubOnStaleBase) => GateStatus::Failed(
            "a hub file was edited from a stale merge-base; rebase onto the destination tip. \
             Only a verified predecessor promotion may instead prove equal complete base trees"
                .to_owned(),
        ),
    }
}

/// The refusal, unless a human has taken it on the record.
///
/// `Warning` and never `Passed`: an overridden admission must not be
/// indistinguishable from a measured disjointness in anything that reads the
/// status later.
fn held(reason: String, override_label: bool) -> GateStatus {
    if override_label {
        GateStatus::Warning(format!(
            "admitted over occupancy by the `{OVERRIDE_LABEL}` label: {reason}"
        ))
    } else {
        GateStatus::Failed(reason)
    }
}

fn owner_of(path: &str, in_flight: &[(String, BTreeSet<String>)]) -> String {
    in_flight
        .iter()
        .find(|(_, paths)| paths.contains(path))
        .map_or_else(|| "an in-flight hop".to_owned(), |(id, _)| id.clone())
}

fn hub_holder(hubs: &BTreeSet<String>, in_flight: &[(String, BTreeSet<String>)]) -> String {
    in_flight
        .iter()
        .find(|(_, paths)| paths.iter().any(|p| hubs.contains(p)))
        .map_or_else(|| "an in-flight hop".to_owned(), |(id, _)| id.clone())
}

fn admits(status: &GateStatus) -> bool {
    matches!(status, GateStatus::Passed | GateStatus::Warning(_))
}

#[cfg(test)]
mod tests;
