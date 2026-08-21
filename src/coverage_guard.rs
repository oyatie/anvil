//! Differential coverage of the lines this PR added.
//!
//! # What this gate used to do
//!
//! It ran no coverage tool. It counted added lines in test files against added
//! lines in source files, scaled by a constant factor, floored the result at the
//! threshold, and then compared the floored value against that same threshold.
//! The comparison was therefore true for every input: the gate could not fail.
//! The floor and the factor are both gone; nothing here manufactures a number.
//!
//! # What it does now
//!
//! `cargo llvm-cov --lcov` runs through `crate::exec::run_bounded` at
//! `ExecClass::Build`, and its LCOV report is intersected with the lines the
//! diff added, addressed by absolute new-file line number.
//!
//! Three outcomes, and only three:
//!
//! - `Measured` — the diff added coverable source lines, and the report has an
//!   executable record for at least one of them. The denominator is the added
//!   lines the tool reports as executable; blank lines, braces and `use`
//!   statements carry no record and are neither covered nor uncovered.
//! - `NothingToMeasure` — the diff added no coverable source lines at all
//!   (documentation, configuration, test-only changes). A true statement.
//! - `NotMeasured` — the tool is absent, was killed by the build-class timeout,
//!   exited non-zero, emitted something that is not an LCOV report, or has no
//!   executable record for any line this PR added. Absent evidence: never a
//!   pass (I1), never a percentage that was not measured (I2), and never an
//!   accusation against the PR.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::info;

use crate::exec::{ExecClass, run_bounded};
use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::GateStatus;
use std::collections::{BTreeMap, BTreeSet};

pub const MIN_COVERAGE_THRESHOLD_PERCENT: f64 = 85.0;

/// The field name this gate occupies on `PreMergeCertificationReport`.
///
/// `NotMeasured` is recorded under this id, so `unmeasured_gates` names a gate
/// a reader can actually find.
pub const COVERAGE_GATE_ID: &str = "coverage_status";

/// Repo-relative path (as it appears after `+++ b/`) -> new-file line numbers
/// this PR added. Line numbers are absolute in the head revision, derived from
/// the `@@` hunk headers, because coverage data is addressed by line number.
pub type AddedLines = BTreeMap<String, BTreeSet<u32>>;

/// Repo-relative path -> line number -> execution count, as reported by the
/// coverage tool.
pub type FileLineHits = BTreeMap<String, BTreeMap<u32, u64>>;

/// What came back from the coverage tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverageToolOutcome {
    /// The tool ran to completion and emitted an LCOV report.
    Lcov(String),
    /// No measurement exists: `cargo-llvm-cov` absent, spawn failure, the
    /// build-class timeout, or a non-zero exit. Carries the reason verbatim so
    /// it can be published instead of a number (I1, I2).
    Unavailable(String),
}

/// The result of attempting to measure coverage of the lines this PR added.
///
/// There is deliberately no variant that carries a percentage without having
/// measured one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CoverageMeasurement {
    Measured {
        percent: f64,
        covered_added_lines: usize,
        /// Denominator: added lines the coverage tool reports as executable.
        measured_added_lines: usize,
    },
    /// The PR added no executable lines, so there is nothing to cover. A true
    /// statement, not a fabricated pass.
    NothingToMeasure,
    /// Absent evidence. Never a pass, never an accusation.
    NotMeasured { reason: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageFinding {
    pub file_path: String,
    pub unasserted_functions: Vec<String>,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageReport {
    /// Whether this report permits certification. False for a deficit AND for
    /// absent evidence -- an unmeasured gate must not certify.
    pub is_sufficient: bool,
    /// Legacy projection of `measurement`, retained only because callers this
    /// lane does not own still read it. `f64::NAN` when nothing was measured,
    /// so no caller can format a number that was never taken (I2).
    pub estimated_diff_coverage_percent: f64,
    /// Added lines in coverable source files, counted from the diff.
    pub executable_lines_added: usize,
    /// Added lines in test files, counted from the diff. Reported because it is
    /// observable; it takes no part in the coverage arithmetic.
    pub test_lines_added: usize,
    pub findings: Vec<CoverageFinding>,
    pub summary: String,
    /// What was actually measured, if anything.
    pub measurement: CoverageMeasurement,
}

impl CoverageReport {
    /// The measured percentage, or `None` when nothing was measured.
    pub fn measured_percent(&self) -> Option<f64> {
        match &self.measurement {
            CoverageMeasurement::Measured { percent, .. } => Some(*percent),
            CoverageMeasurement::NothingToMeasure | CoverageMeasurement::NotMeasured { .. } => None,
        }
    }

    /// The gate status this report is entitled to publish.
    ///
    /// Absent evidence becomes `GateStatus::NotMeasured` under this gate's own
    /// id, which blocks merge-queue admission via `unmeasured_gates` while
    /// making no claim against the PR. It is deliberately not `Warning` (which
    /// is acceptable and untracked, so nothing would block) and not `Errored`
    /// (which attributes the fault to the PR).
    pub fn gate_status(&self) -> GateStatus {
        match &self.measurement {
            CoverageMeasurement::NotMeasured { reason } => GateStatus::NotMeasured {
                gate_id: COVERAGE_GATE_ID.to_string(),
                reason: reason.clone(),
            },
            CoverageMeasurement::NothingToMeasure => GateStatus::Passed,
            CoverageMeasurement::Measured { .. } => {
                if self.is_sufficient {
                    GateStatus::Passed
                } else {
                    GateStatus::Failed(self.summary.clone())
                }
            }
        }
    }
}

impl CoverageReport {
    fn not_measured(
        reason: String,
        executable_lines_added: usize,
        test_lines_added: usize,
    ) -> Self {
        CoverageReport {
            is_sufficient: false,
            estimated_diff_coverage_percent: f64::NAN,
            executable_lines_added,
            test_lines_added,
            findings: Vec::new(),
            summary: format!("Differential coverage not measured: {}", reason.trim()),
            measurement: CoverageMeasurement::NotMeasured {
                reason: reason.trim().to_string(),
            },
        }
    }

    fn nothing_to_measure(test_lines_added: usize) -> Self {
        CoverageReport {
            is_sufficient: true,
            estimated_diff_coverage_percent: f64::NAN,
            executable_lines_added: 0,
            test_lines_added,
            findings: Vec::new(),
            summary: "Differential coverage: this PR adds no coverable source lines, so there \
                      is nothing to cover."
                .to_string(),
            measurement: CoverageMeasurement::NothingToMeasure,
        }
    }
}

pub struct CoverageGuard;

impl Default for CoverageGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl CoverageGuard {
    pub fn new() -> Self {
        Self
    }

    /// Legacy synchronous entry point, retained only so the call sites this
    /// lane does not own keep compiling.
    ///
    /// A coverage tool cannot be run from a synchronous function, so this
    /// reports absent evidence rather than the number the removed heuristic
    /// used to invent. The real entry point is `measure_diff_coverage`.
    ///
    /// Not marked `#[deprecated]`: CI runs `clippy -- -D warnings`, so the
    /// attribute would break the two call sites this lane does not own. The
    /// mechanism that actually blocks a false green is `gate_status()`
    /// returning `NotMeasured`, which is enforced by test, not by attribute.
    pub fn evaluate_diff_coverage(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<CoverageReport> {
        let added = added_lines_by_file(&diff_ctx.diff_content);
        let (coverable, test_lines_added) = partition_added_lines(&added);
        let executable_lines_added: usize = coverable.values().map(|s| s.len()).sum();
        if executable_lines_added == 0 {
            return Ok(CoverageReport::nothing_to_measure(test_lines_added));
        }
        Ok(CoverageReport::not_measured(
            "the synchronous entry point cannot run a coverage tool; call \
             CoverageGuard::measure_diff_coverage"
                .to_string(),
            executable_lines_added,
            test_lines_added,
        ))
    }

    /// Runs the coverage tool in `repo_dir` and returns its raw report, or the
    /// reason there is none.
    ///
    /// Every failure mode -- no manifest, `cargo-llvm-cov` not installed, spawn
    /// failure, the build-class timeout, a non-zero exit -- becomes
    /// `Unavailable` carrying the reason verbatim. None of them can become a
    /// report (I1).
    pub async fn run_llvm_cov(&self, repo_dir: &Path) -> CoverageToolOutcome {
        let manifest = repo_dir.join("Cargo.toml");
        if !manifest.is_file() {
            return CoverageToolOutcome::Unavailable(format!(
                "no Cargo.toml at {}, so no coverage run is possible",
                manifest.display()
            ));
        }

        let mut cmd = Command::new("cargo");
        cmd.current_dir(repo_dir)
            .arg("llvm-cov")
            .arg("--workspace")
            .arg("--all-targets")
            .arg("--lcov")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        match run_bounded(cmd, ExecClass::Build, "cargo llvm-cov").await {
            Ok(out) if out.status.success() => {
                CoverageToolOutcome::Lcov(String::from_utf8_lossy(&out.stdout).into_owned())
            }
            Ok(out) => CoverageToolOutcome::Unavailable(format!(
                "cargo llvm-cov exited with status {}: {}",
                out.status,
                tail(&String::from_utf8_lossy(&out.stderr))
            )),
            Err(e) => CoverageToolOutcome::Unavailable(e.to_string()),
        }
    }

    /// Pure: turns a tool outcome plus the PR diff into a report. No I/O, so
    /// every threshold and every absent-evidence path is testable without a
    /// toolchain.
    pub fn report_from_outcome(
        &self,
        outcome: CoverageToolOutcome,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> CoverageReport {
        let added = added_lines_by_file(&diff_ctx.diff_content);
        let (coverable, test_lines_added) = partition_added_lines(&added);
        let executable_lines_added: usize = coverable.values().map(|s| s.len()).sum();

        // Decided before the tool output is consulted: a PR that adds no
        // coverable source line has nothing to cover whatever the report says,
        // and blocking it would be a false red on every docs-only change.
        if executable_lines_added == 0 {
            return CoverageReport::nothing_to_measure(test_lines_added);
        }

        let raw = match outcome {
            CoverageToolOutcome::Lcov(raw) => raw,
            CoverageToolOutcome::Unavailable(reason) => {
                return CoverageReport::not_measured(
                    reason,
                    executable_lines_added,
                    test_lines_added,
                );
            }
        };

        let hits = match parse_lcov(&raw) {
            Ok(h) => normalize_hit_paths(h, repo_dir),
            Err(e) => {
                return CoverageReport::not_measured(
                    format!("the coverage report could not be parsed: {e}"),
                    executable_lines_added,
                    test_lines_added,
                );
            }
        };

        let mut covered_added_lines = 0usize;
        let mut measured_added_lines = 0usize;
        let mut findings = Vec::new();

        for (path, lines) in &coverable {
            let file_hits = hits.get(path);
            let mut uncovered: Vec<u32> = Vec::new();
            for line in lines {
                // No record means the tool does not consider this line
                // executable -- a blank line, a brace, a `use`. It is neither
                // covered nor uncovered, so it stays out of the denominator.
                let Some(count) = file_hits.and_then(|f| f.get(line)) else {
                    continue;
                };
                measured_added_lines += 1;
                if *count > 0 {
                    covered_added_lines += 1;
                } else {
                    uncovered.push(*line);
                }
            }
            if !uncovered.is_empty() {
                findings.push(CoverageFinding {
                    file_path: path.clone(),
                    unasserted_functions: Vec::new(),
                    recommendation: format!(
                        "Added lines never executed by the test suite: {}. Add tests that \
                         exercise them.",
                        render_line_list(&uncovered)
                    ),
                });
            }
        }

        if measured_added_lines == 0 {
            return CoverageReport::not_measured(
                format!(
                    "the coverage report contains no executable record for any line this PR \
                     added to {}",
                    render_path_list(coverable.keys())
                ),
                executable_lines_added,
                test_lines_added,
            );
        }

        // Numerator first, so the division is over integers scaled once: 849
        // covered of 1000 measured is exactly the f64 84.9, and the boundary
        // test depends on that rather than on rounding luck.
        let percent = (covered_added_lines as f64) * 100.0 / (measured_added_lines as f64);
        let is_sufficient = percent >= MIN_COVERAGE_THRESHOLD_PERCENT;

        let summary = if is_sufficient {
            format!(
                "Differential coverage: {:.1}% of the added lines llvm-cov reports as executable \
                 were executed ({} of {}).",
                percent, covered_added_lines, measured_added_lines
            )
        } else {
            format!(
                "Differential coverage deficit: {:.1}% of added executable lines were executed \
                 ({} of {}), below the required {:.0}%.",
                percent, covered_added_lines, measured_added_lines, MIN_COVERAGE_THRESHOLD_PERCENT
            )
        };

        CoverageReport {
            is_sufficient,
            estimated_diff_coverage_percent: percent,
            executable_lines_added,
            test_lines_added,
            findings,
            summary,
            measurement: CoverageMeasurement::Measured {
                percent,
                covered_added_lines,
                measured_added_lines,
            },
        }
    }

    /// The real gate entry point: measure, then report.
    pub async fn measure_diff_coverage(
        &self,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<CoverageReport> {
        info!(
            "Running CoverageGuard (differential coverage >= {}%) on {}#{}",
            MIN_COVERAGE_THRESHOLD_PERCENT, diff_ctx.repo, diff_ctx.pr_number
        );
        let outcome = self.run_llvm_cov(repo_dir).await;
        Ok(self.report_from_outcome(outcome, repo_dir, diff_ctx))
    }
}

/// Extracts the lines this PR added, by file, with absolute new-file line
/// numbers taken from the `@@` hunk headers.
///
/// Coverage data is addressed by line number in the head revision, so an offset
/// within the diff is useless here: the numbers must be the ones llvm-cov will
/// report.
pub fn added_lines_by_file(diff_content: &str) -> AddedLines {
    let mut out = AddedLines::new();
    let mut current: Option<String> = None;
    let mut next_line: u32 = 0;
    let mut in_hunk = false;

    for line in diff_content.lines() {
        if line.starts_with("diff --git ") {
            current = None;
            in_hunk = false;
            continue;
        }
        if line.starts_with("--- ") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("+++ ") {
            let path = rest.split('\t').next().unwrap_or(rest).trim();
            current = if path == "/dev/null" {
                None
            } else {
                Some(path.strip_prefix("b/").unwrap_or(path).to_string())
            };
            in_hunk = false;
            continue;
        }
        if line.starts_with("@@") {
            match parse_hunk_new_start(line) {
                Some(start) => {
                    next_line = start;
                    in_hunk = true;
                }
                None => in_hunk = false,
            }
            continue;
        }
        if !in_hunk {
            continue;
        }
        let Some(file) = current.as_ref() else {
            continue;
        };
        match line.as_bytes().first() {
            Some(b'+') => {
                out.entry(file.clone()).or_default().insert(next_line);
                next_line += 1;
            }
            Some(b'-') => {}
            Some(b'\\') => {}
            // Context lines advance the new-file cursor. An empty line in a
            // unified diff is a context line whose single space was stripped.
            _ => next_line += 1,
        }
    }
    out
}

/// `@@ -10,2 +12,4 @@` -> 12. `None` when the header is not a hunk header.
fn parse_hunk_new_start(header: &str) -> Option<u32> {
    let plus = header.split('+').nth(1)?;
    let digits: String = plus.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

/// Splits the added lines into the coverable source files and a count of added
/// test-file lines.
///
/// Added test lines are counted and reported, never divided by: a PR that adds
/// a thousand lines of test file and one unexecuted production line is at 0%,
/// which is the whole point.
fn partition_added_lines(added: &AddedLines) -> (AddedLines, usize) {
    let mut coverable = AddedLines::new();
    let mut test_lines = 0usize;
    for (path, lines) in added {
        if lines.is_empty() {
            continue;
        }
        if is_test_path(path) {
            test_lines += lines.len();
        } else if is_coverable_source(path) {
            coverable.insert(path.clone(), lines.clone());
        }
    }
    (coverable, test_lines)
}

/// Whether a coverage tool could plausibly report on this file.
///
/// Extension-based on purpose: a Markdown or TOML change has no executable
/// lines, so demanding coverage of it is a false red, and a gate that cannot be
/// satisfied gets bypassed.
fn is_coverable_source(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "rs" | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "go"
            | "py"
            | "c"
            | "cc"
            | "cpp"
            | "cxx"
            | "h"
            | "hh"
            | "hpp"
            | "java"
            | "kt"
            | "rb"
            | "cs"
            | "swift"
            | "scala"
    ) && path.contains('.')
}

/// Whether this path is test code.
///
/// Matched on whole path components and on the file name, never as a bare
/// substring: `src/latest_state.rs` contains the letters `test` and is not a
/// test file, and silently dropping it from the denominator would be a hole
/// wide enough to drive a PR through.
fn is_test_path(path: &str) -> bool {
    for part in path.split('/') {
        if matches!(
            part,
            "tests" | "test" | "__tests__" | "spec" | "specs" | "testdata"
        ) {
            return true;
        }
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.split('.').next().unwrap_or(name);
    stem.starts_with("test_")
        || stem.ends_with("_test")
        || stem.ends_with("_tests")
        || stem.ends_with("_spec")
        || name.contains(".test.")
        || name.contains(".spec.")
}

/// Parses an LCOV report into per-file, per-line execution counts.
///
/// Errors when the payload is not an LCOV report, so unparseable output can be
/// reported as absent evidence rather than as zero coverage. An empty map is
/// never returned from a successful parse: it would read as 0% or as 100%
/// depending on which side of the division it landed on, and both are lies.
pub fn parse_lcov(raw: &str) -> Result<FileLineHits> {
    let mut out = FileLineHits::new();
    let mut current: Option<String> = None;
    let mut saw_sf = false;
    let mut saw_da = false;

    for line in raw.lines() {
        let line = line.trim();
        if let Some(path) = line.strip_prefix("SF:") {
            saw_sf = true;
            let path = path.trim();
            if path.is_empty() {
                current = None;
            } else {
                out.entry(path.to_string()).or_default();
                current = Some(path.to_string());
            }
            continue;
        }
        if line == "end_of_record" {
            current = None;
            continue;
        }
        if let Some(rec) = line.strip_prefix("DA:") {
            let Some(file) = current.as_ref() else {
                bail!("DA record outside any SF record");
            };
            let mut parts = rec.split(',');
            let (Some(num), Some(count)) = (parts.next(), parts.next()) else {
                bail!("malformed DA record: {line}");
            };
            let num: u32 = num
                .trim()
                .parse()
                .map_err(|_| anyhow::anyhow!("malformed DA line number: {line}"))?;
            // Counts are emitted as integers; a saturating parse would let a
            // corrupt payload read as "not executed".
            let count: u64 = count
                .trim()
                .parse()
                .map_err(|_| anyhow::anyhow!("malformed DA hit count: {line}"))?;
            saw_da = true;
            out.entry(file.clone()).or_default().insert(num, count);
        }
    }

    if !saw_sf {
        bail!("no SF record: this is not an LCOV report");
    }
    if !saw_da {
        bail!("no DA record: the LCOV report carries no line data");
    }
    Ok(out)
}

/// Rewrites LCOV source paths to the repo-relative form the diff uses.
///
/// llvm-cov emits absolute paths. The diff is addressed relative to the repo
/// root. Without this the intersection is empty on every PR, which reads either
/// as absent evidence forever or, worse, as full coverage.
fn normalize_hit_paths(hits: FileLineHits, repo_dir: &Path) -> FileLineHits {
    let mut out = FileLineHits::new();
    for (path, lines) in hits {
        let rel = Path::new(&path)
            .strip_prefix(repo_dir)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.clone());
        let rel = rel.trim_start_matches("./").to_string();
        out.entry(rel).or_default().extend(lines);
    }
    out
}

/// Renders at most a readable number of line numbers, so a wholly untested file
/// does not publish a wall of digits.
fn render_line_list(lines: &[u32]) -> String {
    const MAX: usize = 40;
    let shown: Vec<String> = lines.iter().take(MAX).map(|l| l.to_string()).collect();
    if lines.len() > MAX {
        format!(
            "{} and {} further lines",
            shown.join(", "),
            lines.len() - MAX
        )
    } else {
        shown.join(", ")
    }
}

fn render_path_list<'a, I: Iterator<Item = &'a String>>(paths: I) -> String {
    let v: Vec<&str> = paths.map(|s| s.as_str()).collect();
    v.join(", ")
}

/// Last few lines of a tool's stderr, so the published reason is the tool's own
/// words rather than a paraphrase.
fn tail(s: &str) -> String {
    let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(4);
    lines[start..].join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    // =====================================================================
    // Real-measurement suite. Written before the implementation exists.
    //
    // Premortem: this gate has already failed in production. The ways it did:
    //   P1  the tool never ran and the gate passed anyway (the current bug);
    //   P2  the tool ran but its output was addressed by absolute path while
    //       the diff is addressed by repo-relative path, so nothing matched
    //       and the gate either passed on an empty intersection or blocked
    //       every legitimate PR;
    //   P3  a PR added a wall of test code and zero real assertions, and the
    //       line-ratio heuristic minted a pass out of it;
    //   P4  cargo-llvm-cov was missing on the runner, or the build-class
    //       timeout killed it, and the gate reported the last number it knew;
    //   P5  llvm-cov emitted an error page / partial payload and the parser
    //       silently produced an empty map, which reads as 100% or as 0%;
    //   P6  coverage of the whole workspace was reported instead of coverage
    //       of the added lines, so an untested new module rode in on the
    //       existing suite's coverage;
    //   P7  the threshold comparison was floored, so 85 >= 85 always held.
    // =====================================================================

    fn ctx(diff: &str, changed: &[&str]) -> PrDiffContext {
        PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 4242,
            base_branch: "main".to_string(),
            base_sha: "base".to_string(),
            head_sha: "head".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp/anvil-cov"),
            diff_content: diff.to_string(),
            changed_files: changed.iter().map(|s| s.to_string()).collect(),
            is_incremental: false,
        }
    }

    /// A unified diff that adds `count` executable lines to `path`, starting at
    /// new-file line `start`.
    fn diff_adding(path: &str, start: u32, count: u32) -> String {
        let mut s = format!(
            "diff --git a/{p} b/{p}\n--- a/{p}\n+++ b/{p}\n@@ -{s},0 +{s},{c} @@\n",
            p = path,
            s = start,
            c = count
        );
        for i in 0..count {
            s.push_str(&format!("+    let v{i} = compute({i});\n"));
        }
        s
    }

    /// An LCOV record for `path` where lines `start..start+count` are
    /// executable and the first `covered` of them were executed.
    fn lcov_for(path: &str, start: u32, count: u32, covered: u32) -> String {
        let mut s = format!("TN:\nSF:{path}\n");
        for i in 0..count {
            let hits = if i < covered { 3 } else { 0 };
            s.push_str(&format!("DA:{},{}\n", start + i, hits));
        }
        s.push_str(&format!("LF:{count}\nLH:{covered}\nend_of_record\n"));
        s
    }

    fn repo() -> std::path::PathBuf {
        std::path::PathBuf::from("/tmp/anvil-cov")
    }

    /// Absent evidence must reach the certification report as
    /// `GateStatus::NotMeasured` under this gate's own id.
    ///
    /// Asserting only "not Passed" is not enough, and the gap is not academic:
    /// `Warning(_)` is `is_acceptable()` AND has no `unmeasured_gate_id()`, so
    /// a gate that downgraded absent evidence to a warning would be admitted
    /// to the merge queue having measured nothing -- the exact I1 hole
    /// `unmeasured_gates` exists to close. `Errored(_)` is the symmetric
    /// mistake: it blocks certification, but as a fault attributed to the PR.
    /// The gate id must be the field name on `PreMergeCertificationReport`
    /// (`coverage_status`), or the sweep records a gate nobody can find.
    fn assert_absent_evidence(status: &GateStatus, ctx: &str) {
        match status {
            GateStatus::NotMeasured { gate_id, reason } => {
                assert_eq!(
                    gate_id, "coverage_status",
                    "{ctx}: must block admission under this gate's own id"
                );
                assert!(
                    !reason.trim().is_empty(),
                    "{ctx}: absent evidence must carry the reason it is absent (I2)"
                );
            }
            other => {
                panic!("{ctx}: absent evidence must be GateStatus::NotMeasured, got {other:?}")
            }
        }
        assert_eq!(
            status.unmeasured_gate_id(),
            Some("coverage_status"),
            "{ctx}"
        );
        assert!(!status.is_measured(), "{ctx}");
    }

    /// Percentages are compared with a tolerance: 849/1000 is 84.9% but is not
    /// bit-identical to the literal 84.9 under every valid order of operations,
    /// and the gate's correctness does not depend on which one is chosen.
    fn assert_pct(actual: Option<f64>, expected: f64, ctx: &str) {
        match actual {
            Some(p) => assert!(
                (p - expected).abs() < 1e-9,
                "{ctx}: expected {expected}%, got {p}%"
            ),
            None => panic!("{ctx}: expected a measured {expected}%, but nothing was measured"),
        }
    }

    // ------------------------------------------------------------------
    // 1. red -> green: the measurement itself
    // ------------------------------------------------------------------

    #[test]
    fn test_added_lines_are_extracted_with_absolute_new_file_line_numbers() {
        let diff = "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n\
                    @@ -10,2 +12,4 @@ fn existing()\n \
                    context_line();\n\
                    +    let x = 1;\n\
                    +    let y = 2;\n\
                    -    removed();\n \
                    trailing();\n";
        let added = added_lines_by_file(diff);
        let lines = added
            .get("src/a.rs")
            .expect("src/a.rs must appear in the added-line map");
        assert_eq!(
            lines,
            &[13u32, 14u32].into_iter().collect::<BTreeSet<u32>>(),
            "added lines must be absolute new-file line numbers taken from the @@ header, \
             not diff offsets"
        );
    }

    #[test]
    fn test_lcov_report_parses_into_per_line_hit_counts() {
        let raw = "TN:\nSF:/w/src/a.rs\nDA:12,4\nDA:13,0\nLF:2\nLH:1\nend_of_record\n";
        let parsed = parse_lcov(raw).expect("valid lcov must parse");
        let file = parsed
            .get("/w/src/a.rs")
            .expect("SF record must become a map key");
        assert_eq!(file.get(&12), Some(&4u64));
        assert_eq!(file.get(&13), Some(&0u64));
    }

    #[test]
    fn test_diff_coverage_is_computed_only_over_lines_this_pr_added() {
        // P6: pre-existing, fully covered lines must not lift the diff figure.
        let diff = diff_adding("src/calc.rs", 100, 4);
        let mut lcov = lcov_for("src/calc.rs", 100, 4, 2);
        lcov = lcov.replace(
            "end_of_record\n",
            "DA:1,999\nDA:2,999\nDA:3,999\nend_of_record\n",
        );
        let report = CoverageGuard::new().report_from_outcome(
            CoverageToolOutcome::Lcov(lcov),
            &repo(),
            &ctx(&diff, &["src/calc.rs"]),
        );
        assert_pct(
            report.measured_percent(),
            50.0,
            "2 of 4 added lines executed, regardless of coverage elsewhere in the file",
        );
        assert!(
            matches!(
                report.measurement,
                CoverageMeasurement::Measured {
                    covered_added_lines: 2,
                    measured_added_lines: 4,
                    ..
                }
            ),
            "the denominator is the 4 added lines, not the 7 lines the file has \
             coverage data for: {:?}",
            report.measurement
        );
        assert!(!report.is_sufficient);
    }

    #[test]
    fn test_uncovered_added_lines_are_named_in_findings() {
        let diff = diff_adding("src/calc.rs", 100, 4);
        let report = CoverageGuard::new().report_from_outcome(
            CoverageToolOutcome::Lcov(lcov_for("src/calc.rs", 100, 4, 2)),
            &repo(),
            &ctx(&diff, &["src/calc.rs"]),
        );
        let f = report
            .findings
            .iter()
            .find(|f| f.file_path.contains("src/calc.rs"))
            .expect("the file with uncovered added lines must be named");
        assert!(
            f.recommendation.contains("102") && f.recommendation.contains("103"),
            "the finding must cite the uncovered added line numbers, got: {}",
            f.recommendation
        );
        // Without this, echoing every added line back -- which requires no
        // coverage data at all -- would satisfy the assertion above, and the
        // published output would accuse two covered lines (I1: never a
        // fabricated accusation).
        let cited: Vec<&str> = f
            .recommendation
            .split(|c: char| !c.is_ascii_digit())
            .filter(|t| !t.is_empty())
            .collect();
        assert!(
            !cited.contains(&"100") && !cited.contains(&"101"),
            "lines 100 and 101 were executed and must not be cited as uncovered, got: {}",
            f.recommendation
        );
    }

    // ------------------------------------------------------------------
    // 2. FALSE-GREEN PREVENTION
    // ------------------------------------------------------------------

    #[test]
    fn test_false_green_prevention_added_lines_never_executed_must_fail() {
        let diff = diff_adding("src/calc.rs", 1, 20);
        let report = CoverageGuard::new().report_from_outcome(
            CoverageToolOutcome::Lcov(lcov_for("src/calc.rs", 1, 20, 0)),
            &repo(),
            &ctx(&diff, &["src/calc.rs"]),
        );
        assert_pct(
            report.measured_percent(),
            0.0,
            "Expected False Green prevention: 20 added lines with zero executions",
        );
        assert!(
            !report.is_sufficient,
            "Expected False Green prevention: wholly unexecuted added code must FAIL"
        );
        assert!(matches!(report.gate_status(), GateStatus::Failed(_)));
    }

    #[test]
    fn test_false_green_prevention_many_test_lines_do_not_manufacture_coverage() {
        // P3: the exact shape the old line-ratio heuristic scored at 100%.
        let mut diff = diff_adding("src/calc.rs", 1, 10);
        diff.push_str(&diff_adding("tests/calc_test.rs", 1, 100));
        let report = CoverageGuard::new().report_from_outcome(
            CoverageToolOutcome::Lcov(lcov_for("src/calc.rs", 1, 10, 0)),
            &repo(),
            &ctx(&diff, &["src/calc.rs", "tests/calc_test.rs"]),
        );
        assert!(
            !report.is_sufficient,
            "Expected False Green prevention: 100 added test lines must not cover 10 unexecuted \
             production lines"
        );
        assert_pct(report.measured_percent(), 0.0, "no added line executed");
        assert!(
            matches!(
                report.measurement,
                CoverageMeasurement::Measured {
                    measured_added_lines: 10,
                    ..
                }
            ),
            "the denominator is added production lines, not added test lines: {:?}",
            report.measurement
        );
    }

    #[test]
    fn test_false_green_prevention_missing_tool_is_not_a_pass() {
        // P1/P4
        let diff = diff_adding("src/calc.rs", 1, 10);
        let report = CoverageGuard::new().report_from_outcome(
            CoverageToolOutcome::Unavailable("cargo-llvm-cov is not installed".to_string()),
            &repo(),
            &ctx(&diff, &["src/calc.rs"]),
        );
        assert!(
            matches!(report.measurement, CoverageMeasurement::NotMeasured { .. }),
            "Expected False Green prevention: a missing tool is absent evidence, got {:?}",
            report.measurement
        );
        assert!(
            !matches!(report.gate_status(), GateStatus::Passed),
            "Expected False Green prevention: absent evidence must never be Passed"
        );
        assert_absent_evidence(&report.gate_status(), "missing tool");
        assert_eq!(
            report.measured_percent(),
            None,
            "Expected False Green prevention: no measurement means no number (I2)"
        );
    }

    #[test]
    fn test_false_green_prevention_timeout_is_not_a_pass() {
        // P4: run_bounded reports a timeout as an error, never a partial Output.
        let diff = diff_adding("src/calc.rs", 1, 10);
        let report = CoverageGuard::new().report_from_outcome(
            CoverageToolOutcome::Unavailable(
                "cargo llvm-cov timed out after 1800s (build class)".to_string(),
            ),
            &repo(),
            &ctx(&diff, &["src/calc.rs"]),
        );
        match &report.measurement {
            CoverageMeasurement::NotMeasured { reason } => {
                assert!(
                    reason.contains("timed out"),
                    "the published reason must state the timeout verbatim, got: {reason}"
                );
            }
            other => panic!(
                "Expected False Green prevention: a killed run must be NotMeasured, got {other:?}"
            ),
        }
        assert!(!matches!(report.gate_status(), GateStatus::Passed));
        assert_absent_evidence(&report.gate_status(), "build-class timeout");
    }

    #[test]
    fn test_false_green_prevention_unparseable_output_is_not_a_pass() {
        // P5
        let diff = diff_adding("src/calc.rs", 1, 10);
        let report = CoverageGuard::new().report_from_outcome(
            CoverageToolOutcome::Lcov("<html><body>502 Bad Gateway</body></html>".to_string()),
            &repo(),
            &ctx(&diff, &["src/calc.rs"]),
        );
        assert!(
            matches!(report.measurement, CoverageMeasurement::NotMeasured { .. }),
            "Expected False Green prevention: unparseable output is absent evidence, got {:?}",
            report.measurement
        );
        assert!(!matches!(report.gate_status(), GateStatus::Passed));
        assert_absent_evidence(&report.gate_status(), "unparseable payload");
    }

    #[test]
    fn test_false_green_prevention_coverage_for_unrelated_files_is_not_a_pass() {
        // P2: an empty intersection is not full coverage.
        let diff = diff_adding("src/calc.rs", 1, 10);
        let report = CoverageGuard::new().report_from_outcome(
            CoverageToolOutcome::Lcov(lcov_for("src/unrelated.rs", 1, 10, 10)),
            &repo(),
            &ctx(&diff, &["src/calc.rs"]),
        );
        assert!(
            matches!(report.measurement, CoverageMeasurement::NotMeasured { .. }),
            "Expected False Green prevention: no coverage record for a changed source file is \
             absent evidence, not 100%, got {:?}",
            report.measurement
        );
        assert!(!matches!(report.gate_status(), GateStatus::Passed));
        assert!(
            !matches!(report.gate_status(), GateStatus::Failed(_)),
            "and not a fabricated accusation either"
        );
        assert_absent_evidence(&report.gate_status(), "empty path intersection");
    }

    #[test]
    fn test_not_measured_report_states_no_percentage_it_did_not_measure() {
        // I2: no constant standing in for a measurement.
        let diff = diff_adding("src/calc.rs", 1, 10);
        let report = CoverageGuard::new().report_from_outcome(
            CoverageToolOutcome::Unavailable("cargo-llvm-cov is not installed".to_string()),
            &repo(),
            &ctx(&diff, &["src/calc.rs"]),
        );
        assert!(
            !report.summary.contains('%'),
            "a report with no measurement must publish no percentage, got: {}",
            report.summary
        );
        assert!(
            report.summary.contains("cargo-llvm-cov is not installed"),
            "it must publish the reason instead, got: {}",
            report.summary
        );
    }

    // ------------------------------------------------------------------
    // 3. FALSE-RED PREVENTION
    // ------------------------------------------------------------------

    #[test]
    fn test_false_red_prevention_fully_covered_added_lines_pass() {
        let diff = diff_adding("src/calc.rs", 1, 12);
        let report = CoverageGuard::new().report_from_outcome(
            CoverageToolOutcome::Lcov(lcov_for("src/calc.rs", 1, 12, 12)),
            &repo(),
            &ctx(&diff, &["src/calc.rs"]),
        );
        assert_pct(
            report.measured_percent(),
            100.0,
            "Expected False Red prevention: every added line executed",
        );
        assert!(report.is_sufficient);
        assert_eq!(report.gate_status(), GateStatus::Passed);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn test_false_red_prevention_documentation_only_change_is_not_blocked() {
        let diff = "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n\
                    @@ -1,0 +1,2 @@\n+# Title\n+prose\n";
        let report = CoverageGuard::new().report_from_outcome(
            CoverageToolOutcome::Lcov(lcov_for("src/unrelated.rs", 1, 5, 5)),
            &repo(),
            &ctx(diff, &["README.md"]),
        );
        assert!(
            matches!(report.measurement, CoverageMeasurement::NothingToMeasure),
            "Expected False Red prevention: a docs-only PR adds no executable lines, got {:?}",
            report.measurement
        );
        assert_eq!(
            report.measured_percent(),
            None,
            "and it must still not invent a percentage"
        );
        assert_eq!(report.gate_status(), GateStatus::Passed);
    }

    #[test]
    fn test_false_red_prevention_absolute_lcov_paths_match_repo_relative_diff_paths() {
        // P2 in the other direction: path normalisation failure would block
        // every legitimate PR, and a blocked-by-plumbing gate gets bypassed.
        let diff = diff_adding("src/calc.rs", 1, 4);
        let report = CoverageGuard::new().report_from_outcome(
            CoverageToolOutcome::Lcov(lcov_for("/tmp/anvil-cov/src/calc.rs", 1, 4, 4)),
            &repo(),
            &ctx(&diff, &["src/calc.rs"]),
        );
        assert_pct(
            report.measured_percent(),
            100.0,
            "Expected False Red prevention: absolute SF paths under the repo root must resolve \
             to the diff's repo-relative paths",
        );
        assert!(report.is_sufficient);
    }

    #[test]
    fn test_false_red_prevention_added_lines_with_no_executable_record_are_not_uncovered() {
        // The denominator is documented as "added lines the coverage tool
        // reports as executable". Nothing else pins that: every other fixture
        // gives a DA record for every added line, so an implementation that
        // counted ALL added lines in a covered file and treated a missing DA
        // record as a miss would be green throughout. In production that puts
        // a ceiling below 100% on every real PR -- blank lines, closing braces,
        // `use` statements and comments carry no DA record -- and a gate that
        // cannot be satisfied gets bypassed.
        let diff = diff_adding("src/calc.rs", 1, 5);
        // Line 4 has no DA record: not executable, so neither covered nor
        // uncovered.
        let lcov =
            "TN:\nSF:src/calc.rs\nDA:1,7\nDA:2,7\nDA:3,7\nDA:5,7\nLF:4\nLH:4\nend_of_record\n";
        let report = CoverageGuard::new().report_from_outcome(
            CoverageToolOutcome::Lcov(lcov.to_string()),
            &repo(),
            &ctx(&diff, &["src/calc.rs"]),
        );
        assert_pct(
            report.measured_percent(),
            100.0,
            "Expected False Red prevention: an added line the coverage tool does not report \
             as executable is not uncovered code",
        );
        assert!(
            matches!(
                report.measurement,
                CoverageMeasurement::Measured {
                    covered_added_lines: 4,
                    measured_added_lines: 4,
                    ..
                }
            ),
            "the non-executable added line must leave the denominator, not enter it as a \
             miss: {:?}",
            report.measurement
        );
        assert!(report.is_sufficient);
        assert_eq!(report.gate_status(), GateStatus::Passed);
    }

    // ------------------------------------------------------------------
    // 4. ABSENT EVIDENCE
    // ------------------------------------------------------------------

    #[test]
    fn test_absent_evidence_unparseable_payload_is_an_error_not_an_empty_map() {
        let err = parse_lcov("<html><body>502 Bad Gateway</body></html>");
        assert!(
            err.is_err(),
            "a payload with no SF/DA records must be an error, not an empty map that reads as \
             either 0% or 100%"
        );
    }

    #[tokio::test]
    async fn test_absent_evidence_no_cargo_project_reports_not_measured() {
        // Whether cargo-llvm-cov is absent or present, an empty directory
        // yields no measurement -- and never a pass.
        let dir = tempfile::tempdir().expect("tempdir");
        let outcome = CoverageGuard::new().run_llvm_cov(dir.path()).await;
        assert!(
            matches!(outcome, CoverageToolOutcome::Unavailable(_)),
            "a coverage run that cannot produce a report must be Unavailable, got {outcome:?}"
        );

        let diff = diff_adding("src/calc.rs", 1, 10);
        let report = CoverageGuard::new()
            .measure_diff_coverage(dir.path(), &ctx(&diff, &["src/calc.rs"]))
            .await
            .expect("the gate must return a report, not an Err");
        assert!(
            matches!(report.measurement, CoverageMeasurement::NotMeasured { .. }),
            "got {:?}",
            report.measurement
        );
        assert!(!matches!(report.gate_status(), GateStatus::Passed));
        assert_absent_evidence(&report.gate_status(), "no cargo project");
    }

    // ------------------------------------------------------------------
    // 5. BOUNDARY (one below / exactly at / one above)
    // ------------------------------------------------------------------

    #[test]
    fn test_boundary_coverage_of_84_9_percent_fails() {
        // P7: the mandatory boundary. 849/1000 = 84.9%.
        let diff = diff_adding("src/calc.rs", 1, 1000);
        let report = CoverageGuard::new().report_from_outcome(
            CoverageToolOutcome::Lcov(lcov_for("src/calc.rs", 1, 1000, 849)),
            &repo(),
            &ctx(&diff, &["src/calc.rs"]),
        );
        assert_pct(report.measured_percent(), 84.9, "849 of 1000 added lines");
        assert!(
            !report.is_sufficient,
            "84.9% is below the {MIN_COVERAGE_THRESHOLD_PERCENT}% threshold and must FAIL"
        );
        assert!(matches!(report.gate_status(), GateStatus::Failed(_)));
    }

    #[test]
    fn test_boundary_coverage_of_exactly_85_percent_passes() {
        let diff = diff_adding("src/calc.rs", 1, 1000);
        let report = CoverageGuard::new().report_from_outcome(
            CoverageToolOutcome::Lcov(lcov_for("src/calc.rs", 1, 1000, 850)),
            &repo(),
            &ctx(&diff, &["src/calc.rs"]),
        );
        assert_pct(report.measured_percent(), 85.0, "850 of 1000 added lines");
        assert!(matches!(
            report.measurement,
            CoverageMeasurement::Measured {
                covered_added_lines: 850,
                measured_added_lines: 1000,
                ..
            }
        ));
        assert!(report.is_sufficient, "exactly at the threshold passes");
        assert_eq!(report.gate_status(), GateStatus::Passed);
    }

    #[test]
    fn test_boundary_coverage_of_85_1_percent_passes() {
        let diff = diff_adding("src/calc.rs", 1, 1000);
        let report = CoverageGuard::new().report_from_outcome(
            CoverageToolOutcome::Lcov(lcov_for("src/calc.rs", 1, 1000, 851)),
            &repo(),
            &ctx(&diff, &["src/calc.rs"]),
        );
        assert_pct(report.measured_percent(), 85.1, "851 of 1000 added lines");
        assert!(report.is_sufficient);
    }

    #[test]
    fn test_boundary_a_single_uncovered_added_line_fails() {
        let diff = diff_adding("src/calc.rs", 7, 1);
        let report = CoverageGuard::new().report_from_outcome(
            CoverageToolOutcome::Lcov(lcov_for("src/calc.rs", 7, 1, 0)),
            &repo(),
            &ctx(&diff, &["src/calc.rs"]),
        );
        assert_pct(
            report.measured_percent(),
            0.0,
            "the one added line was never executed",
        );
        assert!(!report.is_sufficient);
    }

    // ------------------------------------------------------------------
    // 6. MECHANISM (I22: enforced structurally, not by convention)
    // ------------------------------------------------------------------

    /// The gate's own source, excluding this test module.
    fn production_source() -> &'static str {
        // The marker is assembled at runtime so this scan does not match the
        // attribute literal written inside the test module itself.
        let src = include_str!("coverage_guard.rs");
        let marker = ["#[cfg(te", "st)]"].concat();
        let end = src.find(&marker).expect("test module marker");
        Box::leak(src[..end].to_string().into_boxed_str())
    }

    /// Removes whole-line comments from Rust source.
    ///
    /// Deliberately line-oriented rather than a lexer: a token scanner would
    /// have to know about string literals, and this file contains the literals
    /// `"//"` and `"/*"` in ordinary code. Mistaking one of those for a comment
    /// opener would swallow the rest of the file, and a scan that forbids a
    /// needle would then pass because the needle was eaten -- a false green in
    /// the very mechanism meant to prevent one.
    ///
    /// Consequence, accepted: a trailing comment on a line of code is NOT
    /// stripped. Prose about the removed floor therefore belongs on its own
    /// line, which is where doc comments live anyway.
    fn strip_comment_lines(src: &str) -> String {
        let mut out = String::with_capacity(src.len());
        let mut in_block = false;
        for line in src.lines() {
            let t = line.trim_start();
            if in_block {
                if t.contains("*/") {
                    in_block = false;
                }
                continue;
            }
            if t.starts_with("//") {
                continue;
            }
            if t.starts_with("/*") {
                if !t.contains("*/") {
                    in_block = true;
                }
                continue;
            }
            out.push_str(line);
            out.push('\n');
        }
        out
    }

    /// The gate's production source, comments removed and whitespace squeezed
    /// out, so the scans below judge code rather than prose.
    ///
    /// Both directions matter. An honest doc comment recording that the
    /// `.max(85.0)` floor was removed -- exactly the note `fidelity/mod.rs`
    /// already carries -- must not read as the floor still being present, or
    /// the mechanism punishes the documentation. Conversely a comment naming
    /// `run_bounded` must not satisfy the requirement to actually call it.
    /// Whitespace is dropped so no needle can be evaded by reformatting
    /// (`.max( 85.0 )`, `*0.4`).
    fn production_code() -> String {
        strip_comment_lines(production_source())
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect()
    }

    #[test]
    fn test_the_comment_stripper_the_mechanism_scans_rely_on_is_itself_correct() {
        // A mechanism test is worth exactly as much as the text it reads.
        let stripped = strip_comment_lines(
            "let a = 1;\n\
             /// doc about .max(85.0)\n\
             //! module note\n\
             // plain\n\
             /* block\n\
             still block */\n\
             let b = 2;\n\
             let lit = \"//\";\n\
             let lit2 = \"/*\";\n\
             let c = 3;\n",
        );
        // Prose goes.
        assert!(
            !stripped.contains("doc about"),
            "doc comments must be stripped"
        );
        assert!(!stripped.contains("module note"));
        assert!(!stripped.contains("plain"));
        assert!(
            !stripped.contains("still block"),
            "block comments must be stripped"
        );
        // Code stays -- including code AFTER a line holding a comment-shaped
        // string literal. If this regressed, every "needle must be absent"
        // scan would pass because the source had been eaten.
        assert!(stripped.contains("let a = 1;"));
        assert!(stripped.contains("let b = 2;"));
        assert!(
            stripped.contains("let c = 3;"),
            "a `\"/*\"` literal must not swallow the rest of the file: {stripped}"
        );
    }

    #[test]
    fn test_the_scanned_source_is_the_real_source() {
        // Guards the same failure from the other side: whatever the stripper
        // does, the constant and the gate type must survive it, or the scans
        // below are vacuous.
        let code = production_code();
        assert!(
            code.contains("MIN_COVERAGE_THRESHOLD_PERCENT:f64=85.0"),
            "the scans must be reading this gate's actual code"
        );
        assert!(code.contains("implCoverageGuard"));
    }

    #[test]
    fn test_the_coverage_floor_arithmetic_is_absent_from_the_gate() {
        let code = production_code();
        let floor = [".ma", "x(85.0)"].concat();
        assert!(
            !code.contains(&floor),
            "the .max floor made 85 >= 85 unfailable; it must not exist"
        );
        let ratio = ["*", "0.4"].concat();
        assert!(
            !code.contains(&ratio),
            "the added-test-lines / added-code-lines * 0.4 heuristic is not a measurement (I2)"
        );
    }

    #[test]
    fn test_every_subprocess_in_this_gate_goes_through_run_bounded() {
        let code = production_code();
        assert!(
            code.contains("run_bounded"),
            "I5: the coverage tool must be spawned through crate::exec::run_bounded"
        );
        assert!(
            code.contains("ExecClass::Build"),
            "I5: a coverage run is build-class work"
        );
        let raw_wait = [".out", "put()"].concat();
        assert!(
            !code.contains(&raw_wait),
            "I5: no unbounded direct subprocess wait in this gate"
        );
    }

    #[test]
    fn test_threshold_constant_is_the_documented_85_percent() {
        assert_eq!(MIN_COVERAGE_THRESHOLD_PERCENT, 85.0);
    }
}
