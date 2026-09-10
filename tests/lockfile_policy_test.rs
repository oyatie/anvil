//! The toolchain pin and the lockfile format are declared in two places that
//! must agree: `rust-toolchain.toml` and the `Cargo.lock` header. A drift
//! between them is how CI ends up building on a toolchain nobody chose. Anvil
//! once ran on 1.97.1 by accident: the host had it installed, CI said `stable`,
//! and the two agreed by coincidence.
//!
//! `[package] rust-version` was the third place, and is gone. An MSRV is a
//! contract with consumers; anvil is `publish = false` with no dependent, so
//! it had no counterparty and no job ever built under it.

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn toolchain_channel() -> String {
    let raw = fs::read_to_string(repo_root().join("rust-toolchain.toml"))
        .expect("rust-toolchain.toml must exist at the repo root");
    // One parser. This was the last hand-rolled copy, and it diverged: on
    // `channel = "..." # bumped weekly` its `trim_matches('"')` kept the
    // trailing comment, which would have marked every workflow wrong.
    anvil::toolchain::channel_text(&raw)
        .expect("rust-toolchain.toml must declare a channel")
        .to_string()
}

#[test]
fn the_toolchain_is_pinned_to_an_exact_build_not_a_moving_channel() {
    // The point is not the shape, it is that the pin names ONE compiler.
    // `stable` and `nightly` are different compilers on different days, so a
    // build under either is not reproducible and a bump has nothing to move
    // from. A release triple and a dated nightly both name exactly one.
    let channel = toolchain_channel();
    let exact_release = channel.split('.').count() == 3
        && channel
            .split('.')
            .all(|p| p.chars().all(|c| c.is_ascii_digit()));
    let dated_nightly = channel.strip_prefix("nightly-").is_some_and(|date| {
        match date.split('-').collect::<Vec<_>>()[..] {
            [y, m, d] => {
                y.len() == 4
                    && m.len() == 2
                    && d.len() == 2
                    && [y, m, d]
                        .iter()
                        .all(|p| p.bytes().all(|b| b.is_ascii_digit()))
            }
            _ => false,
        }
    });
    assert!(
        exact_release || dated_nightly,
        "rust-toolchain.toml channel must name one compiler -- an exact release \
         (x.y.z) or a dated nightly (nightly-YYYY-MM-DD) -- got {channel:?}; \
         `stable`/`nightly` make the build depend on the day it runs"
    );
}

#[test]
fn no_msrv_is_promised_because_nothing_would_hold_us_to_it() {
    // This replaces a test comparing MSRV against the channel. The pair is
    // gone, and what is worth guarding is that it stays gone: a `rust-version`
    // reintroduced without a job that builds under it is a promise with no
    // measurement behind it, which is worse than no promise at all.
    let manifest = fs::read_to_string(repo_root().join("Cargo.toml")).expect("Cargo.toml reads");
    let declared = manifest
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("rust-version"));
    assert!(
        declared.is_none(),
        "anvil is publish = false with no dependent, so an MSRV has no \
         counterparty; if one is reintroduced it needs a CI job that builds \
         under it. Found: {declared:?}"
    );
}

#[test]
fn every_workflow_installs_the_pinned_toolchain_and_nothing_else() {
    // The pin is one fact. Every `toolchain:` input in every workflow is
    // checked against it, not just the merge path and not just one match.
    //
    // The previous form asked whether the concatenated text of three files
    // CONTAINED one occurrence of the pin, which a single matching copy
    // satisfied -- so a different date in `nightly.yml` or
    // `supply-chain-weekly.yml` passed the whole suite. The copies are the
    // thing being policed; a check that stops at the first one polices nothing.
    let dir = repo_root().join(".github/workflows");
    let channel = toolchain_channel();
    let mut checked = 0;
    let mut wrong = Vec::new();
    let mut missing = Vec::new();
    let mut canary = Vec::new();

    for entry in fs::read_dir(&dir).expect("workflows directory") {
        let path = entry.expect("workflow entry").path();
        // GitHub accepts `.yaml` equally; skipping it is an evasion nobody
        // has used yet.
        if !matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("yml" | "yaml")
        ) {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let text = fs::read_to_string(&path).expect("workflow reads");
        let doc: serde_yaml::Value = serde_yaml::from_str(&text).expect("workflow parses");
        for job in steps_without_a_named_toolchain(&doc) {
            missing.push(format!("{name}: {job:?}"));
        }
        for (job, found) in toolchain_inputs(&doc) {
            // The canary is the one lane that must NOT be on the pin: it runs
            // ahead of it so a break is met before it is adopted. Named, and
            // asserted below rather than skipped -- an exemption nothing checks
            // is a hole, and this file exists because of holes like that.
            if name == "toolchain-weekly.yml" && job.as_deref() == Some("nightly") {
                canary.push(found);
                continue;
            }
            checked += 1;
            if found != channel {
                wrong.push(format!("{name}: {job:?} toolchain: \"{found}\""));
            }
        }
    }

    assert_eq!(
        canary,
        vec!["nightly".to_string()],
        "the canary lane must install floating `nightly`, which is what puts it \
         ahead of the pin; on the pin it would measure what CI already measures"
    );

    assert!(
        checked > 0,
        "no `toolchain:` input was found in any workflow; this check would pass \
         over a tree that installs whatever rustup defaults to"
    );
    assert!(
        wrong.is_empty(),
        "every workflow must install the pinned channel {channel:?}; these do not: {wrong:?}"
    );
    // The message used to say an omitted input "installs '' and rustup falls
    // back to stable", which this check could not see and which is not what
    // the action does. Seeded: deleting the input entirely PASSED. So the
    // omission is policed rather than described.
    assert!(
        missing.is_empty(),
        "every dtolnay/rust-toolchain step must name the toolchain explicitly; these do not: \
         {missing:?}. Without the input the action installs an empty toolchain and the \
         resolved compiler is whatever the pin file or rustup default supplies -- which is \
         the drift this file exists to catch."
    );
}

/// Jobs with a `dtolnay/rust-toolchain` step that names no toolchain.
///
/// The version has to be explicit: with no input the action installs an empty
/// toolchain, and what actually compiles is then decided by the pin file or the
/// rustup default rather than by the workflow. Enumerating the values is not
/// enough on its own -- a step that states nothing has no value to compare.
fn steps_without_a_named_toolchain(doc: &serde_yaml::Value) -> Vec<Option<String>> {
    let mut found = Vec::new();
    let Some(jobs) = doc.get("jobs").and_then(|j| j.as_mapping()) else {
        return found;
    };
    for (name, job) in jobs {
        let Some(steps) = job.get("steps").and_then(|s| s.as_sequence()) else {
            continue;
        };
        for step in steps {
            let uses = step
                .get("uses")
                .and_then(|u| u.as_str())
                .unwrap_or_default();
            if uses.starts_with("dtolnay/rust-toolchain@")
                && step
                    .get("with")
                    .and_then(|w| w.get("toolchain"))
                    .and_then(|t| t.as_str())
                    .is_none()
            {
                found.push(name.as_str().map(str::to_string));
            }
        }
    }
    found
}

/// Every `with.toolchain` value in a workflow, paired with the job holding it.
fn toolchain_inputs(doc: &serde_yaml::Value) -> Vec<(Option<String>, String)> {
    let mut found = Vec::new();
    let Some(jobs) = doc.get("jobs").and_then(|j| j.as_mapping()) else {
        return found;
    };
    for (name, job) in jobs {
        let job_name = name.as_str().map(str::to_string);
        for pin in pins_within(job) {
            found.push((job_name.clone(), pin));
        }
    }
    found
}

fn pins_within(node: &serde_yaml::Value) -> Vec<String> {
    let mut found = Vec::new();
    match node {
        serde_yaml::Value::Mapping(map) => {
            for (key, value) in map {
                if key.as_str() == Some("with")
                    && let Some(pin) = value.get("toolchain").and_then(|v| v.as_str())
                {
                    found.push(pin.to_string());
                }
                found.extend(pins_within(value));
            }
        }
        serde_yaml::Value::Sequence(items) => {
            for item in items {
                found.extend(pins_within(item));
            }
        }
        _ => {}
    }
    found
}

#[test]
fn ci_and_hooks_build_with_locked_dependencies() {
    let ci = merge_path_text();
    // Cheap local: pre-push `cargo check --locked`.
    // Pre-merge: clippy + nextest, both --locked.
    // Post-submit: `cargo build --release --locked` (release-profile compile check).
    for step in ["cargo clippy", "cargo nextest run", "cargo build --release"] {
        let line = ci
            .lines()
            .find(|l| l.contains(step))
            .unwrap_or_else(|| panic!("ci.yml must run `{step}`"));
        assert!(
            line.contains("--locked"),
            "`{step}` in ci.yml must pass --locked: {line}"
        );
    }
    // Parsed, not substring-matched. The property is that the release build is
    // post-submit, and the rung split makes that structural rather than a
    // condition: `release` lives in the file whose only trigger is `push`, so
    // there is no `if:` left to get wrong. Asserting the trigger asserts more
    // than the old guard did -- an `if:` can be edited off a job, a file's `on:`
    // cannot be without moving the job.
    let post: serde_yaml::Value = serde_yaml::from_str(
        &fs::read_to_string(repo_root().join(".github/workflows/postsubmit.yml"))
            .expect("postsubmit.yml"),
    )
    .expect("postsubmit.yml must be valid YAML");
    assert!(
        !post["jobs"]["release"].is_null(),
        "postsubmit.yml must define the `release` job; found: {:?}",
        post["jobs"]
            .as_mapping()
            .map(|m| m.keys().collect::<Vec<_>>())
    );
    let triggers = post["on"]
        .as_mapping()
        .expect("postsubmit.yml declares triggers");
    let names: Vec<&str> = triggers.keys().filter_map(|k| k.as_str()).collect();
    assert_eq!(
        names,
        vec!["push"],
        "the release build is post-submit: postsubmit.yml must trigger on push and nothing else"
    );
    let pre_push =
        fs::read_to_string(repo_root().join("src/git_manager/hooks/pre-push")).expect("pre-push");
    assert!(
        pre_push.contains("rustfmt --check"),
        "pre-push must rustfmt --check the changed *.rs list"
    );
    assert!(
        pre_push.contains("cargo check") && pre_push.contains("--locked"),
        "pre-push must `cargo check --locked`"
    );
    // The SUITE belongs in CI. Named source-only scans do not.
    //
    // This was a blanket ban on `cargo test` in the hook, and its intent -- keep
    // the hook fast -- is right and kept. But the ban also refused a class of
    // check that costs almost nothing and whose whole value is being early: a
    // scan that reads source, runs no service and touches no network, catching
    // a duplication or a stale count before it reaches a reviewer rather than
    // after.
    //
    // Measured on a warm tree rather than argued: the five scans below take
    // 1.08s, against the 74.7s `cargo check --all-targets` this hook already
    // pays two steps above. That is 1.4%, and `--all-targets` has already
    // type-checked them.
    //
    // So the rule is narrowed, not dropped: no bare `cargo test`, which would
    // run the whole corpus, and every invocation must name its targets.
    for line in pre_push.lines() {
        let l = line.trim();
        if !l.contains("cargo test") && !l.contains("cargo nextest") {
            continue;
        }
        assert!(
            l.contains("--test ") || l.ends_with('\\'),
            "pre-push runs an unbounded test invocation, which makes it the suite \
             and the suite belongs in CI: {l}"
        );
    }
    assert!(
        !pre_push.contains("cargo nextest run\n") && !pre_push.contains("cargo test\n"),
        "pre-push must not run the whole corpus; name the scans it needs"
    );
}

#[test]
fn lockfile_is_format_version_4() {
    let lock = fs::read_to_string(repo_root().join("Cargo.lock")).expect("Cargo.lock");
    let version_line = lock
        .lines()
        .find(|l| l.starts_with("version = "))
        .expect("Cargo.lock must carry a format version header");
    assert_eq!(
        version_line, "version = 4",
        "Cargo.lock must stay at format v4 (smaller merge diffs; the 1.83+ default)"
    );
}

/// The workflows on the merge path, as one text.
///
/// Presubmit, the lane it calls, and postsubmit. Deliberately NOT the scheduled
/// lanes: `toolchain-weekly` installs `stable` on purpose, because resolving
/// what latest stable IS is the question it exists to answer. A pin is a
/// merge-path property, and asking it of a drift detector inverts the rule.
fn merge_path_text() -> String {
    let dir = repo_root().join(".github/workflows");
    let mut all = String::new();
    for name in ["presubmit.yml", "build-and-test.yml", "postsubmit.yml"] {
        all.push_str(&fs::read_to_string(dir.join(name)).unwrap_or_else(|_| panic!("{name}")));
        all.push('\n');
    }
    all
}
