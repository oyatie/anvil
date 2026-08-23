//! Mechanical sync of published gate-count claims to `TOTAL_GATES`.
//!
//! DocGuard used to ask a model whether docs were sufficient, then only
//! create stubs for missing files. Published counts (23 / 60 / 68 / 70)
//! drifted. This module owns those claims: rewrite when it can, fail
//! closed if a page still disagrees after the write.

use crate::config::SELF_REPO;
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

/// Whether the repository under review is Anvil's own.
///
/// Compared against `config::SELF_REPO`, the compile-time constant, and never
/// against `Config::self_repo`: that field reads the `SELF_REPO` environment
/// variable through `Config::from_env`, which calls `dotenvy::dotenv()` and
/// mutates the process environment, so a mis-set value there would hand a
/// watched repository's published documents to a rewrite the review pipeline
/// then commits and pushes — issue #27 reached by a second route.
///
/// STATED COST: renaming or moving this repository switches gate 1's corpus
/// enforcement off until the constant is updated in the same commit. The
/// failure direction is safe — Anvil stops repairing its own pages, and no
/// watched repository is ever corrupted.
///
/// GitHub slugs are case-insensitive identities and arrive in whatever case the
/// event payload carried, so `Oyatie/Anvil` is this repository. That is safe
/// only because it applies to the WHOLE slug: `attacker/anvil`,
/// `oyatie/anvil-sdk` and `notoyatie/anvil` all belong to somebody else.
fn is_anvils_own_repository(repo: &str) -> bool {
    repo.trim().eq_ignore_ascii_case(SELF_REPO)
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
    if !is_anvils_own_repository(repo) {
        // Not one byte of another repository is read, let alone written. The
        // ownership decision precedes BOTH of this function's filesystem reads:
        // an unreadable `docs/adr` in somebody else's checkout is not Anvil's
        // failure to report, and erroring on it would block every pull request
        // on every watched repository at gate 1 — issue #27's harm arrived at
        // from the fail-closed side.
        return Ok(CorpusSync {
            rewritten: Vec::new(),
            remaining_drift: Vec::new(),
            not_applicable: Some(format!(
                "{repo:?} is not {SELF_REPO}, so Anvil's published gate counts and \
                 owned pages say nothing about it"
            )),
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
    // The exemption deletion takes the marker's own SENTENCE, not its line.
    // `while let`, not `if let`: `remaining_claim` fails the gate on any
    // surviving marker, so a first-occurrence-only deletion turns a page the
    // sync was supposed to have repaired into an unfixable hard failure. Each
    // pass re-finds the marker in the already-edited string, so the offsets a
    // previous deletion shifted are never stale.
    for marker in EXEMPTION_MARKERS {
        while let Some(idx) = out.find(marker) {
            let (start, end) = exemption_sentence_range(&out, idx, marker.len());
            out.replace_range(start..end, "");
        }
    }
    out
}

/// The byte range of the sentence the exemption marker at `idx` belongs to.
///
/// A sentence is bounded by a TERMINATOR — `.`, `?`, `!`, `。` — which belongs
/// to the sentence it ends, or by a CLAMP — the markdown cell delimiter `|` or a
/// newline — which belongs to the surrounding text and stays where it is, or by
/// the ends of the page. The same set applies walking backwards from the marker
/// and walking forwards from it; two scans with different boundary sets is the
/// shape that destroys half the layouts a corpus really has.
///
/// `.` is a terminator unless the next character is ASCII alphanumeric, which is
/// what keeps `README.md`, `CHANGELOG.md` and `v1.2` from ending a sentence
/// mid-word. The exception is specific to `.`: `。` ends a sentence whatever
/// follows it, because Korean and Japanese prose puts no space after it, and
/// Anvil's own corpus carries Korean. `is_ascii_alphanumeric`, not
/// `is_alphanumeric` — `.고시` ends a sentence and `.md` does not.
///
/// The trailing newline is consumed only when the deletion both STARTED at a
/// line start and leaves nothing but whitespace on that line after it. Both
/// halves are whitespace-blind: an exemption sentence occupying a whole indented
/// line (a YAML block scalar in `openapi/openapi.yaml`) takes its indentation
/// with it, and one whose line ends in a markdown hard break takes that too,
/// rather than leaving a whitespace-only line on a page the pipeline commits and
/// pushes. When the line SURVIVES, its own leading and trailing whitespace
/// survives with it — the deletion was never asked to edit how that line renders.
fn exemption_sentence_range(text: &str, idx: usize, marker_len: usize) -> (usize, usize) {
    let mut start = sentence_start(text, idx);
    let mut end = sentence_end(text, idx + marker_len);

    let line_start = text[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = text[end..]
        .find('\n')
        .map(|rel| end + rel)
        .unwrap_or(text.len());
    let began_the_line = text[line_start..start].trim().is_empty();
    let nothing_survives_after = text[end..line_end].trim().is_empty();
    if began_the_line && nothing_survives_after {
        start = line_start;
        end = (line_end + 1).min(text.len());
    }

    (start, end)
}

/// Whether the character at `at` closes the sentence before it.
fn is_sentence_boundary(text: &str, at: usize, ch: char) -> bool {
    match ch {
        '\n' | '|' | '?' | '!' | '。' => true,
        '.' => !text[at + 1..]
            .chars()
            .next()
            .is_some_and(|next| next.is_ascii_alphanumeric()),
        _ => false,
    }
}

/// Where the marker's own sentence begins: just after the nearest boundary
/// before it, then forward over the horizontal whitespace separating them.
///
/// Newlines are not skipped over: a boundary that is itself a newline leaves
/// `start` at the beginning of the marker's line, which is what the trailing
/// newline rule then reads.
fn sentence_start(text: &str, marker_idx: usize) -> usize {
    let mut start = 0;
    for (i, ch) in text[..marker_idx].char_indices().rev() {
        if is_sentence_boundary(text, i, ch) {
            start = i + ch.len_utf8();
            break;
        }
    }
    let gap = &text[start..marker_idx];
    start + (gap.len() - gap.trim_start_matches([' ', '\t']).len())
}

/// Where the marker's own sentence ends: after the nearest terminator following
/// it, before the nearest clamp, or at the end of the page.
///
/// A terminator is consumed by its own encoded length, never by one byte: `。`
/// is three bytes, and `end += 1` there lands mid-character and panics inside
/// `replace_range` — in the review pipeline, on a real pull request.
fn sentence_end(text: &str, from: usize) -> usize {
    for (rel, ch) in text[from..].char_indices() {
        let at = from + rel;
        match ch {
            // Clamps belong to the surrounding text and stay where they are.
            '\n' | '|' => return at,
            '?' | '!' | '。' => return at + ch.len_utf8(),
            '.' if is_sentence_boundary(text, at, ch) => return at + ch.len_utf8(),
            _ => {}
        }
    }
    text.len()
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

    /// The `include_str!` pins above name one page each, so a decision record
    /// added later is owned by `collect_owned_pages` at runtime while no test
    /// can see it. This walks the same directories the runtime walks.
    #[test]
    fn every_decision_record_is_owned_and_honest() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut checked = 0usize;

        for dir in ADR_DIRS {
            let full = root.join(dir);
            if !full.is_dir() {
                continue;
            }
            for entry in std::fs::read_dir(&full).expect("read decision-record directory") {
                let path = entry.expect("decision-record entry").path();
                if path.extension().and_then(|e| e.to_str()) != Some("md") {
                    continue;
                }
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                let page = std::fs::read_to_string(&path).expect("read decision record");

                assert_published_page_has_no_count_drift(&rel, &page);

                // FrontmatterValidator makes this mandatory under the decision
                // directories, but it only ever sees files in a PR's diff. Absent
                // this check, a record can sit on main for months and fail the
                // next PR that touches it, for a defect that PR did not introduce.
                assert!(
                    page.trim_start().starts_with("---"),
                    "{rel} carries no frontmatter; frontmatter is mandatory for \
                     decision records, so the next PR touching it fails DocGuard"
                );
                checked += 1;
            }
        }

        assert!(
            checked > 0,
            "no decision records found under {ADR_DIRS:?}; the walk is broken, not the corpus"
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
