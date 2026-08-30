//! `gh` is spawned in one place, or the environment decision is optional.
//!
//! Thirty-four sites across ten modules built their own `gh` command, so every
//! call to the forge carried `GITHUB_WEBHOOK_SECRET`, every model provider key
//! and `SSH_AUTH_SOCK` into a process that needs none of them. That was the
//! default rather than thirty-four oversights, and the thirty-fifth site would
//! have inherited it.
//!
//! `exec::gh` is the seam. This refuses a spawn that skips it, and measures a
//! real child to show the bound is applied rather than merely written down.

use std::fs;
use std::path::{Path, PathBuf};

/// The seam itself, which is where the bare `Command::new` is supposed to be.
const SEAM: &str = "src/exec/gh.rs";

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Every bare `gh` spawn in production `src/`, as repo-relative paths.
///
/// `without_test_modules` for the reason the model-spawn seam gives: a fixture
/// that builds a `Command` to read its argv back is not a spawn site, and
/// judging it as one would push every such fixture out of the module it tests.
///
/// `without_commentary` because the seam's own module documentation names the
/// call it replaced, and this scan counted the sentence as a thirty-fifth site.
/// `code_only` would blank the `"gh"` in the needle along with the prose; this
/// strips comments and leaves string literals standing, which is the
/// distinction between the two.
fn gh_spawns() -> Vec<String> {
    let mut files = Vec::new();
    rust_sources(&repo().join("src"), &mut files);
    files.sort();
    let mut found = Vec::new();
    for p in files {
        let rel = p
            .strip_prefix(repo())
            .unwrap_or(&p)
            .to_string_lossy()
            .replace('\\', "/");
        let Ok(raw) = fs::read_to_string(&p) else {
            continue;
        };
        let body =
            anvil::source_scan::without_commentary(&anvil::source_scan::without_test_modules(&raw));
        for _ in body.matches("Command::new(\"gh\")") {
            found.push(rel.clone());
        }
    }
    found
}

/// The scan must be able to find its subject, or it reports nothing wrong with
/// anything.
#[test]
fn the_seam_holds_the_only_bare_gh_spawn() {
    let spawns = gh_spawns();
    assert_eq!(
        spawns,
        vec![SEAM.to_string()],
        "the only bare `gh` spawn must be the seam's own. Either the seam moved \
         -- in which case this test must follow it -- or a spawn site was \
         written outside it."
    );
}

#[test]
fn no_gh_command_is_built_outside_the_seam() {
    let offenders: Vec<String> = gh_spawns().into_iter().filter(|r| r != SEAM).collect();
    assert!(
        offenders.is_empty(),
        "`gh` is spawned outside `exec::gh`: {offenders:?}\n\
         A bare `Command::new` inherits the daemon's whole environment, so the \
         spawn carries the webhook secret and every provider key into a process \
         that needs neither. Build it with `crate::exec::gh()`."
    );
}

/// The list is an allowlist, so what is NOT on it is the assertion.
#[test]
fn the_seam_hands_over_no_secret_it_has_no_use_for() {
    for forbidden in anvil::exec::gh::NEVER_HANDED_OVER {
        assert!(
            !anvil::exec::gh::GH_INHERITED.contains(forbidden),
            "{forbidden} reaches the forge client, which has no use for it."
        );
    }
}

/// The forge credential is allowed here and nowhere else.
///
/// Asserted in both directions: dropping it from this seam silently converts a
/// `GH_TOKEN`-authenticated deployment to keyring-or-nothing, and adding it to
/// the build seam hands a contributor's test suite the daemon's forge
/// authority.
#[test]
fn the_forge_credential_reaches_this_seam_and_only_this_seam() {
    for name in ["GH_TOKEN", "GITHUB_TOKEN"] {
        assert!(
            anvil::exec::gh::GH_INHERITED.contains(&name),
            "{name} is how a deployment without a keyring authenticates `gh`; \
             dropping it here unauthenticates the daemon silently."
        );
        assert!(
            !anvil::exec::build_env::BUILD_INHERITED.contains(&name),
            "{name} must not reach a build, which runs a contributor's code."
        );
    }
}

/// A real child, not the list.
///
/// `/usr/bin/env` rather than `gh`: the subject is the environment the seam
/// hands over, and a runner without `gh` installed must still be able to
/// measure it.
#[tokio::test]
async fn a_real_child_receives_no_webhook_secret_and_keeps_its_path() {
    let sentinel = "anvil-gh-seam-sentinel";
    unsafe {
        std::env::set_var("GITHUB_WEBHOOK_SECRET", sentinel);
        std::env::set_var("ANTHROPIC_API_KEY", sentinel);
    }

    let mut cmd = tokio::process::Command::new("/usr/bin/env");
    anvil::exec::gh::apply(&mut cmd);
    let out = cmd.output().await.expect("env runs");
    let seen = String::from_utf8_lossy(&out.stdout).to_string();

    unsafe {
        std::env::remove_var("GITHUB_WEBHOOK_SECRET");
        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    let leaked: Vec<&str> = anvil::exec::gh::NEVER_HANDED_OVER
        .iter()
        .copied()
        .filter(|n| seen.lines().any(|l| l.starts_with(&format!("{n}="))))
        .collect();
    assert!(
        leaked.is_empty(),
        "{} secret name(s) reached the forge client",
        leaked.len()
    );
    assert!(
        seen.lines().any(|l| l.starts_with("PATH=")),
        "PATH did not survive, so `gh` itself would not be found and every \
         forge call would fail as a spawn error"
    );
}
