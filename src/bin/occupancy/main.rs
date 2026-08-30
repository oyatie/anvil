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
//! is 0 only for `Passed`:
//!
//! - `Failed` — occupancy measured an overlap, a second hub hop, or a hub
//!   off trunk HEAD. A defect this gate found.
//! - `Errored` — the gate was configured and had a data source but could
//!   not produce a measurement: an unreadable list, a malformed line, a
//!   merge-base answer that is neither `true` nor `false`. Invariant I1:
//!   absent evidence is never a pass.
//! - `NotMeasured` is never emitted. It is acceptable by construction
//!   (`GateStatus::is_acceptable`), so reporting it here would turn a
//!   forge that did not answer into "no overlap" — the exact false green
//!   this check exists to prevent.

use anvil::change_delivery::facade::occupancy::{Hop, SpawnRefused, admit_in_queue, anvil_hubs};
use anvil::pre_merge_guard::report::GateStatus;

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
    match collect(args) {
        Ok(i) => verdict(
            &i.this,
            i.this_pr,
            &i.in_flight,
            i.at_trunk,
            i.override_label,
        ),
        Err(reason) => GateStatus::Errored(reason),
    }
}

fn verdict(
    this: &BTreeSet<String>,
    this_pr: u64,
    in_flight: &[(String, BTreeSet<String>)],
    merge_base_is_trunk: bool,
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
    match admit_in_queue(this, this_pr, &hubs, &open, merge_base_is_trunk) {
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
            "a hub file was edited from a stale merge-base; hubs are N=1 at trunk HEAD, \
             so rebase onto the trunk tip"
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
mod tests {
    use super::*;

    fn set(paths: &[&str]) -> BTreeSet<String> {
        paths.iter().map(|s| (*s).to_string()).collect()
    }

    fn occupied(id: &str, paths: &[&str]) -> (String, BTreeSet<String>) {
        (id.to_owned(), set(paths))
    }

    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| (*s).to_string()).collect()
    }

    /// This hop, at number `pr`, against the open set.
    fn admit(
        pr: u64,
        this: &[&str],
        in_flight: &[(String, BTreeSet<String>)],
        at_trunk: bool,
    ) -> GateStatus {
        verdict(&set(this), pr, in_flight, at_trunk, false)
    }

    #[test]
    fn an_unreadable_list_is_errored_not_passed() {
        let status = run(&args(&[
            "--this",
            "/nonexistent/this.txt",
            "--in-flight",
            "/nonexistent/in-flight.txt",
            "--merge-base-is-trunk",
            "true",
            "--this-pr",
            "7",
        ]));
        assert!(
            matches!(status, GateStatus::Errored(_)),
            "a list the check could not read is absent evidence, not an empty path-set: {status:?}"
        );
        assert!(!admits(&status));
    }

    #[test]
    fn an_overlap_is_failed_and_names_the_pull_request_holding_the_path() {
        let status = admit(
            9,
            &["tests/lane_a.rs"],
            &[occupied("pr-7", &["tests/lane_a.rs"])],
            true,
        );
        let GateStatus::Failed(reason) = &status else {
            panic!("an overlap is a defect this gate measured: {status:?}");
        };
        assert!(reason.contains("tests/lane_a.rs"), "{reason}");
        assert!(
            reason.contains("pr-7"),
            "the refusal must name the occupant: {reason}"
        );
        assert!(!admits(&status));
    }

    /// An owner the collecting step wrote in a shape this binary does not
    /// understand is a hop that would silently stop being compared against.
    #[test]
    fn an_unparseable_owner_is_errored_not_a_hop_quietly_dropped() {
        let status = verdict(
            &set(&["tests/lane_a.rs"]),
            9,
            &[occupied("branch-foo", &["tests/lane_a.rs"])],
            true,
            false,
        );
        let GateStatus::Errored(reason) = &status else {
            panic!("an owner that does not parse is absent evidence: {status:?}");
        };
        assert!(reason.contains("branch-foo"), "{reason}");
        assert!(!admits(&status));
    }

    /// The override is visible in the status, so nothing downstream can read it
    /// as a measured disjointness.
    #[test]
    fn the_override_label_admits_an_overlap_as_a_warning_never_as_a_pass() {
        let file = &["tests/shared.rs"];
        let status = verdict(&set(file), 9, &[occupied("pr-7", file)], true, true);
        let GateStatus::Warning(reason) = &status else {
            panic!("an audited override admits, and says so: {status:?}");
        };
        assert!(
            reason.contains(OVERRIDE_LABEL),
            "the warning must name the label that admitted it: {reason}"
        );
        assert!(reason.contains("pr-7"), "and what it overrode: {reason}");
        assert!(admits(&status));
        assert_ne!(
            status,
            GateStatus::Passed,
            "an overridden admission is not a measurement"
        );
    }

    /// The one refusal the label may not lift.
    #[test]
    fn the_override_label_does_not_admit_a_hub_off_a_stale_base() {
        let status = verdict(&set(&["src/main.rs"]), 7, &[], false, true);
        assert!(
            matches!(status, GateStatus::Failed(_)),
            "the other refusals order hops that were each measured; this one says \
             the measurement was taken against a combination the queue will not \
             build, and admitting it publishes a verdict about a tree that does \
             not exist: {status:?}"
        );
        assert!(!admits(&status));
    }

    #[test]
    fn errored_is_the_state_for_a_forge_that_did_not_answer_and_it_does_not_admit() {
        let status = GateStatus::Errored("rate limited".to_owned());
        assert!(
            !admits(&status),
            "a check that could not measure must not read as no overlap"
        );
    }

    #[test]
    fn not_measured_would_admit_which_is_why_occupancy_never_reports_it() {
        let unmeasured = GateStatus::NotMeasured {
            gate_id: "occupancy".to_owned(),
            reason: "no data source".to_owned(),
        };
        assert!(
            unmeasured.is_acceptable(),
            "NotMeasured is acceptable by construction, so occupancy must never emit it"
        );
        assert!(!admits(&unmeasured), "and it is not an admission either");
    }
}
