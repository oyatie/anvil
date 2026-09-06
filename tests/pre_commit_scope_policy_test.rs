//! Executes only the installed template's functions-only policy block, with
//! inert argv/stdin records. Never sources the hook's Git/metadata/format tail.
//! Acquisition assertions below are source contracts, not filesystem or Git
//! execution evidence. The runtime seam requires a POSIX shell.
#![cfg(unix)]

use std::io::Write;
use std::process::{Command, Output, Stdio};

const HOOK: &str = include_str!("../src/git_manager/hooks/pre-commit");
const BEGIN: &str = "# BEGIN ANVIL SCOPE POLICY FUNCTIONS\n";
const END: &str = "# END ANVIL SCOPE POLICY FUNCTIONS\n";

fn policy() -> &'static str {
    assert_eq!(HOOK.matches(BEGIN).count(), 1, "one policy block start");
    assert_eq!(HOOK.matches(END).count(), 1, "one policy block end");
    HOOK.split_once(BEGIN)
        .expect("policy starts")
        .1
        .split_once(END)
        .expect("policy ends after it starts")
        .0
}

enum Entry {
    State,
    Ignore,
    Check,
}

fn decide(entry: Entry, argument: &str, records: &str) -> Output {
    let invocation = match entry {
        Entry::State => "anvil_scope_state \"$1\"",
        Entry::Ignore => "anvil_ignore_status \"$1\"",
        Entry::Check => "anvil_scope_check \"$1\"",
    };
    let script = format!("set -eu\n{}\n{invocation}\n", policy());
    let mut child = Command::new("sh")
        .args(["-c", &script, "scope-policy", argument])
        .env_remove("ENV")
        .env_remove("BASH_ENV")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("POSIX shell starts");
    if let Err(error) = child
        .stdin
        .take()
        .expect("record input")
        .write_all(records.as_bytes())
    {
        // A declaration can refuse before consuming records. Its exit status
        // remains the assertion; only that ordinary closed pipe is tolerated.
        assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
    }
    child.wait_with_output().expect("policy terminates")
}

fn status(out: Output, expected: i32) {
    assert_eq!(out.status.code(), Some(expected), "{out:?}");
    assert!(out.stdout.is_empty(), "policy emits no stdout: {out:?}");
}

#[test]
fn scope_policy_block_is_the_hook_call_site_owner() {
    let body = policy();
    let tail = HOOK.split_once(END).unwrap().1;
    for name in [
        "anvil_scope_state",
        "anvil_ignore_status",
        "anvil_scope_literal",
        "anvil_scope_check",
    ] {
        assert_eq!(HOOK.matches(&format!("{name}() ")).count(), 1);
        assert!(body.contains(&format!("{name}() ")));
        assert!(tail.contains(&format!("{name} \"")), "uncalled {name}");
    }
    assert!(HOOK.find(END).unwrap() < HOOK.find("ANVIL_SKIP_HOOKS").unwrap());
    // A bounded ownership check, not a general shell parser or tamper proof.
    for acquisition in ["git ", "mktemp", "scope_file=", "rustfmt --"] {
        assert!(!body.contains(acquisition));
    }
}

#[test]
fn scope_declaration_state_decisions_are_explicit() {
    for (state, expected) in [
        ("absent", 10),
        ("readable_regular", 0),
        ("symlink", 2),
        ("non_regular", 2),
        ("unreadable", 2),
        ("parent_symlink", 2),
        ("parent_non_directory", 2),
        ("parent_unsearchable", 2),
        ("unknown", 2),
    ] {
        status(decide(Entry::State, state, ""), expected);
    }
}

#[test]
fn scope_ignore_query_status_is_three_way() {
    for (query_status, expected, reason) in [
        ("0", 1, "gitignored path is staged"),
        ("1", 0, ""),
        ("2", 2, "ignore query failed"),
        ("128", 2, "ignore query failed"),
        ("unknown", 2, "ignore query failed"),
    ] {
        let out = decide(Entry::Ignore, query_status, "");
        assert!(String::from_utf8_lossy(&out.stderr).contains(reason));
        status(out, expected);
    }
}

#[test]
fn scope_literal_membership_preserves_supported_bytes() {
    for prefix in ["docs/road map", "src/café", "notes/[draft]*?", " spaced "] {
        status(decide(Entry::Check, prefix, &format!("{prefix}\n")), 0);
        status(
            decide(Entry::Check, prefix, &format!("{prefix}/page.txt\n")),
            0,
        );
        status(
            decide(Entry::Check, prefix, &format!("{prefix}-other/page.txt\n")),
            1,
        );
    }
    status(
        decide(Entry::Check, "notes/[draft]*?", "notes/draft/page.txt\n"),
        1,
    );
}

#[test]
fn scope_declaration_is_fully_validated_before_membership() {
    for records in ["docs/page.txt\n", ""] {
        status(decide(Entry::Check, "docs\nnotes//drafts", records), 2);
    }
}

#[test]
fn scope_record_framing_and_non_grants_are_explicit() {
    for declaration in ["docs", "docs\n", "docs/", "\n# ordinary comment\ndocs"] {
        status(decide(Entry::Check, declaration, "docs/page.txt\n"), 0);
    }
    for declaration in ["", "\n# ordinary comment\n"] {
        status(decide(Entry::Check, declaration, "docs/page.txt\n"), 1);
        status(decide(Entry::Check, declaration, ""), 0);
    }
    status(decide(Entry::Check, "docs", "docs/page.txt"), 0);
}

#[test]
fn scope_unsupported_representations_are_refused() {
    // Representation data only: none of these names is used for filesystem IO.
    for declaration in [
        "/docs",
        "C:/docs",
        ".",
        "..",
        "docs/./page",
        "docs/../page",
        "docs//page",
        "docs//",
        "docs\\page",
        "docs/\"page",
        "docs/\tpage",
        "docs/\rpage",
        "docs/\u{7f}page",
    ] {
        status(decide(Entry::Check, declaration, ""), 2);
    }
    for record in [
        "\"docs/page\"\n",
        "docs//page\n",
        "docs/\n",
        "docs/\tpage\n",
    ] {
        status(decide(Entry::Check, "docs", record), 2);
    }
}

#[test]
fn scope_acquisition_commands_and_order_are_bound() {
    let tail = HOOK
        .split_once(END)
        .expect("policy and acquisition are separate")
        .1;
    let parent_link = tail.find("if [ -L \"$scope_parent\" ]; then").unwrap();
    let parent_kind = tail
        .find("elif [ -e \"$scope_parent\" ] && [ ! -d \"$scope_parent\" ]; then")
        .unwrap();
    let parent_search = tail
        .find("elif [ -d \"$scope_parent\" ] && [ ! -x \"$scope_parent\" ]; then")
        .unwrap();
    let link = tail.find("[ -L \"$scope_file\" ]; then").unwrap();
    let absent = tail.find("elif [ ! -e \"$scope_file\" ]; then").unwrap();
    let regular = tail.find("elif [ ! -f \"$scope_file\" ]; then").unwrap();
    let readable = tail.find("elif [ ! -r \"$scope_file\" ]; then").unwrap();
    assert!(link < absent && absent < regular && regular < readable);
    assert!(parent_link < parent_kind && parent_kind < parent_search && parent_search < link);
    for command in [
        "if cat -- \"$scope_file\" > \"$anvil_scope_tmp/declaration\"; then",
        "if LC_ALL=C tr -d '\\000-\\011\\013-\\037\\177'",
        "if cmp -s \"$anvil_scope_tmp/declaration\" \"$anvil_scope_tmp/filtered\"; then",
        "if anvil_scope_text=$(cat \"$anvil_scope_tmp/declaration\"); then",
        "if git -c core.quotepath=false diff --cached --name-only --no-renames >",
        "if git -c core.quotepath=false diff --cached --name-only --diff-filter=ACM >",
        "if git check-ignore --no-index -q -- \"$f\" 2>/dev/null; then",
        "anvil_ignore_rc=$?",
        "if anvil_ignore_status \"$anvil_ignore_rc\"; then",
        "if anvil_scope_check \"$anvil_scope_text\" < \"$anvil_scope_tmp/staged\"; then",
    ] {
        assert!(
            tail.contains(command),
            "missing owning acquisition: {command}"
        );
    }
    assert!(tail.find("if cmp -s").unwrap() < tail.find("if anvil_scope_text=").unwrap());
    assert!(!tail.contains("--diff-filter=ACM | while"));
}
