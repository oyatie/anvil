//! Doc comments state the contract. History goes in the commit message.
//!
//! Google's style guides put it directly: a comment should be about the code,
//! not about the history of the code. Version control already holds the
//! history, holds it accurately, and does not go stale when the code moves.
//!
//! This repository writes the incident into the source as well, so the same
//! account exists twice and only one copy is maintained. `cedar_guard` and
//! `coverage_guard` both open with a section titled "What this gate used to
//! do" — a heading with no reader, since nobody arriving at the code needs the
//! version they cannot run.
//!
//! Rust's API guidelines want a summary line, then the contract: `# Errors`,
//! `# Panics`, `# Examples`. An example teaches what three paragraphs of
//! rationale do not, and it cannot rot silently because it compiles.

use std::path::Path;
use std::sync::LazyLock;

/// Phrases that only ever introduce a story about a previous revision.
///
/// Deliberately narrow. "Because" and "why" are legitimate in a doc comment —
/// rationale is contract. What is not contract is the state of the code before
/// this commit, which no reader can act on.
static NARRATIVE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"(?i)\b(previously|used to|the first (run|draft|cut)|originally|had been|was written|this was found|before this|on the first run)\b",
    )
    .expect("static pattern")
});

/// Comment lines narrating a previous revision, as committed today.
///
/// A floor, not the assertion. The real bound is derived from the merge-base
/// by `derived_baseline_test`; this exists so the check still runs with no
/// git available, and so a catastrophic regression is caught even then.
const NARRATIVE_COMMENT_LINES_CEILING: usize = 157;

#[path = "source_acquisition/mod.rs"]
mod source_acquisition;

#[path = "source_acquisition/merge_base.rs"]
mod merge_base_sources;

fn offenders() -> Vec<String> {
    let files =
        source_acquisition::rust_sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"))
            .expect("source corpus");
    let mut found = Vec::new();
    for file in files {
        for (n, line) in file.text.lines().enumerate() {
            let t = line.trim();
            if (t.starts_with("//") || t.starts_with("//!")) && NARRATIVE.is_match(t) {
                found.push(format!("{}:{}", file.path.display(), n + 1));
            }
        }
    }
    found
}

#[test]
fn narrative_comments_do_not_exceed_the_recorded_ceiling() {
    // `<=`, not `==`. An exact literal is a global variable every lane must
    // edit: two branches that both lower it write the same line, git merges
    // them cleanly, and the merged tree carries a number that is wrong by one
    // with no conflict to catch it. The monotone bound comes from the
    // merge-base instead — see `narrative_comments_do_not_grow_against_the_merge_base`.
    let found = offenders();
    assert!(
        found.len() <= NARRATIVE_COMMENT_LINES_CEILING,
        "comment lines narrating history grew to {} against a ceiling of {}. \
         State the contract in the doc comment and put the incident in the \
         commit message, where it is recorded once and cannot go stale.",
        found.len(),
        NARRATIVE_COMMENT_LINES_CEILING
    );
}

/// The bound that actually holds: no growth against this change's own base.
///
/// Requires a readable merge-base against `origin/dev`; unavailable evidence
/// fails this comparison. The standalone ceiling remains usable without Git.
#[test]
fn narrative_comments_do_not_grow_against_the_merge_base() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let derived = rt.block_on(anvil::ratchet::facade::derived::at_merge_base(
        repo,
        "origin/dev",
        "HEAD",
        |p| p.starts_with("src/") && p.ends_with(".rs"),
        |tree| {
            Ok(merge_base_sources::rust_sources(tree)?
                .iter()
                .flat_map(|(_, body)| body.lines().map(str::trim))
                .filter(|t| (t.starts_with("//") || t.starts_with("//!")) && NARRATIVE.is_match(t))
                .count())
        },
    ));
    let base = merge_base_sources::required(derived).expect("required merge-base measurement");
    let now = offenders().len();
    assert!(
        now <= base.at_merge_base,
        "narrative comment lines grew from {} at merge-base {} to {} here. \
         Nothing needs editing when this number falls, and nothing may raise \
         it: the bound is derived, so two branches that both improve it cannot \
         disagree.",
        base.at_merge_base,
        &base.merge_base[..12],
        now
    );
}

#[test]
fn the_matcher_accepts_rationale_and_rejects_only_history() {
    // Rationale is contract and must never be flagged; a story about a
    // previous revision must always be.
    assert!(!NARRATIVE.is_match("/// Returns None because no telemetry endpoint is configured."));
    assert!(!NARRATIVE.is_match("/// Why a string match is not a load: prose is not a loader."));
    assert!(NARRATIVE.is_match("/// This previously returned a pass for an absent track."));
    assert!(NARRATIVE.is_match("//! # What this gate used to do"));
}
