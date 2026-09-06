//! Read-only finite docs-claim evaluator, shared by two integration binaries.
//!
//! In a supported standalone fenced body, `#=` marks one assertion:
//! `count '<regex>' in <glob> #= <expected>`. This counts matching **lines**
//! across matching files; document text is never used as a program name.
//! Malformed specifications, empty expectations and unreadable inputs fail.
//!
//! The corpus and glob inputs are the live working tree, including admitted
//! untracked regular files. A clean CI checkout can differ from a local tree.
//! The source proxy inventories the helper subtree plus the published consumer;
//! it is lexical, not a semantic no-execution or complete-compilation proof.
//!
//! Limitations: wrong queries can agree with their numbers; unmarked numbers
//! and deleted/unmarked assertions are invisible while another assertion remains.
//! Every marker is retained, but only supported standalone fence bodies qualify.
//! This is not a full Markdown container parser or a claim registry.

mod markdown;
mod paths;
mod proxy;

pub use markdown::*;
pub use paths::*;
pub use proxy::*;

use regex::Regex;
use std::path::{Path, PathBuf};

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
            "{at}: `#=` outside a fenced block body, so it cannot be evaluated.\n    {}",
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
