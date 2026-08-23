//! Occupancy proof, lane B. Disjoint from lane A by construction.
//!
//! Two hops combine iff their write-sets are disjoint. This one and lane A
//! share no path, so both should be admitted and both should squash onto
//! the trunk without either rebasing on the other.

#[test]
fn lane_b_is_its_own_crate() {
    assert_eq!(2 + 2, 4);
}
