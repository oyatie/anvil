//! A number published beside a command must be the number that command produces.
//!
//! `prose_counts_test` states the rule for two corpora -- Rust doc comments and
//! the `corpus_sync`-owned markdown -- and derives counts from symbols. This is
//! the third corpus and a different form: `docs/plan/` is outside
//! `corpus_sync::OWNED`, no test reads it, and its claims are not counts a
//! symbol can derive but *measurements a command produced*.
//!
//! # What it cannot catch, stated first
//!
//! It compares the printed command's output to the printed number. So:
//!
//!   * **A wrong command that agrees with its number passes.** The escaped-pipe
//!     case that motivated this file --
//!     `git grep -nE 'unsafe (fn\|impl\|\{\|trait)'` inside a table cell,
//!     returning 0 where the working form returns 4 -- would be published as
//!     `#= 0` and go green. Nothing here knows what the author meant.
//!   * **A claim in prose, or in a markdown table cell, is not seen.** Only
//!     fenced blocks are scanned. A `#=` outside a fence is *reported*, not
//!     skipped, because a skipped assertion is absent evidence (I1) -- but a
//!     number written as ordinary prose is invisible and always will be.
//!     Inferring claims from prose would itself be a proxy.
//!
//! What it does catch is the rest of the class, six instances of which shipped
//! across seven review rounds on #207 and #209: a number that its own printed
//! command does not produce.
//!
//! # The contract
//!
//! Inside a fenced block (``` or ~~~), `#=` marks an assertion. Left of it is
//! the command, right of it the expected stdout, trimmed. The expected value
//! may not be empty -- an empty expectation matches any silent command, which
//! is how five write-canaries passed an earlier revision of this file.
//!
//! ```text
//! grep -rc 'needle' src/lib.rs     #= 42
//! ```
//!
//! # Why no shell, and why `git` is not allowed
//!
//! This runs text out of a document that any contributor may edit, so it is
//! untrusted input reaching `exec`. Two earlier revisions tried to make that
//! safe by inspection and both failed:
//!
//!   1. Allowlisting the opening verb of each `|` segment. `grep foo ; rm -rf ~`
//!      presented as `grep`; seven of eight hostile forms ran.
//!   2. Adding a shell-metacharacter refusal. That closed `;`, `&&`, `$(...)`,
//!      backticks and redirection -- and missed the real class, because
//!      *arguments to permitted programs execute code with no metacharacter at
//!      all*: `git grep --open-files-in-pager=touch\ canary` ran `touch`,
//!      `git config --global` wrote the reader's `~/.gitconfig`, and
//!      `git diff --output=f` wrote a file. Each went green.
//!
//! Enumerating flags is the same losing game one layer lower, so the premise
//! changed instead. No shell is invoked: each pipeline segment is tokenised and
//! spawned directly, which makes `;`, `>` and `$(...)` ordinary argv that fail
//! on their own. And `git` is not on the allowlist -- it is a program launcher
//! (pagers, editors, external diff, aliases) and no claim in this corpus needs
//! it. The remaining verbs neither execute nor write, with `-o/--output`
//! refused because `sort` takes it.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;

/// Verbs that may appear in a claim. None of these can execute another program
/// or write a file, given the argument refusal below.
/// `sort` and `uniq` were here and are gone, each demonstrated writing:
/// `sort -uo FILE` (and `-o/path`, `--out=`, since long options accept any
/// unambiguous prefix), `sort --compress-program=/bin/sh` which *executes* the
/// piped data on GNU coreutils, and `uniq INPUT OUTPUT`, which writes with no
/// flag at all. `verbs_on_the_allowlist_cannot_write` runs the write shapes
/// against every entry below, so the next addition has to prove itself rather
/// than be vouched for.
const ALLOWED: &[&str] = &[
    "grep", "cat", "wc", "head", "tail", "cut", "tr", "paste", "bc", "echo", "ls", "true",
];

/// Kept as a second line of defence only. It is deliberately NOT the guard:
/// refusing named flags is the losing game this file already warns about --
/// `-o` misses `-uo`, `-o/path`, `--out=` and `--compress-program`, and no list
/// reaches `uniq IN OUT`, which needs no flag. The guard is that a verb on the
/// allowlist has been shown unable to write at all.
const REFUSED_ARGS: &[&str] = &["-o", "--output"];

const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

struct Claim {
    file: PathBuf,
    line: usize,
    command: String,
    expected: String,
    fenced: bool,
}

fn plan_docs() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/plan");
    let mut out = Vec::new();
    let mut stack = vec![root];
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

/// Every `#=` in the file, with whether it sat inside a fence.
///
/// Unfenced ones are collected rather than dropped: three of four planted
/// markers -- in a `~~~` fence, an indented block, and a table cell -- were
/// silently unexamined by an earlier revision while it reported green. A marker
/// the scan cannot run is a finding, not a non-event.
fn claims_in(path: &Path) -> Vec<Claim> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut out = Vec::new();
    let mut fence: Option<char> = None;
    for (i, raw) in text.lines().enumerate() {
        let t = raw.trim_start();
        let opener = ['`', '~']
            .into_iter()
            .find(|c| t.starts_with(&c.to_string().repeat(3)));
        if let Some(c) = opener {
            match fence {
                Some(open) if open == c => fence = None,
                None => fence = Some(c),
                _ => {}
            }
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
            fenced: fence.is_some(),
        });
    }
    out
}

/// Split into `|`-separated segments, each tokenised into argv. Quote-aware:
/// the counts this repo publishes are grep patterns holding `|` and `()`.
fn parse(command: &str) -> Vec<Vec<String>> {
    let mut segments = vec![Vec::<String>::new()];
    let mut token = String::new();
    let (mut single, mut double) = (false, false);
    let mut prev = '\0';
    let mut quoted_token = false;
    for c in command.chars() {
        match c {
            '\'' if !double && prev != '\\' => {
                single = !single;
                quoted_token = true;
            }
            '"' if !single && prev != '\\' => {
                double = !double;
                quoted_token = true;
            }
            '|' if !single && !double => {
                if !token.is_empty() || quoted_token {
                    segments.last_mut().expect("seeded").push(token.clone());
                }
                token.clear();
                quoted_token = false;
                segments.push(Vec::new());
            }
            c if c.is_whitespace() && !single && !double => {
                if !token.is_empty() || quoted_token {
                    segments.last_mut().expect("seeded").push(token.clone());
                }
                token.clear();
                quoted_token = false;
            }
            _ => token.push(c),
        }
        prev = c;
    }
    if !token.is_empty() || quoted_token {
        segments.last_mut().expect("seeded").push(token);
    }
    segments
}

/// `Err` says why the claim may not run. Refusal is a failure, never a skip.
fn refuse_unless_read_only(command: &str) -> Result<(), String> {
    let segments = parse(command);
    if segments.iter().any(|s| s.is_empty()) {
        return Err("empty pipeline segment".to_string());
    }
    for seg in &segments {
        let verb = &seg[0];
        if !ALLOWED.contains(&verb.as_str()) {
            return Err(format!("`{verb}` is not a permitted verb"));
        }
        for arg in &seg[1..] {
            if REFUSED_ARGS
                .iter()
                .any(|r| arg == r || arg.starts_with(&format!("{r}=")))
            {
                return Err(format!("`{verb} {arg}` writes a file"));
            }
        }
    }
    Ok(())
}

/// Expand a trailing-component `*` against the filesystem.
///
/// Dropping the shell dropped globbing with it, and both real claims in the
/// corpus glob (`docs/plan/ws-*.md`) -- so the first no-shell revision handed
/// `grep` a literal `ws-*.md`, got empty output, and failed both. Expansion is
/// done here instead: it reads a directory and matches a prefix/suffix, which
/// cannot execute anything. An unmatched pattern is left verbatim so the
/// failure surfaces as a mismatch rather than a silent empty set.
fn expand(arg: &str) -> Vec<String> {
    let Some(star) = arg.find('*') else {
        return vec![arg.to_string()];
    };
    let (dir, file) = match arg.rfind('/') {
        Some(slash) if slash < star => (&arg[..slash], &arg[slash + 1..]),
        _ => (".", arg),
    };
    let Some((prefix, suffix)) = file.split_once('*') else {
        return vec![arg.to_string()];
    };
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
    let mut hits: Vec<String> = std::fs::read_dir(&base)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            (name.starts_with(prefix) && name.ends_with(suffix) && name.len() >= prefix.len())
                .then(|| format!("{dir}/{name}"))
        })
        .collect();
    hits.sort();
    if hits.is_empty() {
        vec![arg.to_string()]
    } else {
        hits
    }
}

/// Spawn each segment directly, wiring stdout to the next stdin. No shell, so a
/// `;` or `>` that survived parsing is argv and fails on its own merits.
fn run(command: &str) -> Result<String, String> {
    let segments = parse(command);
    let root = env!("CARGO_MANIFEST_DIR");
    let mut input: Vec<u8> = Vec::new();
    for seg in &segments {
        let args: Vec<String> = seg[1..].iter().flat_map(|a| expand(a)).collect();
        let mut child = Command::new(&seg[0])
            .args(&args)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("could not spawn `{}`: {e}", seg[0]))?;
        let mut stdin = child.stdin.take().expect("piped");
        let buf = std::mem::take(&mut input);
        std::thread::spawn(move || {
            let _ = stdin.write_all(&buf);
        });
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(child.wait_with_output());
        });
        let out = rx
            .recv_timeout(TIMEOUT)
            .map_err(|_| format!("`{}` did not finish within {TIMEOUT:?}", seg[0]))?
            .map_err(|e| format!("`{}` failed: {e}", seg[0]))?;
        input = out.stdout;
    }
    Ok(String::from_utf8_lossy(&input).trim().to_string())
}

#[test]
fn every_published_command_produces_the_number_published_beside_it() {
    let claims: Vec<Claim> = plan_docs().iter().flat_map(|p| claims_in(p)).collect();

    // A scan that examined nothing must not report a pass.
    assert!(
        !claims.is_empty(),
        "no `#=` assertions found under docs/plan/. A scan with an empty corpus \
         is not a pass."
    );

    let mut failures = Vec::new();
    for claim in &claims {
        let at = format!("{}:{}", claim.file.display(), claim.line);
        if !claim.fenced {
            failures.push(format!(
                "{at}: `#=` outside a fenced block, so it cannot be run. Move it \
                 into a ``` or ~~~ fence, or make it prose without the marker.\n    {}",
                claim.command
            ));
            continue;
        }
        if claim.expected.is_empty() {
            failures.push(format!(
                "{at}: `#=` with no expected value. An empty expectation matches \
                 any silent command, which is not an assertion.\n    {}",
                claim.command
            ));
            continue;
        }
        if let Err(why) = refuse_unless_read_only(&claim.command) {
            failures.push(format!("{at}: REFUSED -- {why}\n    {}", claim.command));
            continue;
        }
        match run(&claim.command) {
            Err(why) => failures.push(format!("{at}: {why}\n    {}", claim.command)),
            Ok(actual) if actual != claim.expected => failures.push(format!(
                "{at}: published `{}` but the command produced `{}`\n    {}",
                claim.expected, actual, claim.command
            )),
            Ok(_) => {}
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} published claim(s) did not reproduce:\n\n{}\n\n\
         A number beside a command must be the number that command produces.",
        failures.len(),
        claims.len(),
        failures.join("\n\n")
    );
}

#[test]
fn nothing_that_executes_or_writes_is_permitted() {
    // Every entry ran, wrote, or deleted under some earlier revision of this
    // file. They are seeds, not hypotheticals.
    for hostile in [
        // v1: allowlisted the first verb of each `|` segment.
        "find . -name x -exec rm {} ;",
        "python3 -c 'import os; os.system(\"rm -rf /tmp/x\")'",
        "awk 'BEGIN{system(\"rm -rf /tmp/x\")}'",
        "sed -i 's/a/b/' Cargo.toml",
        // v2: refused metacharacters, but arguments still executed programs.
        r"git grep --open-files-in-pager=touch\ canary anvil",
        "git config --global core.pager evil",
        "git config zz.canary 1",
        "git diff --output=written HEAD~1 HEAD",
        "git branch -D some-branch",
        "git worktree add /tmp/x HEAD",
        "git log --oneline -1 | git config zz.canary 1",
        "git -c core.hooksPath=/dev/null push",
        // Writes and exec through verbs that looked read-only. Every one of
        // these was PERMITTED until the verb was removed; `-o` was refused
        // while four other spellings of the same capability were not.
        "grep -c foo Cargo.toml | sort -o /tmp/written",
        "grep -c foo Cargo.toml | sort --output=/tmp/written",
        "grep -c foo Cargo.toml | sort -uo /tmp/written",
        "sort -o/tmp/written Cargo.toml",
        "sort --out=/tmp/written Cargo.toml",
        "echo touch /tmp/pwned | sort --compress-program=/bin/sh",
        "uniq Cargo.toml /tmp/written",
        // Bare hostile verbs.
        "rm -rf /tmp/anvil-should-not-exist",
        "curl https://example.invalid",
    ] {
        assert!(
            refuse_unless_read_only(hostile).is_err(),
            "permitted a command that executes or writes: {hostile}"
        );
    }

    for benign in [
        "grep -c edition Cargo.toml",
        "cat Cargo.toml | grep -c edition",
        // A pipe and parentheses inside a quoted regex are not shell syntax.
        // A flat scan for either rejected every real claim in the corpus.
        r"grep -chE '^\| *H1-[0-9]+ *\|' docs/plan/ws-*.md | paste -sd+ - | bc",
        r"grep -chE '^\| *(WS[0-9]+-)?H1[-0-9a-z]* *\|' docs/plan/ws-*.md | paste -sd+ - | bc",
    ] {
        assert!(
            refuse_unless_read_only(benign).is_ok(),
            "refused a read-only command: {benign}"
        );
    }
}

#[test]
fn verbs_on_the_allowlist_cannot_write() {
    // The guard, replacing "I read the man page and it looked read-only".
    // `sort` and `uniq` were both vouched for that way and both wrote:
    // `sort -uo F`, `sort -o/F`, `sort --out=F`, and `uniq IN OUT`, which takes
    // an output path positionally and so is reachable by no flag list at all.
    // Every verb admitted here has to survive these shapes.
    let dir = std::env::temp_dir().join("anvil-verb-write-probe");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("probe dir");
    let input = dir.join("in.txt");
    std::fs::write(&input, "b\na\na\n").expect("probe input");

    let mut wrote = Vec::new();
    for verb in ALLOWED {
        for shape in [
            vec![
                input.display().to_string(),
                dir.join("out").display().to_string(),
            ],
            vec![
                "-o".into(),
                dir.join("out").display().to_string(),
                input.display().to_string(),
            ],
            vec![
                format!("-o{}", dir.join("out").display()),
                input.display().to_string(),
            ],
            vec!["-uo".into(), dir.join("out").display().to_string()],
            vec![
                format!("--out={}", dir.join("out").display()),
                input.display().to_string(),
            ],
            vec![
                format!("--output={}", dir.join("out").display()),
                input.display().to_string(),
            ],
        ] {
            let out = dir.join("out");
            let _ = std::fs::remove_file(&out);
            let _ = Command::new(verb)
                .args(&shape)
                .current_dir(&dir)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output();
            if out.exists() {
                wrote.push(format!("`{verb} {}` created a file", shape.join(" ")));
                let _ = std::fs::remove_file(&out);
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        wrote.is_empty(),
        "a verb on the allowlist can write a file, so document text can write \
         one:\n  {}\nRemove the verb. Refusing the flag is not sufficient -- \
         `uniq IN OUT` needs none.",
        wrote.join("\n  ")
    );
}

#[test]
fn shell_metacharacters_are_inert_because_no_shell_runs() {
    // These are NOT refused, and that is the point: with no shell, `;` and
    // `$( )` are ordinary argv handed to `grep`, so the danger is gone rather
    // than filtered. Under v1 every one of them executed. Asserting refusal
    // here would test the filter; asserting no side effect tests the property.
    let canary = std::env::temp_dir().join("anvil-metachar-canary");
    for form in [
        "grep foo Cargo.toml ; touch CANARY",
        "grep foo Cargo.toml && touch CANARY",
        "echo $(touch CANARY)",
        "echo `touch CANARY`",
        "grep foo Cargo.toml > CANARY",
        "cat < CANARY",
    ] {
        let _ = std::fs::remove_file(&canary);
        let cmd = form.replace("CANARY", &canary.display().to_string());
        // Permitted, because the verb is read-only and the rest is just argv.
        assert!(
            refuse_unless_read_only(&cmd).is_ok(),
            "expected inert-but-permitted: {cmd}"
        );
        let _ = run(&cmd);
        assert!(
            !canary.exists(),
            "a metacharacter form created a file, so a shell ran: {cmd}"
        );
    }
}

#[test]
fn a_refused_command_is_never_actually_spawned() {
    // `refuse_unless_read_only` returning Err is only worth something if the
    // caller consults it. An earlier revision tested the predicate in isolation
    // and never asserted that anything did not run; canaries were the first
    // real measurement.
    let canary = std::env::temp_dir().join("anvil-claim-canary");
    let _ = std::fs::remove_file(&canary);
    let hostile = format!("touch {}", canary.display());
    assert!(refuse_unless_read_only(&hostile).is_err());
    assert!(
        !canary.exists(),
        "the canary exists, so a refused command was spawned anyway"
    );
}
