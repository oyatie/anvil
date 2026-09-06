//! Data-only contracts for the actual workflow/private-parser seam.
//! Exact shell text is change detection, not a proof of shell semantics.

use serde_yaml::Value;

const ACQUISITION: &str = r#"git fetch --no-tags origin "+refs/heads/${BASE}:refs/remotes/origin/${BASE}"
git fetch --no-tags origin "${HEAD_SHA}"
destination_tip="$(git rev-parse --verify "refs/remotes/origin/${BASE}^{commit}")"
resolved_head="$(git rev-parse --verify "${HEAD_SHA}^{commit}")"
merge_bases="$(git merge-base --all "${resolved_head}" "${destination_tip}")"
mapfile -t merge_base_ids <<< "${merge_bases}"
if [ "${#merge_base_ids[@]}" -ne 1 ] || [ -z "${merge_base_ids[0]}" ]; then
  echo "occupancy: expected one actual merge-base; refusing."
  exit 1
fi
merge_base="${merge_base_ids[0]}"
destination_tree="$(git rev-parse --verify "${destination_tip}^{tree}")"
merge_base_tree="$(git rev-parse --verify "${merge_base}^{tree}")"
printf '%s\n' \
  occupancy-freshness-v1 \
  "${EXPECTED_REPOSITORY}" "${BASE_REPOSITORY}" "${HEAD_REPOSITORY}" \
  "${BASE}" "${HEAD_REF}" "${HEAD_SHA}" "${resolved_head}" \
  "${destination_tip}" "${merge_base}" \
  "${destination_tree}" "${merge_base_tree}" > freshness.txt
"#;

fn workflow() -> Value {
    serde_yaml::from_str(include_str!("../.github/workflows/presubmit.yml")).unwrap()
}

#[test]
fn actual_acquisition_pins_unique_merge_base_and_complete_trees() {
    let doc = workflow();
    let steps = doc["jobs"]["occupancy"]["steps"].as_sequence().unwrap();
    let collector = steps
        .iter()
        .find(|s| s["env"]["HEAD_SHA"].is_string())
        .unwrap();
    let run = collector["run"].as_str().unwrap();
    assert!(run.starts_with("set -euo pipefail\n"));
    let start = run.find("git fetch --no-tags origin").unwrap();
    let end = run[start..]
        .find(" > freshness.txt\n")
        .expect("fixed evidence record")
        + start
        + " > freshness.txt\n".len();
    assert_eq!(&run[start..end], ACQUISITION);
    assert!(!run.contains("at-trunk.txt"));
    assert!(collector["continue-on-error"].is_null());
}

#[test]
fn evidence_identity_is_event_bound_without_fallbacks() {
    let doc = workflow();
    let steps = doc["jobs"]["occupancy"]["steps"].as_sequence().unwrap();
    let collector = steps
        .iter()
        .find(|s| s["env"]["HEAD_SHA"].is_string())
        .unwrap();
    for (name, binding) in [
        ("EXPECTED_REPOSITORY", "github.repository"),
        (
            "BASE_REPOSITORY",
            "github.event.pull_request.base.repo.full_name",
        ),
        (
            "HEAD_REPOSITORY",
            "github.event.pull_request.head.repo.full_name",
        ),
        ("BASE", "github.event.pull_request.base.ref"),
        ("HEAD_REF", "github.event.pull_request.head.ref"),
        ("HEAD_SHA", "github.event.pull_request.head.sha"),
    ] {
        assert_eq!(
            collector["env"][name].as_str(),
            Some(format!("${{{{ {binding} }}}}").as_str())
        );
    }
}

#[test]
fn actual_consumer_requires_the_record_and_retains_read_only_guard_graph() {
    let doc = workflow();
    let steps = doc["jobs"]["occupancy"]["steps"].as_sequence().unwrap();
    let consumer = steps
        .iter()
        .filter_map(|s| s["run"].as_str())
        .find(|s| s.contains("cargo run --bin occupancy"))
        .unwrap();
    assert!(consumer.contains("--freshness-file freshness.txt"));
    assert!(!consumer.contains("--merge-base-is-trunk"));
    assert!(consumer.contains("--this-pr \"${PR}\""));
    assert_eq!(doc["permissions"]["contents"].as_str(), Some("read"));
    assert_eq!(doc["permissions"]["pull-requests"].as_str(), Some("read"));
    assert_eq!(doc["permissions"].as_mapping().unwrap().len(), 2);
    assert_eq!(
        doc["jobs"]["occupancy"]["if"].as_str(),
        Some("github.event_name == 'pull_request'")
    );
    assert_eq!(
        doc["jobs"]["fast-checks"]["needs"],
        serde_yaml::from_str::<Value>("[build-and-test, occupancy]").unwrap()
    );
}

#[test]
fn the_private_consumer_connects_required_record_read_to_actual_verdict() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let main = anvil::source_scan::paths::module_source("src/bin/occupancy/main", root);
    let inputs = anvil::source_scan::paths::module_source("src/bin/occupancy/inputs", root);
    assert!(main.contains("evaluate(collect(args))"));
    assert!(inputs.contains("let evidence_path = freshness_flag(args)?;"));
    assert!(inputs.contains("let freshness = read_record(evidence)?;"));
    assert!(main.contains("admit_in_queue_with_freshness("));
    assert!(main.contains("freshness.kind()"));
}

#[test]
fn existing_workflows_keep_the_closed_predecessor_pairs() {
    let predecessor = include_str!("../.github/workflows/promotion-predecessor.yml");
    let opener = include_str!("../.github/workflows/promotion-open-next.yml");
    assert!(
        predecessor.contains(
            r#"const pred = { staging: "dev", canary: "staging", production: "canary" };"#
        )
    );
    assert!(
        opener.contains(
            r#"const next = { dev: "staging", staging: "canary", canary: "production" };"#
        )
    );
}
