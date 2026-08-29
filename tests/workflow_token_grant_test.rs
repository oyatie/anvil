//! Every workflow states what the GITHUB_TOKEN may do.

use serde_yaml::Value;
use std::fs;
use std::path::PathBuf;

/// Every `.github/workflows/*.yml`, as name and raw text.
///
/// Discovered rather than listed: a workflow added next week is governed on
/// the day it lands, and a rename cannot silently empty the corpus.
fn workflows() -> Vec<(String, String)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".github/workflows");
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).expect("workflow directory").flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yml") {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let body = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        out.push((name, body));
    }
    out
}

#[test]
fn every_workflow_parses_as_yaml() {
    let found = workflows();
    assert!(!found.is_empty(), "no workflow files found");

    let broken: Vec<String> = found
        .iter()
        .filter_map(|(name, body)| {
            serde_yaml::from_str::<Value>(body)
                .err()
                .map(|e| format!("{name}: {e}"))
        })
        .collect();

    assert!(
        broken.is_empty(),
        "a workflow that does not parse never runs, and the forge reports that \
         in its own UI rather than in this repository's checks.\n  {}",
        broken.join("\n  ")
    );
}

/// A job's own `permissions:` also satisfies the rule, so ask both levels.
fn grants_are_declared(doc: &Value) -> bool {
    if !doc["permissions"].is_null() {
        return true;
    }
    let Some(jobs) = doc["jobs"].as_mapping() else {
        return false;
    };
    !jobs.is_empty() && jobs.values().all(|j| !j["permissions"].is_null())
}

#[test]
fn every_workflow_declares_the_token_grant_it_needs() {
    let found = workflows();
    assert!(
        !found.is_empty(),
        "no workflow files found; this check would pass vacuously"
    );

    let silent: Vec<&str> = found
        .iter()
        .filter_map(|(name, body)| serde_yaml::from_str::<Value>(body).ok().map(|d| (name, d)))
        .filter(|(_, doc)| !grants_are_declared(doc))
        .map(|(name, _)| name.as_str())
        .collect();

    assert!(
        silent.is_empty(),
        "a workflow with no `permissions:` runs on whatever the repository \
         default happens to be, which is a grant nobody in the diff chose. \
         State it -- `contents: read` is the usual floor, and a job that needs \
         more names the more it needs.\n  {}",
        silent.join("\n  ")
    );
}

#[test]
fn the_rule_can_tell_a_declared_grant_from_a_missing_one() {
    let declared: Value =
        serde_yaml::from_str("permissions:\n  contents: read\njobs:\n  a:\n    steps: []\n")
            .unwrap();
    let per_job: Value = serde_yaml::from_str(
        "jobs:\n  a:\n    permissions:\n      contents: read\n    steps: []\n",
    )
    .unwrap();
    let partial: Value = serde_yaml::from_str(
        "jobs:\n  a:\n    permissions:\n      contents: read\n    steps: []\n  b:\n    steps: []\n",
    )
    .unwrap();
    let silent: Value = serde_yaml::from_str("jobs:\n  a:\n    steps: []\n").unwrap();

    assert!(grants_are_declared(&declared));
    assert!(grants_are_declared(&per_job));
    assert!(
        !grants_are_declared(&partial),
        "one job covered is not all of them"
    );
    assert!(!grants_are_declared(&silent));
}
