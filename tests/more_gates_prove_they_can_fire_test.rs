//! More gates demonstrate both halves: they fire on a seeded defect, and they
//! spare a conformant subject.
//!
//! Continues the ledger begun in `four_gates_prove_they_can_fire_test`. The
//! diffs are built with `concat!`, one line per string, because a `\`-newline
//! continuation eats the leading space that marks a CONTEXT line in a unified
//! diff — a fixture written that way lost its context and a gate flagged a file
//! that was clean.

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

fn scratch(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("anvil-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("scratch");
    d
}

// ---------------------------------------------------------------------------
// ephemeral_secret_status — EphemeralSecretInjector
// ---------------------------------------------------------------------------
//
// A long-lived AWS key handed to a workflow from repository secrets is the
// credential this gate exists to refuse: it outlives the job, it is copied by
// every fork of the workflow, and rotating it is a manual act nobody schedules.
// The OIDC form mints a token per run instead.

#[test]
fn ephemeral_secret_fires_on_a_static_aws_key_in_a_workflow() {
    let dir = scratch("secret-red");
    let diff = concat!(
        "diff --git a/.github/workflows/deploy.yml b/.github/workflows/deploy.yml\n",
        "--- a/.github/workflows/deploy.yml\n",
        "+++ b/.github/workflows/deploy.yml\n",
        "@@ -1,2 +1,3 @@\n",
        " env:\n",
        "+  AWS_SECRET_ACCESS_KEY: ${{ secrets.AWS_SECRET }}\n",
    );
    let report = anvil::ephemeral_secrets::EphemeralSecretInjector::new()
        .evaluate_secret_policies(&dir, &ctx(diff, vec![".github/workflows/deploy.yml"]))
        .expect("the gate runs");
    assert!(
        !report.is_zero_trust,
        "a static AWS key was handed to a workflow from repository secrets and \
         the gate did not see it"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ephemeral_secret_spares_a_workflow_that_assumes_a_role_by_oidc() {
    let dir = scratch("secret-green");
    let diff = concat!(
        "diff --git a/.github/workflows/deploy.yml b/.github/workflows/deploy.yml\n",
        "--- a/.github/workflows/deploy.yml\n",
        "+++ b/.github/workflows/deploy.yml\n",
        "@@ -1,2 +1,4 @@\n",
        " permissions:\n",
        "+  id-token: write\n",
        "+- uses: aws-actions/configure-aws-credentials@v4\n",
        "+  with: { role-to-assume: arn:aws:iam::1:role/ci }\n",
    );
    let report = anvil::ephemeral_secrets::EphemeralSecretInjector::new()
        .evaluate_secret_policies(&dir, &ctx(diff, vec![".github/workflows/deploy.yml"]))
        .expect("the gate runs");
    assert!(
        report.is_zero_trust,
        "a role assumed by OIDC mints a token per run and is the conformant \
         form; flagging it is a fabricated accusation. {} finding(s).\n\
         The count, not the summary: this report derives from a fixture that \
         carries a credential-shaped line, and echoing text tainted by one is \
         how a real key reaches a log the day somebody swaps the fixture for a \
         live value. `rust/cleartext-logging` flagged exactly that here.",
        report.findings.len()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// cross_service_status — CrossServiceImpactEngine
// ---------------------------------------------------------------------------
//
// The only rule in the corpus whose SUBJECT is a removal: a required field that
// disappears from a wire contract breaks every consumer that still sends it, so
// this scan compares the two sides of the diff rather than the added lines.

#[test]
fn cross_service_fires_when_a_required_field_leaves_a_wire_contract() {
    let dir = scratch("xsvc-red");
    let diff = concat!(
        "diff --git a/api/openapi.yaml b/api/openapi.yaml\n",
        "--- a/api/openapi.yaml\n",
        "+++ b/api/openapi.yaml\n",
        "@@ -1,2 +1,2 @@\n",
        "-        required: [tenant_id, amount]\n",
        "+        required: [amount]\n",
    );
    let report = anvil::cross_service_impact::CrossServiceImpactEngine::new()
        .evaluate_cross_service_impact(&dir, &ctx(diff, vec!["api/openapi.yaml"]))
        .expect("the engine runs");
    assert!(
        !report.is_compatible,
        "`tenant_id` stopped being required and every consumer still sending it \
         is now sending a field the contract does not declare"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cross_service_spares_a_required_field_being_added() {
    let dir = scratch("xsvc-green");
    let diff = concat!(
        "diff --git a/api/openapi.yaml b/api/openapi.yaml\n",
        "--- a/api/openapi.yaml\n",
        "+++ b/api/openapi.yaml\n",
        "@@ -1,2 +1,2 @@\n",
        "-        required: [amount]\n",
        "+        required: [amount, tenant_id]\n",
    );
    let report = anvil::cross_service_impact::CrossServiceImpactEngine::new()
        .evaluate_cross_service_impact(&dir, &ctx(diff, vec!["api/openapi.yaml"]))
        .expect("the engine runs");
    assert!(
        report.is_compatible,
        "nothing was removed, so no consumer broke. A gate that flags the \
         forward direction refuses every contract that grows: {}",
        report.summary
    );
    let _ = std::fs::remove_dir_all(&dir);
}
