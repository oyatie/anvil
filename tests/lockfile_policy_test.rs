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
    let ci = fs::read_to_string(repo_root().join(".github/workflows/ci.yml")).expect("ci.yml");
    let channel = toolchain_channel();
    assert!(
        ci.contains(&format!("toolchain: \"{channel}\"")),
        "ci.yml must install toolchain \"{channel}\" explicitly"
    );
    assert!(
        !ci.contains("toolchain: stable"),
        "ci.yml must not install `stable`; the pin exists so CI and local builds agree"
    );
}

#[test]
fn ci_and_hooks_build_with_locked_dependencies() {
    let ci = fs::read_to_string(repo_root().join(".github/workflows/ci.yml")).expect("ci.yml");
    for step in ["cargo clippy", "cargo test", "cargo build --release"] {
        let line = ci
            .lines()
            .find(|l| l.contains(step))
            .unwrap_or_else(|| panic!("ci.yml must run `{step}`"));
        assert!(
            line.contains("--locked"),
            "`{step}` in ci.yml must pass --locked: {line}"
        );
    }
    let pre_push = fs::read_to_string(repo_root().join(".githooks/pre-push")).expect("pre-push");
    assert!(
        pre_push.contains("cargo check") && pre_push.contains("--locked"),
        "pre-push must `cargo check --locked`"
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
