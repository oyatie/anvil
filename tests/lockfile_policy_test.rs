//! The toolchain pin and the lockfile format are declared in three places that
//! must agree: `rust-toolchain.toml`, `[package] rust-version`, and the
//! `Cargo.lock` header. A drift between them is how CI ends up building on a
//! toolchain nobody chose — which is exactly how Anvil ran on 1.97.1 before
//! this pin existed: the host had it installed, CI said `stable`, and the two
//! agreed by coincidence.

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn toolchain_channel() -> String {
    let raw = fs::read_to_string(repo_root().join("rust-toolchain.toml"))
        .expect("rust-toolchain.toml must exist at the repo root");
    raw.lines()
        .find_map(|l| {
            let l = l.trim();
            l.strip_prefix("channel").map(|rest| {
                rest.trim_start_matches([' ', '='])
                    .trim()
                    .trim_matches('"')
                    .to_string()
            })
        })
        .expect("rust-toolchain.toml must declare a channel")
}

fn package_rust_version() -> String {
    let raw = fs::read_to_string(repo_root().join("Cargo.toml")).expect("Cargo.toml");
    raw.lines()
        .find_map(|l| {
            let l = l.trim();
            l.strip_prefix("rust-version").map(|rest| {
                rest.trim_start_matches([' ', '='])
                    .trim()
                    .trim_matches('"')
                    .to_string()
            })
        })
        .expect("Cargo.toml must declare [package] rust-version")
}

#[test]
fn toolchain_is_pinned_to_an_exact_version_not_a_channel_name() {
    let channel = toolchain_channel();
    let looks_like_version = channel.split('.').count() == 3
        && channel
            .split('.')
            .all(|p| p.chars().all(|c| c.is_ascii_digit()));
    assert!(
        looks_like_version,
        "rust-toolchain.toml channel must be an exact version (x.y.z), got {channel:?}; \
         `stable`/`nightly` make the build depend on the day it runs"
    );
}

#[test]
fn package_rust_version_matches_the_toolchain_pin() {
    assert_eq!(
        package_rust_version(),
        toolchain_channel(),
        "[package] rust-version and rust-toolchain.toml must name the same version"
    );
}

#[test]
fn ci_installs_the_pinned_toolchain_not_stable() {
    let ci = merge_path_text();
    // rust-toolchain.toml is the pin. Repeating the version in YAML is a drift
    // surface; dtolnay/rust-toolchain with no `toolchain:` input honours the file.
    let channel = toolchain_channel();
    assert!(
        ci.contains(&format!("toolchain: \"{channel}\"")),
        "dtolnay/rust-toolchain@pinned SHA requires toolchain: \"{channel}\"; omitting it installs '' and rustup default becomes stable"
    );
    assert!(
        !ci.contains("toolchain: stable"),
        "ci.yml must not install `stable`; rust-toolchain.toml is the pin"
    );
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
