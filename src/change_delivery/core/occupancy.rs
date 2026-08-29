//! Path occupancy. Two hops combine iff their write-sets are disjoint.
//!
//! A `git mv` occupies both ends. Hubs (barrels, lockfile, doctrine) are
//! N=1 and only at trunk HEAD. `tests/*.rs` is the open set on this tree:
//! Cargo autoloads each file as its own crate, so no `lib.rs` edit.
//!
//! Overlap orders hops; it does not refuse them in pairs. Comparing a hop
//! against every other open hop is symmetric, so an overlapping pair is told to
//! wait for each other and neither can ever land -- a standoff that can only be
//! broken by closing one of them. [`admit_in_queue`] compares a hop only
//! against those *ahead* of it, which is a total order on integers: some hop is
//! always compared against nothing, so some hop can always land.

use std::collections::BTreeSet;

/// Closed hub set for this repository. Editing any of these serialises.
pub fn anvil_hubs() -> BTreeSet<String> {
    [
        "Cargo.toml",
        "Cargo.lock",
        "src/lib.rs",
        "src/main.rs",
        "README.md",
        "CHANGELOG.md",
        "docs/doctrine.md",
        ".github/workflows/ci.yml",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

pub fn occupy_move(old: &str, new: &str) -> BTreeSet<String> {
    [old, new].into_iter().map(str::to_string).collect()
}

pub fn path_sets_disjoint(a: &BTreeSet<String>, b: &BTreeSet<String>) -> bool {
    a.is_disjoint(b)
}

/// `tests/foo.rs` at the crate root: Cargo integration-test autodiscovery.
pub fn is_open_test_crate(path: &str) -> bool {
    let rest = match path.strip_prefix("tests/") {
        Some(r) => r,
        None => return false,
    };
    rest.ends_with(".rs") && !rest.contains('/')
}

pub fn hits_hub(write: &BTreeSet<String>, hubs: &BTreeSet<String>) -> bool {
    write.iter().any(|p| hubs.contains(p))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnKind {
    /// Disjoint from hubs and from every in-flight write-set.
    Parallel,
    /// Touches a hub. At most one such hop, and only at trunk HEAD.
    Hub,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpawnRefused {
    Overlap { path: String },
    HubAlreadyInFlight,
    HubOnStaleBase,
}

/// Admit a hop before spawn. `in_flight` is the union of write-sets of
/// live worktrees, open PRs, and merge_group. `merge_base_is_trunk` is
/// whether this hop's merge-base equals current trunk HEAD.
pub fn admit_spawn(
    write: &BTreeSet<String>,
    hubs: &BTreeSet<String>,
    in_flight: &[BTreeSet<String>],
    merge_base_is_trunk: bool,
) -> Result<SpawnKind, SpawnRefused> {
    for other in in_flight {
        if let Some(path) = write.intersection(other).next() {
            return Err(SpawnRefused::Overlap { path: path.clone() });
        }
    }
    if hits_hub(write, hubs) {
        if !merge_base_is_trunk {
            return Err(SpawnRefused::HubOnStaleBase);
        }
        if in_flight.iter().any(|w| hits_hub(w, hubs)) {
            return Err(SpawnRefused::HubAlreadyInFlight);
        }
        return Ok(SpawnKind::Hub);
    }
    Ok(SpawnKind::Parallel)
}

/// One open hop: where it sits in the queue, and what it writes.
///
/// `position` is any total order the caller can supply for every open hop at
/// once. Pull request number is the one this repository uses, because it is
/// assigned by the forge, never reused, and orders by when the hop was opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hop {
    pub position: u64,
    pub write: BTreeSet<String>,
}

/// The open hops ahead of `position`.
pub fn ahead_of(position: u64, open: &[Hop]) -> Vec<&Hop> {
    open.iter().filter(|h| h.position < position).collect()
}

/// [`admit_spawn`], against the hops ahead of this one rather than all of them.
///
/// The hub rules are ordered by the same comparison, and for the same reason: a
/// hub change is exactly the one that cannot be split by path, so a symmetric
/// hub refusal is the standoff in the shape that hurts most.
/// `HubOnStaleBase` does not depend on the open set and is unchanged.
pub fn admit_in_queue(
    write: &BTreeSet<String>,
    position: u64,
    hubs: &BTreeSet<String>,
    open: &[Hop],
    merge_base_is_trunk: bool,
) -> Result<SpawnKind, SpawnRefused> {
    let ahead: Vec<BTreeSet<String>> = ahead_of(position, open)
        .into_iter()
        .map(|h| h.write.clone())
        .collect();
    admit_spawn(write, hubs, &ahead, merge_base_is_trunk)
}

#[cfg(test)]
mod queue_tests {
    use super::*;

    fn hop(position: u64, paths: &[&str]) -> Hop {
        Hop {
            position,
            write: paths.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    fn write(paths: &[&str]) -> BTreeSet<String> {
        paths.iter().map(|s| (*s).to_string()).collect()
    }

    /// The standoff, as both halves of one pair: #7 waits for #9 and #9 waits
    /// for #7, so the pair can only be broken by closing one.
    #[test]
    fn two_hops_on_one_file_order_rather_than_refuse_each_other() {
        let file = &["tests/shared.rs"];
        assert_eq!(
            admit_in_queue(&write(file), 7, &anvil_hubs(), &[hop(9, file)], true),
            Ok(SpawnKind::Parallel),
            "#7 is ahead of #9 and is compared against nothing, so it lands"
        );
        assert_eq!(
            admit_in_queue(&write(file), 9, &anvil_hubs(), &[hop(7, file)], true),
            Err(SpawnRefused::Overlap {
                path: "tests/shared.rs".to_string()
            }),
            "#9 is behind #7 and waits"
        );
    }

    /// Three hops on one file resolve to one order, not a cycle and not an
    /// empty admission.
    #[test]
    fn exactly_one_of_an_overlapping_set_is_admitted() {
        let file = &["tests/shared.rs"];
        let open = [hop(4, file), hop(7, file), hop(9, file)];
        let admitted: Vec<u64> = [4u64, 7, 9]
            .into_iter()
            .filter(|p| {
                let others: Vec<Hop> = open.iter().filter(|h| h.position != *p).cloned().collect();
                admit_in_queue(&write(file), *p, &anvil_hubs(), &others, true).is_ok()
            })
            .collect();
        assert_eq!(
            admitted,
            vec![4],
            "the lowest lands; the rest queue behind it"
        );
    }

    #[test]
    fn the_lower_numbered_hub_hop_lands() {
        assert_eq!(
            admit_in_queue(
                &write(&["docs/doctrine.md"]),
                4,
                &anvil_hubs(),
                &[hop(9, &["Cargo.lock"])],
                true
            ),
            Ok(SpawnKind::Hub),
            "#4 is ahead of #9, so the hub is free in front of it"
        );
    }

    #[test]
    fn a_hub_hop_behind_another_hub_hop_waits() {
        assert_eq!(
            admit_in_queue(
                &write(&["docs/doctrine.md"]),
                9,
                &anvil_hubs(),
                &[hop(4, &["Cargo.lock"])],
                true
            ),
            Err(SpawnRefused::HubAlreadyInFlight)
        );
    }

    /// Ordering does not lift the rule that a hub must be measured against the
    /// combination the queue will build.
    #[test]
    fn a_stale_hub_base_is_refused_however_far_ahead_the_hop_is() {
        assert_eq!(
            admit_in_queue(&write(&["src/main.rs"]), 1, &anvil_hubs(), &[], false),
            Err(SpawnRefused::HubOnStaleBase)
        );
    }

    #[test]
    fn disjoint_hops_are_admitted_in_either_order() {
        for (mine, theirs) in [(7u64, 9u64), (9, 7)] {
            assert_eq!(
                admit_in_queue(
                    &write(&["tests/lane_a.rs"]),
                    mine,
                    &anvil_hubs(),
                    &[hop(theirs, &["tests/lane_b.rs"])],
                    true
                ),
                Ok(SpawnKind::Parallel)
            );
        }
    }
}
