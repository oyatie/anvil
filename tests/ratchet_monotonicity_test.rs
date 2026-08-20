//! The ratchet only turns one way (I7). Growth without a signed one-way door
//! fails; shrink passes; advisory rules never block; an inert door fails.

use anvil::ratchet::core::{
    BASELINE_SCHEMA_V1, Baseline, Growth, Mode, RuleBaseline, Signing, Signoff, compare,
    regen_is_monotonic,
};
use std::collections::{BTreeMap, BTreeSet};

fn keys(v: &[&str]) -> BTreeSet<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn baseline(rules: &[(&str, Mode, bool, &[&str])]) -> Baseline {
    Baseline {
        schema: BASELINE_SCHEMA_V1.into(),
        measured_at: "0".repeat(40),
        rules: rules
            .iter()
            .map(|(r, m, fe, k)| {
                (
                    r.to_string(),
                    RuleBaseline {
                        mode: *m,
                        frozen_empty: *fe,
                        keys: keys(k),
                    },
                )
            })
            .collect(),
    }
}

fn current(pairs: &[(&str, &[&str])]) -> BTreeMap<String, BTreeSet<String>> {
    pairs
        .iter()
        .map(|(r, k)| (r.to_string(), keys(k)))
        .collect()
}

fn signed(rule: &str, k: &[&str]) -> Signoff {
    let mut s = Signoff::default();
    s.additions.insert(rule.into(), keys(k));
    s.signings.push(Signing {
        by: "owner".into(),
        date: "2026-08-20".into(),
        note: "test".into(),
    });
    s
}

#[test]
fn a_new_key_under_a_blocking_rule_fails() {
    let frozen = baseline(&[("file_misplaced", Mode::BlockOnNew, false, &["a"])]);
    let v = compare(
        &frozen,
        &current(&[("file_misplaced", &["a", "b"])]),
        &Signoff::default(),
        |_| None,
    );
    assert!(v.fails);
    assert_eq!(v.per_rule["file_misplaced"].regressions, keys(&["b"]));
    assert_eq!(v.per_rule["file_misplaced"].tolerated, keys(&["a"]));
}

#[test]
fn a_fixed_key_passes_and_is_reported_as_fixed() {
    let frozen = baseline(&[("file_misplaced", Mode::BlockOnNew, false, &["a", "b"])]);
    let v = compare(
        &frozen,
        &current(&[("file_misplaced", &["a"])]),
        &Signoff::default(),
        |_| None,
    );
    assert!(!v.fails);
    assert_eq!(v.per_rule["file_misplaced"].fixed, keys(&["b"]));
}

#[test]
fn a_signed_off_key_passes_and_an_inert_signoff_fails() {
    let frozen = baseline(&[("file_misplaced", Mode::BlockOnNew, false, &["a"])]);
    let v = compare(
        &frozen,
        &current(&[("file_misplaced", &["a", "b"])]),
        &signed("file_misplaced", &["b"]),
        |_| None,
    );
    assert!(!v.fails, "{v:?}");
    assert_eq!(v.per_rule["file_misplaced"].signed_off, keys(&["b"]));

    // The door was left open for a key that is no longer there.
    let v = compare(
        &frozen,
        &current(&[("file_misplaced", &["a"])]),
        &signed("file_misplaced", &["b"]),
        |_| None,
    );
    assert!(v.fails);
    assert_eq!(
        v.inert_signoff,
        vec![("file_misplaced".to_string(), "b".to_string())]
    );
}

#[test]
fn an_advisory_rule_never_fails_however_much_it_grows() {
    let frozen = baseline(&[("unit_missing_face", Mode::Advisory, false, &[])]);
    let v = compare(
        &frozen,
        &current(&[("unit_missing_face", &["x", "y", "z"])]),
        &Signoff::default(),
        |_| None,
    );
    assert!(!v.fails);
    assert_eq!(
        v.per_rule["unit_missing_face"].regressions.len(),
        3,
        "still counted"
    );
}

#[test]
fn a_frozen_empty_rule_fails_on_its_first_key() {
    let frozen = baseline(&[("root_file_unallowlisted", Mode::BlockOnNew, true, &[])]);
    let v = compare(
        &frozen,
        &current(&[("root_file_unallowlisted", &["scratch.txt"])]),
        &Signoff::default(),
        |_| None,
    );
    assert!(v.fails);
}

#[test]
fn a_rule_unknown_to_the_frozen_baseline_takes_the_spec_mode_or_advisory() {
    let frozen = baseline(&[]);
    let cur = current(&[("crate_layer_suffix", &["k"])]);
    let v = compare(&frozen, &cur, &Signoff::default(), |r| {
        (r == "crate_layer_suffix").then_some((Mode::BlockOnNew, false))
    });
    assert!(v.fails, "the spec says block");
    let v = compare(&frozen, &cur, &Signoff::default(), |_| None);
    assert!(!v.fails, "a rule nobody declared cannot block");
}

#[test]
fn regeneration_may_only_shrink_unless_signed() {
    let frozen = baseline(&[("file_misplaced", Mode::BlockOnNew, false, &["a", "b"])]);
    let shrunk = baseline(&[("file_misplaced", Mode::BlockOnNew, false, &["a"])]);
    assert!(regen_is_monotonic(&frozen, &shrunk, &Signoff::default()).is_ok());

    let grown = baseline(&[("file_misplaced", Mode::BlockOnNew, false, &["a", "b", "c"])]);
    let err = regen_is_monotonic(&frozen, &grown, &Signoff::default()).unwrap_err();
    assert_eq!(
        err,
        vec![Growth::KeyAdded {
            rule: "file_misplaced".into(),
            key: "c".into()
        }]
    );
    assert!(regen_is_monotonic(&frozen, &grown, &signed("file_misplaced", &["c"])).is_ok());

    let downgraded = baseline(&[("file_misplaced", Mode::Advisory, false, &["a", "b"])]);
    let err = regen_is_monotonic(&frozen, &downgraded, &Signoff::default()).unwrap_err();
    assert_eq!(
        err,
        vec![Growth::ModeDowngraded {
            rule: "file_misplaced".into()
        }]
    );

    let relaxed_from = baseline(&[("root_file_unallowlisted", Mode::BlockOnNew, true, &[])]);
    let relaxed_to = baseline(&[("root_file_unallowlisted", Mode::BlockOnNew, false, &[])]);
    let err = regen_is_monotonic(&relaxed_from, &relaxed_to, &Signoff::default()).unwrap_err();
    assert_eq!(
        err,
        vec![Growth::FrozenEmptyRelaxed {
            rule: "root_file_unallowlisted".into()
        }]
    );
}

#[test]
fn baseline_documents_reject_a_frozen_empty_rule_that_carries_keys() {
    let raw = r#"{"schema":"anvil/ratchet-baseline/v1","measured_at":"x","rules":{"r":{"mode":"baseline-block-on-new","frozen_empty":true,"keys":["k"]}}}"#;
    assert!(Baseline::parse(raw.as_bytes()).is_err());
}

#[test]
fn a_signoff_with_entries_but_no_signing_is_rejected() {
    let raw = r#"{"schema":"anvil/ratchet-signoff/v1","_sign_off_additions":{"r":["k"]}}"#;
    assert!(Signoff::parse(raw.as_bytes()).is_err());
}
