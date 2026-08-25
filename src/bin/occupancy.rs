//! `occupancy`: admit this pull request's path-set, or refuse it.
//!
//! Two hops combine iff their write-sets are disjoint. The verdict is
//! `change_delivery::core::shard::admit_spawn` — this binary is only the
//! shell that hands it path lists and prints what it said. There is no
//! second copy of the rule here to drift from the first.
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

use anvil::change_delivery::core::shard::{SpawnRefused, admit_spawn, anvil_hubs};
use anvil::pre_merge_guard::report::GateStatus;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;
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
        Ok((this, in_flight, at_trunk)) => verdict(&this, &in_flight, at_trunk),
        Err(reason) => GateStatus::Errored(reason),
    }
}

type Inputs = (BTreeSet<String>, Vec<(String, BTreeSet<String>)>, bool);

fn collect(args: &[String]) -> Result<Inputs, String> {
    let this = read(Path::new(&flag(args, "--this")?))?;
    let in_flight = read(Path::new(&flag(args, "--in-flight")?))?;
    let at_trunk = parse_bool(&flag(args, "--merge-base-is-trunk")?)?;
    Ok((parse_paths(&this), parse_in_flight(&in_flight)?, at_trunk))
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))
}

fn flag(args: &[String], name: &str) -> Result<String, String> {
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == name {
            return rest
                .next()
                .cloned()
                .ok_or_else(|| format!("{name} requires a value"));
        }
    }
    Err(format!("missing {name}"))
}

fn parse_bool(raw: &str) -> Result<bool, String> {
    match raw {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!(
            "--merge-base-is-trunk must be `true` or `false`, got `{other}`"
        )),
    }
}

fn parse_paths(body: &str) -> BTreeSet<String> {
    body.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `<owner>\t<path>` lines, one per changed file of one open pull request.
fn parse_in_flight(body: &str) -> Result<Vec<(String, BTreeSet<String>)>, String> {
    let mut by_owner: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (i, line) in body.lines().enumerate() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        let Some((owner, path)) = line.split_once('\t') else {
            return Err(format!(
                "in-flight line {}: expected `<owner>\\t<path>`, got `{line}`",
                i + 1
            ));
        };
        if owner.trim().is_empty() || path.trim().is_empty() {
            return Err(format!("in-flight line {}: empty owner or path", i + 1));
        }
        by_owner
            .entry(owner.trim().to_owned())
            .or_default()
            .insert(path.trim().to_owned());
    }
    Ok(by_owner.into_iter().collect())
}

fn verdict(
    this: &BTreeSet<String>,
    in_flight: &[(String, BTreeSet<String>)],
    merge_base_is_trunk: bool,
) -> GateStatus {
    let hubs = anvil_hubs();
    let sets: Vec<BTreeSet<String>> = in_flight.iter().map(|(_, p)| p.clone()).collect();
    match admit_spawn(this, &hubs, &sets, merge_base_is_trunk) {
        Ok(_) => GateStatus::Passed,
        Err(SpawnRefused::Overlap { path }) => GateStatus::Failed(format!(
            "`{path}` is already occupied by {}; two hops combine only when their \
             write-sets are disjoint",
            owner_of(&path, in_flight)
        )),
        Err(SpawnRefused::HubAlreadyInFlight) => GateStatus::Failed(format!(
            "a hub file is already in flight in {}; hubs are N=1",
            hub_holder(&hubs, in_flight)
        )),
        Err(SpawnRefused::HubOnStaleBase) => GateStatus::Failed(
            "a hub file was edited from a stale merge-base; hubs are N=1 at trunk HEAD, \
             so rebase onto the trunk tip"
                .to_owned(),
        ),
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
    matches!(status, GateStatus::Passed)
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

    #[test]
    fn a_flag_without_a_value_is_an_error_not_an_empty_string() {
        assert!(flag(&args(&["--this"]), "--this").is_err());
        assert!(flag(&args(&[]), "--this").is_err());
        assert_eq!(
            flag(&args(&["--this", "a.txt"]), "--this"),
            Ok("a.txt".to_owned())
        );
    }

    #[test]
    fn an_unreadable_merge_base_answer_is_not_a_true() {
        assert_eq!(parse_bool("true"), Ok(true));
        assert_eq!(parse_bool("false"), Ok(false));
        assert!(
            parse_bool("").is_err(),
            "an empty answer is absent evidence, not `at trunk HEAD`"
        );
        assert!(parse_bool("TRUE").is_err());
    }

    #[test]
    fn paths_are_trimmed_and_blank_lines_dropped() {
        assert_eq!(
            parse_paths("tests/a.rs\n\n  tests/b.rs  \n"),
            set(&["tests/a.rs", "tests/b.rs"])
        );
    }

    #[test]
    fn in_flight_lines_group_by_owner() {
        let parsed = parse_in_flight("pr-7\ttests/a.rs\npr-7\ttests/b.rs\npr-9\ttests/c.rs\n")
            .expect("well-formed");
        assert_eq!(
            parsed,
            vec![
                occupied("pr-7", &["tests/a.rs", "tests/b.rs"]),
                occupied("pr-9", &["tests/c.rs"]),
            ]
        );
    }

    #[test]
    fn a_malformed_in_flight_line_is_an_error_not_an_empty_set() {
        let err = parse_in_flight("pr-7 tests/a.rs\n").unwrap_err();
        assert!(
            err.contains("line 1"),
            "the reason must locate itself: {err}"
        );
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
        ]));
        assert!(
            matches!(status, GateStatus::Errored(_)),
            "a list the check could not read is absent evidence, not an empty path-set: {status:?}"
        );
        assert!(!admits(&status));
    }

    #[test]
    fn disjoint_open_test_crates_are_admitted() {
        let status = verdict(
            &set(&["tests/lane_a.rs"]),
            &[occupied("pr-9", &["tests/lane_b.rs"])],
            true,
        );
        assert_eq!(status, GateStatus::Passed);
        assert!(admits(&status));
    }

    #[test]
    fn an_overlap_is_failed_and_names_the_pull_request_holding_the_path() {
        let status = verdict(
            &set(&["tests/lane_a.rs"]),
            &[occupied("pr-9", &["tests/lane_a.rs"])],
            true,
        );
        let GateStatus::Failed(reason) = &status else {
            panic!("an overlap is a defect this gate measured: {status:?}");
        };
        assert!(reason.contains("tests/lane_a.rs"), "{reason}");
        assert!(
            reason.contains("pr-9"),
            "the refusal must name the occupant: {reason}"
        );
        assert!(!admits(&status));
    }

    #[test]
    fn a_hub_edited_from_a_stale_merge_base_is_failed() {
        let status = verdict(&set(&["src/main.rs"]), &[], false);
        let GateStatus::Failed(reason) = &status else {
            panic!("a hub off trunk HEAD is a defect: {status:?}");
        };
        assert!(
            reason.contains("merge-base"),
            "the reason must say the base is stale: {reason}"
        );
        assert!(!admits(&status));
    }

    #[test]
    fn a_second_hub_hop_is_failed() {
        let status = verdict(
            &set(&["docs/doctrine.md"]),
            &[occupied("pr-4", &["Cargo.lock"])],
            true,
        );
        let GateStatus::Failed(reason) = &status else {
            panic!("hubs are N=1: {status:?}");
        };
        assert!(reason.contains("pr-4"), "{reason}");
        assert!(!admits(&status));
    }

    #[test]
    fn a_hub_at_trunk_head_with_nothing_else_in_flight_is_admitted() {
        assert_eq!(
            verdict(&set(&[".github/workflows/ci.yml"]), &[], true),
            GateStatus::Passed
        );
    }

    #[test]
    fn an_open_path_off_a_stale_base_is_still_admitted() {
        assert_eq!(
            verdict(&set(&["tests/lane_a.rs"]), &[], false),
            GateStatus::Passed,
            "only hubs are pinned to trunk HEAD; the open set is N-wide"
        );
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
