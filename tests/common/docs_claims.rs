//! The docs-claim evaluator: read-only, and the only thing that ever sees
//! corpus text.
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
//! value may not be empty -- an empty expectation matches a silent command.
//!
//! **No process is spawned**, and what carries that is the design rather than a
//! scan: one form, `count '<regex>' in <glob>`, with no branch anywhere that
//! takes a program name from the document. `forbidden_hits` is a *proxy* over
//! this file, and which forms it does and does not catch is asserted by
//! `docs_claims_hardening_test`, not described here. An earlier header
//! published `use std::{process as p}` as a form that **trips**; it did not,
//! and a process really ran through it while every test stayed green.
//!
//! # What it still cannot catch
//!
//!   * **A wrong claim that agrees with its number.** If the regex is wrong and
//!     the author publishes what the wrong regex returns, this passes. Nothing
//!     here knows what was meant.
//!   * **A number in prose, or in a markdown table cell.** Only fenced blocks
//!     are scanned; a `#=` outside a fence is *reported* rather than skipped,
//!     but an unmarked number is invisible.
//!   * **Un-marking a checked claim is invisible too**, and that is worse than
//!     never marking one: deleting the `=` from a `#=` returns a verified
//!     number to prose, and the run stays green so long as any other claim
//!     exists anywhere in the corpus. Deleting this file has the same effect.
//!     Both want the gate registry issue #210 needs -- one registry, not a
//!     third bespoke guard.

use regex::Regex;
use std::path::{Path, PathBuf};

pub struct Claim {
    pub file: PathBuf,
    pub line: usize,
    pub spec: String,
    pub expected: String,
    pub fenced: bool,
}

pub fn repo_root() -> Result<PathBuf, String> {
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
pub fn gate(root: &Path, p: &Path, shown: &str) -> Result<(), String> {
    let refuse = || {
        Err(format!(
            "`{shown}` is not a readable path inside the repository"
        ))
    };
    // symlink_metadata does not follow the link, so this observes the link
    // itself rather than whatever it points at.
    //
    // A symlink refuses with the SAME message as absent and as outside-the-repo.
    // Naming it separately was one measured bit of out-of-repo existence per
    // claim: on ubuntu-24.04 `/bin`, `/lib` and `/sbin` are symlinks, so
    // `count 'x' in /bin` and `count 'x' in /biXXX` answered differently.
    match std::fs::symlink_metadata(p) {
        Ok(meta) if meta.file_type().is_symlink() => return refuse(),
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
pub fn md_files_under(root: &Path, dir: &Path) -> Result<Vec<PathBuf>, String> {
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

pub fn plan_docs() -> Result<Vec<PathBuf>, String> {
    let root = repo_root()?;
    let dir = root.join("docs/plan");
    md_files_under(&root, &dir)
}

/// Every `#=` in the file, with whether it sat inside a fence. Unfenced ones are
/// collected rather than dropped: markers in `~~~` fences, indented blocks and
/// table cells were silently unexamined by an earlier revision while it reported
/// green. A marker the scan cannot evaluate is a finding, not a non-event.
pub fn claims_in(path: &Path) -> Result<Vec<Claim>, String> {
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
pub fn expand(root: &Path, glob: &str) -> Result<Vec<PathBuf>, String> {
    // No path component may begin with a dot. `.git/config` is inside the
    // repository, so containment admits it -- and it holds the `actions/checkout`
    // token, which a failing claim's reported count can binary-search out
    // through a public CI log. `..` begins with a dot too, so one rule closes
    // the traversal and the dotfile class together.
    if glob.split('/').any(|c| c.starts_with('.')) {
        return Err(format!(
            "`{glob}` has a path component beginning with `.`; the corpus is the \
             tracked tree, not the machinery beside it"
        ));
    }
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
        gate(root, &p, glob)?;
        if !p.is_file() {
            return Err(format!(
                "`{glob}` is not a readable path inside the repository"
            ));
        }
        return Ok(vec![p]);
    };
    let base = root.join(dir);
    gate(root, &base, glob)?;
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
        gate(root, &p, glob)?;
        if p.is_file() {
            hits.push(p);
        }
    }
    hits.sort();
    Ok(hits)
}

/// The one claim form. `Err` says why a line is not a claim; malformed is a
/// failure, never a skip.
pub fn evaluate(root: &Path, spec: &str) -> Result<String, String> {
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
    let files = expand(root, glob)?;
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

/// The needles, assembled from fragments so the list cannot match itself.
pub fn forbidden_needles() -> Vec<String> {
    vec![
        format!("{}::{}", "Command", "new"),
        format!("{}::{}", "process", "Command"),
        format!("{}::{}", "std", "process"),
        format!("{}::{}", "libc", "system"),
        format!("{}::{}", "std", "net"),
        format!("{}::{}", "fs", "write"),
        format!("{}::{}", "File", "create"),
        // `File::options()` returns an OpenOptions without ever naming the type,
        // so the `OpenOptions` needle below does not see it.
        format!("{}::{}", "File", "options"),
        format!("{}{}", "Open", "Options"),
        format!("{}::{}", "fs", "remove"),
        format!("{}::{}", "fs", "rename"),
        format!("{}!", "include"),
        format!("#[{}", "path"),
    ]
}

/// Which needles `source` contains, after normalisation.
///
/// It takes the source as an argument rather than reading `file!()` itself, and
/// that is the whole point: the forms this scan does and does not catch are now
/// asserted by `the_forms_the_header_names_are_the_forms_the_scan_catches`
/// against real snippets, instead of described in a comment beside it. The
/// header previously published `use std::{process as p}` as a form that
/// **trips**. It did not, and a process really ran through it while every test
/// stayed green -- a false measurement in the paragraph warning that a false
/// measurement is worse than none.
pub fn forbidden_hits(source: &str) -> Vec<String> {
    let body: String = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        // Braces are stripped with whitespace. Grouping was the only thing
        // hiding the defeat: `use std::{process as p}` is `std::process` once
        // `{` is gone, and so is `use std::{process::{Command as C}}`.
        .filter(|c| !c.is_whitespace() && *c != '{' && *c != '}')
        .collect();
    forbidden_needles()
        .into_iter()
        .filter(|n| body.contains(n.as_str()))
        .collect()
}

/// One published claim's verdict, or `None` when it reproduced.
///
/// Extracted so the refusals below are reachable from a corpus a test builds.
/// They used to live inline in the corpus test, where only `docs/plan/` drove
/// them -- and `docs/plan/` contains no unfenced marker and no empty
/// expectation, so both refusals could be deleted with the suite still green.
pub fn check_claim(root: &Path, claim: &Claim) -> Option<String> {
    let at = format!("{}:{}", claim.file.display(), claim.line);
    if !claim.fenced {
        return Some(format!(
            "{at}: `#=` outside a fenced block, so it cannot be evaluated.\n    {}",
            claim.spec
        ));
    }
    if claim.expected.is_empty() {
        return Some(format!(
            "{at}: `#=` with no expected value, which asserts nothing.\n    {}",
            claim.spec
        ));
    }
    match evaluate(root, &claim.spec) {
        Err(why) => Some(format!("{at}: {why}")),
        Ok(actual) if actual != claim.expected => Some(format!(
            "{at}: published `{}` but the claim measures `{}`\n    {}",
            claim.expected, actual, claim.spec
        )),
        Ok(_) => None,
    }
}

/// Every claim under `docs`, and every one that did not reproduce.
pub fn check_corpus(root: &Path, docs: &[PathBuf]) -> Result<(usize, Vec<String>), String> {
    let mut claims: Vec<Claim> = Vec::new();
    for p in docs {
        claims.extend(claims_in(p)?);
    }
    let failures = claims.iter().filter_map(|c| check_claim(root, c)).collect();
    Ok((claims.len(), failures))
}
