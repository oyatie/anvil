use super::signoff_tests::{REPO, approved, ordinary, signing};
use super::{LAYOUT_DISCLAIMER, SEMANTIC_ABI_GATE_ID, SignatureScanner, assess};
use crate::pre_merge_guard::report::{GateStatus, PreMergeCertificationReport};

#[test]
fn authority_header_admission_requires_complete_unambiguous_raw_paths() {
    use super::signature_scanner::evidence_header_path;
    for (header, prefix, expected) in [
        ("a/src/api.rs", "a/", Some("src/api.rs")),
        ("b/src/api.rs", "b/", Some("src/api.rs")),
        ("\"a/src/api.rs\"", "a/", None),
        ("a/src/api.rs\u{7}", "a/", None),
        ("a/src/api.rs", "b/", None),
        ("b/src/api.rs", "a/", None),
        ("", "a/", None),
        ("a/", "a/", None),
        ("b/", "b/", None),
        ("/dev/null", "b/", None),
        ("a/src/api.rs copy.rs", "a/", None),
        ("b/src/api.rs\t", "b/", None),
        ("a/src/api.rs\\copy.rs", "a/", None),
    ] {
        assert_eq!(evidence_header_path(header, prefix), expected, "{header:?}");
    }
}

#[test]
fn ambiguous_header_paths_cannot_inherit_an_exact_signoff() {
    for (before_header, after_header) in [
        ("a/src/api.rs copy.rs", "b/src/api.rs"),
        ("a/src/api.rs", "b/src/api.rs copy.rs"),
        ("a/src/api.rs copy.rs", "b/src/api.rs copy.rs"),
        ("a/src/api.rs\tcopy.rs", "b/src/api.rs"),
        ("a/src/api.rs", "b/src/api.rs\t"),
        ("a/src/api.rs\\copy.rs", "b/src/api.rs"),
    ] {
        let diff = format!(
            "diff --git a/src/api.rs b/src/api.rs\n--- {before_header}\n+++ {after_header}\n@@ -1 +1 @@\n-pub fn api() -> u8 {{\n+pub fn api() -> u16 {{\n"
        );
        let scan = SignatureScanner::new().scan_abi_diff(&diff);
        assert_eq!(scan.findings.len(), 1, "detector behavior is preserved");
        assert!(super::abi_key(REPO, &scan.findings[0]).is_none());
        assert!(matches!(
            assess(REPO, scan, &approved()).status,
            GateStatus::Failed(_)
        ));
    }
}

#[test]
fn ambiguous_removed_path_cannot_inherit_a_removal_signoff() {
    let removal = |header: &str| {
        SignatureScanner::new().scan_abi_diff(&format!(
        "diff --git a/src/api.rs b/src/api.rs\n--- {header}\n+++ /dev/null\n@@ -1 +0,0 @@\n-pub fn api() -> u8 {{\n"
    ))
    };
    let ordinary = removal("a/src/api.rs");
    assert_eq!(ordinary.findings.len(), 1);
    let key = super::abi_key(REPO, &ordinary.findings[0]).expect("supported removal");
    let signed = signing(&[key]);
    assert!(matches!(
        assess(REPO, ordinary, &signed).status,
        GateStatus::Warning(_)
    ));
    let ambiguous = removal("a/src/api.rs copy.rs");
    assert_eq!(
        ambiguous.findings.len(),
        1,
        "detector behavior is preserved"
    );
    assert!(super::abi_key(REPO, &ambiguous.findings[0]).is_none());
    assert!(matches!(
        assess(REPO, ambiguous, &signed).status,
        GateStatus::Failed(_)
    ));
}

#[test]
fn accepted_warning_keeps_uncompared_name_disclosure() {
    let diff = concat!(
        "diff --git a/src/api.rs b/src/api.rs\n--- a/src/api.rs\n+++ b/src/api.rs\n@@ -1 +1 @@\n",
        "-pub fn api() -> u8 {\n+pub fn api() -> u16 {\n",
        "diff --git a/src/other.rs b/src/other.rs\n--- a/src/other.rs\n+++ b/src/other.rs\n@@ -1 +1 @@\n",
        "-pub fn other(\n+pub fn other(a: u8) {\n",
    );
    let scan = SignatureScanner::new().scan_abi_diff(diff);
    assert_eq!(scan.unpaired_names, 1);
    assert_eq!(scan.findings.len(), 1);
    let report = assess(REPO, scan, &approved());
    let GateStatus::Warning(reason) = &report.status else {
        panic!("{:?}", report.status)
    };
    assert!(reason.contains("1 name(s)"));
    assert!(reason.contains("not compared"));
    assert!(reason.contains("accepted by exact recorded signoff"));
    assert_eq!(reason, &report.summary);
}

#[test]
fn an_accepted_break_is_observed_not_reported_as_abi_stable() {
    let report = assess(REPO, ordinary(), &approved());
    assert!(!report.is_abi_stable);
    assert_eq!(report.breaking_findings.len(), 1);
    assert!(report.breaking_findings[0].detail.contains("u8"));
    assert!(report.breaking_findings[0].detail.contains("u16"));
    assert_eq!(report.status, GateStatus::Warning(report.summary.clone()));
    assert!(
        report
            .summary
            .contains("accepted by exact recorded signoff")
    );
    assert!(
        report
            .summary
            .contains("SIGNATURE_CHANGE api at src/api.rs")
    );
    assert!(!report.summary.contains("no compared signature changed"));
    assert!(report.summary.contains(LAYOUT_DISCLAIMER));
    assert!(report.status.is_acceptable());
}

#[test]
fn an_unaccepted_break_still_fails() {
    let report = assess(REPO, ordinary(), &signing(&[]));
    assert_eq!(report.status, GateStatus::Failed(report.summary.clone()));
    assert!(!report.status.is_acceptable());
    assert!(!report.is_abi_stable);
    assert_eq!(report.breaking_findings.len(), 1);
    assert!(report.summary.contains("1 unaccepted"));
    assert!(report.summary.contains(LAYOUT_DISCLAIMER));
}

#[test]
fn accepted_and_unaccepted_breaks_remain_visible_together() {
    let mut scan = ordinary();
    let other = SignatureScanner::new().scan_abi_diff(
        "diff --git a/src/other.rs b/src/other.rs\n--- a/src/other.rs\n+++ /dev/null\n\
         @@ -1 +0,0 @@\n-pub fn other() -> u8 {\n",
    );
    scan.findings.extend(other.findings);
    let report = assess(REPO, scan, &approved());
    assert_eq!(report.status, GateStatus::Failed(report.summary.clone()));
    assert_eq!(report.breaking_findings.len(), 2);
    for text in [
        "1 unaccepted",
        "REMOVAL other",
        "accepted by exact recorded signoff",
        "SIGNATURE_CHANGE api",
        LAYOUT_DISCLAIMER,
    ] {
        assert!(report.summary.contains(text), "{text}: {}", report.summary);
    }
}

#[test]
fn layout_still_withholds_and_its_status_retains_the_accepted_change() {
    let diff = concat!(
        "diff --git a/src/api.rs b/src/api.rs\n--- a/src/api.rs\n+++ b/src/api.rs\n@@ -1 +1 @@\n",
        "-pub fn api() -> u8 {\n+pub fn api() -> u16 {\n",
        "diff --git a/src/wire.rs b/src/wire.rs\n--- a/src/wire.rs\n+++ b/src/wire.rs\n@@ -1 +1 @@\n",
        "+#[repr(C)]\n",
    );
    let report = assess(
        REPO,
        SignatureScanner::new().scan_abi_diff(diff),
        &approved(),
    );
    let GateStatus::NotMeasured { gate_id, reason } = &report.status else {
        panic!("{:?}", report.status)
    };
    assert_eq!(gate_id, SEMANTIC_ABI_GATE_ID);
    assert!(crate::pre_merge_guard::absence_blocks(gate_id));
    for text in [
        "add or remove",
        "accepted by exact recorded signoff",
        "SIGNATURE_CHANGE api",
        LAYOUT_DISCLAIMER,
    ] {
        assert!(reason.contains(text));
        assert!(report.summary.contains(text));
    }
    assert!(!report.is_abi_stable);
    assert_eq!(report.breaking_findings.len(), 1);
    let unsigned = assess(
        REPO,
        SignatureScanner::new().scan_abi_diff(diff),
        &signing(&[]),
    );
    assert!(matches!(unsigned.status, GateStatus::Failed(_)));
}

#[test]
fn clean_and_unpaired_comparisons_keep_their_scoped_disclosure() {
    let clean = assess(REPO, SignatureScanner::new().scan_abi_diff(""), &approved());
    assert_eq!(clean.status, GateStatus::Passed);
    assert!(clean.is_abi_stable);
    assert!(clean.breaking_findings.is_empty());
    assert!(clean.summary.contains("no compared signature changed"));
    assert!(clean.summary.contains(LAYOUT_DISCLAIMER));
    let diff = "diff --git a/src/api.rs b/src/api.rs\n--- a/src/api.rs\n+++ b/src/api.rs\n\
                @@ -1 +1 @@\n-pub fn api(\n+pub fn api(a: u8) {\n";
    let report = assess(
        REPO,
        SignatureScanner::new().scan_abi_diff(diff),
        &approved(),
    );
    assert_eq!(report.status, GateStatus::Passed);
    assert!(report.summary.contains("1 name(s)"));
    assert!(report.summary.contains("not compared"));
}

#[test]
fn accepted_warning_reaches_both_real_scorecard_render_branches() {
    let abi = assess(REPO, ordinary(), &approved());
    for blocked in [false, true] {
        // Pure composition fixtures, not measurements or queue admission.
        let outcomes: Vec<_> = PreMergeCertificationReport::unmeasured("fixture")
            .named_statuses()
            .into_iter()
            .map(|(id, _)| {
                let status = if id == SEMANTIC_ABI_GATE_ID {
                    abi.status.clone()
                } else if blocked && id == "test_suite_status" {
                    GateStatus::Failed("fixture failure".into())
                } else {
                    GateStatus::Passed
                };
                (id, status)
            })
            .collect();
        let report = PreMergeCertificationReport::from_gate_outcomes(&outcomes).unwrap();
        let rendered = crate::publish::scorecard::render(&report);
        assert!(rendered.contains(if blocked { "Blocked" } else { "Certified" }));
        for text in [
            "accepted by exact recorded signoff",
            "SIGNATURE_CHANGE api at src/api.rs",
            "for an unaccepted change",
            LAYOUT_DISCLAIMER,
        ] {
            assert!(rendered.contains(text), "{text}: {rendered}");
        }
        assert!(!rendered.contains("no compared signature changed"));
        assert!(!rendered.contains("bump major"));
    }
}
