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

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

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

/// The workflow's inline `script:` block, from `script: |` to end of file.
fn inline_script(src: &str) -> &str {
    let (before, script) = src
        .split_once("script: |")
        .expect("promotion-open-next.yml must carry an inline `script:` block");
    assert!(
        !before.contains("script: |") && !script.contains("script: |"),
        "more than one inline `script:` block; this scan only reaches the last one"
    );
    script
}

/// Every `github.<path>` the inline script names, e.g. `github.rest.pulls.create`.
fn api_calls(script: &str) -> BTreeSet<String> {
    let mut calls = BTreeSet::new();
    for (at, _) in script.match_indices("github.") {
        let rest = &script[at..];
        let end = rest
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '_')
            .unwrap_or(rest.len());
        calls.insert(rest[..end].trim_end_matches('.').to_string());
    }
    assert!(
        !calls.is_empty(),
        "the script names no `github.` call at all; this scanner has drifted from the workflow \
         and would allow anything"
    );
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
fn the_opener_can_only_open_never_merge_approve_or_delete() {
    let src = workflow("promotion-open-next.yml");

    // Stops a push to a rung and a branch deletion, and nothing else. It does
    // NOT stop a merge: `pulls.merge`, `mergePullRequest` and
    // `enablePullRequestAutoMerge` all run on `pull-requests: write` alone.
    assert!(
        src.contains("contents: read"),
        "promotion-open-next.yml must declare `contents: read`"
    );
    assert!(
        !src.contains("contents: write"),
        "promotion-open-next.yml must never take `contents: write`; opening a pull request needs \
         only `pull-requests: write`, and `contents: write` would let it push to a rung directly"
    );

    let script = inline_script(&src);

    // An allow-list, not a forbidden list. A forbidden list only stops the
    // merges somebody thought to name: `enablePullRequestAutoMerge` was on no
    // such list, needs only `pull-requests: write`, and -- with the
    // repository's `allow_auto_merge: true` and no required status check on any
    // rung -- would land the promotion the moment it was opened, unreviewed.
    // Naming what the script MAY call leaves nothing to overlook.
    let allowed: BTreeSet<String> = [
        "github.rest.pulls.list",
        "github.rest.pulls.create",
        "github.rest.repos.compareCommitsWithBasehead",
    ]
    .iter()
    .map(|call| call.to_string())
    .collect();
    assert_eq!(
        api_calls(script),
        allowed,
        "promotion-open-next.yml calls a GitHub API it is not allowed to. It opens promotion \
         pull requests and does nothing else: it may read what is already open, compare two \
         rungs, and create a pull request. Merging, approving, enabling auto-merge, pushing and \
         deleting are not its job, and `github.graphql` reaches every one of them."
    );

    // The allow-list covers the octokit client only. `exec` and a `require`d
    // module reach `gh pr merge`, which names no `github.` call at all.
    for hatch in ["exec.", "require(", "import("] {
        assert!(
            !script.contains(hatch),
            "promotion-open-next.yml uses `{hatch}`; that escapes the allow-list above and can \
             reach a merge without naming one"
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
