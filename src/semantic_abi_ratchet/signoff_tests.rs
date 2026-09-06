use super::{AbiScan, SEMANTIC_ABI_GATE_ID, SignatureScanner, abi_key, assess};
use crate::pre_merge_guard::report::GateStatus;
use crate::ratchet::facade::Signoff;

pub(super) const REPO: &str = "oyatie/anvil";
pub(super) const BEFORE: &str = "pub fn api() -> u8 {";
pub(super) const AFTER: &str = "pub fn api() -> u16 {";

pub(super) fn transition(old: &str, new: &str, before: &str, after: &str) -> AbiScan {
    SignatureScanner::new().scan_abi_diff(&format!(
        "diff --git a/{old} b/{new}\n--- a/{old}\n+++ b/{new}\n@@ -1 +1 @@\n-{before}\n+{after}\n"
    ))
}

pub(super) fn ordinary() -> AbiScan {
    transition("src/api.rs", "src/api.rs", BEFORE, AFTER)
}

pub(super) fn signing(keys: &[String]) -> Signoff {
    let data = serde_json::json!({
        "schema": "anvil/ratchet-signoff/v1",
        "_sign_off_additions": {SEMANTIC_ABI_GATE_ID: keys},
        "signings": [{"by": "test fixture", "date": "2026-09-06", "note": "in-memory only"}]
    });
    Signoff::parse(&serde_json::to_vec(&data).unwrap()).unwrap()
}

pub(super) fn approved() -> Signoff {
    signing(&[abi_key(REPO, &ordinary().findings[0]).expect("complete transition")])
}

#[test]
fn different_transitions_of_one_symbol_have_different_authorization_keys() {
    let scan = |after: &str| {
        SignatureScanner::new().scan_abi_diff(&format!(
            "diff --git a/src/api.rs b/src/api.rs\n--- a/src/api.rs\n+++ b/src/api.rs\n\
             @@ -1 +1 @@\n-pub fn api() -> u8 {{\n+pub fn api() -> {after} {{\n"
        ))
    };
    let first = scan("u16");
    let later = scan("u32");
    assert_eq!(first.findings.len(), 1);
    assert_eq!(later.findings.len(), 1);
    assert_ne!(
        abi_key(REPO, &first.findings[0]).expect("complete first transition"),
        abi_key(REPO, &later.findings[0]).expect("complete later transition"),
        "a recorded transition must not authorize a different signature change"
    );
}

#[test]
fn exact_transition_matches_but_changed_sides_and_reverse_do_not() {
    let signed = approved();
    assert!(matches!(
        assess(REPO, ordinary(), &signed).status,
        GateStatus::Warning(_)
    ));
    for (before, after) in [
        (BEFORE, "pub fn api() -> u32 {"),
        ("pub fn api() -> u64 {", AFTER),
        (AFTER, BEFORE),
    ] {
        let scan = transition("src/api.rs", "src/api.rs", before, after);
        assert_eq!(scan.findings.len(), 1);
        assert!(matches!(
            assess(REPO, scan, &signed).status,
            GateStatus::Failed(_)
        ));
    }
}

#[test]
fn a_signature_signoff_does_not_cover_removal_and_removal_binds_its_before_line() {
    let removal = |line: &str| {
        SignatureScanner::new().scan_abi_diff(&format!(
        "diff --git a/src/api.rs b/src/api.rs\n--- a/src/api.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-{line}\n"
    ))
    };
    let scan = removal(BEFORE);
    assert_eq!(scan.findings.len(), 1);
    let key = abi_key(REPO, &scan.findings[0]).expect("complete removal");
    assert!(key.contains("\"REMOVAL\""));
    assert!(matches!(
        assess(REPO, scan, &approved()).status,
        GateStatus::Failed(_)
    ));
    let signed = signing(&[key]);
    assert!(matches!(
        assess(REPO, removal(BEFORE), &signed).status,
        GateStatus::Warning(_)
    ));
    assert!(matches!(
        assess(REPO, removal(AFTER), &signed).status,
        GateStatus::Failed(_)
    ));
}

#[test]
fn repository_both_paths_and_symbol_are_bound() {
    let signed = approved();
    for (old, new) in [("src/old.rs", "src/api.rs"), ("src/api.rs", "src/new.rs")] {
        let scan = transition(old, new, BEFORE, AFTER);
        assert_eq!(scan.findings.len(), 1);
        assert!(matches!(
            assess(REPO, scan, &signed).status,
            GateStatus::Failed(_)
        ));
    }
    for repo in ["oyatie/other", "OYATIE/anvil", "", " ", " oyatie/anvil"] {
        assert!(matches!(
            assess(repo, ordinary(), &signed).status,
            GateStatus::Failed(_)
        ));
    }
    let scan = transition(
        "src/api.rs",
        "src/api.rs",
        &BEFORE.replace("api", "other"),
        &AFTER.replace("api", "other"),
    );
    assert!(matches!(
        assess(REPO, scan, &signed).status,
        GateStatus::Failed(_)
    ));
}

#[test]
fn key_tuple_contains_true_rename_paths_and_distinct_fields() {
    let scan = transition("src/old.rs", "src/new.rs", BEFORE, AFTER);
    let key = abi_key(REPO, &scan.findings[0]).unwrap();
    let fields: serde_json::Value =
        serde_json::from_str(key.strip_prefix("abi-change/v2:").unwrap()).unwrap();
    assert_eq!(
        fields,
        serde_json::json!([
            REPO,
            "SIGNATURE_CHANGE",
            "src/old.rs",
            "src/new.rs",
            "api",
            BEFORE,
            AFTER
        ])
    );
}

#[test]
fn line_moves_context_and_indentation_leave_authority_unchanged() {
    let moved = SignatureScanner::new().scan_abi_diff(&format!(
        "diff --git a/src/api.rs b/src/api.rs\n--- a/src/api.rs\n+++ b/src/api.rs\n\
         @@ -90,2 +110,2 @@\n // unchanged context\n-    {BEFORE}  \n+  {AFTER}\n"
    ));
    assert_eq!(moved.findings.len(), 1);
    assert_eq!(
        abi_key(REPO, &ordinary().findings[0]),
        abi_key(REPO, &moved.findings[0])
    );
}

#[test]
fn authority_does_not_erase_internal_whitespace() {
    let respaced = transition("src/api.rs", "src/api.rs", "pub fn api()  -> u8 {", AFTER);
    assert_eq!(respaced.findings.len(), 1);
    assert_ne!(
        abi_key(REPO, &ordinary().findings[0]),
        abi_key(REPO, &respaced.findings[0])
    );
}

#[test]
fn incomplete_inline_and_ambiguous_removals_remain_unsigned_findings() {
    for body in [
        "-pub fn api(\n",
        "-pub fn api()\n",
        "-pub fn api() {}\n",
        "-pub fn api() -> u8 {\n-pub fn api() -> u8 {\n",
    ] {
        let scan = SignatureScanner::new().scan_abi_diff(&format!(
            "diff --git a/src/api.rs b/src/api.rs\n--- a/src/api.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n{body}"
        ));
        assert!(!scan.findings.is_empty());
        assert!(
            scan.findings
                .iter()
                .all(|finding| abi_key(REPO, finding).is_none())
        );
        assert!(matches!(
            assess(REPO, scan, &approved()).status,
            GateStatus::Failed(_)
        ));
    }
}

#[test]
fn unsupported_paired_headers_do_not_acquire_authority() {
    for (before, after) in [
        ("pub fn api() -> u8", "pub fn api() -> u16"),
        ("pub fn api() -> u8 {}", "pub fn api() -> u16 {}"),
    ] {
        let scan = transition("src/api.rs", "src/api.rs", before, after);
        assert_eq!(scan.findings.len(), 1);
        assert!(abi_key(REPO, &scan.findings[0]).is_none());
    }
}

#[test]
fn legacy_other_and_malformed_keys_do_not_authorize() {
    for key in [
        "api@src/api.rs",
        "other@src/other.rs",
        "abi-change/v2:not-json",
        "future:api",
    ] {
        assert!(matches!(
            assess(REPO, ordinary(), &signing(&[key.into()])).status,
            GateStatus::Failed(_)
        ));
    }
    for bytes in [b"".as_slice(), b"not json", b"{}"] {
        let signed = Signoff::parse(bytes).unwrap_or_default();
        assert!(matches!(
            assess(REPO, ordinary(), &signed).status,
            GateStatus::Failed(_)
        ));
    }
}

#[test]
fn deserialized_findings_have_no_authorizing_evidence() {
    let bytes = serde_json::to_vec(&ordinary().findings[0]).unwrap();
    let finding = serde_json::from_slice(&bytes).unwrap();
    assert!(abi_key(REPO, &finding).is_none());
}

const COMMITTED: &[u8] = include_bytes!("../../.anvil/baselines/semantic-abi.signoff.json");

pub(super) fn historical_body(before: &str) -> AbiScan {
    transition(
        "src/publish/mod.rs",
        "src/publish/mod.rs",
        before,
        "pub fn body(action: AnvilAction, content: &str, judged: Judged) -> Published {",
    )
}

#[test]
fn committed_keys_bind_only_the_two_historical_decisions() {
    let signed = Signoff::parse(COMMITTED).unwrap();
    assert_eq!(signed.additions.len(), 1);
    assert_eq!(signed.additions[SEMANTIC_ABI_GATE_ID].len(), 2);
    assert!(signed.mode_downgrades.is_empty());
    for scan in [
        historical_body(
            "pub fn body(action: AnvilAction, content: &str, judged: Judged) -> String {",
        ),
        transition(
            "src/shape/facade/sweep.rs",
            "src/shape/facade/sweep.rs",
            "pub async fn sweep_repo(deps: &SweepDeps, repo: &str) -> Result<String, String> {",
            "pub async fn sweep_repo(deps: &SweepDeps, repo: &str) -> Result<Swept, String> {",
        ),
    ] {
        assert_eq!(scan.findings.len(), 1);
        assert!(signed.covers(
            SEMANTIC_ABI_GATE_ID,
            &abi_key(REPO, &scan.findings[0]).unwrap()
        ));
        assert!(matches!(
            assess(REPO, scan, &signed).status,
            GateStatus::Warning(_)
        ));
    }
    let cumulative = historical_body("pub fn body(action: AnvilAction, content: &str) -> String {");
    assert!(matches!(
        assess(REPO, cumulative, &signed).status,
        GateStatus::Failed(_)
    ));
}

#[test]
fn historical_signing_object_is_preserved_without_a_new_signing() {
    let signed = Signoff::parse(COMMITTED).unwrap();
    assert_eq!(signed.signings.len(), 1);
    let original = &signed.signings[0];
    assert_eq!(original.by, "Jason Lee");
    assert_eq!(original.date, "2026-08-28");
    assert_eq!(
        original.note,
        "Both signatures changed to carry a distinction the old ones could not. `body` returns the `Published` newtype so a caller cannot hand back an unpublished string, and `sweep_repo` returns `Swept::{Measured, Skipped}` so a sweep that examined nothing stops reading as a clean one. Every caller moves in the same change; anvil is not a published library, so no consumer exists outside this tree. Both entries go inert once this merges, because the gate reads a diff and the diff will no longer carry them."
    );
}
