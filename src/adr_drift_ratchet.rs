//! Living ADR field-schema ratchet.
//!
//! # What this gate measures
//!
//! For every architecture decision record a pull request touches, whether that
//! record declares each field the *repository* requires of a decision record.
//!
//! Two things in that sentence were previously untrue.
//!
//! The field check ran over `diff_ctx.diff_content` -- the whole pull request
//! diff -- rather than over the record. `\bachieves\b`, `\borigin\b`,
//! `\brule\b` and `\bensure\b` are ordinary English, so four of the five
//! mandatory fields were satisfied by prose in any file the change happened to
//! touch. Only `overturn[-_ ]when` was rare enough to ever go red. The check is
//! now scoped to the record, and looks for a *field* -- a key followed by a
//! colon -- rather than for a word, because scoping a word-match to the ADR
//! leaves a paragraph using all five words in a sentence fully compliant.
//!
//! And the five names were a Rust literal. Nygard's original sections are
//! Title/Context/Decision/Status/Consequences; MADR 4.0 requires Context and
//! Problem Statement, Considered Options and Decision Outcome, and makes every
//! frontmatter key optional. `achieves, origin, rule, ensure, overturn_when` is
//! one house convention, and hardcoding it manufactures five accusations per
//! record against any tenant writing plain MADR. ADR-0006 already settles where
//! such a rule lives: "a rule Anvil cannot state generically belongs in the
//! tenant repository, not in the tool." So the list is read from
//! `docs/decisions/adr-schema.json` or `docs/adr/adr-schema.json`, and a
//! repository declaring neither gets `NotMeasured` -- not a pass, and not an
//! accusation.
//!
//! # What it does not do
//!
//! It does not scaffold. An architectural change arriving with no ADR used to
//! be published as `AUTO-SCAFFOLDED (Draft ADR generated ...)` naming
//! `docs/decisions/ADR-{n:04}-pr-{n}.md`, a path built by `format!` from the PR
//! number and written by nothing. `adr-tools` does write `doc/adr/NNNN-slug.md`
//! to disk, which is what the word means; every credible CI-side tool does the
//! opposite -- adrkit's action states outright that it never creates or commits
//! an ADR, and ADR Guard and Structured MADR fail or comment only. The
//! observation behind that branch is real and survives: the changed paths that
//! look architectural are reported by name. The claim to have acted on them
//! does not, and it does not decide the verdict either, because
//! `ends_with("lib.rs")` is a guess about spelling that this repository's own
//! history trips without any decision going unrecorded.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::GateStatus;

/// Where a repository declares the fields its decision records must carry.
/// Checked in order; the first that parses wins.
const SCHEMA_PATHS: &[&str] = &["docs/decisions/adr-schema.json", "docs/adr/adr-schema.json"];

/// Directories whose Markdown files are decision records.
const ADR_DIRS: &[&str] = &["docs/decisions/", "docs/adr/"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdrReport {
    pub status: GateStatus,
    pub is_compliant: bool,
    pub adrs_evaluated: usize,
    /// The field list this run applied, exactly as the repository declared it.
    /// Empty when the repository declared none, which is also when `status` is
    /// `NotMeasured`.
    pub required_fields: Vec<String>,
    /// Changed paths that look architectural and arrived with no decision
    /// record in the same diff. Every entry is a path from `changed_files`.
    /// Recorded, not charged: the predicate is a spelling guess.
    pub architectural_changes_without_adr: Vec<String>,
    pub violations: Vec<String>,
    pub summary: String,
}

pub struct AdrDriftRatchet;

impl Default for AdrDriftRatchet {
    fn default() -> Self {
        Self::new()
    }
}

/// Reduces a field name or a record's key to comparable form, so that
/// `Overturn-When`, `overturn_when`, `### overturn when` and `**Overturn
/// When**` are one field and `Note` is not.
fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The normalized key a line declares, if it declares one.
///
/// Two shapes count, and only two. A key followed by a colon -- `Rule: use
/// Redis`, `### achieves: a`, `**Origin**: b` -- which is how this repository's
/// own `docs/adr/0001` writes them. And a Markdown *heading* whose whole text is
/// the key -- `## Consequences` -- which is how Nygard's format and MADR's
/// required body sections are written, and what Structured MADR's action checks
/// for.
///
/// A line that is neither declares nothing. `This rule achieves parity` is a
/// sentence and `origin` alone on a line is a word, and admitting either is the
/// `\bword\b` match this replaced, arrived at from the other direction.
fn declared_key(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let is_heading = trimmed.starts_with('#');
    let stripped = trimmed.trim_start_matches(|c: char| {
        c.is_whitespace() || c == '#' || c == '*' || c == '-' || c == '_' || c == '>'
    });
    let key = match stripped.split_once(':') {
        Some((key, _)) => key,
        // A heading is a declaration; a bare line of prose is not.
        None if is_heading => stripped,
        None => return None,
    };
    if key.is_empty() || key.len() > 64 {
        return None;
    }
    let n = normalize(key);
    if n.is_empty() { None } else { Some(n) }
}

/// The lines a diff adds to one file, used when the record is not on disk.
fn added_lines_for(diff: &str, path: &str) -> String {
    diff.split("diff --git")
        .filter(|section| {
            section
                .lines()
                .next()
                .is_some_and(|h| h.contains(&format!("b/{path}")))
        })
        .flat_map(|section| {
            section
                .lines()
                .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
                .map(|l| l[1..].to_string())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl AdrDriftRatchet {
    pub fn new() -> Self {
        Self
    }

    /// The field list this repository requires of a decision record, and the
    /// file it came from.
    fn declared_schema(repo_dir: &Path) -> Option<(Vec<String>, &'static str)> {
        SCHEMA_PATHS.iter().find_map(|rel| {
            let raw = std::fs::read_to_string(repo_dir.join(rel)).ok()?;
            let fields: Vec<String> = serde_json::from_str(&raw).ok()?;
            if fields.is_empty() {
                return None;
            }
            Some((fields, *rel))
        })
    }

    /// Validates every decision record a pull request touches against the field
    /// schema the repository declares.
    pub fn evaluate_adr_parity(
        &self,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<AdrReport> {
        info!(
            "Running AdrDriftRatchet (Living Architecture Decision Record Ratchet) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        // A decision record is a Markdown file in a decisions directory. The
        // schema file itself sits there and is not one.
        let adr_files: Vec<&String> = diff_ctx
            .changed_files
            .iter()
            .filter(|f| {
                f.ends_with(".md")
                    && ADR_DIRS.iter().any(|d| f.contains(d))
                    && !SCHEMA_PATHS.iter().any(|s| f.ends_with(s))
            })
            .collect();

        let architectural_changes_without_adr: Vec<String> = if adr_files.is_empty() {
            diff_ctx
                .changed_files
                .iter()
                .filter(|f| {
                    f.contains("/ports/")
                        || f.contains("/adapters/")
                        || f.contains("/facade/")
                        || f.ends_with("lib.rs")
                        || f.ends_with("schema.sql")
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        };

        let Some((required_fields, schema_path)) = Self::declared_schema(repo_dir) else {
            let reason = format!(
                "no ADR field schema is declared: neither {} exists in this repository, so the \
                 fields a decision record must carry are unknown. Next step is a decision, not a \
                 file: this repository runs two conventions at once -- a `## Schema` block \
                 carrying five fields under docs/adr/, and MADR Context/Decision records under \
                 docs/decisions/ -- and whichever wins is written into {} by whoever owns that \
                 convention. Per ADR-0006 that owner is the repository, not this tool",
                SCHEMA_PATHS.join(" nor "),
                SCHEMA_PATHS[0]
            );
            return Ok(AdrReport {
                status: GateStatus::NotMeasured {
                    gate_id: "adr_status".to_string(),
                    reason: reason.clone(),
                },
                is_compliant: true,
                adrs_evaluated: 0,
                required_fields: Vec::new(),
                architectural_changes_without_adr,
                violations: Vec::new(),
                summary: format!("Nothing measured: {reason}."),
            });
        };

        let wanted: Vec<String> = required_fields.iter().map(|f| normalize(f)).collect();
        let mut violations = Vec::new();

        let mut evaluated = 0usize;
        let mut retired = Vec::new();
        for adr_file in &adr_files {
            // The record is the file. An ADR edited by one line still has to
            // carry every field, which only the whole record can show; the
            // added lines are the fallback for a record not on disk. Only
            // `NotFound` takes that fallback: a permissions error used to
            // degrade silently to the hunks.
            let path = repo_dir.join(adr_file);
            let body = match std::fs::read_to_string(&path) {
                Ok(body) => body,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    added_lines_for(&diff_ctx.diff_content, adr_file)
                }
                Err(e) => {
                    return Err(e).context(format!("reading decision record {}", path.display()));
                }
            };

            // Neither on disk nor added by this diff: the change retired the
            // record. Charging five missing fields against a deletion blocked
            // any pull request superseding a decision.
            if body.trim().is_empty() {
                retired.push((*adr_file).clone());
                continue;
            }
            evaluated += 1;

            let declared: Vec<String> = body.lines().filter_map(declared_key).collect();

            for (field, want) in required_fields.iter().zip(&wanted) {
                if !declared.iter().any(|d| d == want) {
                    violations.push(format!(
                        "ADR `{adr_file}` does not declare the required field `{field}`"
                    ));
                }
            }
        }

        let is_compliant = violations.is_empty();
        let retired_note = if retired.is_empty() {
            String::new()
        } else {
            format!(
                " {} record(s) this diff deletes were not evaluated: {}.",
                retired.len(),
                retired.join(", ")
            )
        };
        let observed = if architectural_changes_without_adr.is_empty() {
            String::new()
        } else {
            format!(
                " {} architectural file(s) changed with no decision record in this diff: {}. \
                 Recorded, not charged: no decision record was generated or written for them.",
                architectural_changes_without_adr.len(),
                architectural_changes_without_adr.join(", ")
            )
        };

        let summary = if is_compliant {
            format!(
                "{} ADR(s) checked against the {} field(s) declared in {}; every field \
                 present.{}{}",
                evaluated,
                required_fields.len(),
                schema_path,
                retired_note,
                observed
            )
        } else {
            format!(
                "{} ADR field schema violation(s) against {}: {}{}",
                violations.len(),
                schema_path,
                violations.join("; "),
                retired_note
            )
        };

        let status = if is_compliant {
            GateStatus::Passed
        } else {
            GateStatus::Failed(summary.clone())
        };

        Ok(AdrReport {
            status,
            is_compliant,
            adrs_evaluated: evaluated,
            required_fields,
            architectural_changes_without_adr,
            violations,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heading_declares_its_own_text_and_a_bare_word_declares_nothing() {
        assert_eq!(
            declared_key("## Consequences").as_deref(),
            Some("consequences")
        );
        assert_eq!(
            declared_key("### Overturn-When").as_deref(),
            Some("overturnwhen")
        );
        assert_eq!(declared_key("origin"), None);
        assert_eq!(declared_key("  ensure  "), None);
        assert_eq!(declared_key("- rule"), None);
    }

    #[test]
    fn a_field_is_a_key_and_a_colon_not_a_word() {
        assert_eq!(
            declared_key("Overturn-When: x").as_deref(),
            Some("overturnwhen")
        );
        assert_eq!(declared_key("### achieves: x").as_deref(), Some("achieves"));
        assert_eq!(declared_key("**Origin**: x").as_deref(), Some("origin"));
        assert_eq!(declared_key("- Rule: x").as_deref(), Some("rule"));
        assert_eq!(declared_key("This rule achieves parity"), None);
        assert_eq!(
            declared_key("1. **Toolchain is pinned**: x").as_deref(),
            Some("1toolchainispinned")
        );
    }
}
