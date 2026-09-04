//! Every workflow states what each GitHub credential may do.

use serde_yaml::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

const APP_TOKEN_ACTION: &str =
    "actions/create-github-app-token@bcd2ba49218906704ab6c1aa796996da409d3eb1";
const APP_TOKEN_OUTPUT: &str = "${{ steps.app-token.outputs.token }}";

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

fn workflow(name: &str) -> (String, Value) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".github/workflows")
        .join(name);
    let body =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} must exist: {e}", path.display()));
    let doc = serde_yaml::from_str(&body)
        .unwrap_or_else(|e| panic!("{} must parse as YAML: {e}", path.display()));
    (body, doc)
}

fn repo_text(path: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} must exist: {e}", path.display()))
}

fn normalized_words(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn job_steps<'a>(doc: &'a Value, workflow: &str, job: &str) -> &'a Vec<Value> {
    doc["jobs"][job]["steps"]
        .as_sequence()
        .unwrap_or_else(|| panic!("{workflow}: jobs.{job}.steps must be a sequence"))
}

fn app_token_step<'a>(steps: &'a [Value], workflow: &str) -> &'a Value {
    let found: Vec<&Value> = steps
        .iter()
        .filter(|step| {
            step["uses"]
                .as_str()
                .is_some_and(|uses| uses.starts_with("actions/create-github-app-token@"))
        })
        .collect();
    assert_eq!(
        found.len(),
        1,
        "{workflow}: expected exactly one actions/create-github-app-token step, found {}",
        found.len()
    );
    found[0]
}

fn action_step<'a>(steps: &'a [Value], workflow: &str, action: &str) -> &'a Value {
    let found: Vec<&Value> = steps
        .iter()
        .filter(|step| {
            step["uses"]
                .as_str()
                .is_some_and(|uses| uses.starts_with(action))
        })
        .collect();
    assert_eq!(
        found.len(),
        1,
        "{workflow}: expected exactly one {action} step, found {}",
        found.len()
    );
    found[0]
}

fn named_step<'a>(steps: &'a [Value], workflow: &str, name: &str) -> &'a Value {
    let found: Vec<&Value> = steps
        .iter()
        .filter(|step| step["name"].as_str() == Some(name))
        .collect();
    assert_eq!(
        found.len(),
        1,
        "{workflow}: expected exactly one step named {name:?}, found {}",
        found.len()
    );
    found[0]
}

fn string_map(value: &Value, context: &str) -> BTreeMap<String, String> {
    value
        .as_mapping()
        .unwrap_or_else(|| panic!("{context} must be a mapping"))
        .iter()
        .map(|(key, value)| {
            let key = key
                .as_str()
                .unwrap_or_else(|| panic!("{context} has a non-string key: {key:?}"));
            let value = value
                .as_str()
                .unwrap_or_else(|| panic!("{context}.{key} must be a string, got {value:?}"));
            (key.to_string(), value.to_string())
        })
        .collect()
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

#[test]
fn app_tokens_are_sha_pinned_repo_scoped_and_explicitly_attenuated() {
    for (workflow_name, job_name, contents) in [
        ("promotion-open-next.yml", "open-next", "read"),
        ("toolchain-weekly.yml", "channel", "write"),
    ] {
        let (body, doc) = workflow(workflow_name);
        assert_eq!(
            body.matches("actions/create-github-app-token@").count(),
            1,
            "{workflow_name}: every App-token grant must pass through the one checked step"
        );
        let step = app_token_step(job_steps(&doc, workflow_name, job_name), workflow_name);

        assert_eq!(
            step["uses"].as_str(),
            Some(APP_TOKEN_ACTION),
            "{workflow_name}: the App-token action must be pinned to the reviewed commit"
        );
        assert_eq!(
            step["id"].as_str(),
            Some("app-token"),
            "{workflow_name}: consumers rely on the token step's stable id"
        );

        let actual = string_map(&step["with"], &format!("{workflow_name}: App token with"));
        let expected = BTreeMap::from([
            ("app-id".to_string(), "${{ vars.ANVIL_APP_ID }}".to_string()),
            ("permission-contents".to_string(), contents.to_string()),
            ("permission-pull-requests".to_string(), "write".to_string()),
            (
                "private-key".to_string(),
                "${{ secrets.ANVIL_APP_PRIVATE_KEY }}".to_string(),
            ),
        ]);
        assert_eq!(
            actual, expected,
            "{workflow_name}: App-token inputs must be exact. With no permission-* inputs the \
             action would inherit every permission granted to the App installation; these \
             explicit inputs attenuate it. owner/repositories stay absent so the action defaults \
             to the current repository. Explicitly naming only that repository would not widen \
             scope, but omission is pinned so any new scope declaration is review-visible."
        );
    }
}

#[test]
fn app_token_consumers_have_no_github_token_fallback() {
    let (promotion_body, promotion) = workflow("promotion-open-next.yml");
    let promotion_steps = job_steps(&promotion, "promotion-open-next.yml", "open-next");
    let github_script = action_step(
        promotion_steps,
        "promotion-open-next.yml",
        "actions/github-script@",
    );
    assert_eq!(
        github_script["with"]["github-token"].as_str(),
        Some(APP_TOKEN_OUTPUT),
        "promotion-open-next.yml: github-script must use the App installation token"
    );

    let (toolchain_body, toolchain) = workflow("toolchain-weekly.yml");
    let toolchain_steps = job_steps(&toolchain, "toolchain-weekly.yml", "channel");
    let checkout = action_step(toolchain_steps, "toolchain-weekly.yml", "actions/checkout@");
    assert_eq!(
        checkout["with"]["token"].as_str(),
        Some(APP_TOKEN_OUTPUT),
        "toolchain-weekly.yml: checkout must persist the App token for the branch push"
    );
    let open_bump = named_step(toolchain_steps, "toolchain-weekly.yml", "Open the bump");
    assert_eq!(
        open_bump["env"]["GH_TOKEN"].as_str(),
        Some(APP_TOKEN_OUTPUT),
        "toolchain-weekly.yml: gh pr create must use the App installation token"
    );

    for (name, job, doc) in [
        ("promotion-open-next.yml", "open-next", &promotion),
        ("toolchain-weekly.yml", "channel", &toolchain),
    ] {
        let permissions = doc["permissions"]
            .as_mapping()
            .unwrap_or_else(|| panic!("{name}: permissions must be an explicit mapping"));
        assert!(
            permissions.is_empty(),
            "{name}: App-token jobs must not inherit repository authority from GITHUB_TOKEN"
        );
        assert!(
            doc["jobs"][job]["permissions"].is_null(),
            "{name}: jobs.{job} must not override the empty ambient GITHUB_TOKEN grant"
        );
    }

    for (name, body) in [
        ("promotion-open-next.yml", promotion_body),
        ("toolchain-weekly.yml", toolchain_body),
    ] {
        for fallback in ["github.token", "secrets.GITHUB_TOKEN", "PROMOTION_PAT"] {
            assert!(
                !body.contains(fallback),
                "{name}: `{fallback}` is a fallback to the Actions or human token; missing App \
                 credentials must fail closed instead"
            );
        }
    }
}

#[test]
fn missing_app_credentials_cannot_skip_token_minting() {
    for (workflow_name, job_name) in [
        ("promotion-open-next.yml", "open-next"),
        ("toolchain-weekly.yml", "channel"),
    ] {
        let (_, doc) = workflow(workflow_name);
        let step = app_token_step(job_steps(&doc, workflow_name, job_name), workflow_name);
        assert!(
            step["if"].is_null(),
            "{workflow_name}: App-token minting must not be conditional"
        );
        assert!(
            step["continue-on-error"].is_null(),
            "{workflow_name}: App-token minting must fail the job when credentials are absent or invalid"
        );
    }
}

#[test]
fn promotion_claims_the_established_merge_permission_boundary() {
    let claims = [
        (
            ".github/workflows/promotion-open-next.yml",
            repo_text(".github/workflows/promotion-open-next.yml"),
        ),
        (
            "tests/promotion_ladder_test.rs",
            repo_text("tests/promotion_ladder_test.rs"),
        ),
        (
            "docs/plan/h1-6-machine-identity/CODE-CHANGES.md",
            repo_text("docs/plan/h1-6-machine-identity/CODE-CHANGES.md"),
        ),
    ];
    for (path, claim) in claims {
        let claim = claim.to_ascii_lowercase();
        for stale in [
            "inherently also authorizes merge",
            "same grant also authorizes merge APIs",
            "`pull-requests: write` can merge",
            "`pull-requests: write` also authorizes merge",
        ] {
            assert!(
                !claim.contains(stale),
                "{path} repeats the stale permission claim {stale:?}"
            );
        }
        assert!(
            claim.contains("rest merge endpoints require `contents: write`"),
            "{path} must state the established REST merge permission boundary"
        );
    }

    let readme = repo_text("docs/plan/h1-6-machine-identity/README.md");
    let pull_permission_row = readme
        .lines()
        .find(|line| line.starts_with("| `pull_requests: write` |"))
        .expect("README App permission table must retain its pull_requests: write row");
    for unsupported in ["auto-merge", "gh pr merge", "--disable-auto"] {
        assert!(
            !pull_permission_row.contains(unsupported),
            "README attributes {unsupported:?} to pull_requests: write without direct support: \
             {pull_permission_row}"
        );
    }
}

#[test]
fn documented_app_token_example_is_explicitly_attenuated_and_repo_scoped() {
    let doc = repo_text("docs/plan/h1-6-machine-identity/CODE-CHANGES.md");
    let marker = "uses: actions/create-github-app-token@<pin-by-sha>";
    let after_marker = doc
        .split_once(marker)
        .expect("CODE-CHANGES must retain the App-token example")
        .1;
    let example = after_marker
        .split_once("```")
        .expect("App-token example fence must close")
        .0;

    for input in [
        "app-id: ${{ vars.ANVIL_APP_ID }}",
        "private-key: ${{ secrets.ANVIL_APP_PRIVATE_KEY }}",
        "permission-contents: read",
        "permission-pull-requests: write",
    ] {
        assert!(
            example.contains(input),
            "documented App-token example omits required input {input:?}"
        );
    }
    let permission_inputs: Vec<&str> = example
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("permission-"))
        .collect();
    assert_eq!(
        permission_inputs,
        [
            "permission-contents: read",
            "permission-pull-requests: write",
        ],
        "documented App-token example must request exactly the reviewed permissions"
    );
    for wider_scope_input in ["owner:", "repositories:"] {
        assert!(
            !example.contains(wider_scope_input),
            "documented App-token example must omit {wider_scope_input} so it defaults to the \
             current repository"
        );
    }
}

#[test]
fn github_token_pr_event_docs_describe_the_approval_hold_not_absent_runs() {
    let code_changes = repo_text("docs/plan/h1-6-machine-identity/CODE-CHANGES.md");
    let code_claim = code_changes
        .split_once("That warning describes obsolete behavior:")
        .expect("CODE-CHANGES must explain why the old warning was removed")
        .1
        .split_once("**Do not add `contents: write`.**")
        .expect("CODE-CHANGES trigger explanation must end before the permission boundary")
        .0;
    let readme = repo_text("docs/plan/h1-6-machine-identity/README.md");
    let readme_claim = readme
        .split_once("**Cause B —")
        .expect("README must retain the second promotion failure cause")
        .1
        .split_once("### 4.3 Is a PAT a valid interim?")
        .expect("README Cause B and App remedy must stay in section 4")
        .0;

    for (path, section) in [
        (
            "docs/plan/h1-6-machine-identity/CODE-CHANGES.md",
            code_claim,
        ),
        ("docs/plan/h1-6-machine-identity/README.md", readme_claim),
    ] {
        let claim = normalized_words(section).to_ascii_lowercase();
        for stale in ["triggers no workflow runs", "triggers no runs"] {
            assert!(
                !claim.contains(stale),
                "{path} repeats obsolete GitHub Actions behavior: {stale:?}"
            );
        }
        for required in [
            "opened",
            "synchronize",
            "reopened",
            "approval-required",
            "manual approval",
            "execute without manual approval",
        ] {
            assert!(
                claim.contains(required),
                "{path} must describe current GITHUB_TOKEN PR-event behavior using {required:?}"
            );
        }
    }
}
