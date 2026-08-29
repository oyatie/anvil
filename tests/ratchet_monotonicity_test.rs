//! The ratchet only turns one way (I7). Growth without a signed one-way door
//! fails; shrink passes; advisory rules never block; an inert door fails.

use std::path::Path;

use anvil::ratchet::core::{
    BASELINE_SCHEMA_V1, Baseline, Growth, Mode, RuleBaseline, Signing, Signoff, compare,
    regen_is_monotonic,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

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

/// Every rule these fixtures ever declare. Existing cases measure what they
/// declare, so they pass the full set; the withdrawal cases pass less.
static ALL: LazyLock<BTreeSet<String>> = LazyLock::new(|| {
    declaring(&[
        "crate_layer_suffix",
        "file_misplaced",
        "root_file_unallowlisted",
        "unit_missing_face",
    ])
});

/// Every rule the head spec declares. Existing cases all declare what they
/// measure; the withdrawal cases below pass a narrower set on purpose.
fn declaring(rules: &[&str]) -> BTreeSet<String> {
    rules.iter().map(|s| s.to_string()).collect()
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
        &ALL,
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
        &ALL,
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
        &ALL,
    );
    assert!(!v.fails, "{v:?}");
    assert_eq!(v.per_rule["file_misplaced"].signed_off, keys(&["b"]));

    // The door was left open for a key that is no longer there.
    let v = compare(
        &frozen,
        &current(&[("file_misplaced", &["a"])]),
        &signed("file_misplaced", &["b"]),
        |_| None,
        &ALL,
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
        &ALL,
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
        &ALL,
    );
    assert!(v.fails);
}

#[test]
fn a_rule_unknown_to_the_frozen_baseline_takes_the_spec_mode_or_advisory() {
    let frozen = baseline(&[]);
    let cur = current(&[("crate_layer_suffix", &["k"])]);
    let v = compare(
        &frozen,
        &cur,
        &Signoff::default(),
        |r| (r == "crate_layer_suffix").then_some((Mode::BlockOnNew, false)),
        &ALL,
    );
    assert!(v.fails, "the spec says block");
    let v = compare(&frozen, &cur, &Signoff::default(), |_| None, &ALL);
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

#[test]
fn withdrawing_a_blocking_rule_does_not_launder_its_baselined_keys() {
    // The baseline is frozen at the merge-base; the rule SET comes from the
    // change. A rule the change stops declaring produces no measurement, so
    // `current` holds no entry and `reference.difference(now)` would report
    // every baselined key as FIXED -- laundering that reads as progress.
    let frozen = baseline(&[("file_misplaced", Mode::BlockOnNew, false, &["a", "b"])]);
    let v = compare(
        &frozen,
        &current(&[]),
        &Signoff::default(),
        |_| None,
        &declaring(&["unit_missing_face"]),
    );
    let r = &v.per_rule["file_misplaced"];
    assert!(r.withdrawn, "the rule was not declared at head");
    assert!(r.fails, "a blocking rule that stopped running must fail");
    assert!(
        r.fixed.is_empty(),
        "nothing ran, so nothing was fixed: {:?}",
        r.fixed
    );
}

#[test]
fn a_rule_that_ran_and_found_nothing_is_a_real_pass() {
    // The other side of the same coin. Declared and clean must stay clean, or
    // closing the hole would refuse every genuine fix.
    let frozen = baseline(&[("file_misplaced", Mode::BlockOnNew, false, &["a"])]);
    let v = compare(
        &frozen,
        &current(&[]),
        &Signoff::default(),
        |_| None,
        &declaring(&["file_misplaced"]),
    );
    let r = &v.per_rule["file_misplaced"];
    assert!(!r.withdrawn && !r.fails);
    assert_eq!(r.fixed.len(), 1, "the baselined key really was fixed");
}

#[test]
fn withdrawing_a_rule_with_an_empty_baseline_is_not_a_failure() {
    // A rule carrying no debt has nothing to launder, so dropping it is a
    // policy change rather than an evasion. Failing here would make the
    // gate fire on every legitimate rule retirement.
    let frozen = baseline(&[("file_misplaced", Mode::BlockOnNew, false, &[])]);
    let v = compare(
        &frozen,
        &current(&[]),
        &Signoff::default(),
        |_| None,
        &declaring(&["unit_missing_face"]),
    );
    assert!(!v.per_rule["file_misplaced"].fails);
}

#[test]
fn an_advisory_rule_that_is_withdrawn_still_does_not_block() {
    let frozen = baseline(&[("file_misplaced", Mode::Advisory, false, &["a"])]);
    let v = compare(
        &frozen,
        &current(&[]),
        &Signoff::default(),
        |_| None,
        &declaring(&["unit_missing_face"]),
    );
    let r = &v.per_rule["file_misplaced"];
    assert!(r.withdrawn, "still reported");
    assert!(!r.fails, "advisory never blocks");
}

/// The second laundering vector, one file over from withdrawing a rule.
///
/// A regeneration may only shrink: `anvil shape baseline --out` must not
/// overwrite the committed document with whatever the tree measures now.
/// These pin the predicate the reseed path consults.
#[test]
fn a_regeneration_that_adds_a_key_is_refused_without_a_signoff() {
    let frozen = baseline(&[("file_misplaced", Mode::BlockOnNew, false, &["a"])]);
    let grown = baseline(&[("file_misplaced", Mode::BlockOnNew, false, &["a", "b"])]);
    let err = regen_is_monotonic(&frozen, &grown, &Signoff::default())
        .expect_err("growth must be refused");
    assert!(
        err.iter()
            .any(|g| matches!(g, Growth::KeyAdded { key, .. } if key == "b")),
        "the new key must be named so it can be acted on: {err:?}"
    );
}

#[test]
fn a_regeneration_that_downgrades_a_blocking_rule_is_refused() {
    // Growth is not only new keys. Turning a blocking rule advisory retires
    // the enforcement while leaving the document looking untouched in size.
    let frozen = baseline(&[("file_misplaced", Mode::BlockOnNew, false, &["a"])]);
    let softened = baseline(&[("file_misplaced", Mode::Advisory, false, &["a"])]);
    let err = regen_is_monotonic(&frozen, &softened, &Signoff::default())
        .expect_err("a downgrade is growth");
    assert!(
        err.iter()
            .any(|g| matches!(g, Growth::ModeDowngraded { .. })),
        "{err:?}"
    );
}

#[test]
fn a_signed_off_addition_is_admitted_by_the_regeneration() {
    let frozen = baseline(&[("file_misplaced", Mode::BlockOnNew, false, &["a"])]);
    let grown = baseline(&[("file_misplaced", Mode::BlockOnNew, false, &["a", "b"])]);
    assert!(
        regen_is_monotonic(&frozen, &grown, &signed("file_misplaced", &["b"])).is_ok(),
        "a visible, signed decision is the one way debt may grow"
    );
}

#[test]
fn the_reseed_path_consults_the_monotonicity_predicate() {
    // The predicate was correct and unreachable. This asserts the wiring, not
    // the logic: a caller that stopped consulting it would leave every test
    // above passing while the CLI overwrote the baseline freely.
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/shape/facade/baseline.rs"
    ))
    .expect("baseline facade source");
    assert!(
        src.contains("regen_is_monotonic"),
        "the reseed path must consult the shrink-only predicate"
    );
    // The whole dispatcher, not one file of it. Splitting the shape arms into
    // `handlers/shape.rs` moved this symbol and broke the check while changing
    // nothing it was checking -- the third time this session a gate keyed on a
    // filename failed for a move.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cli");
    let mut cli = String::new();
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d)
            .expect("cli module is readable")
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                cli.push_str(&std::fs::read_to_string(&path).expect("cli source"));
            }
        }
    }
    assert!(
        !cli.is_empty(),
        "no cli sources read; this assertion would pass vacuously"
    );
    assert!(
        cli.contains("reseed_from_commit"),
        "`shape baseline` must go through the checked reseed, not the raw seed"
    );
}
