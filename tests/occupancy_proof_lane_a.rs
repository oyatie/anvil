//! Occupancy proof, the overlapping third hop.
//!
//! This writes the same path as lane A on purpose. Two hops that write the
//! same path do not combine, so occupancy must refuse this one — and it
//! must refuse it before the workspace test suite compiles anything.

#[test]
fn the_third_hop_writes_a_path_lane_a_already_holds() {
    assert_eq!(3 + 3, 6);
}
