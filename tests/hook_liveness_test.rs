//! The hooks this checkout will actually run.
//!
//! Seeded from the real state of this repository on 2026-08-26, where every
//! local rung had been dead for days while the suite stayed green:
//!
//!     core.hooksPath = <repo>/.githooks      <- directory does not exist
//!     .git/hooks/    = pre-commit.stale-untracked.bak, .bak2, and no pre-commit
//!
//! `tests/git_hooks_installed_test.rs` asserts the templates are tracked and
//! that the installer's source does not set the retired path. Neither question
//! is "does this checkout run hooks", so neither could catch it.

use std::fs;
use std::path::Path;
use std::process::Command;

use anvil::git_manager::hook_liveness::{HookDefect, defects, effective_hooks_dir};

const HOOKS: &[&str] = &["pre-commit", "commit-msg", "pre-push"];

/// A shell line with its comment and its quoted spans removed.
///
/// The shell equivalent of `source_scan::code_only`: what a command actually
/// invokes, with what it merely says about itself taken out.
fn shell_code_only(line: &str) -> String {
    let mut out = String::new();
    let mut quote: Option<char> = None;
    for c in line.chars() {
        match (quote, c) {
            (None, '#') => break,
            (None, '\'') | (None, '"') => quote = Some(c),
            (Some(q), c2) if q == c2 => quote = None,
            (None, _) => out.push(c),
            (Some(_), _) => {}
        }
    }
    out
}

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git")
        .status
        .success();
    assert!(ok, "git {args:?} failed in {}", dir.display());
}

/// A repo with the tracked templates installed correctly.
fn healthy_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let td = tempfile::tempdir().expect("tempdir");
    let root = td.path().to_path_buf();
    git(&root, &["init", "-q", "."]);
    let tmpl = root.join("templates");
    fs::create_dir_all(&tmpl).unwrap();
    let hooks = root.join(".git/hooks");
    fs::create_dir_all(&hooks).unwrap();
    for h in HOOKS {
        fs::write(tmpl.join(h), format!("#!/bin/sh\n# {h}\nexit 0\n")).unwrap();
        fs::write(hooks.join(h), format!("#!/bin/sh\n# {h}\nexit 0\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(hooks.join(h), fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    (td, root)
}

#[test]
fn a_correctly_installed_checkout_is_spared() {
    let (_td, root) = healthy_repo();
    let found = defects(&root, &root.join("templates"), HOOKS);
    assert!(found.is_empty(), "accused a healthy checkout: {found:?}");
}

/// The defect this module exists for. Git runs nothing and says nothing.
#[test]
fn a_dangling_core_hooks_path_is_a_defect() {
    let (_td, root) = healthy_repo();
    git(&root, &["config", "core.hooksPath", ".githooks"]); // never created
    let found = defects(&root, &root.join("templates"), HOOKS);
    assert!(
        matches!(found.as_slice(), [HookDefect::DanglingHooksPath { .. }]),
        "a hooksPath pointing at a missing directory was not reported: {found:?}"
    );
    assert!(found[0].is_always_a_defect(), "never a legitimate state");
    let (dir, dangling) = effective_hooks_dir(&root);
    assert!(dir.is_none() && dangling.is_some(), "git looks nowhere");
}

/// The other half of the real state: hooks renamed aside.
#[test]
fn hooks_renamed_aside_are_reported_missing() {
    let (_td, root) = healthy_repo();
    let hooks = root.join(".git/hooks");
    for h in HOOKS {
        fs::rename(
            hooks.join(h),
            hooks.join(format!("{h}.stale-untracked.bak")),
        )
        .unwrap();
    }
    let found = defects(&root, &root.join("templates"), HOOKS);
    assert_eq!(found.len(), HOOKS.len(), "one per absent hook: {found:?}");
    assert!(
        found
            .iter()
            .all(|d| matches!(d, HookDefect::Missing { .. }))
    );
    // A fresh clone has no hooks either, so this alone must not be treated as
    // the checkout lying about being governed.
    assert!(!found[0].is_always_a_defect());
}

#[test]
fn a_hook_edited_in_place_is_drift() {
    let (_td, root) = healthy_repo();
    fs::write(root.join(".git/hooks/pre-push"), "#!/bin/sh\nexit 0\n").unwrap();
    let found = defects(&root, &root.join("templates"), HOOKS);
    assert!(
        found.iter().any(
            |d| matches!(d, HookDefect::DriftedFromTemplate { hook, .. } if hook == "pre-push")
        ),
        "a hook edited in place is a hook nobody reviewed: {found:?}"
    );
}

#[test]
fn a_hook_that_is_not_executable_is_reported() {
    let (_td, root) = healthy_repo();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            root.join(".git/hooks/commit-msg"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        let found = defects(&root, &root.join("templates"), HOOKS);
        assert!(
            found.iter().any(|d| matches!(
                d, HookDefect::NotExecutable { hook, .. } if hook == "commit-msg"
            )),
            "git skips a non-executable hook without complaint: {found:?}"
        );
    }
}

/// A missing template is never silently a pass: nothing could be installed or
/// compared, so the answer is unknown, not clean.
#[test]
fn an_absent_template_is_reported_rather_than_ignored() {
    let (_td, root) = healthy_repo();
    fs::remove_file(root.join("templates/pre-commit")).unwrap();
    let found = defects(&root, &root.join("templates"), HOOKS);
    assert!(
        found
            .iter()
            .any(|d| matches!(d, HookDefect::TemplateAbsent { hook, .. } if hook == "pre-commit")),
        "a missing template must not read as a clean hook: {found:?}"
    );
}

/// This checkout, right now. A dangling `core.hooksPath` is always wrong, so
/// this asserts on the live repository rather than a fixture.
#[test]
fn this_checkout_has_no_dangling_hooks_path() {
    let root = Path::new(".");
    let (_dir, dangling) = effective_hooks_dir(root);
    assert!(
        dangling.is_none(),
        "core.hooksPath is set to `{}`, which does not exist. Git runs no \
         hooks and reports nothing, so every local rung is dead. Repair with \
         `git config --unset core.hooksPath` and reinstall.",
        dangling.unwrap_or_default()
    );
}

/// A hook that cannot parse the code it guards is inert in the way that
/// matters: it reports errors instead of formatting, on every modern file.
///
/// `rustfmt` is a standalone binary. Given a file list it never reads
/// `Cargo.toml`, so it parses as Rust 2015 unless told otherwise. This repo is
/// edition 2024, so every file holding an `async fn` or a let-chain failed to
/// parse — and had done since the edition moved.
#[test]
fn the_format_hooks_tell_rustfmt_which_edition_this_is() {
    let manifest = fs::read_to_string("Cargo.toml").expect("Cargo.toml");
    assert!(
        manifest.contains(r#"edition = "2024""#),
        "fixture sanity: this test is about a repo past edition 2015"
    );
    for hook in ["pre-commit", "pre-push"] {
        let body = fs::read_to_string(Path::new("src/git_manager/hooks").join(hook))
            .unwrap_or_else(|e| panic!("read {hook}: {e}"));
        for line in body.lines() {
            // Comments explain `rustfmt --check` and an `echo` announces it.
            // Two drafts of this test accused a hook that was already correct,
            // once on a comment and once on a quoted banner -- prose and a
            // string literal read as code, which is the same defect the facade
            // seal had, three times in one day. Strip both before matching.
            let code = shell_code_only(line);
            if !code.contains("rustfmt") || !code.contains("--check") {
                continue;
            }
            let line = code.as_str();
            assert!(
                line.contains("--edition"),
                "{hook} invokes `rustfmt --check` without an edition, so it \
                 parses edition-2024 source as Rust 2015 and reports parse \
                 errors instead of formatting:\n  {}",
                line.trim()
            );
        }
        assert!(
            body.contains("Cargo.toml"),
            "{hook} must derive the edition from the manifest rather than \
             hardcode it, or the two drift apart again"
        );
    }
}

/// ...and the flag is not decorative: prove the difference on real source.
#[test]
fn a_bare_rustfmt_check_really_does_fail_on_this_tree() {
    let subject = "src/git_manager/hook_liveness.rs";
    let bare = Command::new("rustfmt")
        .args(["--check", "--", subject])
        .output()
        .expect("rustfmt");
    let with_edition = Command::new("rustfmt")
        .args(["--edition", "2024", "--check", "--", subject])
        .output()
        .expect("rustfmt");
    assert!(
        !bare.status.success(),
        "fixture sanity: a bare rustfmt was expected to fail on 2024 source; \
         if this now passes, the edition flag may no longer be load-bearing"
    );
    assert!(
        with_edition.status.success(),
        "the same file passes once rustfmt is told the edition, so the hook's \
         failures were parse errors and not formatting: {}",
        String::from_utf8_lossy(&with_edition.stderr)
    );
}
