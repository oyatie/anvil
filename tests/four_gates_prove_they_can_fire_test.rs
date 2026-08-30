//! Four gates demonstrate both halves: they fire on a seeded defect, and they
//! spare a conformant subject.
//!
//! A check is written by asserting what it should catch, and it passes from the
//! moment it compiles — so a green says nothing about whether it CAN fail. The
//! `gate_proof` ledger exists to impose that obligation on the hand-wired
//! gates, and twenty-three of them owed it. These four pay it.
//!
//! Both halves are required and that is the point. A gate with only the red
//! half cannot be shown to discriminate; one with only the green half has never
//! been seen to work. A gate that fires on everything and a gate that fires on
//! nothing are both useless, and only the pair distinguishes them.

use anvil::git_manager::{PrDiffContext, SubjectRoot, Uncloned};

fn ctx(diff: &str, files: Vec<&str>) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: diff.to_string(),
        changed_files: files.into_iter().map(str::to_string).collect(),
        repo_working_dir: SubjectRoot::asserted(
            std::path::PathBuf::from("."),
            Uncloned::TestFixture,
        ),
        is_incremental: false,
        previous_head_sha: None,
    }
}

// ---------------------------------------------------------------------------
// cleartext_transport_status — ZeroTrustWorkload
// ---------------------------------------------------------------------------

#[test]
fn zero_trust_flags_an_added_cleartext_endpoint() {
    let report = anvil::zero_trust_workload::ZeroTrustWorkloadGate::new()
        .evaluate_cleartext_transport(
            "diff --git a/src/client.rs b/src/client.rs\n\
             +++ b/src/client.rs\n\
             +const UPSTREAM: &str = \"http://payments.internal/charge\";\n",
        );
    assert!(
        !report.passed,
        "an added plaintext internal endpoint is CWE-319 and the gate did not see it"
    );
}

#[test]
fn zero_trust_spares_the_same_endpoint_over_tls() {
    let report = anvil::zero_trust_workload::ZeroTrustWorkloadGate::new()
        .evaluate_cleartext_transport(
            "diff --git a/src/client.rs b/src/client.rs\n\
             +++ b/src/client.rs\n\
             +const UPSTREAM: &str = \"https://payments.internal/charge\";\n",
        );
    assert!(
        report.passed,
        "the same endpoint over TLS is the conformant case; flagging it is a \
         fabricated accusation, which is I1's symmetric violation"
    );
}

// ---------------------------------------------------------------------------
// brand_absence_status — BrandAbsenceGate
// ---------------------------------------------------------------------------

/// The gate reads DECLARED NAMES, not prose. A first fixture put the stamp in a
/// doc comment and the gate spared it — correctly, since a comment naming a
/// thing is not a thing named that way. The red half is what surfaced the
/// difference; a green-only proof would have recorded a belief about the gate
/// rather than a fact.
#[test]
fn brand_absence_flags_an_aspirational_stamp_in_source() {
    let report = anvil::brand_absence::BrandAbsenceGate::new()
        .scan_source("src/thing.rs", "pub struct HyperscalerWidget;\n");
    assert!(
        !report.new_violations.is_empty(),
        "`hyperscaler` is a forbidden stamp named in the law, and this declares \
         it as a type name; the gate did not see it"
    );
}

#[test]
fn brand_absence_spares_source_that_claims_nothing() {
    let report = anvil::brand_absence::BrandAbsenceGate::new()
        .scan_source("src/thing.rs", "pub struct WriteBatcher;\n");
    assert!(
        report.new_violations.is_empty(),
        "prose describing what the code does is not an aspiration: {:?}",
        report.new_violations
    );
}

// ---------------------------------------------------------------------------
// semantic_abi_status — SemanticAbiRatchet
// ---------------------------------------------------------------------------

#[test]
fn semantic_abi_flags_a_changed_public_signature() {
    let dir = std::env::temp_dir().join(format!("anvil-abi-red-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch");
    let report = anvil::semantic_abi_ratchet::SemanticAbiRatchet::new()
        .evaluate_abi_stability(
            &dir,
            &ctx(
                "diff --git a/src/lib.rs b/src/lib.rs\n\
                 --- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n\
                 -pub fn charge(cents: u64) -> bool { true }\n\
                 +pub fn charge(cents: u64, idem: &str) -> bool { true }\n",
                vec!["src/lib.rs"],
            ),
        )
        .expect("the ratchet runs");
    assert!(
        !report.is_abi_stable,
        "a public function gained a parameter and the ratchet called the ABI stable"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn semantic_abi_spares_a_body_change_behind_the_same_signature() {
    let dir = std::env::temp_dir().join(format!("anvil-abi-green-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch");
    let report = anvil::semantic_abi_ratchet::SemanticAbiRatchet::new()
        .evaluate_abi_stability(
            &dir,
            &ctx(
                "diff --git a/src/lib.rs b/src/lib.rs\n\
                 --- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,3 @@\n\
                 pub fn charge(cents: u64) -> bool {\n\
                 -    cents > 0\n\
                 +    cents > 0 && cents < 1_000_000\n\
                 }\n",
                vec!["src/lib.rs"],
            ),
        )
        .expect("the ratchet runs");
    assert!(
        report.is_abi_stable,
        "the signature did not move, so the ABI did not: {}",
        report.summary
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// psa_status — PsaAdmissionGuard
// ---------------------------------------------------------------------------

/// ADR-0710 D-1: a Namespace manifest must declare a Pod Security Admission
/// enforce level. The rule reads the file's post-change text, so a manifest
/// that arrives without one is the defect.
#[test]
fn psa_flags_a_namespace_without_an_enforce_label() {
    let dir = std::env::temp_dir().join(format!("anvil-psa-red-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch");
    let report = anvil::psa_admission_guard::PsaAdmissionGuard::new()
        .evaluate_psa_admission(
            &dir,
            &ctx(
                "diff --git a/infra/tenant.yaml b/infra/tenant.yaml\n\
                 --- a/infra/tenant.yaml\n+++ b/infra/tenant.yaml\n@@ -0,0 +1,3 @@\n\
                 +apiVersion: v1\n\
                 +kind: Namespace\n\
                 +metadata:\n",
                vec!["infra/tenant.yaml"],
            ),
        )
        .expect("the guard runs");
    assert!(
        !report.is_compliant,
        "a Namespace with no `pod-security.kubernetes.io/enforce:` label is the \
         defect ADR-0710 D-1 names, and the gate did not see it"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn psa_spares_a_namespace_that_enforces_restricted() {
    let dir = std::env::temp_dir().join(format!("anvil-psa-green-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch");
    let report = anvil::psa_admission_guard::PsaAdmissionGuard::new()
        .evaluate_psa_admission(
            &dir,
            &ctx(
                "diff --git a/infra/tenant.yaml b/infra/tenant.yaml\n\
                 --- a/infra/tenant.yaml\n+++ b/infra/tenant.yaml\n@@ -0,0 +1,5 @@\n\
                 +apiVersion: v1\n\
                 +kind: Namespace\n\
                 +metadata:\n\
                 +  labels:\n\
                 +    pod-security.kubernetes.io/enforce: restricted\n",
                vec!["infra/tenant.yaml"],
            ),
        )
        .expect("the guard runs");
    assert!(
        report.is_compliant,
        "the manifest declares the enforce level the rule asks for: {}",
        report.summary
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// idempotency_status — IdempotencyGuard
// ---------------------------------------------------------------------------
//
// The diffs below are built with `concat!`, one line per string, because a
// `\`-newline continuation eats the leading space that marks a CONTEXT line in
// a unified diff. The first green fixture written that way lost its context and
// the gate flagged it — the fixture was malformed, not the gate, and only the
// green half could have told the difference.

/// A mutating route this change ADDS, in a file that references no
/// Idempotency-Key anywhere.
#[test]
fn idempotency_flags_an_added_mutating_route_with_no_key() {
    let dir = std::env::temp_dir().join(format!("anvil-idem-red-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch");
    let diff = concat!(
        "diff --git a/src/api.rs b/src/api.rs\n",
        "--- a/src/api.rs\n",
        "+++ b/src/api.rs\n",
        "@@ -1,2 +1,3 @@\n",
        " fn router() -> Router {\n",
        "+    Router::new().route(\"/charge\", post(charge))\n",
        " }\n",
    );
    let report = anvil::idempotency_guard::IdempotencyGuard::new()
        .evaluate_idempotency(&dir, &ctx(diff, vec!["src/api.rs"]))
        .expect("the guard runs");
    assert!(
        !report.is_idempotent,
        "a POST route was added and nothing in the file mentions an \
         Idempotency-Key; a retried charge is a double charge"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The same added route, in a file that handles the key in untouched context.
///
/// The rule reads two corpora on purpose: the route must be ADDED, but the key
/// may sit anywhere in the hunk, because a file that already handles it was
/// being judged as though it did not.
#[test]
fn idempotency_spares_the_same_route_where_the_file_handles_the_key() {
    let dir = std::env::temp_dir().join(format!("anvil-idem-green-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch");
    let diff = concat!(
        "diff --git a/src/api.rs b/src/api.rs\n",
        "--- a/src/api.rs\n",
        "+++ b/src/api.rs\n",
        "@@ -1,3 +1,4 @@\n",
        " fn router() -> Router {\n",
        "+    Router::new().route(\"/charge\", post(charge))\n",
        " }\n",
        " fn charge(h: HeaderMap) { let _ = h.get(\"Idempotency-Key\"); }\n",
    );
    let report = anvil::idempotency_guard::IdempotencyGuard::new()
        .evaluate_idempotency(&dir, &ctx(diff, vec!["src/api.rs"]))
        .expect("the guard runs");
    assert!(
        report.is_idempotent,
        "the key is handled in this file, in context the change did not touch, \
         which is the case the two-corpus rule exists to spare: {}",
        report.summary
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// unresolved_review_status — UnresolvedReviewReport
// ---------------------------------------------------------------------------
//
// The chain Issue #18 asks for is GitHub's `isResolved` -> the report -> the
// gate. Both halves are driven from a real GraphQL answer rather than a
// hand-built report, because a report that states its own verdict cannot
// exercise the code that computes one.

const ONE_OPEN_THREAD: &str = r#"{"data":{"repository":{"pullRequest":{"reviewThreads":{
    "pageInfo":{"hasNextPage":false},
    "nodes":[{"id":"T_1","isResolved":false,"comments":{"nodes":[
        {"body":"✅ Fixed: Resolved:","path":"src/main.rs","line":1,
         "author":{"login":"anvil"}}]}}]}}}}}"#;

const NO_OPEN_THREADS: &str = r#"{"data":{"repository":{"pullRequest":{"reviewThreads":{
    "pageInfo":{"hasNextPage":false},
    "nodes":[]}}}}}"#;

#[test]
fn unresolved_review_fires_on_a_thread_github_reports_open() {
    let threads =
        anvil::unresolved_review_guard::parse_review_threads(true, ONE_OPEN_THREAD.as_bytes(), "")
            .expect("a well-formed answer parses");
    let report = anvil::unresolved_review_guard::UnresolvedReviewReport::from_threads(threads);
    assert!(
        matches!(
            anvil::pre_merge_guard::evaluator::unresolved_review_gate(&report),
            anvil::pre_merge_guard::report::GateStatus::Failed(_)
        ),
        "GitHub reported the thread unresolved and the gate did not fail. The \
         comment body in this fixture carries the three words the deleted \
         substring resolver accepted, which is why the decision may not read them."
    );
}

#[test]
fn unresolved_review_spares_a_pull_request_with_no_open_threads() {
    let threads =
        anvil::unresolved_review_guard::parse_review_threads(true, NO_OPEN_THREADS.as_bytes(), "")
            .expect("a well-formed answer parses");
    let report = anvil::unresolved_review_guard::UnresolvedReviewReport::from_threads(threads);
    assert!(
        matches!(
            anvil::pre_merge_guard::evaluator::unresolved_review_gate(&report),
            anvil::pre_merge_guard::report::GateStatus::Passed
        ),
        "no thread is open, so this gate must not withhold the merge — a gate \
         that always fails refuses every pull request and proves nothing"
    );
}
