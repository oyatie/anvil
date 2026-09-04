//! A number published beside a command must be the number that command produces.
//!
//! `prose_counts_test` already states the rule for two corpora -- Rust doc
//! comments and the `corpus_sync`-owned markdown -- and derives counts from
//! symbols. This is the third corpus and the stronger form: `docs/plan/` is
//! outside `corpus_sync::OWNED`, nothing reads it, and its claims are not
//! counts a symbol can derive but *measurements a command produced*.
//!
//! The class it exists to catch, from the review of #207 and #209, where six
//! instances shipped across seven rounds:
//!
//!   * `26/10/3` published beside a pattern that returns `34/11/5` -- the
//!     numbers came from an unstated word-boundary form.
//!   * `git grep -nE 'unsafe (fn\|impl\|\{\|trait)' -- src` printed inside a
//!     markdown table cell, where the escaped pipes make it return 0 -- and 0
//!     agreed with the conclusion, so the broken form could not have failed.
//!   * "unscoped returns 31, because it matches this sentence" surviving after
//!     the matching sentence was deleted; then re-created by its own fix.
//!
//! Every one is mechanically detectable and none needed a reviewer: run the
//! command as printed, compare to the number printed beside it. The reviews
//! were doing slowly, and six times, what this does in seconds.
//!
//! # The contract
//!
//! Inside a fenced block, `#=` marks an assertion. Everything left of it is the
//! command; everything right is the expected stdout, trimmed:
//!
//! ```text
//! git grep -c 'needle' -- src     #= 42
//! ```
//!
//! A plain `#` comment stays prose and is not run, so the existing blocks in
//! `h1-execution-prompt.md` are untouched.
//!
//! # Why an allowlist rather than a shell
//!
//! This runs text lifted out of a document, which is untrusted input reaching
//! `exec`. A document that says `rm -rf ~` must be *refused*, not run. Only the
//! read-only verbs below may open a pipeline segment, and a command naming
//! anything else fails the scan loudly rather than being skipped -- a skipped
//! assertion is absent evidence, and absent evidence is never a pass (I1).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Verbs that may open a pipeline segment. Read-only, no network, no writes.
const ALLOWED: &[&str] = &[
    "git", "grep", "cat", "sed", "awk", "sort", "uniq", "wc", "head", "tail", "cut", "tr", "paste",
    "bc", "echo", "ls", "find", "python3", "true",
];

/// `git` subcommands that only read. `git push`, `git commit`, `git checkout`
/// and friends are refused even though `git` is allowed.
const ALLOWED_GIT: &[&str] = &[
    "grep",
    "log",
    "show",
    "ls-files",
    "ls-tree",
    "cat-file",
    "rev-parse",
    "rev-list",
    "diff",
    "branch",
    "status",
    "config",
    "worktree",
    "shortlog",
];

struct Claim {
    file: PathBuf,
    line: usize,
    command: String,
    expected: String,
}

fn plan_docs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::from("docs/plan")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "md") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Assertions inside fenced blocks. Prose `#` comments are left alone.
fn claims_in(path: &Path) -> Vec<Claim> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut out = Vec::new();
    let mut fenced = false;
    for (i, raw) in text.lines().enumerate() {
        if raw.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if !fenced {
            continue;
        }
        let Some((cmd, expected)) = raw.split_once("#=") else {
            continue;
        };
        if cmd.trim().is_empty() {
            continue;
        }
        out.push(Claim {
            file: path.to_path_buf(),
            line: i + 1,
            command: cmd.trim().to_string(),
            expected: expected.trim().to_string(),
        });
    }
    out
}

/// The first token of every pipeline segment, so each can be checked.
///
/// Quote-aware, because it must be. The first version split on every `|` and
/// so tore `grep -chE '^\| *H1-[0-9]+ *\|'` into pieces at the pipes *inside*
/// the regex, then refused `*H1-[0-9]+` as an unpermitted verb. A splitter that
/// cannot read the commands this repo actually publishes would have rejected
/// every real claim and passed only trivial ones -- the instrument failing in
/// exactly the way the scan exists to catch.
fn segment_verbs(command: &str) -> Vec<String> {
    let mut segments = vec![String::new()];
    let (mut single, mut double) = (false, false);
    let mut prev = '\0';
    for c in command.chars() {
        match c {
            '\'' if !double && prev != '\\' => single = !single,
            '"' if !single && prev != '\\' => double = !double,
            '|' if !single && !double => {
                segments.push(String::new());
                prev = c;
                continue;
            }
            _ => {}
        }
        segments.last_mut().expect("seeded with one").push(c);
        prev = c;
    }
    segments
        .iter()
        .filter_map(|seg| {
            let seg = seg.trim().trim_start_matches('(').trim();
            seg.split_whitespace().next().map(str::to_string)
        })
        .collect()
}

/// `Err` names the verb that is not permitted. Refusal is a failure, never a skip.
fn refuse_unless_read_only(command: &str) -> Result<(), String> {
    for verb in segment_verbs(command) {
        if !ALLOWED.contains(&verb.as_str()) {
            return Err(format!("`{verb}` is not a permitted verb"));
        }
        if verb == "git" {
            let sub = command
                .split_whitespace()
                .skip_while(|t| *t != "git")
                .nth(1)
                .unwrap_or("");
            let sub = if sub.starts_with('-') { "" } else { sub };
            if !ALLOWED_GIT.contains(&sub) {
                return Err(format!("`git {sub}` is not a read-only subcommand"));
            }
        }
    }
    Ok(())
}

fn run(command: &str) -> String {
    let out = Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .expect("sh is available");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn every_published_command_produces_the_number_published_beside_it() {
    let claims: Vec<Claim> = plan_docs().iter().flat_map(|p| claims_in(p)).collect();

    // A scan that examined nothing must not report a pass. If every `#=` were
    // deleted this would go green while checking no claim at all, which is the
    // shape of defect it exists to catch.
    assert!(
        !claims.is_empty(),
        "no `#=` assertions found under docs/plan/. Either the marker was \
         removed or the corpus moved; a scan with an empty corpus is not a pass."
    );

    let mut failures = Vec::new();
    for claim in &claims {
        let at = format!("{}:{}", claim.file.display(), claim.line);
        if let Err(why) = refuse_unless_read_only(&claim.command) {
            failures.push(format!("{at}: REFUSED -- {why}\n    {}", claim.command));
            continue;
        }
        let actual = run(&claim.command);
        if actual != claim.expected {
            failures.push(format!(
                "{at}: published `{}` but the command produced `{}`\n    {}",
                claim.expected, actual, claim.command
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} published command(s) did not reproduce:\n\n{}\n\n\
         A number beside a command must be the number that command produces. \
         If the command is right and the number is stale, update the number; \
         if the number is right and the command cannot express it, the command \
         is the defect.",
        failures.len(),
        claims.len(),
        failures.join("\n\n")
    );
}

#[test]
fn a_command_that_writes_is_refused_rather_than_run() {
    // The scan executes text taken from a document. Refusal must be the
    // behaviour for anything outside the read-only allowlist, and it must be a
    // failure the caller sees rather than a silent skip.
    for hostile in [
        "rm -rf /tmp/anvil-should-not-exist",
        "git push origin dev",
        "curl https://example.invalid",
        "grep -c foo src/lib.rs | xargs rm",
        "git commit -m x",
    ] {
        assert!(
            refuse_unless_read_only(hostile).is_err(),
            "a write/network verb was permitted: {hostile}"
        );
    }
    for benign in [
        "git grep -c 'foo' -- src",
        "cat Cargo.toml | grep -c edition",
        "grep -rn 'needle' src/ | wc -l",
        // A pipe inside a quoted regex is not a pipeline boundary. This is the
        // shape the first splitter got wrong, so it is pinned here.
        r"grep -chE '^\| *H1-[0-9]+ *\|' docs/plan/ws-*.md | paste -sd+ - | bc",
        r#"grep -c "a|b" src/lib.rs"#,
    ] {
        assert!(
            refuse_unless_read_only(benign).is_ok(),
            "a read-only command was refused: {benign}"
        );
    }
}
