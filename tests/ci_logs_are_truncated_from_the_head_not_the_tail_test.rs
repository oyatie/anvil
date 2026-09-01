//! CI logs are truncated from the end that carries the diagnostic.
//!
//! Integration rather than unit: the whole mechanism is `Untrusted`'s public
//! surface, so a test that cannot reach it from outside the crate is testing
//! the wrong thing (doctrine section 6).
//!
//! The defect these replace: `fetch_failed_run_logs` capped the last 20,000
//! CHARS and `cap_declaring` then kept the leading 20,000 BYTES of that. CI
//! output is multibyte, so chars exceeded bytes, the second cap fired, and what
//! survived was the FRONT of a tail -- `test result: FAILED`, the panic and the
//! exit code dropped out of the prompt whose only job is to diagnose them.
use anvil::reviewer::untrusted::{Untrusted, UntrustedLabel};

/// The defect this replaces: two caps in different units retaining opposite
/// ends meant a multibyte log kept the FRONT of a tail, so the diagnostic
/// never reached the model. Seeded here -- the marker sits at the very end,
/// past a body that is over the cap in bytes.
#[test]
fn an_over_cap_log_keeps_the_end_where_the_diagnostic_is() {
    let noise = "✓ ok → next ─────\n".repeat(3000);
    let logs = format!("{noise}error[E0308]: mismatched types\ntest result: FAILED\n");
    assert!(
        logs.len() > UntrustedLabel::CiLogs.max_chars(),
        "fixture must exceed the cap in BYTES, or it proves nothing"
    );

    let rendered = Untrusted::new(UntrustedLabel::CiLogs, &logs).render();

    assert!(
        rendered.contains("test result: FAILED"),
        "the last line of the log must survive truncation"
    );
    assert!(
        rendered.contains("error[E0308]"),
        "the diagnostic must survive truncation"
    );
    assert!(
        rendered.contains("Only the trailing portion is shown below"),
        "the notice must say which end was kept"
    );
}

/// The measured length is the log as fetched, not the size of whatever
/// survived an earlier cut. Truncating before `Untrusted` saw it made this
/// number describe the slice rather than the log (invariant I2).
#[test]
fn the_notice_reports_the_whole_log_not_the_surviving_slice() {
    let logs = "e".repeat(UntrustedLabel::CiLogs.max_chars() * 3);
    let rendered = Untrusted::new(UntrustedLabel::CiLogs, &logs).render();
    assert!(
        rendered.contains(&format!("is {} bytes", logs.len())),
        "the notice must name the original length"
    );
}

/// A log under the cap is passed through whole, with no notice.
#[test]
fn a_short_log_is_not_truncated_and_carries_no_notice() {
    let rendered = Untrusted::new(UntrustedLabel::CiLogs, "short log").render();
    assert!(rendered.contains("short log"));
    assert!(!rendered.contains("TRUNCATED"));
}

/// Multibyte content must not panic the slice, in either direction.
#[test]
fn truncation_lands_on_a_character_boundary() {
    let logs = "日本語テスト".repeat(20_000);
    assert!(logs.len() > UntrustedLabel::CiLogs.max_chars());
    let rendered = Untrusted::new(UntrustedLabel::CiLogs, &logs).render();
    assert!(rendered.contains("TRUNCATED"));
}
