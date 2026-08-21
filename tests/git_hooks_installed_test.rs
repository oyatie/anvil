//! The hooks must exist, be executable, be tracked, and Anvil must still point
//! clones at them. A live `git config` probe of the runner is not that.
//!
//! Anvil's previous hook mechanism wrote scripts into `.git/hooks` of the
//! repositories it managed — untracked, unreviewable, silently different on
//! every machine, and never installed into Anvil itself. That is the same
//! defect as a guard which evaluates other people's repositories and never its
//! own.
//!
//! Four method failures in a single day are the argument for these existing at
//! all: `git add -A` sweeping unrelated in-flight work into a commit (twice), a
//! workflow agent pushing a branch that did not compile, and agent directories
//! reaching the remote. In every case the rule was written down first and
//! written down did not prevent it.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

const REQUIRED_HOOKS: &[&str] = &["pre-commit", "commit-msg", "pre-push"];

#[test]
fn every_required_hook_exists_and_is_executable() {
    for hook in REQUIRED_HOOKS {
        let p = Path::new(".githooks").join(hook);
        assert!(
            p.is_file(),
            "missing .githooks/{hook}: a hook that is not present enforces nothing"
        );
        let mode = fs::metadata(&p).expect("metadata").permissions().mode();
        assert!(
            mode & 0o111 != 0,
            ".githooks/{hook} is not executable ({mode:o}); git silently skips a \
             non-executable hook, so this fails open"
        );
    }
}

#[test]
fn hooks_are_tracked_so_they_can_be_reviewed() {
    let out = Command::new("git")
        .args(["ls-files", ".githooks"])
        .output()
        .expect("git ls-files");
    let tracked = String::from_utf8_lossy(&out.stdout);
    for hook in REQUIRED_HOOKS {
        assert!(
            tracked.contains(hook),
            ".githooks/{hook} is not tracked. An untracked hook cannot be reviewed, \
             cannot be shared, and drifts per machine — which is why the previous \
             mechanism wrote into .git/hooks and nobody could see what it did."
        );
    }
}

#[test]
fn this_repository_actually_uses_them() {
    // The merge result is the installer, not whatever `git config` the runner
    // happens to have. Actions checkout leaves core.hooksPath empty; asserting
    // that value greens or reds independently of the tree.
    let src = fs::read_to_string("src/git_manager/mod.rs").expect("git_manager source");
    let code: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        code.contains(r#"["config", "core.hooksPath", ".githooks"]"#),
        "the managed-clone install path no longer issues `git config core.hooksPath \
         .githooks`, so tracked hooks are not what Anvil points clones at"
    );
}

#[test]
fn the_agent_directory_guard_names_every_dot_dir_we_have_seen() {
    let src = fs::read_to_string(".githooks/pre-commit").expect("pre-commit");
    // Each of these was found tracked in a real repository during the drainage
    // sweep. A guard that lists only the ones we remembered is how .omx
    // survived the first pass.
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
    // A hook with no escape hatch gets disabled wholesale the first time it is
    // wrong. An explicit, greppable variable is better than --no-verify becoming
    // habit.
    for hook in REQUIRED_HOOKS {
        let src = fs::read_to_string(Path::new(".githooks").join(hook)).expect("hook");
        assert!(
            src.contains("ANVIL_SKIP_HOOKS"),
            ".githooks/{hook} has no deliberate bypass; the first false positive will \
             get every hook turned off instead of this one skipped"
        );
    }
}
