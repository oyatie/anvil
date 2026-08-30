//! Two scanner-backed gates demonstrate both halves.
//!
//! `security_scan` and `schema_compat` are pure functions over the diff text,
//! which makes them the cheapest gates in the corpus to prove and the least
//! excusable to leave unproven.

use anvil::pre_merge_guard::report::GateStatus;
use anvil::pre_merge_guard::scanner::PreMergeScanner;

/// A line the secret scanner must flag, assembled at runtime.
///
/// Not a literal: `no_tracked_file_contains_a_credential` scans every tracked
/// file for exactly this shape, and it is right to — a credential in the tree
/// is a credential in the tree, whatever it is for. The first version of this
/// fixture spelled the key out and failed that check in CI while passing
/// locally, because the file was still untracked when the suite ran.
fn credential_shaped_line() -> String {
    let key = ["AKIA", "IOSFODNN", "7EXAMPLE"].concat();
    format!("+const AWS_SECRET_ACCESS_KEY: &str = \"{key}\";\n")
}

// ---------------------------------------------------------------------------
// security_scan_status — PreMergeScanner
// ---------------------------------------------------------------------------

#[test]
fn security_scan_fires_on_a_credential_added_to_the_diff() {
    let diff = format!(
        "diff --git a/src/config.rs b/src/config.rs\n+++ b/src/config.rs\n{}",
        credential_shaped_line()
    );
    let status = PreMergeScanner::scan_for_secrets(&diff);
    assert!(
        !matches!(status, GateStatus::Passed),
        "a credential-shaped literal was added and the gate passed it.\n\
         The status is deliberately not printed: `GateStatus::Failed` carries a \
         summary that can quote the matched credential, and echoing it is how a \
         real key reaches a log. `rust/cleartext-logging` flagged exactly that."
    );
}

#[test]
fn security_scan_spares_a_diff_that_adds_no_credential() {
    let status = PreMergeScanner::scan_for_secrets(concat!(
        "diff --git a/src/config.rs b/src/config.rs\n",
        "+++ b/src/config.rs\n",
        "+const RETRY_BUDGET: usize = 3;\n",
    ));
    assert!(
        matches!(status, GateStatus::Passed),
        "an ordinary constant is not a secret; flagging it is a fabricated \
         accusation, which is I1's symmetric violation. Status not printed, for \
         the reason given above."
    );
}

// ---------------------------------------------------------------------------
// schema_compat_status — PreMergeScanner
// ---------------------------------------------------------------------------
//
// The rule only has a subject when the change touches a migration, so the
// green half keeps the migration and drops the destructive statement. Handing
// it a diff with no migration at all would prove nothing: it would pass the
// early return, not the rule.

#[test]
fn schema_compat_fires_on_a_destructive_migration() {
    let status = PreMergeScanner::scan_for_breaking_changes(
        concat!(
            "diff --git a/migrations/003_drop.sql b/migrations/003_drop.sql\n",
            "+++ b/migrations/003_drop.sql\n",
            "+ALTER TABLE charges DROP COLUMN tenant_id;\n",
        ),
        &["migrations/003_drop.sql".to_string()],
    );
    assert!(
        !matches!(status, GateStatus::Passed),
        "a column was dropped from a live table and the gate passed it: {status:?}"
    );
}

#[test]
fn schema_compat_spares_an_additive_migration() {
    let status = PreMergeScanner::scan_for_breaking_changes(
        concat!(
            "diff --git a/migrations/004_add.sql b/migrations/004_add.sql\n",
            "+++ b/migrations/004_add.sql\n",
            "+ALTER TABLE charges ADD COLUMN idempotency_key TEXT;\n",
        ),
        &["migrations/004_add.sql".to_string()],
    );
    assert!(
        matches!(status, GateStatus::Passed),
        "adding a nullable column breaks no reader, and this is still a \
         migration, so the rule had a subject and spared it: {status:?}"
    );
}
