//! Mutation adequacy of the lines this pull request changed.
//!
//! # What this gate used to do
//!
//! It published "Critical branches verified against surviving mutants". It
//! compiled no mutant, ran no test, and executed no mutated program. Its whole
//! decision was one line:
//!
//! ```text
//! let has_test_changes = diff_ctx.changed_files.iter().any(|f| f.contains("test") || ...);
//! ```
//!
//! A changed path with the substring `test` anywhere in it -- `docs/latest.md`
//! qualifies -- certified every decision branch in the diff as "protected by
//! test assertions". `AstMutatorEngine` beside it produced "mutations" by
//! `str::replace` on lines of text that were never written to disk, never
//! compiled and never run; nothing read its output. Both are deleted.
//!
//! Mutation testing is the one technique that proves a suite has teeth. A gate
//! that claims it and does not do it is worse than no gate: it is a receipt for
//! an inspection nobody performed.
//!
//! # What it does now
//!
//! `cargo mutants --in-diff` runs through [`crate::exec::run_bounded_for`]
//! against the PR's own diff, so mutants are generated only in the functions
//! this change touched -- the same scoping Google applies at review time
//! (Petrovic & Ivankovic, *State of Mutation Testing at Google*), and the
//! reason a mutation run can be a per-PR gate at all. The tool builds and runs
//! the suite against each mutant; this module reads which ones the suite failed
//! to kill.
//!
//! Four outcomes, and only four:
//!
//! - `Measured`, no survivor -- every mutant on the changed lines was killed.
//!   `Passed`.
//! - `Measured`, survivors -- the suite executed a mutated program and did not
//!   notice. `Failed`, naming each survivor and the mutation applied. This is
//!   the only outcome that accuses the pull request, and it is backed by a
//!   program that really ran.
//! - `NothingToMeasure` -- the change touches no Rust source, or cargo-mutants
//!   generated no mutant on the changed lines. A true statement, and the only
//!   case in which nothing is spawned at all.
//! - `NotMeasured` -- cargo-mutants is not installed, could not be spawned,
//!   exceeded the budget, found the baseline suite already failing, or left a
//!   mutant hanging that it never showed to be killed. Absent evidence: never a
//!   pass (I1), never a kill rate that was not measured (I2), never an
//!   accusation against the pull request.
//!
//! # Cost
//!
//! Every mutant costs one incremental build plus one test run, so the run is
//! bounded twice: `--in-diff` bounds how many mutants exist, and
//! [`MUTATION_BUDGET`] bounds the wall clock. Overrunning the budget is
//! `NotMeasured` -- a partial mutation run is not a kill rate. Where
//! cargo-mutants is not installed the gate costs one process spawn that fails
//! in 17-23 ms, and a change that touches no Rust costs nothing at all because
//! the spawn never happens.
//!
//! Where it IS installed this is a minutes-scale gate, not a milliseconds one.
//! Measured on this repository with cargo-mutants 27.1.0: `--list --in-diff`
//! over this change's own diff finds **41 mutants**, and each costs an
//! incremental build (18 s here) plus a suite run (16 s warm on a laptop,
//! 58.7 s on the reviewer's runner). That is 20-50 minutes against a 600 s
//! budget. See [`MUTATION_BUDGET`].
//!
//! A budget kill reaps the direct child only. `kill_on_drop` (`exec/mod.rs`)
//! kills the `cargo mutants` process; the `cargo build` and `cargo test` it
//! forked per mutant, and the `$TMPDIR/cargo-mutants-*` tree copies they run
//! in, are not its children and survive it. Every other bounded spawn in this
//! tree is one process -- this one is N builds, so a timeout leaves work
//! running and disk allocated on a runner that has just reported
//! `NotMeasured`. Reaping the process group is the fix; it belongs in
//! `exec::run_bounded_for`, where every caller gets it, not here.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tracing::info;

use crate::exec::run_bounded_for;
use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::GateStatus;

/// Matches the `PreMergeCertificationReport` field name, so `unmeasured_gates`
/// names a gate that can be looked up in the fidelity registry and the report.
pub const MUTATION_GATE_ID: &str = "mutation_status";

/// Wall-clock ceiling for one mutation run.
///
/// Ten minutes, not [`crate::exec::ExecClass::Build`]'s thirty: this is the
/// most expensive gate in the matrix and it runs on every pull request, so the
/// answer to "how much does it add" has to be a number a reviewer will accept.
/// It is not enough for a change this size, and the number is measured rather
/// than estimated. A mutant costs one incremental build (18 s here) plus one
/// suite run (16 s warm on a laptop, 58.7 s on the reviewer's runner), so 600 s
/// buys **8-17 mutants**, while `cargo mutants --list --in-diff` over this
/// change's own diff finds **41**. The realistic outcome on a runner that has
/// the tool is therefore ten minutes burned and then `NotMeasured` -- correct,
/// and expensive. Raising the budget or capping the mutant count so it declines
/// cheaply is the follow-up; a run that does not finish inside it reports
/// `NotMeasured`, never a partial kill rate.
pub const MUTATION_BUDGET: Duration = Duration::from_secs(600);

/// What came back from cargo-mutants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutantsOutcome {
    /// The outcome lists the tool wrote, verbatim: one mutant per line, or
    /// empty. Missing files arrive as empty strings, which is what the tool
    /// writes when a category has no members.
    Reported {
        caught: String,
        missed: String,
        timed_out: String,
    },
    /// No measurement exists: cargo-mutants absent, spawn failure, the budget,
    /// or a run that wrote no outcome list at all. Carries the reason verbatim
    /// so it is published instead of a number (I1, I2).
    Unavailable(String),
}

/// The result of attempting to measure mutation adequacy.
///
/// There is deliberately no variant that carries a kill rate without having run
/// a mutant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MutationMeasurement {
    Measured {
        mutants_tested: usize,
        /// Mutants the suite failed on. Deliberately not `mutants_tested` minus
        /// the survivors: a mutant that hung the suite is in neither set.
        killed: usize,
    },
    /// No mutant exists to run: the change touches no Rust source, or
    /// cargo-mutants generated none on the changed lines.
    NothingToMeasure,
    /// Absent evidence. Never a pass, never an accusation.
    NotMeasured { reason: String },
}

/// One mutant the test suite failed to kill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationSurvivingFinding {
    /// Repo-relative path of the mutated file.
    pub file_path: String,
    /// `file:line:col`, as cargo-mutants reports it.
    pub location: String,
    /// The mutation that was applied, in the tool's own words.
    pub mutation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationAdequacyReport {
    /// Whether this report permits certification. False for a survivor AND for
    /// absent evidence -- an unmeasured gate must not certify.
    pub is_adequate: bool,
    pub surviving_findings: Vec<MutationSurvivingFinding>,
    pub summary: String,
    /// What was actually measured, if anything.
    pub measurement: MutationMeasurement,
}

impl MutationAdequacyReport {
    /// The gate status this report is entitled to publish.
    ///
    /// A survivor is `Failed`, not `Warning`: `Warning` is acceptable to
    /// `is_admissible`, so a gate that can only warn cannot block, and a mutant
    /// that survived is a program that ran with changed behaviour and passed
    /// the suite. Absent evidence is `NotMeasured` under this gate's own id,
    /// which blocks merge-queue admission through `unmeasured_gates` while
    /// making no claim against the pull request.
    pub fn gate_status(&self) -> GateStatus {
        match &self.measurement {
            MutationMeasurement::NotMeasured { reason } => GateStatus::NotMeasured {
                gate_id: MUTATION_GATE_ID.to_string(),
                reason: reason.clone(),
            },
            MutationMeasurement::NothingToMeasure => GateStatus::Passed,
            MutationMeasurement::Measured { .. } => {
                if self.is_adequate {
                    GateStatus::Passed
                } else {
                    GateStatus::Failed(self.summary.clone())
                }
            }
        }
    }

    fn not_measured(reason: String) -> Self {
        let reason = reason.trim().to_string();
        MutationAdequacyReport {
            is_adequate: false,
            surviving_findings: Vec::new(),
            summary: format!("Mutation adequacy not measured: {}", reason),
            measurement: MutationMeasurement::NotMeasured { reason },
        }
    }

    fn nothing_to_measure(why: &str) -> Self {
        MutationAdequacyReport {
            is_adequate: true,
            surviving_findings: Vec::new(),
            summary: format!("Mutation adequacy: {}", why),
            measurement: MutationMeasurement::NothingToMeasure,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ChaosMutationGuard;

impl ChaosMutationGuard {
    pub fn new() -> Self {
        Self
    }

    /// Legacy synchronous entry point, retained for its one caller:
    /// `tests/enlist_authority_coverage_test.rs:404`, which builds every gate's
    /// report synchronously.
    ///
    /// cargo-mutants cannot be spawned from a synchronous function under the
    /// bounded-exec invariant, so this reports absent evidence rather than the
    /// filename match it used to return. The real entry point is
    /// [`ChaosMutationGuard::measure_diff_mutants`].
    pub fn evaluate_mutation_adequacy(
        &self,
        diff_ctx: &PrDiffContext,
    ) -> Result<MutationAdequacyReport> {
        if !touches_rust_source(diff_ctx) {
            return Ok(MutationAdequacyReport::nothing_to_measure(NO_RUST_SOURCE));
        }
        Ok(MutationAdequacyReport::not_measured(
            "the synchronous entry point cannot run cargo-mutants; call \
             ChaosMutationGuard::measure_diff_mutants"
                .to_string(),
        ))
    }

    /// The review pipeline's entry point: run the mutants this diff creates and
    /// report which ones the suite failed to kill.
    pub async fn measure_diff_mutants(
        &self,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> MutationAdequacyReport {
        info!(
            "Running ChaosMutationGuard on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );
        if !touches_rust_source(diff_ctx) {
            // Decided before anything is spawned: a change with no Rust in it
            // has no mutants whatever the tool would say, and a subprocess per
            // documentation PR is the cost nobody agreed to.
            return MutationAdequacyReport::nothing_to_measure(NO_RUST_SOURCE);
        }
        let outcome = self.run_cargo_mutants(repo_dir, diff_ctx).await;
        self.report_from_outcome(outcome, diff_ctx)
    }

    /// Runs cargo-mutants over the diff and returns its outcome lists, or the
    /// reason there are none.
    ///
    /// Every failure mode -- no manifest, cargo-mutants not installed, spawn
    /// failure, the budget, a run that wrote nothing -- becomes `Unavailable`
    /// carrying the reason verbatim. None of them can become a kill rate (I1).
    ///
    /// A non-zero exit is deliberately NOT a failure mode on its own:
    /// cargo-mutants exits non-zero precisely when it found survivors, which is
    /// the measurement this gate exists to take.
    pub async fn run_cargo_mutants(
        &self,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> MutantsOutcome {
        let manifest = repo_dir.join("Cargo.toml");
        if !manifest.is_file() {
            return MutantsOutcome::Unavailable(format!(
                "no Cargo.toml at {}, so no mutation run is possible",
                manifest.display()
            ));
        }

        let scratch = match tempfile::tempdir() {
            Ok(d) => d,
            Err(e) => {
                return MutantsOutcome::Unavailable(format!(
                    "no scratch directory for the mutation run: {}",
                    e
                ));
            }
        };
        let diff_path = scratch.path().join("pr.diff");
        if let Err(e) = std::fs::write(&diff_path, &diff_ctx.diff_content) {
            return MutantsOutcome::Unavailable(format!(
                "the pull request diff could not be staged for cargo-mutants: {}",
                e
            ));
        }
        let out_dir = scratch.path().join("out");

        let mut cmd = Command::new("cargo");
        cmd.current_dir(repo_dir)
            .arg("mutants")
            .arg("--in-diff")
            .arg(&diff_path)
            .arg("--output")
            .arg(&out_dir)
            .arg("--colors")
            .arg("never")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let out = match run_bounded_for(cmd, MUTATION_BUDGET, "cargo mutants").await {
            Ok(out) => out,
            Err(e) => return MutantsOutcome::Unavailable(e.to_string()),
        };

        outcome_from_run(
            out.status.code(),
            [
                read_outcome_list(&out_dir, "caught.txt"),
                read_outcome_list(&out_dir, "missed.txt"),
                read_outcome_list(&out_dir, "timeout.txt"),
            ],
            &String::from_utf8_lossy(&out.stderr),
        )
    }

    /// Pure: turns a tool outcome plus the PR diff into a report. No I/O, so
    /// every verdict and every absent-evidence path is testable without a
    /// mutation toolchain, and the unit suite spawns nothing.
    pub fn report_from_outcome(
        &self,
        outcome: MutantsOutcome,
        diff_ctx: &PrDiffContext,
    ) -> MutationAdequacyReport {
        if !touches_rust_source(diff_ctx) {
            return MutationAdequacyReport::nothing_to_measure(NO_RUST_SOURCE);
        }

        let (caught, missed, timed_out) = match outcome {
            MutantsOutcome::Reported {
                caught,
                missed,
                timed_out,
            } => (caught, missed, timed_out),
            MutantsOutcome::Unavailable(reason) => {
                return MutationAdequacyReport::not_measured(reason);
            }
        };

        let survivors: Vec<MutationSurvivingFinding> =
            outcome_lines(&missed).map(finding_from).collect();
        let killed = outcome_lines(&caught).count();
        let hung = outcome_lines(&timed_out).count();
        let mutants_tested = killed + hung + survivors.len();

        if mutants_tested == 0 {
            return MutationAdequacyReport::nothing_to_measure(
                "cargo-mutants generated no viable mutant on the lines this pull request \
                 changed.",
            );
        }

        // A mutant whose tests hung was neither killed nor shown to survive.
        // cargo-mutants keeps it in its own category and exits non-zero for it,
        // and this gate follows the tool it invokes rather than Stryker's
        // convention of counting a timeout as detected: certifying on a run
        // that hung would claim a kill nobody observed. Survivors still decide
        // first -- a mutant that provably survived is evidence, and one that
        // hung does not erase it.
        if survivors.is_empty() && hung > 0 {
            return MutationAdequacyReport::not_measured(format!(
                "{} of {} mutant(s) on the changed lines made the test suite hang, so the suite \
                 was never shown to kill them",
                hung, mutants_tested
            ));
        }

        let summary = if survivors.is_empty() {
            format!(
                "Mutation adequacy: the test suite killed all {} mutant(s) cargo-mutants \
                 generated on the lines this pull request changed.",
                mutants_tested
            )
        } else {
            format!(
                "Mutation adequacy: {} of {} mutant(s) on the changed lines survived the test \
                 suite, so a changed program behaved differently and no test noticed: {}",
                survivors.len(),
                mutants_tested,
                survivors
                    .iter()
                    .take(3)
                    .map(|f| format!("{} -- {}", f.location, f.mutation))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };

        MutationAdequacyReport {
            is_adequate: survivors.is_empty(),
            surviving_findings: survivors,
            summary,
            measurement: MutationMeasurement::Measured {
                mutants_tested,
                killed,
            },
        }
    }
}

/// Said in one place so the pipeline path and the synchronous path cannot drift
/// into two different statements about the same diff.
const NO_RUST_SOURCE: &str =
    "this pull request changes no Rust source, so there is no mutant to generate.";

/// Whether cargo-mutants could generate anything for this change.
///
/// Both the file list and the diff are consulted: a context carrying only one
/// of the two must not be read as "no Rust here", because that answer becomes a
/// pass. This is a statement about the language of the changed files, not about
/// the adequacy of anything -- the adequacy question is answered by mutants
/// that were built and run.
fn touches_rust_source(diff_ctx: &PrDiffContext) -> bool {
    diff_ctx.changed_files.iter().any(|f| f.ends_with(".rs"))
        || diff_ctx
            .diff_content
            .lines()
            .filter_map(|l| l.strip_prefix("+++ b/"))
            .any(|p| p.trim().ends_with(".rs"))
}

/// Which of cargo-mutants' exit codes carry a measurement, and which do not.
///
/// The documented contract (mutants.rs, "Exit codes"): 0 every viable mutant
/// was caught, 2 some mutants were missed, 3 some tests timed out -- and 3
/// masks 2, so a run with both reports 3. That is why the outcome lists are
/// read rather than the status alone. Everything else is the tool declining to
/// measure: 1 usage error, 4 the baseline suite is already failing so nothing
/// was mutated, 5 the diff does not match the tree, 6 the diff would not parse,
/// 101 cargo has no such subcommand.
///
/// Exit 0 with no output directory at all is the ordinary `--in-diff` no-op:
/// the filter matches nothing and the run ends before `mutants.out` is created.
/// That is a clean nothing, and returning empty lists makes it
/// `NothingToMeasure` rather than an absence of evidence.
///
/// The exit codes that say a mutant was NOT killed are read only when a list
/// names one. "Some list is readable" is not enough: with `caught.txt` read and
/// `missed.txt` not -- a non-UTF8 byte in a path, a truncated write, an fs
/// error, a layout change touching one filename -- an empty `missed` would
/// publish `Passed` over the tool's own statement that a mutant survived. An
/// exit code claiming survivors can only produce `Failed` or `NotMeasured`.
///
/// Split out from the spawn so the discrimination that decides pass, fail and
/// no-claim is exercised by the unit suite without a mutation toolchain.
fn outcome_from_run(code: Option<i32>, lists: [Option<String>; 3], stderr: &str) -> MutantsOutcome {
    let [caught, missed, timed_out] = lists;
    // 2 and 3 are the tool saying a mutant was not killed. Their evidence is
    // `missed.txt`/`timeout.txt`, and only those: a readable `caught.txt` is
    // not permission to read the other two as empty. Without a named mutant to
    // show for the exit code the run measured nothing, so it declines.
    let a_survivor_is_named = [&missed, &timed_out].into_iter().any(|l| {
        l.as_deref()
            .is_some_and(|s| outcome_lines(s).next().is_some())
    });
    match code {
        // 0 is "every viable mutant was caught", so an unreadable list cannot
        // be hiding a survivor -- including the `--in-diff` no-op, where the
        // filter matched nothing and no list exists at all.
        Some(0) => MutantsOutcome::Reported {
            caught: caught.unwrap_or_default(),
            missed: missed.unwrap_or_default(),
            timed_out: timed_out.unwrap_or_default(),
        },
        Some(2 | 3) if a_survivor_is_named => MutantsOutcome::Reported {
            caught: caught.unwrap_or_default(),
            missed: missed.unwrap_or_default(),
            timed_out: timed_out.unwrap_or_default(),
        },
        _ => MutantsOutcome::Unavailable(format!(
            "cargo mutants produced no mutation result: it exited with {} -- {}",
            match code {
                Some(c) => format!("code {}", c),
                None => "a signal".to_string(),
            },
            tail(stderr)
        )),
    }
}

/// cargo-mutants writes its lists under `<--output>/mutants.out/`. `None` is a
/// list that was not read -- absent, not empty -- which is the distinction
/// `outcome_from_run` needs to tell "no mutant survived" from "no run
/// happened". A layout change in the tool degrades to `Unavailable`, which is
/// `NotMeasured`, never a pass.
fn read_outcome_list(out_dir: &Path, name: &str) -> Option<String> {
    std::fs::read_to_string(out_dir.join("mutants.out").join(name)).ok()
}

fn outcome_lines(list: &str) -> impl Iterator<Item = &str> {
    list.lines().map(str::trim).filter(|l| !l.is_empty())
}

/// Reads one line of `missed.txt`, which cargo-mutants writes as
/// `src/foo.rs:41:9: replace is_adult -> bool with true`.
///
/// Tolerant on purpose: a line whose shape the tool changes is still recorded
/// whole. The verdict never depends on this parse -- a survivor is a survivor
/// because the tool listed it, not because this function understood it.
fn finding_from(line: &str) -> MutationSurvivingFinding {
    let (location, mutation) = match line.find(": ") {
        Some(i) => (&line[..i], line[i + 2..].trim()),
        None => (line, ""),
    };
    MutationSurvivingFinding {
        file_path: location.split(':').next().unwrap_or(location).to_string(),
        location: location.to_string(),
        mutation: mutation.to_string(),
    }
}

/// The last of a subprocess's stderr, so a reason is publishable without
/// pasting a build log onto a pull request.
fn tail(s: &str) -> String {
    const MAX: usize = 400;
    let t = s.trim();
    match t.char_indices().nth_back(MAX) {
        Some((i, _)) => format!("...{}", &t[i..]),
        None => t.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(diff: &str, changed: &[&str]) -> PrDiffContext {
        PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 201,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: diff.to_string(),
            changed_files: changed.iter().map(|s| s.to_string()).collect(),
            is_incremental: false,
        }
    }

    /// A diff that adds one decision branch to production code.
    fn a_branch_in_src() -> PrDiffContext {
        ctx(
            "+++ b/src/auth.rs\n+ if user.age >= 18 { allow(); }",
            &["src/auth.rs"],
        )
    }

    const A_SURVIVOR: &str = "src/auth.rs:41:9: replace is_adult -> bool with true";
    const A_KILLED: &str = "src/auth.rs:41:9: replace is_adult -> bool with false";

    fn reported(caught: &str, missed: &str, timed_out: &str) -> MutantsOutcome {
        MutantsOutcome::Reported {
            caught: caught.to_string(),
            missed: missed.to_string(),
            timed_out: timed_out.to_string(),
        }
    }

    /// Absent evidence must be `GateStatus::NotMeasured` under this gate's own
    /// id, and must not certify.
    fn assert_absent_evidence(report: &MutationAdequacyReport, ctx: &str) {
        assert!(
            !report.is_adequate,
            "{ctx}: absent evidence certified the pull request (I1)"
        );
        match report.gate_status() {
            GateStatus::NotMeasured { gate_id, reason } => {
                assert_eq!(gate_id, MUTATION_GATE_ID, "{ctx}: wrong gate id");
                assert!(
                    !reason.trim().is_empty(),
                    "{ctx}: unmeasured with no reason"
                );
            }
            other => panic!("{ctx}: expected NotMeasured, got {other:?}"),
        }
        assert!(
            report.surviving_findings.is_empty(),
            "{ctx}: a run that measured nothing must not accuse the pull request (I1)"
        );
    }

    // ------------------------------------------------------------------
    // 1. FALSE GREEN prevention
    // ------------------------------------------------------------------

    /// THE DEFECT. A path containing the substring "test" -- here inside
    /// "latest" -- used to certify every branch in the diff as verified against
    /// surviving mutants. It is not evidence that any program ran.
    #[test]
    fn a_filename_containing_test_is_not_evidence_that_a_mutant_was_killed() {
        let guard = ChaosMutationGuard::new();
        let diff = ctx(
            "+++ b/src/auth.rs\n+ if user.age >= 18 { allow(); }",
            &["src/auth.rs", "docs/latest_notes.md", "tests/auth_test.rs"],
        );

        let report = guard.report_from_outcome(reported("", A_SURVIVOR, ""), &diff);

        assert!(
            !report.is_adequate,
            "a surviving mutant is a survivor whatever the changed files are called: {}",
            report.summary
        );
        assert!(matches!(report.gate_status(), GateStatus::Failed(_)));
    }

    #[test]
    fn a_surviving_mutant_fails_the_gate_and_is_named() {
        let guard = ChaosMutationGuard::new();
        let report =
            guard.report_from_outcome(reported(A_KILLED, A_SURVIVOR, ""), &a_branch_in_src());

        assert!(!report.is_adequate);
        assert_eq!(report.surviving_findings.len(), 1);
        let f = &report.surviving_findings[0];
        assert_eq!(f.file_path, "src/auth.rs");
        assert_eq!(f.location, "src/auth.rs:41:9");
        assert_eq!(f.mutation, "replace is_adult -> bool with true");
        assert_eq!(
            report.measurement,
            MutationMeasurement::Measured {
                mutants_tested: 2,
                killed: 1
            }
        );
        match report.gate_status() {
            GateStatus::Failed(reason) => assert!(
                reason.contains("src/auth.rs:41:9") && reason.contains("survived"),
                "the failure must name the mutant that survived: {reason}"
            ),
            other => panic!("a survivor must fail the gate, got {other:?}"),
        }
    }

    #[test]
    fn an_absent_tool_is_not_a_pass() {
        let guard = ChaosMutationGuard::new();
        let report = guard.report_from_outcome(
            MutantsOutcome::Unavailable(
                "cargo mutants wrote no outcome list and exited with status exit status: 101: \
                 error: no such command: `mutants`"
                    .to_string(),
            ),
            &a_branch_in_src(),
        );
        assert_absent_evidence(&report, "cargo-mutants not installed");
        assert!(
            report.summary.contains("no such command"),
            "the reason must survive to the reader: {}",
            report.summary
        );
    }

    #[test]
    fn a_run_that_exceeded_the_budget_is_not_a_pass() {
        let guard = ChaosMutationGuard::new();
        let report = guard.report_from_outcome(
            MutantsOutcome::Unavailable("cargo mutants timed out after 600s".to_string()),
            &a_branch_in_src(),
        );
        assert_absent_evidence(&report, "budget exceeded");
    }

    /// The half-measured run: the budget expired with survivors already found.
    /// It is still `NotMeasured` -- a partial mutation run is not a kill rate,
    /// and the partial result must not be laundered into either verdict.
    #[test]
    fn a_partial_run_publishes_no_kill_rate() {
        let guard = ChaosMutationGuard::new();
        let report = guard.report_from_outcome(
            MutantsOutcome::Unavailable("cargo mutants timed out after 600s".to_string()),
            &a_branch_in_src(),
        );
        assert!(
            !report.summary.contains('%'),
            "no percentage may appear in a report that measured nothing (I2): {}",
            report.summary
        );
        assert!(matches!(
            report.measurement,
            MutationMeasurement::NotMeasured { .. }
        ));
    }

    /// A mutant that hung the suite was never shown to be killed. cargo-mutants
    /// keeps it in its own category and exits non-zero for it, so a run with
    /// one and no survivor certifies nothing.
    #[test]
    fn a_mutant_that_hung_the_suite_is_not_counted_as_killed() {
        let guard = ChaosMutationGuard::new();
        let report =
            guard.report_from_outcome(reported(A_KILLED, "", A_SURVIVOR), &a_branch_in_src());
        assert_absent_evidence(&report, "a mutant hung the suite");
        assert!(
            report.summary.contains("hang"),
            "the reader must be told why: {}",
            report.summary
        );
    }

    /// ...but a mutant that provably survived is evidence, and a second mutant
    /// hanging must not launder that failure into "not measured".
    #[test]
    fn a_survivor_still_fails_the_gate_when_another_mutant_hung() {
        let guard = ChaosMutationGuard::new();
        let report =
            guard.report_from_outcome(reported("", A_SURVIVOR, A_KILLED), &a_branch_in_src());
        assert!(!report.is_adequate);
        assert!(matches!(report.gate_status(), GateStatus::Failed(_)));
        assert_eq!(
            report.measurement,
            MutationMeasurement::Measured {
                mutants_tested: 2,
                killed: 0
            },
            "a hung mutant is in neither the killed set nor the survivor set"
        );
    }

    #[test]
    fn the_synchronous_entry_point_makes_no_claim_about_rust_it_did_not_mutate() {
        let guard = ChaosMutationGuard::new();
        let report = guard
            .evaluate_mutation_adequacy(&a_branch_in_src())
            .expect("the legacy entry point answers");
        assert_absent_evidence(&report, "synchronous entry point");
    }

    /// The absent-evidence path that can be exercised for real without a
    /// toolchain: a directory with no manifest is refused before anything is
    /// spawned, so this test starts no subprocess and costs nothing.
    #[tokio::test]
    async fn a_directory_with_no_cargo_project_reports_not_measured() {
        let dir = tempfile::tempdir().expect("tempdir");
        let guard = ChaosMutationGuard::new();
        let report = guard
            .measure_diff_mutants(dir.path(), &a_branch_in_src())
            .await;
        assert_absent_evidence(&report, "no Cargo.toml");
        assert!(
            report.summary.contains("Cargo.toml"),
            "the reason must name what is missing: {}",
            report.summary
        );
    }

    // ------------------------------------------------------------------
    // 2. FALSE RED prevention
    // ------------------------------------------------------------------

    #[test]
    fn every_mutant_killed_passes() {
        let guard = ChaosMutationGuard::new();
        let report = guard.report_from_outcome(
            reported(&format!("{A_KILLED}\n{A_SURVIVOR}"), "", ""),
            &a_branch_in_src(),
        );
        assert!(report.is_adequate, "{}", report.summary);
        assert_eq!(report.gate_status(), GateStatus::Passed);
        assert_eq!(
            report.measurement,
            MutationMeasurement::Measured {
                mutants_tested: 2,
                killed: 2
            }
        );
    }

    #[test]
    fn a_change_with_no_rust_in_it_is_not_accused() {
        let guard = ChaosMutationGuard::new();
        let diff = ctx("+++ b/README.md\n+ a documentation line", &["README.md"]);
        let report = guard.report_from_outcome(reported("", A_SURVIVOR, ""), &diff);
        assert!(report.is_adequate);
        assert_eq!(report.measurement, MutationMeasurement::NothingToMeasure);
        assert_eq!(report.gate_status(), GateStatus::Passed);
        assert!(
            report.surviving_findings.is_empty(),
            "a change with no Rust cannot own a mutant"
        );
    }

    #[test]
    fn a_diff_naming_rust_is_measured_even_when_the_file_list_is_empty() {
        let guard = ChaosMutationGuard::new();
        let diff = ctx("+++ b/src/auth.rs\n+ if x { y(); }", &[]);
        let report = guard.report_from_outcome(reported("", A_SURVIVOR, ""), &diff);
        assert!(
            !report.is_adequate,
            "an empty changed-file list must not read as `no Rust here`, which is a pass"
        );
    }

    #[test]
    fn a_tool_run_that_generated_no_mutant_says_so_rather_than_claiming_a_kill_rate() {
        let guard = ChaosMutationGuard::new();
        let report = guard.report_from_outcome(reported("", "", ""), &a_branch_in_src());
        assert_eq!(report.measurement, MutationMeasurement::NothingToMeasure);
        assert_eq!(report.gate_status(), GateStatus::Passed);
        assert!(
            report.summary.contains("no viable mutant"),
            "the reader must be told nothing was mutated: {}",
            report.summary
        );
    }

    // ------------------------------------------------------------------
    // 3. PARSING
    // ------------------------------------------------------------------

    #[test]
    fn outcome_lists_are_counted_by_mutant_not_by_byte() {
        let guard = ChaosMutationGuard::new();
        let caught = format!("{A_KILLED}\n\n  {A_KILLED}  \n");
        let report = guard.report_from_outcome(reported(&caught, "", ""), &a_branch_in_src());
        assert_eq!(
            report.measurement,
            MutationMeasurement::Measured {
                mutants_tested: 2,
                killed: 2
            },
            "blank and padded lines are not mutants"
        );
    }

    #[test]
    fn a_survivor_line_the_parser_does_not_understand_is_still_a_survivor() {
        let guard = ChaosMutationGuard::new();
        let report = guard.report_from_outcome(
            reported("", "something new from the tool", ""),
            &a_branch_in_src(),
        );
        assert!(
            !report.is_adequate,
            "the verdict must not depend on parsing the tool's prose"
        );
        assert_eq!(
            report.surviving_findings[0].location,
            "something new from the tool"
        );
    }

    /// The cheap path a documentation pull request takes: no subprocess, no
    /// scratch directory, no diff written anywhere. Runs against a directory
    /// that HAS a manifest, so nothing but the early return can be producing
    /// this answer.
    #[tokio::test]
    async fn a_change_with_no_rust_in_it_never_reaches_the_tool() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"\n")
            .expect("manifest");
        let diff = ctx("+++ b/README.md\n+ a documentation line", &["README.md"]);

        let report = ChaosMutationGuard::new()
            .measure_diff_mutants(dir.path(), &diff)
            .await;

        assert_eq!(report.measurement, MutationMeasurement::NothingToMeasure);
        assert_eq!(report.gate_status(), GateStatus::Passed);
    }

    // ------------------------------------------------------------------
    // 4. THE TOOL'S EXIT-CODE CONTRACT
    // ------------------------------------------------------------------

    fn lists(caught: &str, missed: &str, timed_out: &str) -> [Option<String>; 3] {
        [
            Some(caught.to_string()),
            Some(missed.to_string()),
            Some(timed_out.to_string()),
        ]
    }

    const NO_LISTS: [Option<String>; 3] = [None, None, None];

    #[test]
    fn exit_zero_with_no_output_directory_is_the_in_diff_no_op_not_an_absence() {
        // `--in-diff` matched nothing, so cargo-mutants never created
        // mutants.out. Reporting NotMeasured here would block the merge queue
        // on every pull request whose changed lines carry no mutant.
        assert_eq!(
            outcome_from_run(Some(0), NO_LISTS, ""),
            MutantsOutcome::Reported {
                caught: String::new(),
                missed: String::new(),
                timed_out: String::new()
            }
        );
    }

    #[test]
    fn the_exit_codes_that_carry_a_measurement_are_read_from_the_lists() {
        // Each code paired with the evidence it claims: 0 caught everything,
        // 2 missed a mutant, 3 hung on one.
        for (code, l) in [
            (0, lists(A_KILLED, "", "")),
            (2, lists(A_KILLED, A_SURVIVOR, "")),
            (3, lists(A_KILLED, "", A_SURVIVOR)),
        ] {
            assert!(
                matches!(
                    outcome_from_run(Some(code), l, ""),
                    MutantsOutcome::Reported { .. }
                ),
                "exit {code} carries a mutation result and must be read"
            );
        }
    }

    #[test]
    fn a_survivor_exit_code_with_only_the_caught_list_readable_is_not_a_pass() {
        // The asymmetric absence: `caught.txt` read, `missed.txt` did not --
        // a non-UTF8 byte in a mutated path, a truncated write, an fs error on
        // one file. All three lists absent was already covered; this is the
        // partial set, where `wrote_nothing` is false and an exit code saying a
        // mutant survived would otherwise be read as "all caught" and certify.
        for code in [2, 3] {
            for l in [
                [Some(A_KILLED.to_string()), None, None],
                [Some(A_KILLED.to_string()), Some(String::new()), None],
                [None, None, Some(String::new())],
            ] {
                let outcome = outcome_from_run(Some(code), l.clone(), "");
                assert!(
                    matches!(outcome, MutantsOutcome::Unavailable(_)),
                    "exit {code} says a mutant was not killed; with no list naming \
                     one there is nothing to certify: {l:?} -> {outcome:?}"
                );
                let report =
                    ChaosMutationGuard::new().report_from_outcome(outcome, &a_branch_in_src());
                assert_ne!(
                    report.gate_status(),
                    GateStatus::Passed,
                    "exit {code} with a partial list set must never certify: {l:?}"
                );
                assert_absent_evidence(&report, &format!("exit {code}, partial lists"));
            }
        }
    }

    #[test]
    fn a_failing_baseline_suite_is_not_a_pass() {
        // Exit 4: the tests were already failing, so nothing was mutated --
        // and cargo-mutants still created the four empty list files. Reading
        // those as "no mutants, nothing to measure" would certify a pull
        // request against a suite that does not even run.
        let outcome = outcome_from_run(Some(4), lists("", "", ""), "baseline tests failed");
        assert!(
            matches!(outcome, MutantsOutcome::Unavailable(_)),
            "a broken baseline must not read as an empty mutant list: {outcome:?}"
        );
        let report = ChaosMutationGuard::new().report_from_outcome(outcome, &a_branch_in_src());
        assert_absent_evidence(&report, "exit 4, baseline already failing");
    }

    #[test]
    fn the_undocumented_and_the_broken_exit_codes_measure_nothing() {
        // 101 no such cargo subcommand, 5 the diff does not match the tree,
        // 6 the diff would not parse, 1 usage, None killed by a signal.
        for code in [Some(1), Some(5), Some(6), Some(101), None] {
            let outcome = outcome_from_run(code, NO_LISTS, "boom");
            assert!(
                matches!(outcome, MutantsOutcome::Unavailable(_)),
                "exit {code:?} produced no measurement and must say so, got {outcome:?}"
            );
        }
    }

    /// The file-layout adapter, exercised against a directory laid out the way
    /// cargo-mutants lays one out. This is the piece most likely to go stale
    /// when the tool changes, and the only one a run without the tool can pin.
    #[test]
    fn the_outcome_lists_are_read_from_the_directory_cargo_mutants_writes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let out = dir.path().join("mutants.out");
        std::fs::create_dir_all(&out).expect("mutants.out");
        std::fs::write(out.join("missed.txt"), format!("{A_SURVIVOR}\n")).expect("missed.txt");

        assert_eq!(
            read_outcome_list(dir.path(), "missed.txt").as_deref(),
            Some(format!("{A_SURVIVOR}\n").as_str())
        );
        assert_eq!(
            read_outcome_list(dir.path(), "caught.txt"),
            None,
            "a list the tool did not write is absent, not empty -- the difference \
             between `no mutant survived` and `no run happened`"
        );
    }

    #[test]
    fn a_published_reason_does_not_carry_a_build_log_onto_the_pull_request() {
        let long = "e".repeat(2_000);
        assert!(tail(&long).len() < 500, "stderr must be trimmed");
        assert!(tail("short").starts_with("short"));
    }

    #[test]
    fn a_claimed_survivor_with_no_list_to_show_for_it_is_not_an_accusation() {
        let outcome = outcome_from_run(Some(2), NO_LISTS, "");
        assert!(
            matches!(outcome, MutantsOutcome::Unavailable(_)),
            "exit 2 says mutants were missed; with no list naming them there is \
             nothing to publish and nothing to accuse: {outcome:?}"
        );
    }

    // ------------------------------------------------------------------
    // 5. THE REAL TOOL (seeded defect, run on demand)
    // ------------------------------------------------------------------

    /// Everything above feeds this gate a tool outcome. This one builds a crate
    /// whose test suite is genuinely inadequate, runs the real cargo-mutants
    /// over it, and requires the gate to fail -- the only check that proves the
    /// flags, the output layout and the exit codes are what this module thinks
    /// they are, rather than what its author read.
    ///
    /// `#[ignore]` because it builds and tests a crate: seconds, not
    /// milliseconds, and it needs cargo-mutants installed. Run it with
    /// `cargo test --lib -- --ignored a_seeded_defect`.
    ///
    /// `is_adult(30)` is asserted true, which kills `with false` and `>= -> <`
    /// but leaves `-> bool with true` alive: the suite never exercises the
    /// boundary the function exists for. That is exactly the class of hole a
    /// filename match cannot see.
    #[tokio::test]
    #[ignore = "builds and tests a crate; requires cargo-mutants"]
    async fn a_seeded_defect_is_caught_by_the_real_tool() {
        const LIB: &str = "pub fn is_adult(age: u32) -> bool {\n    age >= 18\n}\n\n\
                           #[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {\n        \
                           assert!(super::is_adult(30));\n    }\n}\n";
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"seeded\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("manifest");
        std::fs::create_dir_all(dir.path().join("src")).expect("src");
        std::fs::write(dir.path().join("src/lib.rs"), LIB).expect("lib.rs");

        // A unified diff whose new side is the file just written, which is what
        // `--in-diff` validates against the tree.
        let mut diff = String::from("--- /dev/null\n+++ b/src/lib.rs\n@@ -0,0 +1,11 @@\n");
        for line in LIB.lines() {
            diff.push('+');
            diff.push_str(line);
            diff.push('\n');
        }
        let pr = ctx(&diff, &["src/lib.rs"]);

        let report = ChaosMutationGuard::new()
            .measure_diff_mutants(dir.path(), &pr)
            .await;

        assert!(
            !report.is_adequate,
            "the suite never tests the boundary, so `-> bool with true` survives: {}",
            report.summary
        );
        assert!(
            report
                .surviving_findings
                .iter()
                .any(|f| f.file_path == "src/lib.rs" && f.mutation.contains("with true")),
            "the survivor must be named: {:?}",
            report.surviving_findings
        );
        assert!(matches!(report.gate_status(), GateStatus::Failed(_)));
    }

    // ------------------------------------------------------------------
    // 6. MECHANISM (the gate cannot pass by measuring nothing)
    // ------------------------------------------------------------------

    /// This gate's own source, excluding the test module. The marker is
    /// assembled at runtime so the scan does not match the attribute written
    /// inside the test module itself.
    fn production_code() -> String {
        let src = include_str!("chaos_mutation_guard.rs");
        let marker = ["#[cfg(te", "st)]"].concat();
        let end = src.find(&marker).expect("test module marker");
        src[..end]
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<String>()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect()
    }

    #[test]
    fn the_scanned_source_is_the_real_source() {
        let code = production_code();
        assert!(
            code.contains("implChaosMutationGuard"),
            "the scans below must be reading this gate's actual code"
        );
        assert!(
            !code.contains("thescansbelow"),
            "comments must be stripped, or a needle could be satisfied by prose"
        );
    }

    #[test]
    fn the_filename_heuristic_is_absent_from_the_gate() {
        let code = production_code();
        let needle = ["contains(\"te", "st\")"].concat();
        assert!(
            !code.contains(&needle),
            "the gate decided adequacy by matching `test` in a path; it must not return"
        );
        let flag = ["has_test", "_changes"].concat();
        assert!(
            !code.contains(&flag),
            "the filename verdict must not return"
        );
    }

    #[test]
    fn the_gate_really_spawns_the_mutation_tool_and_bounds_it() {
        let code = production_code();
        assert!(
            code.contains("Command::new(\"cargo\")") && code.contains(".arg(\"mutants\")"),
            "a mutation gate that spawns no mutation tool measures nothing"
        );
        assert!(
            code.contains("\"--in-diff\""),
            "mutants must be scoped to the lines this pull request changed, \
             or the gate cannot run per PR"
        );
        assert!(
            code.contains("run_bounded_for(cmd,MUTATION_BUDGET"),
            "I5: every subprocess is bounded and killed on drop"
        );
        let raw_wait = [".out", "put()"].concat();
        assert!(
            !code.contains(&raw_wait),
            "I5: no unbounded direct subprocess wait in this gate"
        );
    }

    #[test]
    fn the_budget_is_the_documented_ten_minutes() {
        assert_eq!(MUTATION_BUDGET, Duration::from_secs(600));
    }
}
