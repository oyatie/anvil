//! No gate may publish a path it invented.
//!
//! Six more gates carried the block that #117 removed from three, and these
//! six defaulted to something worse than `unknown.rs`:
//!
//! | gate                  | invented path                  |
//! |-----------------------|--------------------------------|
//! | ci_runner_economics   | `.github/workflows/ci.yaml`    |
//! | ephemeral_secrets     | `.github/workflows/deploy.yaml`|
//! | psa_admission_guard   | `infra/ns.yaml`                |
//! | compile_time_profiler | `Cargo.toml`                   |
//! | cross_service_impact  | `api.yaml`                     |
//! | gitops_promotion      | `manifest.yaml`                |
//!
//! `unknown.rs` at least announces itself. Every one of these is a path that
//! could plausibly exist, published in the field a reviewer reads as the
//! location of the defect, with nothing to mark it as a guess.
//!
//! Worse, the invented path was often the very thing that put the file in
//! scope: `scan_workflow_runners` returns early unless the path contains
//! `.github/workflows/`, and the default IS `.github/workflows/ci.yaml`. So a
//! chunk whose path could not be read was not skipped -- it was admitted, under
//! a name that guaranteed admission.

use anvil::ci_runner_economics::CiRunnerEconomicsOptimizer;
use anvil::compile_time_profiler::CompileTimeProfiler;
use anvil::cross_service_impact::CrossServiceImpactEngine;
use anvil::ephemeral_secrets::EphemeralSecretInjector;
use anvil::git_manager::diff_context::PrDiffContext;
use anvil::gitops_promotion::GitOpsPromotionEngine;
use anvil::psa_admission_guard::PsaAdmissionGuard;
use std::path::{Path, PathBuf};

fn ctx(diff: &str) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".into(),
        pr_number: 1,
        base_branch: "dev".into(),
        base_sha: "a".into(),
        head_sha: "b".into(),
        is_incremental: false,
        previous_head_sha: None,
        diff_content: diff.to_string(),
        changed_files: vec![],
        repo_working_dir: PathBuf::from("."),
    }
}

/// A hunk that states no path at all, in the exact shape that reached the
/// invented default.
///
/// The leading newline matters and is the whole point. The old block read
/// `lines.first().split_whitespace().last()`; on a non-empty first line that
/// yields SOME token, and the gate used that token as the path -- garbage, but
/// garbage that usually failed the scope test and so looked harmless. It is
/// when the first line is EMPTY that `split_whitespace().last()` returns `None`
/// and `current_file` keeps the fabricated literal -- a path chosen precisely
/// because it passes the scope test.
///
/// Measured against the old code, this input produced:
///
/// ```text
/// ci  cost_optimal=false findings=[".github/workflows/ci.yaml"]
/// psa compliant=false    findings=["infra/ns.yaml"]
/// ```
fn headerless(body: &str) -> PrDiffContext {
    let added: String = body.lines().map(|l| format!("+{l}\n")).collect();
    ctx(&format!("\n{added}"))
}

fn hunk(path: &str, body: &str) -> PrDiffContext {
    let added: String = body.lines().map(|l| format!("+{l}\n")).collect();
    ctx(&format!(
        "diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n{added}"
    ))
}

const MACOS_PR: &str = "on:\n  pull_request:\njobs:\n  b:\n    runs-on: macos-14";
const STATIC_SECRET: &str = "env:\n  AWS_SECRET_ACCESS_KEY: ${{ secrets.AWS_KEY }}";
const BARE_NS: &str = "apiVersion: v1\nkind: Namespace\nmetadata:\n  name: app";
const SYN_FULL: &str = r#"syn = { version = "2", features = ["full"] }"#;
const MUTABLE_IMAGE: &str = "image: ghcr.io/oyatie/api:latest";

// ------------------------------------------------------------ each gate fires

#[test]
fn every_gate_still_fires_on_a_real_added_violation() {
    let p = Path::new(".");

    let r = CiRunnerEconomicsOptimizer::new()
        .evaluate_runner_economics(p, &hunk(".github/workflows/ci.yml", MACOS_PR))
        .unwrap();
    assert!(!r.is_cost_optimal, "ci_runner_economics went inert");
    assert_eq!(r.findings[0].workflow_file, ".github/workflows/ci.yml");

    let r = EphemeralSecretInjector::new()
        .evaluate_secret_policies(p, &hunk(".github/workflows/deploy.yml", STATIC_SECRET))
        .unwrap();
    assert!(!r.is_zero_trust, "ephemeral_secrets went inert");
    assert_eq!(r.findings[0].workflow_file, ".github/workflows/deploy.yml");

    let r = PsaAdmissionGuard::new()
        .evaluate_psa_admission(p, &hunk("infra/team/app-ns.yaml", BARE_NS))
        .unwrap();
    assert!(!r.is_compliant, "psa_admission_guard went inert");
    assert_eq!(r.findings[0].file_path, "infra/team/app-ns.yaml");

    let r = CompileTimeProfiler::new()
        .evaluate_compile_profile(p, &hunk("crates/macro/Cargo.toml", SYN_FULL))
        .unwrap();
    assert!(!r.is_lean, "compile_time_profiler went inert");
    assert_eq!(r.findings[0].file_path, "crates/macro/Cargo.toml");

    let r = GitOpsPromotionEngine::new()
        .evaluate_manifest_promotions(p, &hunk("k8s/prod/api.yaml", MUTABLE_IMAGE))
        .unwrap();
    assert!(!r.is_pinned, "gitops_promotion went inert");
    assert_eq!(r.unpinned_findings[0].file_path, "k8s/prod/api.yaml");
}

// -------------------------------------------------- nothing is invented

#[test]
fn a_hunk_that_names_no_file_is_attributed_to_no_file() {
    let p = Path::new(".");

    // Each payload is one that WOULD fire if the gate believed its own default
    // path. That is the sharp end of this: the invented path was frequently the
    // thing that admitted the chunk to the scan in the first place.
    assert!(
        CiRunnerEconomicsOptimizer::new()
            .evaluate_runner_economics(p, &headerless(MACOS_PR))
            .unwrap()
            .is_cost_optimal,
        "admitted under the invented `.github/workflows/ci.yaml`"
    );
    assert!(
        EphemeralSecretInjector::new()
            .evaluate_secret_policies(p, &headerless(STATIC_SECRET))
            .unwrap()
            .is_zero_trust,
        "admitted under the invented `.github/workflows/deploy.yaml`"
    );
    assert!(
        PsaAdmissionGuard::new()
            .evaluate_psa_admission(p, &headerless(BARE_NS))
            .unwrap()
            .is_compliant,
        "admitted under the invented `infra/ns.yaml`"
    );
    assert!(
        CompileTimeProfiler::new()
            .evaluate_compile_profile(p, &headerless(SYN_FULL))
            .unwrap()
            .is_lean,
        "admitted under the invented `Cargo.toml`"
    );
    assert!(
        GitOpsPromotionEngine::new()
            .evaluate_manifest_promotions(p, &headerless(MUTABLE_IMAGE))
            .unwrap()
            .is_pinned,
        "admitted under the invented `manifest.yaml`"
    );
}

// -------------------------------------------------------------- scope by path

#[test]
fn gitops_scope_is_the_path_not_a_word_in_the_text() {
    // The predicate was `file_diff.contains(".yaml")` over the chunk's CONTENT,
    // so a Rust file that merely mentioned a manifest was scanned as a GitOps
    // manifest -- and then filed under the literal `manifest.yaml`.
    let report = GitOpsPromotionEngine::new()
        .evaluate_manifest_promotions(
            Path::new("."),
            // The payload is a real `image:` line the pinner WILL flag, inside
            // a Rust file. Only the scope test stands between them.
            &hunk(
                "src/deploy.rs",
                "// deploys k8s/prod/api.yaml\n// image: ghcr.io/oyatie/api:latest",
            ),
        )
        .unwrap();
    assert!(
        report.is_pinned,
        "a Rust file was scanned as a manifest: {:?}",
        report.unpinned_findings
    );
}

// --------------------------------------------------- removals are not additions

#[test]
fn removing_a_violation_is_not_committing_one() {
    let removed: String = format!(
        "diff --git a/k8s/prod/api.yaml b/k8s/prod/api.yaml\n\
         --- a/k8s/prod/api.yaml\n+++ b/k8s/prod/api.yaml\n\
         -{MUTABLE_IMAGE}\n+image: ghcr.io/oyatie/api@sha256:{}\n",
        "a".repeat(64)
    );
    let report = GitOpsPromotionEngine::new()
        .evaluate_manifest_promotions(Path::new("."), &ctx(&removed))
        .unwrap();
    assert!(
        report.is_pinned,
        "the change that PINS the image was refused for the tag it removes: {:?}",
        report.unpinned_findings
    );
}

/// A wire-contract rule whose subject IS the removal keeps both sides.
#[test]
fn a_removed_required_field_is_still_a_breaking_change() {
    let diff = "diff --git a/api/openapi.yaml b/api/openapi.yaml\n\
                --- a/api/openapi.yaml\n+++ b/api/openapi.yaml\n\
                 required:\n-  - tenant_id\n+  - name\n";
    let report = CrossServiceImpactEngine::new()
        .evaluate_cross_service_impact(Path::new("."), &ctx(diff))
        .unwrap();
    assert!(
        !report.is_compatible,
        "cross_service_impact must keep reading removals: that is its subject"
    );
    assert_eq!(
        report.breaking_findings[0].contract_file,
        "api/openapi.yaml"
    );
}
