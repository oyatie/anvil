//! MSRV and the toolchain channel are two promises, not one number.
//!
//! Anvil declared `1.97.1` in `rust-toolchain.toml` and `1.97.1` as
//! `rust-version` while stable was `1.98.0`. That is not a coincidence to be
//! tidied; it is the signature of a pair nobody was managing. The two move in
//! opposite directions for opposite reasons -- the channel should chase stable
//! because each release carries soundness fixes and new deny-by-default lints,
//! and MSRV should lag because raising it strands consumers.

use anvil::toolchain::{Declared, Drift, Version, channel_from_toml, drift, msrv_from_manifest};

fn v(s: &str) -> Version {
    Version::parse(s).expect("parses")
}

#[test]
fn version_ordering_is_numeric_not_lexical() {
    // The window this module exists for: three-digit minors have arrived, and
    // "1.100.0" < "1.98.0" as text.
    assert!(v("1.100.0") > v("1.98.0"));
    assert_eq!(v("1.98.0").minors_behind(v("1.100.0")), 2);
}

#[test]
fn a_nightly_or_beta_suffix_still_parses() {
    assert_eq!(v("1.100.0-nightly"), v("1.100.0"));
}

#[test]
fn one_number_for_both_facts_is_itself_the_finding() {
    let d = Declared {
        channel: Some(v("1.97.1")),
        msrv: Some(v("1.97.1")),
    };
    let found = drift(&d, Some(v("1.98.0")), true);
    assert!(
        found.iter().any(|f| matches!(f, Drift::Conflated { .. })),
        "equal values are not 'consistent', they are unmanaged: {found:?}"
    );
}

#[test]
fn distinct_and_current_values_are_clean() {
    let d = Declared {
        channel: Some(v("1.98.0")),
        msrv: Some(v("1.94.0")),
    };
    assert!(drift(&d, Some(v("1.98.0")), true).is_empty());
}

#[test]
fn a_channel_inside_the_budget_is_not_a_finding() {
    // Two trains of slack. A budget of zero makes every release Tuesday a
    // finding and teaches readers to ignore the gate.
    let d = Declared {
        channel: Some(v("1.96.0")),
        msrv: Some(v("1.90.0")),
    };
    assert!(drift(&d, Some(v("1.98.0")), true).is_empty());
}

#[test]
fn a_channel_past_the_budget_is_reported_with_the_distance() {
    let d = Declared {
        channel: Some(v("1.94.0")),
        msrv: Some(v("1.90.0")),
    };
    let found = drift(&d, Some(v("1.98.0")), true);
    assert!(matches!(
        found.first(),
        Some(Drift::ChannelBehind { by: 4, .. })
    ));
}

#[test]
fn an_msrv_newer_than_the_channel_is_unbuildable_and_says_so() {
    let d = Declared {
        channel: Some(v("1.96.0")),
        msrv: Some(v("1.98.0")),
    };
    let found = drift(&d, Some(v("1.98.0")), true);
    assert!(
        found
            .iter()
            .any(|f| matches!(f, Drift::MsrvAheadOfChannel { .. })),
        "{found:?}"
    );
}

#[test]
fn an_undeclared_fact_is_reported_rather_than_defaulted() {
    let d = Declared {
        channel: None,
        msrv: Some(v("1.90.0")),
    };
    assert!(matches!(
        drift(&d, Some(v("1.98.0")), true).first(),
        Some(Drift::Undeclared { .. })
    ));
}

#[test]
fn an_unknown_latest_stable_withholds_the_lag_verdict_only() {
    // No network in a hermetic build. Not knowing stable must silence the lag
    // finding without silencing the two that need no external fact.
    let d = Declared {
        channel: Some(v("1.90.0")),
        msrv: Some(v("1.90.0")),
    };
    let found = drift(&d, None, true);
    assert!(found.iter().any(|f| matches!(f, Drift::Conflated { .. })));
    assert!(
        !found
            .iter()
            .any(|f| matches!(f, Drift::ChannelBehind { .. }))
    );
}

#[test]
fn a_commented_out_pin_is_not_the_pin() {
    let toml = "[toolchain]\n# channel = \"nightly\"\nchannel = \"1.98.0\"\n";
    assert_eq!(channel_from_toml(toml), Some(v("1.98.0")));
}

#[test]
fn the_manifest_field_is_found_among_others() {
    let m = "[package]\nname = \"x\"\nversion = \"0.1.0\"\nrust-version = \"1.94.0\"\n";
    assert_eq!(msrv_from_manifest(m), Some(v("1.94.0")));
    // `version` must not be mistaken for `rust-version`.
    assert_ne!(msrv_from_manifest(m), Some(v("0.1.0")));
}

#[test]
fn every_drift_explains_itself_in_terms_a_person_can_act_on() {
    for d in [
        Drift::Conflated { at: v("1.97.1") },
        Drift::ChannelBehind {
            channel: v("1.94.0"),
            by: 4,
        },
        Drift::MsrvAheadOfChannel {
            msrv: v("1.98.0"),
            channel: v("1.96.0"),
        },
        Drift::Undeclared { which: "MSRV" },
    ] {
        let e = d.explain();
        assert!(e.len() > 40, "{d:?} explains nothing useful");
        assert!(
            !e.contains("  "),
            "wrapped literal leaked indentation: {e:?}"
        );
    }
}

#[test]
fn anvils_own_pair_is_distinct() {
    // This asserted `Conflated` when it was written, and said the change that
    // fixed it would update it. That change was `anvil toolchain --apply`,
    // which probed 1.98 and moved the channel while MSRV stayed at the last
    // version this tree was actually tested on.
    let d = anvil::toolchain::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
    let found = drift(&d, None, true);
    assert!(
        !found.iter().any(|f| matches!(f, Drift::Conflated { .. })),
        "the channel and MSRV must stay two decisions. Got: {found:?}"
    );
    assert!(
        d.channel > d.msrv,
        "the channel leads MSRV: we compile with something at least as new as \
         what we promise consumers. channel={:?} msrv={:?}",
        d.channel,
        d.msrv
    );
}

#[test]
fn a_trailing_comment_does_not_hide_the_pin() {
    // The first draft returned None here: after trimming quotes,
    // `1.98.0"  # bumped` has a patch component that does not parse. A version
    // this module cannot see is one it cannot report as behind -- the quiet
    // direction of the failure.
    //
    // The test above was meant to cover commentary and proved nothing: a
    // whole-line comment never matches the key anyway, because the key carries
    // the `#`. Seeding it is what showed the assertion was blind.
    let toml = "[toolchain]\nchannel = \"1.98.0\"  # bumped 2026-08\n";
    assert_eq!(channel_from_toml(toml), Some(v("1.98.0")));
}

#[test]
fn an_msrv_nothing_builds_under_is_a_claim_not_a_measurement() {
    // Separating MSRV from the channel is only an improvement if the lower
    // number is then PROVEN. Unproven, it is worse than conflation: it looks
    // managed and is not. Anvil is in exactly this state -- every CI job
    // installs the channel and nothing installs 1.97.1.
    let d = Declared {
        channel: Some(v("1.98.0")),
        msrv: Some(v("1.97.1")),
    };
    let found = drift(&d, Some(v("1.98.0")), false);
    assert!(
        found
            .iter()
            .any(|f| matches!(f, Drift::MsrvUnverified { .. })),
        "an MSRV no build exercises must be reported: {found:?}"
    );
    // And it is NOT reported as conflated -- the two are different failures
    // with different remedies.
    assert!(!found.iter().any(|f| matches!(f, Drift::Conflated { .. })));
}

#[test]
fn a_proven_msrv_is_clean() {
    let d = Declared {
        channel: Some(v("1.98.0")),
        msrv: Some(v("1.97.1")),
    };
    assert!(drift(&d, Some(v("1.98.0")), true).is_empty());
}
