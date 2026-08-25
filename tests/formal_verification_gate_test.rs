//! Gate 41 must say what its scan did, and only what its scan did.
//!
//! `formal_verification/policy_scanner.rs` is honest about itself: *"This is a
//! keyword scan, not a decision procedure: it proves nothing and its coverage
//! is exactly the patterns written below. A match is real evidence; the absence
//! of a match is not evidence of safety."* The fidelity registry agrees --
//! `Fidelity::Heuristic`, "No solver exists" -- and the module was renamed out
//! of `smt_solver.rs` to stop claiming otherwise.
//!
//! The rename stopped one line short. Three defects survived it, each a
//! different way of publishing more than the scan performed.

use anvil::formal_verification::{FormalVerificationGuard, is_policy_path};

fn scan(diff: &str) -> anvil::formal_verification::FormalVerificationReport {
    FormalVerificationGuard::new().evaluate_formal_invariants(diff)
}

const WILDCARD: &str = r#"permit(principal == Principal::"*", action, resource);"#;

#[test]
fn a_pull_request_that_deletes_a_wildcard_policy_is_not_a_wildcard_policy() {
    // The scan used to read the whole diff, removals included, so the change
    // that REMOVES the dangerous grant was refused for containing it. The
    // identical inversion was found and fixed in the credential scanner; it was
    // still live here, in the gate that claims to be the formal one.
    let cleanup = format!(
        "--- a/iam/policy.cedar\n+++ b/iam/policy.cedar\n-{WILDCARD}\n\
         +permit(principal == Principal::\"User:1\", action, resource);\n"
    );
    let report = scan(&cleanup);
    assert!(
        report.findings.is_empty(),
        "the cleanup was refused for the grant it removes: {:?}",
        report.findings
    );
    assert_eq!(
        report.policy_files_seen,
        vec!["iam/policy.cedar".to_string()],
        "it did examine the file, so this is a pass and not a withholding"
    );
}

#[test]
fn adding_a_wildcard_policy_is_still_caught() {
    // The other half of the red/green pair: the inversion fix must not have
    // bought its result by making the rule inert.
    let report = scan(&format!(
        "--- a/iam/policy.cedar\n+++ b/iam/policy.cedar\n+{WILDCARD}\n"
    ));
    assert_eq!(report.findings.len(), 1, "{report:?}");
    assert_eq!(report.findings[0].rule, "CedarPrincipalWildcard");
    assert!(!report.passed);
}

#[test]
fn a_change_with_no_policy_examined_no_policy() {
    // `passed` is true here and was published as a green formal-verification
    // gate. Nothing was scanned: there is no policy in this diff at all.
    let report = scan("--- a/src/main.rs\n+++ b/src/main.rs\n+fn main() {}\n");
    assert!(report.findings.is_empty());
    assert!(
        report.policy_files_seen.is_empty(),
        "nothing was examined, and the report must be able to say so"
    );
}

#[test]
fn a_policy_file_with_only_deletions_gives_the_scan_nothing() {
    // Removing lines from a policy leaves this scan with no added text to
    // judge. Recording the file anyway would let a deletion-only change claim
    // coverage it does not have.
    let report = scan(&format!(
        "--- a/iam/policy.cedar\n+++ b/iam/policy.cedar\n-{WILDCARD}\n"
    ));
    assert!(report.policy_files_seen.is_empty(), "{report:?}");
}

#[test]
fn coverage_is_the_stated_set_of_paths_and_nothing_wider() {
    // The predicate is the gate's declared reach. It is written down so that a
    // policy living outside it reports as unmeasured rather than as passed.
    assert!(is_policy_path("iam/authz.cedar"));
    assert!(is_policy_path("deploy/NetworkPolicy.yaml"));
    assert!(is_policy_path("k8s/netpol-db.yml"));
    assert!(is_policy_path("infra/rbac/roles.yaml"));
    assert!(!is_policy_path("src/main.rs"));
    assert!(!is_policy_path("deploy/service.yaml"));
}
