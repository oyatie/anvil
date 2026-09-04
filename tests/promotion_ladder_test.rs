//! The promotion ladder is described in two workflows that must agree.
//!
//! `promotion-predecessor.yml` holds `pred`, mapping each rung to the only
//! branch allowed to promote into it. `promotion-open-next.yml` holds `next`,
//! mapping each rung to the rung it feeds. They are inverses of one another,
//! and nothing but these tests makes them stay that way: a `next` that has
//! drifted from `pred` opens promotion pull requests the guard is guaranteed
//! to reject, which reads as a broken repository rather than as a broken map.
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

/// The exact JavaScript value GitHub Actions passes to `actions/github-script`.
fn inline_script(src: &str) -> String {
    let doc: serde_yaml::Value =
        serde_yaml::from_str(src).expect("promotion-open-next.yml must parse as YAML");
    let steps = doc["jobs"]["open-next"]["steps"]
        .as_sequence()
        .expect("promotion-open-next.yml must declare jobs.open-next.steps");
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
        "promotion-open-next.yml must carry exactly one actions/github-script step"
    );
    found[0]["with"]["script"]
        .as_str()
        .expect("promotion-open-next.yml github-script step must carry an inline script")
        .to_string()
}

fn script_fingerprint(script: &str) -> String {
    hex::encode(Sha256::digest(script.as_bytes()))
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

    // This is a real capability boundary for refs: the token cannot push or
    // delete one. It is NOT a merge boundary. Creating a pull request requires
    // `pull-requests: write`, and that same grant also authorizes merge APIs.
    assert!(
        src.contains("permission-contents: read"),
        "promotion-open-next.yml must request `permission-contents: read`"
    );
    assert!(
        !src.contains("permission-contents: write"),
        "promotion-open-next.yml must never request `permission-contents: write`; that would let \
         its App token push to a rung directly"
    );

    let script = inline_script(&src);
    assert_eq!(
        script_fingerprint(&script),
        REVIEWED_OPENER_SCRIPT_SHA256,
        "promotion-open-next.yml's executable script changed. `pull-requests: write` can merge, \
         so every script change needs line-by-line review and a deliberate fingerprint update. \
         The fingerprint is a change-detection boundary, not permission attenuation."
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
fn alternate_javascript_authority_paths_trip_the_script_fingerprint() {
    let script = inline_script(&workflow("promotion-open-next.yml"));
    let old_inventory = dot_form_api_inventory(&script);
    let alternates = [
        (
            "bracket access",
            format!(r#"{script}\nawait github["rest"]["pulls"]["merge"]({{}});"#),
        ),
        (
            "optional chaining",
            format!("{script}\nawait github?.rest?.pulls?.merge({{}});"),
        ),
        (
            "destructured alias",
            format!("{script}\nconst {{ rest }} = github; await rest.pulls.merge({{}});"),
        ),
        (
            "direct fetch",
            format!(
                "{script}\nawait fetch('https://api.' + 'github' + '.com/repos/o/r/pulls/1/merge');"
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
