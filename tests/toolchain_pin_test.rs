//! One pin, named exactly, and what the chore may decide on its own.
//!
//! This file replaces a suite about a PAIR: a channel and an MSRV, which move
//! in opposite directions for opposite reasons. That pair is gone -- anvil is
//! `publish = false` with no dependent in the organisation, so the MSRV
//! contract had no counterparty and no job ever built under it.
//!
//! What survives is the principle the pair was there to serve: a number this
//! repository declares must be one it exercises, and a chore must be able to
//! see the number move.

use anvil::toolchain::{Channel, Declared, Drift, Version, channel_from_toml, channel_text, drift};

fn nightly(date: &str) -> Channel {
    Channel::parse(&format!("nightly-{date}")).expect("a dated nightly parses")
}

#[test]
fn a_dated_nightly_and_a_release_are_both_channels() {
    assert_eq!(
        Channel::parse("nightly-2026-09-09"),
        Some(Channel::Nightly("2026-09-09".to_string()))
    );
    assert_eq!(
        Channel::parse("1.98.1"),
        Some(Channel::Release(Version::parse("1.98.1").unwrap()))
    );
    // Floating `nightly` is not pinnable: it names a different compiler every
    // day, so a build under it is not reproducible and a bump has nothing to
    // move from.
    assert_eq!(Channel::parse("nightly"), None);
    assert_eq!(Channel::parse("stable"), None);
    // Shape, not calendar. A date rustup does not publish fails at install.
    assert_eq!(Channel::parse("nightly-2026-9-9"), None);
    assert_eq!(Channel::parse("nightly-20260909"), None);
}

#[test]
fn dates_order_as_text_across_day_and_month_boundaries() {
    // The whole reason the date is kept as text rather than parsed.
    assert_eq!(
        nightly("2026-09-10").newer_than(&nightly("2026-09-09")),
        Some(true)
    );
    assert_eq!(
        nightly("2026-10-01").newer_than(&nightly("2026-09-30")),
        Some(true)
    );
    assert_eq!(
        nightly("2026-09-09").newer_than(&nightly("2026-09-10")),
        Some(false)
    );
}

#[test]
fn no_ordering_is_defined_between_a_release_and_a_nightly() {
    let release = Channel::parse("1.98.1").unwrap();
    // Not "equal" and not "unknown": undefined. A caller that read `None` as
    // either would let the weekly chore move this repository between channel
    // kinds, which is a decision about how it is built.
    assert_eq!(release.newer_than(&nightly("2026-09-09")), None);
    assert_eq!(nightly("2026-09-09").newer_than(&release), None);
}

#[test]
fn a_pin_behind_the_channel_is_the_finding_and_names_both() {
    let declared = Declared {
        channel: Some(nightly("2026-09-09")),
    };
    let found = drift(&declared, Some(nightly("2026-09-16")));
    let Some(Drift::ChannelBehind { channel, latest }) = found.first() else {
        panic!("a pin a week behind must be reported. Got: {found:?}");
    };
    assert_eq!(channel, &nightly("2026-09-09"));
    assert_eq!(latest, &nightly("2026-09-16"));
    assert!(
        channel.to_string().contains("nightly-2026-09-09"),
        "the finding must name the pin as written"
    );
}

#[test]
fn a_current_pin_and_an_unknown_latest_are_both_clean() {
    let declared = Declared {
        channel: Some(nightly("2026-09-09")),
    };
    assert!(drift(&declared, Some(nightly("2026-09-09"))).is_empty());
    // Absent evidence is not a finding: if the manifest could not be read,
    // nothing is known about whether the pin is behind.
    assert!(drift(&declared, None).is_empty());
}

#[test]
fn a_change_of_channel_kind_is_reported_as_a_decision_not_a_bump() {
    let declared = Declared {
        channel: Some(nightly("2026-09-09")),
    };
    let found = drift(&declared, Some(Channel::parse("1.98.1").unwrap()));
    assert!(
        matches!(found.first(), Some(Drift::ChannelKindChanged { .. })),
        "moving between a release and a nightly is not a bump. Got: {found:?}"
    );
    assert!(
        found[0].explain().contains("decision"),
        "the explanation must say why it is not proposed automatically: {}",
        found[0].explain()
    );
}

#[test]
fn an_undeclared_pin_is_measured_as_absent_rather_than_assumed() {
    let found = drift(&Declared { channel: None }, Some(nightly("2026-09-09")));
    assert!(
        matches!(found.first(), Some(Drift::Undeclared { .. })),
        "an absent pin cannot be measured. Got: {found:?}"
    );
}

#[test]
fn a_trailing_comment_does_not_hide_the_pin() {
    // The quiet direction of the failure: a pin this module cannot see is one
    // it cannot report as behind.
    let text = "[toolchain]\nchannel = \"nightly-2026-09-09\" # bumped weekly\n";
    assert_eq!(channel_text(text), Some("nightly-2026-09-09"));
    assert_eq!(channel_from_toml(text), Some(nightly("2026-09-09")));
    assert_eq!(channel_text("# channel = \"nightly-2026-09-09\"\n"), None);
}

#[test]
fn this_repository_pins_a_dated_nightly_and_promises_no_msrv() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let declared = anvil::toolchain::read(root);
    let channel = declared
        .channel
        .expect("this repository declares a channel");
    assert!(
        matches!(channel, Channel::Nightly(_)),
        "the pin is a dated nightly, tracked ahead of stable so a breaking lint \
         is met in a weekly bump rather than on the day someone moves stable. \
         Got: {channel}"
    );

    // The MSRV is not merely absent from the type: it must be absent from the
    // manifest, or it is a promise nothing exercises.
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).expect("the manifest reads");
    assert!(
        !manifest.contains("rust-version"),
        "anvil is publish = false with no dependent, so an MSRV has no \
         counterparty and no job builds under it"
    );

    // The binary's own pin and the file agree, because there is only one.
    assert_eq!(anvil::toolchain::pinned_channel(), channel.to_string());
}
