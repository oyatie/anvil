//! Anvil's own shape baseline, with a reviewed integer that must move.
//!
//! A baseline regeneration that adds a key and a regeneration that removes
//! one both produce a valid file; only a number a human has to edit makes the
//! direction visible in review (oyatie pins its dark-gate baseline the same
//! way: `EXPECTED_BASELINED_DARK_GATE_CRATES`). `assert_eq!`, not `<=`,
//! so every shrink is a deliberate edit here too.

use anvil::ratchet::core::{Baseline, Mode};
use std::path::PathBuf;

/// Keys baselined across all rules at the seeding commit. 87 module units,
/// 86 of them missing all three required faces (core, ports, facade), one
/// (`shape`) missing none: 86 * 3 = 258. Edit deliberately, downward.
const EXPECTED_BASELINED_SHAPE_FINDINGS: usize = 258;

fn baseline() -> Baseline {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".anvil/baselines/shape.baseline.json");
    Baseline::parse(&std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
        .expect("Anvil's own baseline must parse")
}

#[test]
fn the_baselined_key_count_is_exactly_the_reviewed_number() {
    let b = baseline();
    assert_eq!(
        b.total_keys(),
        EXPECTED_BASELINED_SHAPE_FINDINGS,
        "Anvil's shape baseline moved; if it shrank, lower the constant in the same change — \
         if it grew, something was added to the baseline that a ratchet must refuse"
    );
}

#[test]
fn the_baseline_names_the_commit_it_was_measured_at() {
    let b = baseline();
    assert_eq!(
        b.measured_at.len(),
        40,
        "a baseline is reproducible only from a full sha"
    );
    assert!(b.measured_at.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn blocking_rules_with_frozen_empty_carry_no_keys() {
    let b = baseline();
    for (rule, rb) in &b.rules {
        if rb.frozen_empty {
            assert!(
                rb.keys.is_empty(),
                "{rule} is frozen_empty yet baselined keys"
            );
            assert_eq!(rb.mode, Mode::BlockOnNew);
        }
    }
    assert!(
        b.rules.values().any(|r| r.mode == Mode::BlockOnNew),
        "at least one rule blocks on Anvil's own tree"
    );
}
