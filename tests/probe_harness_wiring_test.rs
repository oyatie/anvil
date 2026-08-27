//! The probe grades the developer's commit, or says it did not grade one.
//!
//! # Two defects, one class
//!
//! The probe passed the literal `"chore: probe check"` to the commit-header
//! check -- a constant this codebase wrote, answering a constant this codebase
//! also wrote. It could not fail, and its green described a string in
//! `cli/handlers.rs` rather than the commit in front of the developer.
//!
//! `local_inner_loop`'s own module documentation records exactly that defect
//! being found and removed from `evaluate_local_probe`. It survived at the
//! other call site, which is what fixing an instance rather than a class
//! leaves behind.
//!
//! Separately, `Harness::run` had no production consumer at all. Four rules --
//! including `secret_on_added_line` -- were registered, fixtured and examined
//! nothing. This is its first caller, placed at the probe because a secret is
//! worth catching before the commit exists rather than on a scorecard
//! afterwards.

use anvil::local_inner_loop::{FastValidator, harness_findings};

/// A diff carrying something credential-shaped, assembled at runtime.
///
/// Never a contiguous literal in committed source. `SecretOnAddedLine`'s own
/// fixture already carried this warning -- "a seeded defect for a credential
/// scanner necessarily contains something credential-shaped, and writing it
/// whole makes this file a finding against itself" -- and this file was
/// written with the token spelled out anyway. Two of anvil's own guards
/// refused the commit: the tracked-file scan and the last-20-commits scan.
fn diff_with_secret() -> String {
    let token = format!("ghp{}{}", "_", "A".repeat(36));
    format!(
        "diff --git a/src/config.rs b/src/config.rs\n\
         --- a/src/config.rs\n+++ b/src/config.rs\n@@ -1,0 +1,1 @@\n\
         +const TOKEN: &str = \"{token}\";\n"
    )
}

const CLEAN_DIFF: &str = "\
diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,0 +1,1 @@
+pub fn added() {}
";

#[test]
fn the_harness_actually_runs_and_reports_per_rule() {
    let findings = harness_findings(CLEAN_DIFF, None);
    assert!(
        !findings.is_empty(),
        "Harness::run had no production consumer at all; this is the assertion \
         that it has one now"
    );
    assert!(
        findings
            .iter()
            .all(|f| f.check_name.starts_with("harness:")),
        "every finding must name the rule it came from"
    );
}

#[test]
fn a_rule_that_could_not_run_is_reported_not_omitted() {
    // No commit message, so the subject rule has no input.
    let findings = harness_findings(CLEAN_DIFF, None);
    let subject = findings
        .iter()
        .find(|f| f.check_name.contains("conventional_commit_subject"))
        .expect(
            "the subject rule must appear even with no message. Omitting it \
             would be indistinguishable from a rule that ran and found nothing \
             -- the exact confusion the hardcoded commit literal created.",
        );
    assert!(
        subject.message.contains("NOT MEASURED"),
        "got: {}",
        subject.message
    );
}

#[test]
fn a_supplied_commit_message_is_actually_measured() {
    let findings = harness_findings(CLEAN_DIFF, Some("feat(probe): wire the harness"));
    let subject = findings
        .iter()
        .find(|f| f.check_name.contains("conventional_commit_subject"))
        .expect("subject rule present");
    assert!(
        !subject.message.contains("NOT MEASURED"),
        "with a message supplied the rule must actually measure it. Got: {}",
        subject.message
    );
}

#[test]
fn the_secret_rule_fires_on_a_secret_in_the_staged_diff() {
    let findings = harness_findings(&diff_with_secret(), None);
    let secret: Vec<_> = findings
        .iter()
        .filter(|f| f.check_name.contains("secret_on_added_line") && !f.is_valid)
        .collect();
    assert_eq!(
        secret.len(),
        1,
        "a credential on an added line must be caught at pre-commit, which is \
         the whole reason the harness is wired here rather than at \
         certification. Got: {findings:#?}"
    );
}

#[test]
fn the_secret_rule_spares_a_clean_diff() {
    let findings = harness_findings(CLEAN_DIFF, None);
    assert!(
        findings
            .iter()
            .filter(|f| f.check_name.contains("secret_on_added_line"))
            .all(|f| f.is_valid),
        "a rule that flags everything is as useless as one that flags nothing"
    );
}

#[test]
fn the_validator_no_longer_needs_an_invented_commit_message() {
    // `scan_staged_diff` is the half the probe can perform with no message.
    // Its existence as a separate entry point is what lets the probe stop
    // inventing one.
    let only_diff = FastValidator::new().scan_staged_diff(&diff_with_secret());
    assert!(
        !only_diff.is_valid,
        "the diff half must stand alone, so the header half can be omitted \
         honestly rather than fed a literal"
    );
}
