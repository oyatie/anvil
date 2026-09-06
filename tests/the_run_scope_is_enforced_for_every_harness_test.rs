//! Commit-time run-scope guardrail controls for the installed pre-commit
//! template, not every harness or write-time containment. No production scope
//! producer is supplied. Invocation depends on the installed hook set and Git
//! operation; this does not prove other commit routes or tamper resistance.
//! Genuine absence deliberately preserves ordinary work. Historical negative
//! fixtures remain below; their earlier measurement narratives are not renewed
//! evidence. The bounded correction selects only the documented positive and
//! source-only controls, plus the separate inert policy owner.

use std::fs;
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env_remove("ANVIL_SKIP_HOOKS")
        .output()
        .expect("git must run")
}

fn git_ok(dir: &Path, args: &[&str]) {
    let out = git(dir, args);
    assert!(out.status.success(), "fixture git {args:?} failed: {out:?}");
}

/// A throwaway repository with this repository's tracked hook installed.
fn lab(name: &str) -> std::path::PathBuf {
    let dir = tempfile::Builder::new()
        .prefix(&format!("anvil-scope-{name}-"))
        .tempdir()
        .expect("unique fixture directory")
        .keep();
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::create_dir_all(dir.join(".anvil")).unwrap();
    git_ok(&dir, &["init", "-q"]);
    git_ok(&dir, &["config", "user.email", "t@t"]);
    git_ok(&dir, &["config", "user.name", "t"]);
    git_ok(&dir, &["config", "commit.gpgsign", "false"]);
    git_ok(&dir, &["config", "core.hooksPath", ".git/hooks"]);

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

/// Refused for the scope reason, rather than an unrelated fixture failure.
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
    let mut args = vec!["add", "--"];
    args.extend_from_slice(paths);
    git_ok(dir, &args);
    git(dir, &["commit", "-m", msg])
}

fn seed(dir: &Path, paths: &[&str]) {
    let out = commit(dir, paths, "seed");
    assert!(out.status.success(), "fixture seed failed: {out:?}");
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

/// The tracked ignore policy names run state; this alone proves no hook refusal.
#[test]
fn the_scope_file_is_gitignored() {
    let ignore = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(".gitignore"))
        .expect(".gitignore must exist");
    assert!(
        ignore.lines().any(|l| l.trim() == "/.anvil/run-scope"),
        "the tracked ignore rule must name .anvil/run-scope"
    );
}

// Historical negative/transition fixtures, not selected by the bounded fix.

/// Deleting a file is a write. `--diff-filter=ACMR` omitted `D`, so removing an
/// out-of-scope file was authorised by a scope that did not mention it.
#[test]
fn deleting_an_out_of_scope_file_is_refused() {
    let d = lab("delete");
    seed(&d, &["src/inside.txt", "outside.txt"]);
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
    seed(&d, &["src/inside.txt", "outside.txt"]);
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
    seed(&d, &["src/inside.txt", "outside.txt"]);
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
    seed(&d, &["src/inside.txt", "s*"]);
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

/// The producer half of the guardrail, end to end.
///
/// Every test above writes `.anvil/run-scope` itself. That is what let #206
/// merge with the check inert: the hook read a file nothing in production
/// wrote, so the branch below `if [ -f "$scope_file" ]` never executed in a
/// real checkout, and the tests passed because they supplied the input
/// production never would.
///
/// This one declares the scope the way a run does -- through
/// `ai_driver::chain`, from `config/model-routing.toml` -- and then attempts
/// the commit the guardrail exists to refuse.
#[test]
fn a_scope_declared_the_way_a_run_declares_it_refuses_an_out_of_scope_commit() {
    let d = lab("producer");
    // No hand-written scope file. The declaration comes from the same call the
    // dispatcher makes, against the tracked routing table.
    let scope = anvil::ai_driver::chain::declare_run_scope_for_test(
        &d,
        anvil::ai_driver::Stage::Implementation,
    )
    .expect("a run must be able to declare its scope");

    let declared = fs::read_to_string(d.join(".anvil/run-scope")).expect("declaration on disk");
    assert!(
        declared.lines().any(|l| l.trim() == "src/"),
        "implementation declares src/, and the file must say so: {declared:?}"
    );

    let out = commit(
        &d,
        &["outside.txt"],
        "out of scope under a real declaration",
    );
    refused_for_scope(&out, "outside.txt");

    // A refused commit leaves its paths staged, and the next `git add` would
    // carry them into the following commit. Unstage so the in-scope case is
    // testing the in-scope path and not the leftovers of the previous one.
    git(&d, &["reset", "-q"]);

    let ok = commit(&d, &["src/inside.txt"], "in scope under a real declaration");
    assert!(
        ok.status.success(),
        "an in-scope path must still commit: {}",
        String::from_utf8_lossy(&ok.stderr)
    );

    // And the declaration is run state, not repository state: it goes when the
    // run does, so ordinary work sees no constraint.
    drop(scope);
    assert!(
        !d.join(".anvil/run-scope").exists(),
        "the declaration must not outlive the run that made it"
    );
    git(&d, &["reset", "-q"]);
    let after = commit(&d, &["outside.txt"], "unconstrained once the run ends");
    assert!(
        after.status.success(),
        "with no run in flight the check is absent, not permissive: {}",
        String::from_utf8_lossy(&after.stderr)
    );
    let _ = fs::remove_dir_all(&d);
}
