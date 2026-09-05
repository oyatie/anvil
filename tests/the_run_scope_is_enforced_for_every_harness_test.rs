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

/// Refused *for the scope reason*, and naming the offending path.
///
/// Every one of these tests used to assert only `!status.success()`. Against a
/// hook whose refusal message was replaced with unrelated text, all five stayed
/// green -- they could not tell a right refusal from a wrong one, which is the
/// "passes for the wrong reason" failure they exist to prevent.
fn refused_for_scope(out: &std::process::Output, path: &str) {
    let err = String::from_utf8_lossy(&out.stderr);
    let all = format!("{}{}", String::from_utf8_lossy(&out.stdout), err);
    assert!(
        !out.status.success(),
        "must be refused, and was not. Output: {all}"
    );
    assert!(
        err.contains("outside the declared run scope"),
        "refused, but not for the scope reason: {all}"
    );
    assert!(err.contains(path), "the refusal must name `{path}`: {all}");
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

// ---------------------------------------------------------------------------
// The review's attacks, as assertions. Each of these committed successfully
// against the first revision of the hook; the exit code, not the message, is
// what changed.
// ---------------------------------------------------------------------------

/// Deleting a file is a write. `--diff-filter=ACMR` omitted `D`, so removing an
/// out-of-scope file was authorised by a scope that did not mention it.
#[test]
fn deleting_an_out_of_scope_file_is_refused() {
    let d = lab("delete");
    commit(&d, &["src/inside.txt", "outside.txt"], "seed");
    fs::write(d.join(".anvil/run-scope"), "src/\n").unwrap();
    fs::remove_file(d.join("outside.txt")).unwrap();
    let out = commit(&d, &["outside.txt"], "delete out of scope");
    refused_for_scope(&out, "outside.txt");
    let _ = fs::remove_dir_all(&d);
}

/// A rename reports only its destination under `--diff-filter=ACMR`, so moving
/// an out-of-scope file INTO scope hid the deletion of the source.
#[test]
fn renaming_an_out_of_scope_file_into_scope_is_refused() {
    let d = lab("rename");
    commit(&d, &["src/inside.txt", "outside.txt"], "seed");
    fs::write(d.join(".anvil/run-scope"), "src/\n").unwrap();
    fs::rename(d.join("outside.txt"), d.join("src/moved.txt")).unwrap();
    let out = commit(&d, &["outside.txt", "src/moved.txt"], "rename into scope");
    refused_for_scope(&out, "outside.txt");
    let _ = fs::remove_dir_all(&d);
}

/// Replacing a file with a symlink is a typechange (`T`), which `ACMR` also
/// omitted -- and under it the hook saw NO staged paths, so the loop body never
/// ran and nothing was checked at all.
#[cfg(unix)]
#[test]
fn replacing_an_out_of_scope_file_with_a_symlink_is_refused() {
    let d = lab("typechange");
    commit(&d, &["src/inside.txt", "outside.txt"], "seed");
    fs::write(d.join(".anvil/run-scope"), "src/\n").unwrap();
    fs::remove_file(d.join("outside.txt")).unwrap();
    std::os::unix::fs::symlink("/etc/hosts", d.join("outside.txt")).unwrap();
    let out = commit(&d, &["outside.txt"], "retarget out of scope");
    refused_for_scope(&out, "outside.txt");
    let _ = fs::remove_dir_all(&d);
}

/// `src` must not authorise `srcfoo/`. The prefix was matched as a string, so
/// any path merely beginning with those characters was in scope.
#[test]
fn a_scope_prefix_is_a_path_boundary_not_a_string_prefix() {
    let d = lab("prefix");
    fs::create_dir_all(d.join("srcfoo")).unwrap();
    fs::write(d.join("srcfoo/evil.txt"), "a\n").unwrap();
    fs::write(d.join(".anvil/run-scope"), "src\n").unwrap();
    let out = commit(&d, &["srcfoo/evil.txt"], "prefix collision");
    refused_for_scope(&out, "srcfoo/evil.txt");

    // ...and stripping the trailing slash did not change what `src/` means.
    let d2 = lab("prefix-slash");
    fs::write(d2.join(".anvil/run-scope"), "src/\n").unwrap();
    let ok = commit(&d2, &["src/inside.txt"], "still in scope");
    assert!(
        ok.status.success(),
        "`src/` must still authorise `src/inside.txt`: {}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let _ = fs::remove_dir_all(&d);
    let _ = fs::remove_dir_all(&d2);
}

/// The scope line reached the decision as shell-glob syntax rather than as
/// data, so `sr?/` matched `src/` and a scope of `*` authorised everything.
#[test]
fn a_scope_line_is_data_not_a_glob_pattern() {
    for (scope, staged) in [("sr?/", "src/inside.txt"), ("*", "outside.txt")] {
        let d = lab(&format!("glob-{}", scope.len()));
        fs::write(d.join(".anvil/run-scope"), format!("{scope}\n")).unwrap();
        let out = commit(&d, &[staged], "glob scope");
        refused_for_scope(&out, staged);
        let _ = fs::remove_dir_all(&d);
    }
}

/// The staged list is read one path per line, not split into shell words.
///
/// `for f in $(git diff ...)` split a path on IFS, so a file named `src src`
/// became two fields that each matched the scope `src/` and committed cleanly.
#[test]
fn a_path_containing_a_space_is_one_path_not_two() {
    let d = lab("wordsplit");
    fs::write(d.join("src src"), "a\n").unwrap();
    fs::write(d.join(".anvil/run-scope"), "src/\n").unwrap();
    let out = commit(&d, &["src src"], "word split");
    refused_for_scope(&out, "src src");
    let _ = fs::remove_dir_all(&d);
}

/// ...and not glob-expanded against the working tree either.
///
/// A staged path named `s*` expanded to `src` and matched the scope. Staged as
/// a deletion, which is how the expansion had a directory left to match.
#[test]
fn a_path_containing_a_glob_character_is_not_expanded() {
    let d = lab("globchar");
    fs::write(d.join("s*"), "a\n").unwrap();
    commit(&d, &["src/inside.txt", "s*"], "seed");
    fs::write(d.join(".anvil/run-scope"), "src/\n").unwrap();
    fs::remove_file(d.join("s*")).unwrap();
    let out = commit(&d, &["s*"], "glob char");
    refused_for_scope(&out, "s*");
    let _ = fs::remove_dir_all(&d);
}

/// A non-ASCII path in scope is allowed, rather than refused because git
/// C-quoted it into `"src/caf\303\251.txt"` and the leading quote matched no
/// prefix. Over-refusal teaches the operator to disable the check.
#[test]
fn a_non_ascii_path_inside_the_scope_is_allowed() {
    let d = lab("utf8");
    fs::write(d.join("src/café.txt"), "a\n").unwrap();
    fs::write(d.join(".anvil/run-scope"), "src/\n").unwrap();
    let out = commit(&d, &["src/café.txt"], "utf8 in scope");
    assert!(
        out.status.success(),
        "an in-scope non-ASCII path was refused: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&d);
}
