//! Occupancy proof, lane A. One path nothing else on this trunk writes.
//!
//! `tests/*.rs` is the open set on this tree: Cargo autoloads each file at
//! the crate root as its own integration-test crate, so adding one occupies
//! exactly one path and touches no barrel, no manifest and no lockfile.

#[test]
fn lane_a_is_its_own_crate() {
    assert_eq!(1 + 1, 2);
}
