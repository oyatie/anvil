//! A milestone run may write only what it was authorised to write, and that
//! holds for whichever agent is driving.
//!
//! The rule lives in `pre-commit` rather than in a harness setting because the
//! commit is the only thing every agent has in common. `.claude/settings.json`
//! is read by one harness; this repository is also touched by codex, cursor,
//! grok and agy, and by a human. A policy one harness enforces is a policy the
//! others never see.
//!
//! It is also why the rule is not an enumeration of writing verbs. `cp`, `dd`,
//! `mv`, `install`, `truncate`, a shell redirect, `python3 -c` -- a closed list
//! goes stale in silence, which is the same reason `occupancy` keys hubs on a
//! directory rather than a path list. Gating the artifact closes the class.
//!
//! Absent scope file means no constraint, deliberately: ordinary work is not a
//! milestone run, and a check that fires when nobody declared a scope would
//! teach the operator to disable it.

use std::fs;
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git must run")
}

/// A throwaway repository with this repository's tracked hook installed.
fn lab(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("anvil-scope-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::create_dir_all(dir.join(".anvil")).unwrap();
    git(&dir, &["init", "-q"]);
    git(&dir, &["config", "user.email", "t@t"]);
    git(&dir, &["config", "user.name", "t"]);

    let template = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/git_manager/hooks/pre-commit");
    let hook = dir.join(".git/hooks/pre-commit");
    fs::copy(&template, &hook).expect("the tracked hook template must exist");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::write(dir.join("src/inside.txt"), "a\n").unwrap();
    fs::write(dir.join("outside.txt"), "a\n").unwrap();
    dir
}

fn commit(dir: &Path, paths: &[&str], msg: &str) -> std::process::Output {
    let mut args = vec!["add"];
    args.extend_from_slice(paths);
    git(dir, &args);
    git(dir, &["commit", "-m", msg])
}

/// Ordinary work declares no scope, and is not constrained by one.
#[test]
fn with_no_scope_declared_the_check_is_absent_not_permissive() {
    let d = lab("none");
    let out = commit(&d, &["src/inside.txt", "outside.txt"], "unscoped");
    assert!(
        out.status.success(),
        "a commit with no run scope declared must not be refused: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&d);
}

/// The path the run was authorised to write is allowed.
#[test]
fn a_staged_path_inside_the_declared_scope_is_allowed() {
    let d = lab("inside");
    fs::write(d.join(".anvil/run-scope"), "src/\n").unwrap();
    let out = commit(&d, &["src/inside.txt"], "in scope");
    assert!(
        out.status.success(),
        "an in-scope path must commit: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&d);
}

/// The defect this exists to catch: a run writing outside what it was given.
#[test]
fn a_staged_path_outside_the_declared_scope_is_refused() {
    let d = lab("outside");
    fs::write(d.join(".anvil/run-scope"), "src/\n").unwrap();
    let out = commit(&d, &["outside.txt"], "out of scope");
    assert!(
        !out.status.success(),
        "an out-of-scope path must be refused, and was not"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("outside the declared run scope"),
        "the refusal must say why: {err}"
    );
    assert!(
        err.contains("outside.txt"),
        "the refusal must name the offending path: {err}"
    );
    let _ = fs::remove_dir_all(&d);
}

/// A mixed commit is refused, and names only the path that broke the rule --
/// a refusal that lists the innocent files with the guilty one gets skimmed.
#[test]
fn a_mixed_commit_is_refused_and_names_only_the_offender() {
    let d = lab("mixed");
    fs::write(d.join(".anvil/run-scope"), "src/\n").unwrap();
    let out = commit(&d, &["src/inside.txt", "outside.txt"], "mixed");
    assert!(!out.status.success(), "a mixed commit must be refused");
    let err = String::from_utf8_lossy(&out.stderr);
    let offenders: Vec<&str> = err
        .lines()
        .skip_while(|l| !l.contains("outside the declared run scope"))
        .skip(1)
        .take_while(|l| l.starts_with("  ") && !l.contains("in scope for this run"))
        .collect();
    assert!(
        offenders.iter().any(|l| l.contains("outside.txt")),
        "must name the offender: {err}"
    );
    assert!(
        !offenders.iter().any(|l| l.contains("inside.txt")),
        "must not name the in-scope path as an offender: {err}"
    );
    let _ = fs::remove_dir_all(&d);
}

/// The scope file is never committable: it is run state, not source.
#[test]
fn the_scope_file_is_gitignored() {
    let ignore = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(".gitignore"))
        .expect(".gitignore must exist");
    assert!(
        ignore.lines().any(|l| l.trim() == "/.anvil/run-scope"),
        ".anvil/run-scope must be gitignored, or a run could commit its own authorisation"
    );
}
