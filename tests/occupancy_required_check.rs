//! `admit_spawn` behind a check, not behind a library nobody calls.
//!
//! These assertions read `.github/workflows/ci.yml` as data. They fix the
//! shape of the job graph: occupancy runs on `pull_request` only, and
//! `fast-checks` refuses to start rustc until occupancy has a verdict.

use serde_yaml::Value;
use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn ci_text() -> String {
    fs::read_to_string(repo_root().join(".github/workflows/ci.yml")).expect("ci.yml")
}

fn ci() -> Value {
    serde_yaml::from_str(&ci_text()).expect("ci.yml parses as YAML")
}

fn job(name: &str) -> Value {
    ci()["jobs"][name].clone()
}

fn steps(job_name: &str) -> Vec<Value> {
    job(job_name)["steps"]
        .as_sequence()
        .unwrap_or_else(|| panic!("job `{job_name}` must declare steps"))
        .clone()
}

/// Every `run:` script in a job, concatenated, with shell comments removed.
///
/// The comments are dropped because a comment mentioning a flag is not the
/// same as the flag being passed. A mutant that deleted `previous_filename`
/// from the jq expression survived an earlier version of these assertions
/// by leaving the sentence that described it standing.
fn scripts(job_name: &str) -> String {
    run_steps(job_name).join("\n")
}

fn run_steps(job_name: &str) -> Vec<String> {
    steps(job_name)
        .iter()
        .filter_map(|s| s.get("run").and_then(Value::as_str))
        .map(|body| {
            body.lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect()
}

#[test]
fn occupancy_is_a_job_of_its_own() {
    let j = job("occupancy");
    assert!(
        j.is_mapping(),
        "ci.yml must declare an `occupancy` job; a library nobody calls is not enforcement"
    );
    assert_eq!(
        j["name"].as_str(),
        Some("occupancy"),
        "the check context is a capability name, unprefixed and unbranded"
    );
}

#[test]
fn occupancy_runs_on_pull_request_and_skips_merge_group() {
    let cond = job("occupancy")["if"]
        .as_str()
        .expect("the occupancy job must carry an `if`")
        .to_owned();
    assert!(
        cond.contains("github.event_name == 'pull_request'"),
        "occupancy is a pull_request check: {cond}"
    );
    assert!(
        !cond.contains("merge_group"),
        "merge_group is excluded by omission from the pull_request test, not re-admitted: {cond}"
    );
}

#[test]
fn the_merge_group_skip_is_documented_in_the_workflow() {
    let text = ci_text();
    // The contiguous comment blocks of the file, so the rationale is read
    // as the paragraph it was written as.
    let mut blocks: Vec<String> = Vec::new();
    let mut block: Vec<&str> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(body) = line.strip_prefix('#') {
            block.push(body.trim());
        } else if !block.is_empty() {
            blocks.push(block.join(" "));
            block.clear();
        }
    }
    if !block.is_empty() {
        blocks.push(block.join(" "));
    }
    let joined = blocks
        .into_iter()
        .find(|b| b.contains("merge_group"))
        .expect(
            "the workflow must carry a comment about merge_group, \
             or the next reader deletes the skip",
        );
    assert!(
        joined.contains("deliberate"),
        "the comment must say the skip is deliberate: {joined}"
    );
    assert!(
        joined.contains("combination test"),
        "the comment must say why: the merge-group SHA is the combination test: {joined}"
    );
}

#[test]
fn fast_checks_waits_for_occupancy() {
    let needs = job("fast-checks")["needs"].clone();
    let needs: Vec<String> = needs
        .as_sequence()
        .expect("fast-checks must declare `needs` as a sequence")
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    assert!(
        needs.iter().any(|n| n == "occupancy"),
        "fast-checks must wait on occupancy, so rustc never starts on a refused overlap: {needs:?}"
    );
    let cond = job("fast-checks")["if"]
        .as_str()
        .expect("fast-checks must carry `if: always()` so a red occupancy does not skip it")
        .to_owned();
    assert!(
        cond.contains("always()"),
        "a skipped job reads as success to a required context; fast-checks must still run: {cond}"
    );
}

#[test]
fn fast_checks_refuses_before_it_installs_a_compiler() {
    let steps = steps("fast-checks");
    let guard = steps
        .iter()
        .position(|s| {
            let reads = serde_yaml::to_string(s).unwrap_or_default();
            s.get("run").is_some() && reads.contains("needs.occupancy.result")
        })
        .expect("fast-checks must read the occupancy verdict itself, in a step that runs a script");
    assert_eq!(
        guard, 0,
        "the verdict is read first: before checkout, before the toolchain, before any cargo"
    );
    // `fast-checks` was a job that did the work -- checkout, toolchain, cargo --
    // when this test was written. It is now a fan-in over `fmt` and `test` and
    // runs no actions at all, so "precedes every `uses:` step" is satisfied
    // vacuously. That is a real strengthening rather than a weakening: nothing
    // is installed in this job under any circumstances. The load-bearing
    // assertion is `guard == 0` above, which still holds and is checked whether
    // or not any action follows it.
    if let Some(first_uses) = steps.iter().position(|s| s.get("uses").is_some()) {
        assert!(
            guard < first_uses,
            "the guard must precede every `uses:` step, not merely the cargo ones"
        );
    }
}

#[test]
fn the_guard_is_closed_against_every_result_but_success_and_skipped() {
    let guard = scripts("fast-checks");
    assert!(
        guard.contains("success") && guard.contains("skipped"),
        "success admits; skipped admits, because merge_group skips occupancy"
    );
    assert!(
        guard.contains("exit 1"),
        "failure, cancellation and a missing verdict all have to exit non-zero"
    );
    assert!(
        !guard.contains("continue-on-error"),
        "a guard that cannot fail is not a guard"
    );
}

#[test]
fn occupancy_reads_changed_files_from_the_rest_api() {
    let s = scripts("occupancy");
    assert!(
        s.contains("/files"),
        "changed-file sets come from the pulls files endpoint"
    );
    assert!(
        s.contains("previous_filename"),
        "a rename occupies both ends; the old path must be collected too"
    );
    assert!(
        !s.contains("gh pr diff"),
        "the unified-diff endpoint 406s on a large PR; names only"
    );
}

#[test]
fn occupancy_reads_every_open_pull_request_on_the_same_trunk() {
    let s = scripts("occupancy");
    assert!(
        s.contains("--base \"${BASE}\""),
        "occupancy is per-trunk: a PR onto another base is not combining with this one"
    );
    let base_env = steps("occupancy")
        .iter()
        .filter_map(|s| s.get("env").and_then(|e| e.get("BASE")).cloned())
        .filter_map(|v| v.as_str().map(str::to_owned))
        .next()
        .expect("the occupancy job must bind BASE");
    assert!(
        base_env.contains("github.event.pull_request.base.ref"),
        "the trunk is the one this PR targets, not a name pinned in the workflow: {base_env}"
    );
    let limit = s
        .split("--limit ")
        .nth(1)
        .and_then(|t| t.split_whitespace().next())
        .and_then(|n| n.parse::<u32>().ok())
        .expect("the open-PR listing must pass an explicit --limit");
    assert!(
        limit > 30,
        "`gh pr list` defaults to 30; a silently truncated list reads as no overlap"
    );
}

#[test]
fn occupancy_fails_closed_on_a_forge_that_does_not_answer() {
    let j = job("occupancy");
    assert_ne!(
        j["continue-on-error"].as_bool(),
        Some(true),
        "a rate limit must not read as no overlap"
    );
    for s in steps("occupancy") {
        assert_ne!(
            s["continue-on-error"].as_bool(),
            Some(true),
            "no step in occupancy may swallow its own failure"
        );
    }
    for script in run_steps("occupancy") {
        assert!(
            script.contains("set -euo pipefail"),
            "every script in occupancy aborts on an unset variable or a failed pipe stage; \
             one that does not can hand the next step an empty path set: {script}"
        );
    }
    let s = scripts("occupancy");
    assert!(
        !s.contains("|| true"),
        "`|| true` is how a forge error becomes a green check"
    );
    assert!(
        s.contains("changedFiles"),
        "the collected name count is cross-checked against the count the forge itself \
         reports, so a truncated or malformed response cannot read as an empty path set"
    );
    assert!(
        s.contains(r#""${counted}" -ne "${declared}""#),
        "the two readings are compared, and a disagreement refuses rather than picking one"
    );
}

#[test]
fn occupancy_runs_the_library_not_a_reimplementation() {
    let s = scripts("occupancy");
    assert!(
        s.contains("--bin occupancy"),
        "the verdict comes from `admit_spawn`, through the crate's own binary"
    );
    assert!(
        s.contains("--locked"),
        "the check builds the dependency versions the lockfile pins"
    );
}
