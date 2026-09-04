//! A number published beside a claim must be the number that claim measures.
//!
//! `prose_counts_test` states the rule for two corpora -- Rust doc comments and
//! the `corpus_sync`-owned markdown -- deriving counts from symbols. This is the
//! third corpus: `docs/plan/` is outside `corpus_sync::OWNED`, no test reads it,
//! and its claims are measurements rather than counts a symbol can derive.
//!
//! # Nothing is executed
//!
//! Three earlier revisions ran text out of the document, and each was shown to
//! execute or write on a corpus any contributor can edit:
//!
//!   1. Allowlisting the first verb of each `|` segment -- `grep foo ; rm -rf ~`
//!      presented as `grep`. Seven of eight hostile forms ran.
//!   2. Refusing shell metacharacters -- closed `;`, `$( )`, `>`; missed that
//!      *arguments* execute code with no metacharacter:
//!      `git grep --open-files-in-pager=touch\ canary` ran `touch`, and
//!      `git config --global` wrote the reader's `~/.gitconfig`.
//!   3. Dropping the shell and allowlisting read-only verbs -- missed that
//!      `sort --compress-program=/bin/sh` *executes* the piped data on GNU
//!      coreutils, that `sort -uo F` / `-o/F` / `--out=F` all write, and that
//!      `uniq IN OUT` writes with no flag at all, which no list of refused flags
//!      could ever reach.
//!
//! Each fix closed the layer it had been shown, and the pattern was the finding:
//! a design and the seeds proving it came from one author, so the seeds could
//! only confirm the design. The premise is therefore gone rather than patched.
//! **No process is spawned.** `no_process_is_ever_spawned` holds that as a fact
//! about this source instead of an argument about flags -- a grep a reader can
//! run, rather than a claim they must take on trust.
//!
//! # The contract
//!
//! Inside a fenced block (``` or ~~~), `#=` marks an assertion. Left of it is a
//! claim in exactly one form; right of it the expected value:
//!
//! ```text
//! count '<regex>' in <glob>     #= 42
//! ```
//!
//! It counts lines matching `<regex>` across files matching `<glob>`, summed --
//! what `grep -chE ... | paste -sd+ - | bc` did, and what every claim in this
//! corpus and every defect in its history reduces to. The syntax is deliberately
//! not shell-shaped: a line that *looks* like a pipeline invites the assumption
//! that pipelines work, and that gap between what a line appears to do and what
//! it does was itself a source of defects here.
//!
//! Anything else is refused as malformed rather than ignored, and an expected
//! value may not be empty -- an empty expectation matches a silent command,
//! which is how five write-canaries passed revision three.
//!
//! # What it still cannot catch
//!
//!   * **A wrong claim that agrees with its number.** If the regex is wrong and
//!     the author publishes what the wrong regex returns, this passes. Nothing
//!     here knows what was meant.
//!   * **A number in prose, or in a markdown table cell.** Only fenced blocks
//!     are scanned; a `#=` outside a fence is *reported* rather than skipped,
//!     but an unmarked number is invisible. Inferring claims from prose would
//!     itself be a proxy.

use regex::Regex;
use std::path::{Path, PathBuf};

struct Claim {
    file: PathBuf,
    line: usize,
    spec: String,
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

/// Every `#=` in the file, with whether it sat inside a fence. Unfenced ones are
/// collected rather than dropped: markers in `~~~` fences, indented blocks and
/// table cells were silently unexamined by an earlier revision while it reported
/// green. A marker the scan cannot evaluate is a finding, not a non-event.
fn claims_in(path: &Path) -> Vec<Claim> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut out = Vec::new();
    let mut fence: Option<char> = None;
    for (i, raw) in text.lines().enumerate() {
        let t = raw.trim_start();
        if let Some(c) = ['`', '~']
            .into_iter()
            .find(|c| t.starts_with(&c.to_string().repeat(3)))
        {
            match fence {
                Some(open) if open == c => fence = None,
                None => fence = Some(c),
                _ => {}
            }
            continue;
        }
        let Some((spec, expected)) = raw.split_once("#=") else {
            continue;
        };
        if spec.trim().is_empty() {
            continue;
        }
        out.push(Claim {
            file: path.to_path_buf(),
            line: i + 1,
            spec: spec.trim().to_string(),
            expected: expected.trim().to_string(),
            fenced: fence.is_some(),
        });
    }
    out
}

/// Expand a trailing-component `*` by reading the directory. Bounded, and it
/// cannot execute anything; an unmatched pattern yields nothing, which the
/// caller reports rather than treating as an empty-and-green zero.
fn expand(glob: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (dir, file) = match glob.rfind('/') {
        Some(slash) => (&glob[..slash], &glob[slash + 1..]),
        None => (".", glob),
    };
    let Some((prefix, suffix)) = file.split_once('*') else {
        let p = root.join(glob);
        return if p.is_file() { vec![p] } else { Vec::new() };
    };
    let mut hits: Vec<PathBuf> = std::fs::read_dir(root.join(dir))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n.len() >= prefix.len() + suffix.len() && n.starts_with(prefix) && n.ends_with(suffix)
        })
        .map(|e| e.path())
        .collect();
    hits.sort();
    hits
}

/// The one claim form. `Err` says why a line is not a claim; malformed is a
/// failure, never a skip.
fn evaluate(spec: &str) -> Result<String, String> {
    let rest = spec.strip_prefix("count ").ok_or_else(|| {
        format!("not a claim: expected `count '<regex>' in <glob>`, got `{spec}`")
    })?;
    let quoted = rest
        .trim_start()
        .strip_prefix('\'')
        .ok_or_else(|| "the regex must be single-quoted".to_string())?;
    let end = quoted
        .find('\'')
        .ok_or_else(|| "the regex is missing its closing quote".to_string())?;
    let (pattern, tail) = quoted.split_at(end);
    let glob = tail[1..]
        .trim_start()
        .strip_prefix("in ")
        .ok_or_else(|| "expected `in <glob>` after the regex".to_string())?
        .trim();
    if glob.is_empty() {
        return Err("the glob is empty".to_string());
    }
    let re = Regex::new(pattern).map_err(|e| format!("the regex does not compile: {e}"))?;
    let files = expand(glob);
    if files.is_empty() {
        return Err(format!("`{glob}` matched no files"));
    }
    let total: usize = files
        .iter()
        .map(|f| {
            std::fs::read_to_string(f)
                .unwrap_or_default()
                .lines()
                .filter(|l| re.is_match(l))
                .count()
        })
        .sum();
    Ok(total.to_string())
}

#[test]
fn every_published_claim_produces_the_number_published_beside_it() {
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
                "{at}: `#=` outside a fenced block, so it cannot be evaluated.\n    {}",
                claim.spec
            ));
            continue;
        }
        if claim.expected.is_empty() {
            failures.push(format!(
                "{at}: `#=` with no expected value, which asserts nothing.\n    {}",
                claim.spec
            ));
            continue;
        }
        match evaluate(&claim.spec) {
            Err(why) => failures.push(format!("{at}: {why}")),
            Ok(actual) if actual != claim.expected => failures.push(format!(
                "{at}: published `{}` but the claim measures `{}`\n    {}",
                claim.expected, actual, claim.spec
            )),
            Ok(_) => {}
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} published claim(s) did not reproduce:\n\n{}",
        failures.len(),
        claims.len(),
        failures.join("\n\n")
    );
}

#[test]
fn no_process_is_ever_spawned() {
    // The safety property, held as a fact about this source rather than an
    // argument about which flags are dangerous. Three revisions argued the
    // latter and three were wrong; this one a reader checks with a grep.
    let me = std::fs::read_to_string(file!()).expect("this file is readable");
    let body: String = me
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect();
    // Assembled at runtime rather than written as literals, because a literal
    // needle makes the scan match its own forbidden list -- which is exactly
    // what happened, and is the same self-match that has now appeared four
    // times in this session's work. A detector whose corpus includes its own
    // statement measures the statement.
    let forbidden = [
        format!("{}::{}", "Command", "new"),
        format!("{}::{}", "process", "Command"),
        format!("{}::{}", "std", "process"),
    ];
    for forbidden in &forbidden {
        assert!(
            !body.contains(forbidden.as_str()),
            "`{forbidden}` appears in the scan. Document text must never reach a \
             process: `sort --compress-program`, `uniq IN OUT` and \
             `git grep --open-files-in-pager` all executed or wrote through \
             allowlists that looked airtight."
        );
    }
}

#[test]
fn a_malformed_claim_is_reported_rather_than_ignored() {
    // Everything that is not the one supported form fails loudly. Under the
    // spawning revisions, each of these was a command that ran.
    for bad in [
        "rm -rf /tmp/x",
        "grep -c foo Cargo.toml",
        "echo touch /tmp/p | sort --compress-program=/bin/sh",
        "uniq Cargo.toml /tmp/written",
        "git config --global core.pager evil",
        "count missing-quotes in docs/plan/*.md",
        "count 'unterminated in docs/plan/*.md",
        "count 'ok' docs/plan/*.md",
        "count 'ok' in ",
        "count '[' in docs/plan/*.md",
    ] {
        assert!(
            evaluate(bad).is_err(),
            "a malformed or hostile claim was accepted: {bad}"
        );
    }
}

#[test]
fn the_one_claim_form_measures_what_it_says() {
    // Against a known answer rather than trusted: this file holds exactly four
    // `#[test]` lines, including this one.
    let n = evaluate("count '^#\\[test\\]' in tests/published_commands_reproduce_test.rs")
        .expect("well-formed");
    assert_eq!(n, "4", "counted the wrong number of #[test] lines");

    // A glob matching nothing is an error, not an empty-and-green zero.
    assert!(evaluate("count 'x' in docs/plan/zz-nonexistent-*.md").is_err());
}
