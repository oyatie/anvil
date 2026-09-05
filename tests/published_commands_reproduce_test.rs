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
//! **No process is spawned** -- and what carries that is the design, not a
//! scan: the evaluator has one form, `count '<regex>' in <glob>`, with no
//! branch anywhere that takes a program name from the document.
//! `no_source_here_reaches_a_process_or_writes` is a *proxy* guarding future
//! edits to this file. An earlier header called it a fact, under a test name
//! that no longer existed -- an overclaim and a dead pointer in one sentence --
//! and then named the wrong defeat, which is worse than naming none: a reader
//! checks the named hole, finds it closed, and trusts the rest. Measured:
//!
//!   * `use std::{process as p}` -- **trips**. The form the header used to name.
//!   * `use std::{process::{Command as C}}` -- **green, a real defeat**.
//!   * `File::options().create(true).write(true).open(..)` -- **green**, a write
//!     path on no list and named nowhere until now.
//!   * spacing (`std :: process`) and newline-split paths -- trip, since
//!     whitespace is stripped before matching.
//!
//! What carries the property is the design above, not this scan.
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
//!   * **Un-marking a checked claim is invisible too**, and that is worse than
//!     never marking one: deleting the `=` from a `#=` returns a verified
//!     number to prose, and the run stays green so long as any other claim
//!     exists anywhere in the corpus -- the emptiness guard is corpus-wide with
//!     no per-file floor. Deleting this file has the same effect and is
//!     invisible to the hook too, since `--diff-filter=ACMR` excludes
//!     deletions. Both want a gate registry, where a removed gate is a red
//!     rather than a silence -- the mechanism issue #210 needs for a component
//!     constructed and never called. One registry, not a third bespoke guard.

use regex::Regex;
use std::path::{Path, PathBuf};

struct Claim {
    file: PathBuf,
    line: usize,
    spec: String,
    expected: String,
    fenced: bool,
}

fn repo_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .map_err(|e| format!("the repository root is unreadable ({e})"))
}

/// The single gate. Every path -- walked directory, directory entry, or glob
/// target -- passes through this BEFORE any filesystem probe touches it.
///
/// Three revisions got this wrong in the same shape, each a level below the
/// last: containment in `expand` and none in the corpus walk; then containment
/// on every directory *entry* and none on the directory about to be read, so a
/// symlink at `docs/plan` was `read_dir`'d and one of its entry names reached
/// the failure message; then `expand`'s non-glob branch calling `is_file()`
/// before the check, which answers "does this exist" for anything the runner
/// can stat.
///
/// The ordering is the property: **check, then touch**. A predicate that runs
/// after the probe has already leaked whatever the probe observed. And the
/// message names only the caller's own string -- the earlier leak was not the
/// check failing, it was the error printing a path component read from outside
/// the repository.
fn gate(root: &Path, p: &Path, shown: &str) -> Result<(), String> {
    let refuse = || {
        Err(format!(
            "`{shown}` is not a readable path inside the repository"
        ))
    };
    // symlink_metadata does not follow the link, so this observes the link
    // itself rather than whatever it points at.
    match std::fs::symlink_metadata(p) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(format!(
                "`{shown}` is a symlink. Nothing in the corpus may be one: following it \
                 reads outside the repository."
            ));
        }
        Ok(_) => {}
        // Absent and unreadable collapse into the same message as
        // outside-the-repo, so the pair cannot be told apart. Two
        // distinguishable errors are one bit of existence per claim, for any
        // path the runner can stat.
        Err(_) => return refuse(),
    }
    if p.canonicalize()
        .map(|c| c.starts_with(root))
        .unwrap_or(false)
    {
        Ok(())
    } else {
        refuse()
    }
}

/// `.md` files under `dir`, refusing anything that leaves `root`.
///
/// Symlinks are refused rather than followed. Nothing under `docs/plan/` is
/// legitimately a link, git carries symlinks in a PR, and the hook's
/// `--diff-filter=ACMR` includes additions -- so following one is a read
/// outside the repository triggered by a contributor.
fn md_files_under(root: &Path, dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        // The walk root is gated too. It was not, so `docs/plan -> /etc` was
        // read before any check existed and an entry of /etc was named.
        let shown_dir = d.strip_prefix(root).unwrap_or(&d).display().to_string();
        gate(root, &d, &shown_dir)?;
        let entries = std::fs::read_dir(&d)
            .map_err(|e| format!("`{}` cannot be listed ({e})", d.display()))?;
        for entry in entries {
            let p = entry
                .map_err(|e| format!("`{}` cannot be listed ({e})", d.display()))?
                .path();
            let shown = p.strip_prefix(root).unwrap_or(&p).display().to_string();
            gate(root, &p, &shown)?;
            let meta = std::fs::symlink_metadata(&p)
                .map_err(|_| format!("`{shown}` is not a readable path inside the repository"))?;
            if meta.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("md")) {
                out.push(p);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn plan_docs() -> Result<Vec<PathBuf>, String> {
    let root = repo_root()?;
    let dir = root.join("docs/plan");
    md_files_under(&root, &dir)
}

/// Every `#=` in the file, with whether it sat inside a fence. Unfenced ones are
/// collected rather than dropped: markers in `~~~` fences, indented blocks and
/// table cells were silently unexamined by an earlier revision while it reported
/// green. A marker the scan cannot evaluate is a finding, not a non-event.
fn claims_in(path: &Path) -> Result<Vec<Claim>, String> {
    // Every IO error is propagated, never defaulted. `unwrap_or_default()` here
    // turned an unreadable file into a corpus of zero claims, so a single
    // invalid UTF-8 byte appended to this file made both real claims vanish
    // while the run stayed green -- invariant I1 ("absent evidence is never a
    // pass") broken inside the gate that exists to enforce it.
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("{}: cannot be read ({e})", path.display()))?;
    let mut out = Vec::new();
    let mut fence: Option<char> = None;
    for (i, raw) in text.lines().enumerate() {
        // The fence toggle runs for EVERY line, including one carrying a
        // marker. Testing the marker first and `continue`ing was half a fix:
        // the marker was seen, but the toggle it sat on was swallowed, so fence
        // state inverted for the rest of the file and a later, correctly fenced
        // claim was reported "outside a fenced block". Toggling first and
        // recording after keeps both facts.
        let t = raw.trim_start();
        if let Some(c) = ['`', '~']
            .into_iter()
            .find(|c| t.chars().take_while(|x| x == c).count() >= 3)
        {
            match fence {
                Some(open) if open == c => fence = None,
                None => fence = Some(c),
                _ => {}
            }
        }
        if let Some((spec, expected)) = raw.split_once("#=") {
            out.push(Claim {
                file: path.to_path_buf(),
                line: i + 1,
                // An empty left side is NOT skipped: it falls through to
                // `evaluate` and fails loudly as "not a claim". Skipping it
                // silently dropped any claim whose marker sat on its own line.
                spec: spec
                    .trim()
                    .trim_start_matches(['`', '~'])
                    .trim()
                    .to_string(),
                expected: expected.trim().to_string(),
                fenced: fence.is_some(),
            });
            continue;
        }
    }
    Ok(out)
}

/// Expand a trailing-component `*` by reading the directory. Bounded, and it
/// cannot execute anything; an unmatched pattern yields nothing, which the
/// caller reports rather than treating as an empty-and-green zero.
fn expand(glob: &str) -> Result<Vec<PathBuf>, String> {
    let root = repo_root()?;
    let (dir, file) = match glob.rfind('/') {
        Some(slash) => (&glob[..slash], &glob[slash + 1..]),
        None => (".", glob),
    };
    // Containment, checked after canonicalisation. Without it a claim could
    // read `../../../../etc/hosts` or follow a symlink out of the tree, and the
    // failure message printed the count -- one measured bit per claim of
    // anything the CI runner can read.
    let Some((prefix, suffix)) = file.split_once('*') else {
        // Gated BEFORE is_file(). The probe used to run first, so a
        // nonexistent path and an existing-but-outside one gave different
        // messages -- one bit of existence per claim.
        let p = root.join(glob);
        gate(&root, &p, glob)?;
        if !p.is_file() {
            return Err(format!(
                "`{glob}` is not a readable path inside the repository"
            ));
        }
        return Ok(vec![p]);
    };
    let base = root.join(dir);
    gate(&root, &base, glob)?;
    let entries = std::fs::read_dir(&base)
        .map_err(|e| format!("`{}` cannot be listed ({e})", base.display()))?;
    let mut hits = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("`{}` cannot be listed ({e})", base.display()))?;
        let n = entry.file_name().to_string_lossy().to_string();
        // A shell `*` does not match a leading dot; matching one here would
        // silently disagree with the command this replaces.
        if n.starts_with('.') || !n.starts_with(prefix) || !n.ends_with(suffix) {
            continue;
        }
        if n.len() < prefix.len() + suffix.len() {
            continue;
        }
        // Refused rather than skipped: the walk errors on an out-of-repo
        // entry, and a glob silently excluding one had the two paths answering
        // the same question differently.
        let p = entry.path();
        gate(&root, &p, glob)?;
        if p.is_file() {
            hits.push(p);
        }
    }
    hits.sort();
    Ok(hits)
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
    let files = expand(glob)?;
    if files.is_empty() {
        return Err(format!("`{glob}` matched no files"));
    }
    let mut total = 0usize;
    for f in &files {
        // Propagated, not defaulted: an unreadable file is absent evidence,
        // and absent evidence is never a pass.
        let text = std::fs::read_to_string(f)
            .map_err(|e| format!("{}: cannot be read ({e})", f.display()))?;
        total += text.lines().filter(|l| re.is_match(l)).count();
    }
    Ok(total.to_string())
}

#[test]
fn every_published_claim_produces_the_number_published_beside_it() {
    let docs = plan_docs().expect("docs/plan must be listable; an unreadable corpus is not a pass");
    let mut claims: Vec<Claim> = Vec::new();
    for p in &docs {
        claims.extend(claims_in(p).unwrap_or_else(|e| panic!("{e}")));
    }

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
fn no_source_here_reaches_a_process_or_writes() {
    // A PROXY, deliberately labelled as one. An earlier revision called this
    // "a fact about this source"; a review then defeated it two ways, each
    // with a real canary file created:
    //
    //     use std :: process :: Command as X    -- missed, spacing
    //     include!("elsewhere.rs")              -- missed, another file
    //
    // `#[path] mod` and anything under tests/common/ are the same hole, and
    // clippy's disallowed_methods cannot help: it is configured repo-wide, and
    // src/exec legitimately spawns processes. So this is a best-effort scan
    // over one file, and what actually carries the safety property is that the
    // evaluator has one form -- `count '<regex>' in <glob>' -- with no branch
    // that takes a program name from the document at all.
    //
    // Whitespace around `::` is normalised so spacing cannot defeat it, and the
    // needles are assembled at runtime so the scan does not match its own list.
    let me = std::fs::read_to_string(file!()).expect("this file is readable");
    let body: String = me
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let forbidden = [
        format!("{}::{}", "Command", "new"),
        format!("{}::{}", "process", "Command"),
        format!("{}::{}", "std", "process"),
        format!("{}::{}", "libc", "system"),
        format!("{}::{}", "std", "net"),
        format!("{}::{}", "fs", "write"),
        format!("{}::{}", "File", "create"),
        format!("{}{}", "Open", "Options"),
        format!("{}::{}", "fs", "remove"),
        format!("{}::{}", "fs", "rename"),
        format!("{}!", "include"),
        format!("#[{}", "path"),
    ];
    for needle in &forbidden {
        assert!(
            !body.contains(needle.as_str()),
            "`{needle}` appears in the scan. Document text must never reach a \
             process, the network, or a write: `sort --compress-program`, \
             `uniq IN OUT` and `git grep --open-files-in-pager` all executed or \
             wrote through allowlists that looked airtight."
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
    // Derived, not hardcoded: an earlier revision pinned this to "4" and would
    // have broken the moment a test was added, for a reason unrelated to the
    // thing under test.
    let me = std::fs::read_to_string(file!()).expect("readable");
    let expect = me.lines().filter(|l| l.starts_with("#[test]")).count();
    let n = evaluate("count '^#\\[test\\]' in tests/published_commands_reproduce_test.rs")
        .expect("well-formed");
    assert_eq!(
        n,
        expect.to_string(),
        "counted the wrong number of #[test] lines"
    );

    // A glob matching nothing is an error, not an empty-and-green zero.
    assert!(evaluate("count 'x' in docs/plan/zz-nonexistent-*.md").is_err());
}
