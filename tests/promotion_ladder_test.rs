//! The promotion ladder is described in two workflows that must agree.
//!
//! `promotion-predecessor.yml` holds `pred`, mapping each rung to the only
//! branch allowed to promote into it. `promotion-open-next.yml` holds `next`,
//! mapping each rung to the rung it feeds. They are inverses of one another,
//! and nothing but these tests makes them stay that way: a `next` that has
//! drifted from `pred` opens promotion pull requests the guard is guaranteed
//! to reject, which reads as a broken repository rather than as a broken map.
//! The predecessor must also be the branch in this repository: a fork branch
//! with the same short name is not a rung in this ladder.
//!
//! The ladder itself is `dev -> staging -> canary -> production`, with `dev`
//! as the trunk. It went unexercised for its whole existence — `staging`,
//! `canary` and `production` sat on the same commit, 43 behind, while work
//! landed on `main` — so these are the first checks it has ever had.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

const REVIEWED_OPENER_SCRIPT_SHA256: &str =
    "dde564f3ed83d279e5a31b40f7a6ca00d169ac015e79a1b45165abb92d95777d";
const REVIEWED_PREDECESSOR_SCRIPT_SHA256: &str =
    "8ab4b706ee812ca2a849485264d9142012eb519daddf025033f92fcafa210acf";

fn workflow(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".github/workflows")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} must exist: {e}", path.display()))
}

/// Extracts `const <binding> = { a: "b", c: "d" };` from a workflow's inline
/// `script:` block.
///
/// Deliberately not a YAML parse: the maps live *inside* a YAML string, so a
/// YAML parser would hand back the same text to scan anyway.
fn ladder(src: &str, binding: &str) -> BTreeMap<String, String> {
    let needle = format!("const {binding} = {{");
    let start = src
        .find(&needle)
        .unwrap_or_else(|| panic!("no `{needle}` found; the ladder map was renamed or removed"));
    let open = start + needle.len();
    let close = src[open..]
        .find('}')
        .unwrap_or_else(|| panic!("`const {binding}` is not closed; it must stay a one-line map"));

    let mut map = BTreeMap::new();
    for entry in src[open..open + close].split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let (key, value) = entry
            .split_once(':')
            .unwrap_or_else(|| panic!("malformed ladder entry {entry:?} in `const {binding}`"));
        map.insert(
            key.trim().trim_matches('"').to_string(),
            value.trim().trim_matches('"').to_string(),
        );
    }
    assert!(
        !map.is_empty(),
        "`const {binding}` parsed to an empty ladder; this parser has drifted from the workflow \
         and would pass every test below vacuously"
    );
    map
}

/// The `branches: [...]` list a workflow triggers on.
fn push_branches(src: &str, name: &str) -> Vec<String> {
    let lines: Vec<&str> = src
        .lines()
        .filter(|l| l.trim_start().starts_with("branches: ["))
        .collect();
    assert_eq!(
        lines.len(),
        1,
        "{name} must declare exactly one `branches:` list; found {}",
        lines.len()
    );
    let after = lines[0].split_once('[').expect("`branches:` list opens").1;
    let inner = after
        .rsplit_once(']')
        .expect("`branches:` list must close on the same line")
        .0;
    let mut branches: Vec<String> = inner
        .split(',')
        .map(|b| b.trim().to_string())
        .filter(|b| !b.is_empty())
        .collect();
    branches.sort();
    branches
}

/// The exact JavaScript value GitHub Actions passes to one
/// `actions/github-script` job.
fn inline_script(src: &str, workflow_name: &str, job_name: &str) -> String {
    let doc: serde_yaml::Value =
        serde_yaml::from_str(src).unwrap_or_else(|error| panic!("{workflow_name}: {error}"));
    let steps = doc["jobs"][job_name]["steps"]
        .as_sequence()
        .unwrap_or_else(|| panic!("{workflow_name} must declare jobs.{job_name}.steps"));
    let found: Vec<&serde_yaml::Value> = steps
        .iter()
        .filter(|step| {
            step["uses"]
                .as_str()
                .is_some_and(|uses| uses.starts_with("actions/github-script@"))
        })
        .collect();
    assert_eq!(
        found.len(),
        1,
        "{workflow_name} must carry exactly one actions/github-script step"
    );
    found[0]["with"]["script"]
        .as_str()
        .unwrap_or_else(|| panic!("{workflow_name} github-script step must carry an inline script"))
        .to_string()
}

fn script_fingerprint(script: &str) -> String {
    hex::encode(Sha256::digest(script.as_bytes()))
}

fn normalized_words(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_predecessor_source(
    current_repo: &str,
    base_repo: Option<&str>,
    head_repo: Option<&str>,
    expected_ref: &str,
    head_ref: &str,
) -> bool {
    base_repo == Some(current_repo) && head_repo == base_repo && head_ref == expected_ref
}

/// An inventory of canonical dot-form client paths in the already fingerprinted
/// script. This is deliberately not a JavaScript parser or a security boundary:
/// bracket access, optional chaining, aliases, and other transports evade it.
fn dot_form_api_inventory(script: &str) -> BTreeSet<String> {
    let mut calls = BTreeSet::new();
    for (at, _) in script.match_indices("github.") {
        let rest = &script[at..];
        let end = rest
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '_')
            .unwrap_or(rest.len());
        calls.insert(rest[..end].trim_end_matches('.').to_string());
    }
    calls
}

#[test]
fn predecessor_lifecycle_includes_base_edits() {
    let doc: serde_yaml::Value =
        serde_yaml::from_str(&workflow("promotion-predecessor.yml")).unwrap();
    let trigger = &doc["on"]["pull_request"];
    let types = trigger["types"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        types,
        BTreeSet::from(["opened", "reopened", "synchronize", "edited"])
    );
    assert_eq!(trigger["types"].as_sequence().unwrap().len(), 4);
    assert!(doc["on"]["pull_request_target"].is_null());
    let branches = trigger["branches"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        branches,
        BTreeSet::from(["staging", "canary", "production"])
    );
}

#[test]
fn the_opener_and_the_guard_describe_the_same_ladder() {
    let next = ladder(&workflow("promotion-open-next.yml"), "next");
    let pred = ladder(&workflow("promotion-predecessor.yml"), "pred");

    assert_eq!(
        next.len(),
        pred.len(),
        "the ladders have different numbers of rungs: opener {next:?} vs guard {pred:?}"
    );

    for (from, to) in &next {
        assert_eq!(
            pred.get(to).map(String::as_str),
            Some(from.as_str()),
            "opener promotes {from} -> {to}, but the guard says {to} may only be promoted from \
             {:?}. Every pull request the opener creates for this rung would be rejected.",
            pred.get(to)
        );
    }
}

#[test]
fn the_opener_triggers_on_exactly_the_rungs_that_can_advance() {
    let src = workflow("promotion-open-next.yml");
    let next = ladder(&src, "next");
    let triggers = push_branches(&src, "promotion-open-next.yml");
    let rungs: Vec<String> = next.keys().cloned().collect();

    assert_eq!(
        triggers, rungs,
        "promotion-open-next.yml triggers on {triggers:?} but its ladder covers {rungs:?}. \
         A rung in the map but not the trigger never opens its successor; a rung in the trigger \
         but not the map fails the run with `no rung follows`."
    );
}

#[test]
fn the_opener_has_no_ref_write_and_its_reviewed_script_is_pinned() {
    let src = workflow("promotion-open-next.yml");

    // REST merge endpoints require `contents: write`; this contents-read token
    // therefore cannot push, delete a ref, or REST-merge. `pull-requests: write`
    // still permits PR and review mutation, which keeps the executable script
    // worth pinning exactly.
    assert!(
        src.contains("permission-contents: read"),
        "promotion-open-next.yml must request `permission-contents: read`"
    );
    assert!(
        !src.contains("permission-contents: write"),
        "promotion-open-next.yml must never request `permission-contents: write`; that would let \
         its App token push to a rung directly"
    );

    let script = inline_script(&src, "promotion-open-next.yml", "open-next");
    assert_eq!(
        script_fingerprint(&script),
        REVIEWED_OPENER_SCRIPT_SHA256,
        "promotion-open-next.yml's executable script changed. REST merge endpoints require \
         `contents: write`, which this token lacks, but PR-write still grants mutation authority. \
         Every script change needs line-by-line review and a deliberate fingerprint update."
    );

    // This inventory makes the manually reviewed surface legible. The
    // fingerprint above, not this intentionally narrow scanner, catches other
    // JavaScript spellings and transports.
    let reviewed_inventory: BTreeSet<String> = [
        "github.rest.pulls.list",
        "github.rest.pulls.create",
        "github.rest.repos.compareCommitsWithBasehead",
    ]
    .iter()
    .map(|call| call.to_string())
    .collect();
    assert_eq!(
        dot_form_api_inventory(&script),
        reviewed_inventory,
        "the canonical API inventory changed along with the fingerprint; document the reviewed \
         surface before accepting a new script fingerprint"
    );
}

#[test]
fn alternate_javascript_pr_mutations_trip_the_script_fingerprint() {
    let script = inline_script(
        &workflow("promotion-open-next.yml"),
        "promotion-open-next.yml",
        "open-next",
    );
    let old_inventory = dot_form_api_inventory(&script);
    let alternates = [
        (
            "bracket access",
            format!(r#"{script}\nawait github["rest"]["pulls"]["update"]({{}});"#),
        ),
        (
            "optional chaining",
            format!("{script}\nawait github?.rest?.pulls?.update({{}});"),
        ),
        (
            "destructured alias",
            format!("{script}\nconst {{ rest }} = github; await rest.pulls.update({{}});"),
        ),
        (
            "direct fetch",
            format!(
                "{script}\nconst token = core.getInput('github-token'); \
                 await fetch('https://api.' + 'github' + '.com/repos/o/r/pulls/1', \
                 {{ method: 'PATCH', headers: {{ authorization: 'Bearer ' + token }} }});"
            ),
        ),
    ];

    for (name, alternate) in alternates {
        assert_ne!(alternate, script, "{name} seed did not alter the script");
        assert_eq!(
            dot_form_api_inventory(&alternate),
            old_inventory,
            "{name} must reproduce the bypass in the retired dot-form-only guard"
        );
        assert_ne!(
            script_fingerprint(&alternate),
            REVIEWED_OPENER_SCRIPT_SHA256,
            "{name} escaped the whole-script fingerprint"
        );
    }
}

#[test]
fn the_predecessor_guard_rejects_forks_and_missing_head_repositories() {
    let script = inline_script(
        &workflow("promotion-predecessor.yml"),
        "promotion-predecessor.yml",
        "promotion-predecessor",
    );

    for required in [
        "const currentRepo = `${context.repo.owner}/${context.repo.repo}`;",
        "const baseRepo = context.payload.pull_request.base.repo?.full_name;",
        "const headRepo = context.payload.pull_request.head.repo?.full_name;",
    ] {
        assert!(
            script.contains(required),
            "promotion-predecessor.yml must derive repository provenance with {required:?}"
        );
    }

    let normalized = normalized_words(&script);
    let exact_predicate = "const sourceIsPredecessor = baseRepo === currentRepo && \
                           headRepo === baseRepo && head === want;";
    assert!(
        normalized.contains(exact_predicate),
        "promotion-predecessor.yml must derive its decision from the same repository-and-ref \
         predicate exercised below"
    );
    assert!(
        normalized.contains("if (!sourceIsPredecessor) { core.setFailed(")
            && normalized.contains("return; }"),
        "a source that fails the reviewed predicate must fail the check and return"
    );

    let current = "oyatie/anvil";
    let cases = [
        (
            "same-repository predecessor",
            Some(current),
            Some(current),
            "dev",
            true,
        ),
        (
            "same-named fork",
            Some(current),
            Some("attacker/anvil"),
            "dev",
            false,
        ),
        ("deleted fork", Some(current), None, "dev", false),
        ("missing base repository", None, Some(current), "dev", false),
        (
            "foreign base and head",
            Some("attacker/anvil"),
            Some("attacker/anvil"),
            "dev",
            false,
        ),
        (
            "same repository, wrong predecessor ref",
            Some(current),
            Some(current),
            "feature",
            false,
        ),
    ];
    for (name, base_repo, head_repo, head_ref, accepted) in cases {
        assert_eq!(
            is_predecessor_source(current, base_repo, head_repo, "dev", head_ref),
            accepted,
            "unexpected predecessor decision for {name}"
        );
    }
    let forged_head_ref = "dev";
    let expected_ref = "dev";
    let retired_ref_only_accepts = forged_head_ref == expected_ref;
    assert!(
        retired_ref_only_accepts,
        "the retired ref-only predicate accepted attacker/anvil:dev"
    );
    assert!(
        !is_predecessor_source(current, Some(current), Some("attacker/anvil"), "dev", "dev"),
        "the repository-aware predicate must close the demonstrated ref-only false green"
    );

    assert_eq!(
        script_fingerprint(&script),
        REVIEWED_PREDECESSOR_SCRIPT_SHA256,
        "promotion-predecessor.yml's executable script changed. Repository provenance and the \
         predecessor ref are one reviewed decision; every script change needs line-by-line review \
         and a deliberate fingerprint update."
    );
}

#[test]
fn every_rung_is_covered_by_ci() {
    // Both rungs that carry a branch filter. Presubmit admits a change onto a
    // branch; postsubmit compiles what landed there. A rung named in neither is
    // a branch that receives promoted code with nothing reading it.
    let ci = format!(
        "{}\n{}",
        workflow("presubmit.yml"),
        workflow("postsubmit.yml")
    );
    let next = ladder(&workflow("promotion-open-next.yml"), "next");

    // Both ends of every rung: the branch that promotes and the branch promoted
    // into.
    let mut rungs: Vec<&str> = next.keys().map(String::as_str).collect();
    rungs.extend(next.values().map(String::as_str));

    let lists: Vec<Vec<String>> = ci
        .lines()
        .filter(|l| l.trim_start().starts_with("branches: ["))
        .map(|line| {
            let after = line.split_once('[').expect("`branches:` list opens").1;
            let inner = after
                .rsplit_once(']')
                .expect("`branches:` list must close on the same line")
                .0;
            inner
                .split(',')
                .map(|b| b.trim().to_string())
                .filter(|b| !b.is_empty())
                .collect()
        })
        .collect();

    // Without this, a ci.yml that had lost both `branches:` lists would pass
    // every assertion below by never running one.
    assert_eq!(
        lists.len(),
        2,
        "ci.yml must declare a `branches:` list for both `push` and `pull_request`; found {}",
        lists.len()
    );

    for (index, list) in lists.iter().enumerate() {
        for rung in &rungs {
            assert!(
                list.iter().any(|b| b == rung),
                "ci.yml `branches:` list #{index} is {list:?}, which omits the promotion rung \
                 `{rung}`. Code would be promoted onto it with no build and no tests."
            );
        }
    }
}
