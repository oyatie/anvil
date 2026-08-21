//! Mechanical sync of published gate-count claims to `TOTAL_GATES`.
//!
//! DocGuard used to ask a model whether docs were sufficient, then only
//! create stubs for missing files. Published counts (23 / 60 / 68 / 70)
//! drifted. This module owns those claims: rewrite when it can, fail
//! closed if a page still disagrees after the write.

use anyhow::{Context, Result};
use regex::Regex;
use std::path::Path;

/// Result of a corpus sync pass.
pub struct CorpusSync {
    pub rewritten: Vec<String>,
    pub remaining_drift: Vec<String>,
    /// `None` when the sync applied to the repository under review.
    /// `Some(reason)` when it did not, so the gate summary can say the sync
    /// did not apply rather than read as a silent pass.
    pub not_applicable: Option<String>,
}

const OWNED: &[&str] = &["README.md", "docs/doctrine.md", "openapi/openapi.yaml"];
const ADR_DIRS: &[&str] = &["docs/adr", "docs/decisions"];

/// Anvil's own repository: the only one whose published gate counts are
/// `TOTAL_GATES`, and therefore the only one this module may touch.
///
/// A compile-time constant rather than a lookup of `SELF_REPO`, because
/// `Config::from_env` calls `dotenvy::dotenv()` and mutates the process
/// environment: ownership would then depend on whichever `.env` sat in the
/// working directory, and a mis-set `SELF_REPO` would hand a watched
/// repository's published documents to a rewrite that the review pipeline then
/// commits and pushes onto the contributor's branch. That is issue #27 reached
/// through the environment instead of through the argument.
///
/// STATED COST: renaming or moving this repository switches gate 1's corpus
/// enforcement off silently. The failure direction is safe — Anvil stops
/// repairing its own pages, and no watched repository is ever corrupted — but a
/// rename must update this constant in the same commit.
const ANVIL_REPO: &str = "oyatie/anvil";

/// Whether the repository under review is Anvil's own.
///
/// GitHub slugs are case-insensitive identities and arrive in whatever case the
/// event carried, so `Oyatie/Anvil` is Anvil. The comparison is over the WHOLE
/// slug: `attacker/anvil`, `oyatie/anvil-sdk`, `oyatie/anvildocs` and a bare
/// `anvil` all belong to somebody else.
fn is_anvils_own(repo: &str) -> bool {
    repo.eq_ignore_ascii_case(ANVIL_REPO)
}

/// Why the sync declined to run.
///
/// A property of *which repository is under review* and nothing else, so a
/// caller can derive the same sentence without reproducing the checkout.
fn not_applicable_reason(repo: &str) -> String {
    format!(
        "the repository under review is {repo:?}, not {ANVIL_REPO}; Anvil's published \
         counts and owned page set say nothing about it"
    )
}

const EXEMPTION_MARKERS: &[&str] = &[
    "does **not** yet amend existing documents",
    "does not yet amend existing documents",
];

/// Rewrite owned pages so published gate-count claims match `total_gates`.
///
/// On I/O failure, returns `Err`. That is Errored, not Passed.
/// Remaining drift after a successful write is returned in
/// `remaining_drift` and must fail the gate (never AutoUpdated).
///
/// `repo` is the `owner/name` slug of the repository under review. The owned
/// page set and `total_gates` are Anvil's own; they say nothing about any other
/// repository, so the sync must not apply to one.
pub fn sync_published_counts(
    repo: &str,
    repo_dir: &Path,
    total_gates: usize,
) -> Result<CorpusSync> {
    // Ownership is decided BEFORE the corpus is opened at all, not per page and
    // not after `collect_owned_pages`. Both filesystem reads below belong to
    // Anvil's own checkout: listing somebody else's `docs/adr` or reading their
    // `README.md` is something this sync has no reason to do, and an `Err` from
    // either would be mapped onto `errored` at the gate — blocking every pull
    // request on that repository on a directory Anvil should never have opened.
    if !is_anvils_own(repo) {
        return Ok(CorpusSync {
            rewritten: Vec::new(),
            remaining_drift: Vec::new(),
            not_applicable: Some(not_applicable_reason(repo)),
        });
    }

    let mut rewritten = Vec::new();
    let mut remaining_drift = Vec::new();
    let pages = collect_owned_pages(repo_dir)?;

    for rel in pages {
        let path = repo_dir.join(&rel);
        let original = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e).with_context(|| format!("read {rel}")),
        };
        let updated = rewrite_page(&original, total_gates);
        if updated != original {
            std::fs::write(&path, &updated).with_context(|| format!("write {rel}"))?;
            rewritten.push(rel.clone());
        }
        if let Some(why) = remaining_claim(&updated, total_gates) {
            remaining_drift.push(format!("{rel}: {why}"));
        }
    }

    Ok(CorpusSync {
        rewritten,
        remaining_drift,
        not_applicable: None,
    })
}

fn collect_owned_pages(repo_dir: &Path) -> Result<Vec<String>> {
    let mut out: Vec<String> = OWNED.iter().map(|s| (*s).to_string()).collect();
    for dir in ADR_DIRS {
        let root = repo_dir.join(dir);
        if !root.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&root).with_context(|| format!("list {dir}"))? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".md") {
                out.push(format!("{dir}/{name}"));
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn count_regex() -> Regex {
    // Matches "23-gate", "60-Gate", "68 gates". Does not match
    // "100-300 lines", "16-lens", or "380 Rust rules".
    Regex::new(r"(?i)\b(\d+)(\s*-\s*gate|\s+gates?)\b").expect("count regex")
}

fn sixty_regex() -> Regex {
    Regex::new(r"(?i)\bsixty-gate\b").expect("sixty regex")
}

fn rewrite_page(input: &str, total_gates: usize) -> String {
    let n = total_gates.to_string();
    let mut out = count_regex()
        .replace_all(input, |caps: &regex::Captures| format!("{}{}", n, &caps[2]))
        .into_owned();
    out = sixty_regex()
        .replace_all(&out, format!("{n}-Gate"))
        .into_owned();
    // Every occurrence of every variant, taking exactly the marker's own
    // SENTENCE each time. Deleting in ascending order of the *current* string
    // (rather than collecting offsets up front) is what keeps the second
    // occurrence on a line from being sliced at an offset the first deletion
    // has already invalidated.
    //
    // This terminates: `sentence_start` never runs past the marker and
    // `sentence_end` never stops before the end of it, so every pass removes at
    // least the marker's own bytes.
    while let Some((idx, len)) = EXEMPTION_MARKERS
        .iter()
        .filter_map(|marker| out.find(marker).map(|i| (i, marker.len())))
        .min()
    {
        let start = sentence_start(&out, idx);
        let end = with_trailing_newline(&out, start, sentence_end(&out, idx + len));
        out.replace_range(start..end, "");
    }
    out
}

/// Characters that end a sentence and BELONG to it, so the deletion consumes
/// them.
fn is_terminator(c: char) -> bool {
    matches!(c, '.' | '?' | '!' | '。')
}

/// Characters that bound a sentence WITHOUT belonging to it: a markdown cell
/// delimiter and a line break. They are clamps, so the deletion stops in front
/// of them and leaves them where they are — eating the `|` merges two cells and
/// the table stops parsing; eating the `\n` fuses two lines.
fn is_clamp(c: char) -> bool {
    matches!(c, '|' | '\n')
}

/// Whether the terminator at `idx` really ends a sentence here.
///
/// `.` does not when the next character is ASCII alphanumeric, which is what
/// keeps `README.md`, `CHANGELOG.md` and `v1.2` from ending a sentence
/// mid-word. The exception belongs to `.` alone: `。`, `?` and `!` terminate
/// whatever follows them, because Korean and Japanese prose puts no space after
/// `。` and a page may write `Wonderful!Next`. And it is `is_ascii_alphanumeric`
/// rather than `is_alphanumeric`, so `.md` continues a sentence and `.고시` ends
/// one.
fn ends_a_sentence(text: &str, idx: usize, c: char) -> bool {
    if c != '.' {
        return true;
    }
    match text[idx + c.len_utf8()..].chars().next() {
        Some(next) => !next.is_ascii_alphanumeric(),
        None => true,
    }
}

/// The byte offset the marker's sentence starts at: walk back to the nearest
/// boundary, then forward over the spaces that separated it from the sentence
/// before it.
fn sentence_start(text: &str, marker_start: usize) -> usize {
    let mut start = 0;
    for (i, c) in text[..marker_start].char_indices().rev() {
        if is_clamp(c) || (is_terminator(c) && ends_a_sentence(text, i, c)) {
            // A clamp stays put and a terminator belongs to the sentence it
            // ended, so in both cases the deletion begins after it.
            start = i + c.len_utf8();
            break;
        }
    }
    for (i, c) in text[start..marker_start].char_indices() {
        if c != ' ' && c != '\t' {
            return start + i;
        }
    }
    marker_start
}

/// The byte offset the marker's sentence ends at: the first boundary after the
/// marker, with a terminator consumed by its own encoded length (`。` is three
/// bytes, and `+ 1` would land mid-character and panic in `replace_range`) and
/// a clamp left alone. A sentence that simply runs out of page ends there.
fn sentence_end(text: &str, marker_end: usize) -> usize {
    for (i, c) in text[marker_end..].char_indices() {
        let at = marker_end + i;
        if is_clamp(c) {
            return at;
        }
        if is_terminator(c) && ends_a_sentence(text, at, c) {
            return at + c.len_utf8();
        }
    }
    text.len()
}

/// Extends the deletion over the trailing newline only when the whole line goes
/// with it: the sentence began the line and nothing survives on it afterwards.
/// Consuming the newline in any other case fuses a surviving prefix or suffix
/// onto the line below.
fn with_trailing_newline(text: &str, start: usize, end: usize) -> usize {
    let began_the_line = start == 0 || text.as_bytes()[start - 1] == b'\n';
    let line_is_emptied = end < text.len() && text.as_bytes()[end] == b'\n';
    if began_the_line && line_is_emptied {
        end + 1
    } else {
        end
    }
}

fn remaining_claim(text: &str, total_gates: usize) -> Option<String> {
    for caps in count_regex().captures_iter(text) {
        let raw = &caps[1];
        let Ok(got) = raw.parse::<usize>() else {
            return Some(format!("unparseable gate-count claim: {raw}"));
        };
        if got != total_gates {
            return Some(format!("still claims {got}{}", &caps[2]));
        }
    }
    if sixty_regex().is_match(text) {
        return Some("still claims sixty-gate".into());
    }
    for marker in EXEMPTION_MARKERS {
        if text.contains(marker) {
            return Some("still documents the README exemption".into());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn rewrites_handwritten_counts_and_drops_the_exemption() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("README.md"),
            "# Anvil\n\nRuns the complete 23-gate certification.\n\
             Triggers 60-gate pre-merge.\n\
             DocGuard does **not** yet amend existing documents such as `README.md`.\n",
        )
        .unwrap();

        let sync = sync_published_counts("oyatie/anvil", dir.path(), 68).unwrap();
        assert_eq!(sync.rewritten, vec!["README.md".to_string()]);
        assert!(
            sync.remaining_drift.is_empty(),
            "{:?}",
            sync.remaining_drift
        );

        let got = std::fs::read_to_string(dir.path().join("README.md")).unwrap();
        assert!(got.contains("68-gate"));
        assert!(got.contains("68-gate pre-merge") || got.contains("68-gate"));
        assert!(!got.contains("23-gate"));
        assert!(!got.contains("60-gate"));
        assert!(!got.contains("does **not** yet amend existing documents"));
    }

    #[test]
    fn leaves_unrelated_numbers_alone() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("README.md"),
            "16-lens review. Files stay 100-300 lines. 380 Rust rules.\n",
        )
        .unwrap();

        let sync = sync_published_counts("oyatie/anvil", dir.path(), 68).unwrap();
        assert!(sync.rewritten.is_empty());
        assert!(sync.remaining_drift.is_empty());
        let got = std::fs::read_to_string(dir.path().join("README.md")).unwrap();
        assert!(got.contains("16-lens"));
        assert!(got.contains("100-300"));
        assert!(got.contains("380 Rust rules"));
    }

    #[test]
    fn remaining_drift_is_not_autoupdated() {
        // A claim the rewriter does not know how to fix must still fail closed.
        let claim = "the fabric has 12 quality, security gates";
        assert!(remaining_claim(claim, 68).is_none());
        assert_eq!(
            remaining_claim("complete 23-gate certification", 68).as_deref(),
            Some("still claims 23-gate")
        );
    }

    #[test]
    fn remaining_claim_still_catches_the_spelled_out_count_and_both_exemption_markers() {
        // `remaining_claim` is a fail-closed NET, not a summary of what
        // `rewrite_page` happens to do. It is what turns "the rewriter missed an
        // occurrence on a page layout nobody wrote a fixture for" into a blocked
        // pull request instead of a page published mangled and reported clean —
        // and it is the whole basis of the stated exclusion in
        // `tests/docguard_oracle_repair_test.rs`, which declines to drive the
        // gate's `remaining_drift` arm precisely because this net exists.
        //
        // Until now only the digit-count arm was pinned. The `sixty-gate` arm and
        // both `EXEMPTION_MARKERS` arms had no assertion anywhere: every
        // integration case asserts the marker and the `sixty-gate` phrase are
        // GONE FROM DISK, which a correct rewriter satisfies whether or not the
        // checker still looks for them.
        //
        // The implementation that removes the net: an implementer rewriting
        // `rewrite_page` for issue #28 simplifies `remaining_claim` alongside it
        // — "the new rewriter removes every marker, so the marker check is dead
        // code" — and leaves only the count check. Every integration case and
        // every other in-module test stays green, and the first layout the new
        // sentence scan does not handle is published mangled, or still exempted,
        // with gate 1 reporting it clean.
        //
        // `is_some()` rather than an exact string: the wording of a drift reason
        // is the implementer's, the fact that the claim is CAUGHT is not. The
        // count arm keeps its exact-string case above, so the two together pin
        // presence without freezing three sentences of prose.
        const TOTAL: usize = crate::pre_merge_guard::report::TOTAL_GATES;

        assert!(
            remaining_claim("It replaced the sixty-gate pilot programme.", TOTAL).is_some(),
            "a page still publishing the spelled-out `sixty-gate` claim is drift, \
             whatever the rewriter did or did not do to it"
        );
        assert!(
            remaining_claim(
                "DocGuard does **not** yet amend existing documents such as README.md.",
                TOTAL,
            )
            .is_some(),
            "a page still carrying the bold exemption marker is drift: the marker \
             says Anvil does not amend existing documents, which is the claim this \
             module exists to have stopped being true"
        );
        assert!(
            remaining_claim(
                "DocGuard does not yet amend existing documents such as README.md.",
                TOTAL,
            )
            .is_some(),
            "the unbolded variant is the same claim and must be caught the same way; \
             a net that covers only the variant the fixtures happen to use is not a net"
        );
    }

    fn assert_published_page_has_no_count_drift(rel: &str, page: &str) {
        assert_eq!(
            remaining_claim(page, crate::pre_merge_guard::report::TOTAL_GATES),
            None,
            "{rel} still publishes a gate count that is not TOTAL_GATES"
        );
    }

    #[test]
    fn published_readme_matches_live_corpus() {
        assert_published_page_has_no_count_drift(
            "README.md",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md")),
        );
    }

    #[test]
    fn published_doctrine_matches_live_corpus() {
        assert_published_page_has_no_count_drift(
            "docs/doctrine.md",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/doctrine.md")),
        );
    }

    #[test]
    fn published_openapi_matches_live_corpus() {
        assert_published_page_has_no_count_drift(
            "openapi/openapi.yaml",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/openapi/openapi.yaml")),
        );
    }

    #[test]
    fn published_adr_0001_matches_live_corpus() {
        assert_published_page_has_no_count_drift(
            "docs/adr/0001-sixty-gate-hyperscale-matrix.md",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/adr/0001-sixty-gate-hyperscale-matrix.md"
            )),
        );
    }
}
