//! Hook templates live in the crate. Install copies them into
//! `$(git rev-parse --git-common-dir)/hooks`. `core.hooksPath` stays unset.

use std::fs;
use std::path::Path;
use std::process::Command;

const REQUIRED_HOOKS: &[&str] = &["pre-commit", "commit-msg", "pre-push"];

fn template(name: &str) -> std::path::PathBuf {
    Path::new("src/git_manager/hooks").join(name)
}

#[test]
fn every_required_hook_template_is_tracked() {
    let out = Command::new("git")
        .args(["ls-files", "src/git_manager/hooks"])
        .output()
        .expect("git ls-files");
    let tracked = String::from_utf8_lossy(&out.stdout);
    for hook in REQUIRED_HOOKS {
        assert!(
            tracked.contains(hook),
            "src/git_manager/hooks/{hook} is not tracked"
        );
        assert!(
            template(hook).is_file(),
            "missing src/git_manager/hooks/{hook}"
        );
    }
}

#[test]
fn installer_writes_common_dir_not_core_hooks_path() {
    let src = anvil::source_scan::paths::module_source(
        "src/git_manager",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    let code: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        code.contains("git-common-dir") || code.contains("--git-common-dir"),
        "install_repo_hooks must resolve $(git rev-parse --git-common-dir)/hooks"
    );
    assert!(
        !code.contains(r#"["config", "core.hooksPath", ".githooks"]"#),
        "core.hooksPath .githooks is retired"
    );
    assert!(
        !Path::new(".githooks").is_dir(),
        "tracked .githooks/ must not return"
    );
}

#[test]
fn the_agent_directory_guard_names_every_dot_dir_we_have_seen() {
    let src = fs::read_to_string(template("pre-commit")).expect("pre-commit");
    for dir in [
        "claude", "codex", "cursor", "grok", "agents", "beads", "omc", "omx",
    ] {
        assert!(
            src.contains(dir),
            "the pre-commit agent-directory guard does not mention '{dir}'"
        );
    }
}

#[test]
fn hooks_are_escapable_deliberately_but_not_by_accident() {
    for hook in REQUIRED_HOOKS {
        let src = fs::read_to_string(template(hook)).expect("hook");
        assert!(
            src.contains("ANVIL_SKIP_HOOKS"),
            "src/git_manager/hooks/{hook} has no deliberate bypass"
        );
    }
}

#[test]
fn pre_commit_and_pre_push_fmt_the_file_list_not_the_workspace() {
    let pre_commit = fs::read_to_string(template("pre-commit")).expect("pre-commit");
    let pre_push = fs::read_to_string(template("pre-push")).expect("pre-push");
    for (name, src) in [
        ("pre-commit", pre_commit.as_str()),
        ("pre-push", pre_push.as_str()),
    ] {
        assert!(
            src.contains("rustfmt --check"),
            "{name} must rustfmt --check the changed *.rs list"
        );
        assert!(
            !src.contains("cargo fmt"),
            "{name} must not cargo fmt the workspace"
        );
    }
}
