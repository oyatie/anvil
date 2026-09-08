//! Unit tests for [`super::QueueHealer`].
//!
//! Split out because `queue_healer.rs` is 94 lines over the 300-line
//! budget this tree ratchets; the module is declared `#[cfg(test)]` by the
//! parent, so it ships nothing.

use super::*;

#[test]
fn test_extract_pr_number_from_merge_ref() {
    let r1 = "gh-readonly-queue/main/pr-824-7fd7839ed420c8952d5e56c0387350155a8d7fe6";
    assert_eq!(QueueHealer::extract_pr_number_from_merge_ref(r1), Some(824));

    let r2 = "refs/heads/gh-readonly-queue/dev/pr-104-abc";
    assert_eq!(QueueHealer::extract_pr_number_from_merge_ref(r2), Some(104));

    let r3 = "main";
    assert_eq!(QueueHealer::extract_pr_number_from_merge_ref(r3), None);
}

#[test]
fn agy_failure_is_a_failure_even_with_partial_stdout() {
    // 2026-08-20 13:41:45: agy exited 1 ("timeout waiting for response")
    // after streaming text; the healer treated it as a repair and pushed.
    let r = crate::exec::interpret_agy_outcome(
        false,
        "Inspecting the workspace...\n",
        "Error: timeout waiting for response\n",
    );
    let err = r.expect_err("non-zero agy exit must not be a repair");
    assert!(err.to_string().contains("timeout waiting for response"));

    let ok = crate::exec::interpret_agy_outcome(true, "done", "").unwrap();
    assert_eq!(ok, "done");
}

#[test]
fn healer_turn_limit_matches_model_class() {
    assert_eq!(AGY_TURN_LIMIT, crate::exec::ExecClass::Model.timeout());
    assert_eq!(crate::exec::agy_print_timeout_arg(AGY_TURN_LIMIT), "570s");
}

#[test]
fn only_open_prs_are_healed() {
    assert!(QueueHealer::pr_is_healable("OPEN"));
    assert!(QueueHealer::pr_is_healable("open"));
    assert!(!QueueHealer::pr_is_healable("MERGED"));
    assert!(!QueueHealer::pr_is_healable("CLOSED"));
    assert!(!QueueHealer::pr_is_healable(""));
}

#[test]
fn heal_note_reports_the_gate_that_ran() {
    let note = QueueHealer::heal_note("main", &TestGate::Passed("cargo test"), &Ok(()));
    assert!(note.contains("Local gate `cargo test` passed"));
    assert!(note.contains("trunk `main`"));
    assert!(!note.contains("Passed local test verification gate"));

    let note = QueueHealer::heal_note("dev", &TestGate::Unavailable, &Ok(()));
    assert!(note.contains("not verified"));
}

/// The note reports the re-enlistment that happened, not the one that was
/// about to be attempted.
#[test]
fn heal_note_reports_the_re_enlistment_outcome() {
    let enlisted = QueueHealer::heal_note("main", &TestGate::Passed("cargo test"), &Ok(()));
    let withheld = QueueHealer::heal_note(
        "main",
        &TestGate::Passed("cargo test"),
        &Err(anyhow::anyhow!("slo_status produced no measurement")),
    );
    assert_ne!(
        enlisted, withheld,
        "the same note was published for a heal that was re-enlisted and one that was not"
    );
    assert!(!enlisted.contains("Re-enlisting"));
    assert!(!withheld.contains("Re-enlisting"));
    assert!(withheld.contains("Not re-enlisted"));
    assert!(withheld.contains("slo_status produced no measurement"));
}

/// A gate that never completed is not a gate that reported failures.
#[test]
fn heal_note_separates_a_gate_that_did_not_complete_from_one_that_failed() {
    let failed = QueueHealer::heal_note("main", &TestGate::Failed("cargo test"), &Ok(()));
    let errored = QueueHealer::heal_note(
        "main",
        &TestGate::Errored(
            "cargo test",
            "No such file or directory (os error 2)".into(),
        ),
        &Ok(()),
    );
    assert!(failed.contains("FAILED"));
    assert!(!errored.contains("FAILED"));
    assert!(errored.contains("did not complete"));
    assert!(errored.contains("No such file or directory"));
}

#[test]
fn package_json_test_script_detection() {
    assert!(QueueHealer::package_json_has_test_script(
        br#"{"scripts":{"test":"vitest run"}}"#
    ));
    assert!(!QueueHealer::package_json_has_test_script(
        br#"{"scripts":{"build":"tsc"}}"#
    ));
    assert!(!QueueHealer::package_json_has_test_script(
        br#"{"scripts":{"test":"   "}}"#
    ));
    assert!(!QueueHealer::package_json_has_test_script(b"not json"));
}
