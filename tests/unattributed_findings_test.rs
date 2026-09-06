//! A finding with no file is not a finding.
//!
//! The last shape of the attribution class. `feature_flag_ratchet` and
//! `chaos_injector` seeded their file cursor with `String::new()`, so a `+`
//! line arriving before the diff's first `+++ b/` header produced a finding
//! whose path was the empty string. Measured against the old code:
//!
//! ```text
//! flag refs:      [("", "new_billing")]
//! chaos findings: [""]
//! ```
//!
//! This is the mildest member of the class -- an empty path is visibly wrong
//! rather than misleading, unlike `unknown.rs`, `manifest.yaml` or the
//! genuinely-innocent `src/innocent.rs`. It is still a claim the code cannot
//! support: for the flag scan in particular, a reference is the EVIDENCE that
//! a flag is still used, so an unattributed one is evidence of nothing.
//!
//! Two of the seven gates with this seed needed fixing. `debt_shrink_guard` and
//! `modularization_guard` already guarded on `!current_file.is_empty()`,
//! `ghost_migration_harness` is guarded by `is_migration_file("")` being false,
//! and `clean_architecture_guard` by `classify_layer("")` yielding no layer.
//! They are named here so a later reader does not have to re-derive that.

use anvil::chaos_injector::ChaosFaultInjector;
use anvil::feature_flag_ratchet::FeatureFlagRatchet;

const FLAG_USE: &str = r#"if flags.get("new_billing") { }"#;
const BAD_AWAIT: &str = "let v = client.get().await.unwrap();";

#[test]
fn a_flag_reference_before_any_header_is_not_recorded() {
    let refs = FeatureFlagRatchet::scan_flag_references(&format!("+{FLAG_USE}\n"));
    assert!(
        refs.is_empty(),
        "a flag reference was recorded against no file: {:?}",
        refs.iter()
            .map(|r| (r.file_path.clone(), r.flag_key.clone()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_flag_reference_is_recorded_once_the_diff_names_the_file() {
    let refs = FeatureFlagRatchet::scan_flag_references(&format!(
        "--- a/src/billing.rs\n+++ b/src/billing.rs\n+{FLAG_USE}\n"
    ));
    assert_eq!(refs.len(), 1, "the scan must not have gone inert: {refs:?}");
    assert_eq!(refs[0].file_path, "src/billing.rs");
    assert_eq!(refs[0].flag_key, "new_billing");
}

#[test]
fn an_unhandled_await_before_any_header_is_not_accused() {
    let report = ChaosFaultInjector
        .scan_for_unhandled_await_without_a_running_system(&format!("+{BAD_AWAIT}\n"));
    assert!(
        report
            .unhandled_awaits
            .iter()
            .all(|u| !u.file_path.is_empty()),
        "an unhandled-await finding carried an empty path: {:?}",
        report
            .unhandled_awaits
            .iter()
            .map(|u| u.file_path.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn an_unhandled_await_is_still_found_once_the_diff_names_the_file() {
    let report = ChaosFaultInjector.scan_for_unhandled_await_without_a_running_system(&format!(
        "--- a/src/client.rs\n+++ b/src/client.rs\n+{BAD_AWAIT}\n"
    ));
    assert_eq!(
        report.unhandled_awaits.len(),
        1,
        "the scan must not have gone inert: {:?}",
        report.unhandled_awaits
    );
    assert_eq!(report.unhandled_awaits[0].file_path, "src/client.rs");
}
