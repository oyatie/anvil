//! Path occupancy. Two hops combine iff their write-sets are disjoint.
//!
//! A `git mv` occupies both ends. Hubs (barrels, lockfile, doctrine) are
//! N=1 and only at trunk HEAD. `tests/*.rs` is the open set on this tree:
//! Cargo autoloads each file as its own crate, so no `lib.rs` edit.

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
