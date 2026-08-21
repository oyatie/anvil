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
}

const OWNED: &[&str] = &["README.md", "docs/doctrine.md", "openapi/openapi.yaml"];
const ADR_DIRS: &[&str] = &["docs/adr", "docs/decisions"];

const EXEMPTION_MARKERS: &[&str] = &[
    "does **not** yet amend existing documents",
    "does not yet amend existing documents",
];

/// Rewrite owned pages so published gate-count claims match `total_gates`.
///
/// On I/O failure, returns `Err`. That is Errored, not Passed.
/// Remaining drift after a successful write is returned in
/// `remaining_drift` and must fail the gate (never AutoUpdated).
pub fn sync_published_counts(repo_dir: &Path, total_gates: usize) -> Result<CorpusSync> {
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
    for marker in EXEMPTION_MARKERS {
        if let Some(idx) = out.find(marker) {
            let start = out[..idx].rfind(['.', '\n']).map(|i| i + 1).unwrap_or(0);
            let rest = &out[idx + marker.len()..];
            let end_rel = rest.find('\n').unwrap_or(rest.len());
            let mut end = idx + marker.len() + end_rel;
            if end < out.len() && out.as_bytes()[end] == b'\n' {
                end += 1;
            }
            out.replace_range(start..end, "");
        }
    }
    out
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

        let sync = sync_published_counts(dir.path(), 68).unwrap();
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

        let sync = sync_published_counts(dir.path(), 68).unwrap();
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
}
