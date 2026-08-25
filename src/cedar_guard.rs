//! Gate 2: offline verification of the Cedar policies a change touches.
//!
//! # What this gate used to do
//!
//! It spawned `agy` at `ExecClass::Model` -- up to ten minutes -- asked it in
//! strict JSON whether the pull request's routes were covered by Cedar policy,
//! parsed the answer with serde, and then set `is_compliant: true` on all three
//! of its exits. The third exit was the branch reached *after* that evaluation
//! came back `is_cedar_compliant: false`: it spawned a second model turn to
//! write `.cedar` files and reported compliant regardless of what the second
//! turn produced. When the second turn wrote nothing, the published status was
//! `Passed` -- a green from the branch whose precondition is a finding.
//! `GateStatus::Failed` in the evaluator was unreachable from production.
//!
//! `certify.rs` then staged and committed whatever that second turn had
//! written. Authorization policy, authored by a model, committed to the
//! repository, parsed by nothing.
//!
//! # What it does now
//!
//! `cedar check-parse` -- the reference implementation's own CLI, the same
//! `cedar-policy` code AWS runs inside Verified Permissions -- over the
//! `.cedar` files this pull request touched, fed on stdin, one spawn per file
//! at `ExecClass::Quick`. No model, no generation, no commit.
//!
//! That decides exactly one property, and it decides it soundly: the policy set
//! is grammatical Cedar. Four outcomes, and only four:
//!
//! - `Passed` -- the checker ran and accepted every policy file in scope.
//! - `Failed` -- the checker ran and rejected one, carrying its diagnostics.
//! - `NotMeasured` -- the checker is absent, was killed by the timeout, was
//!   called wrongly, or no policy file was in scope. Absent evidence: never a
//!   pass (I1) and never an accusation.
//! - and nothing else. There is no variant that certifies without an answer.
//!
//! # What it deliberately does not claim
//!
//! Everything past the grammar needs a schema. `Validator::new` takes `Schema`
//! by value, not `Option<Schema>`; the `cedar validate` subcommand takes
//! `--schema` as a required argument, and so does `cedar symcc`. Without one,
//! `PolicySet::from_str` returns `Ok` for `principal.totallyFakeAttr` and for
//! `Uzer::"alice"`, because without a schema those may be perfectly real. So
//! unrecognised entity types, attribute typos, operand type errors,
//! action-applicability and every "is this policy equivalent to that one"
//! question are out of reach here, and the gate's headline claim -- that a
//! policy covers the route this pull request added -- is out of reach with
//! them. This repository has no Cedar schema and no entity store. The gate
//! reports that rather than asking a model to guess.

use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::info;

use crate::exec::{ExecClass, run_bounded_with_stdin};
use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::GateStatus;

/// The field name this gate occupies on `PreMergeCertificationReport`.
///
/// `NotMeasured` is recorded under this id, so `unmeasured_gates` names a gate
/// a reader can look up in `src/fidelity/registry.rs`.
pub const CEDAR_GATE_ID: &str = "cedar_status";

/// What came back from the policy checker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CedarToolOutcome {
    /// The checker ran and accepted every policy it was given.
    Accepted,
    /// The checker ran and rejected a policy. Carries its diagnostics verbatim:
    /// a finding a reader cannot act on is barely better than no finding.
    Rejected(String),
    /// No verdict exists -- the binary is absent, the spawn failed, the timeout
    /// fired, or Anvil called the tool in a way it did not understand. Never a
    /// pass and never an accusation.
    Unavailable(String),
}

/// This gate's finding, as the evaluator publishes it.
///
/// The verdict is a `GateStatus` and not a boolean on purpose. A boolean cannot
/// distinguish "the checker accepted the policies" from "there was no checker",
/// and it was that collapse -- `is_compliant: true` on the exit reached after a
/// non-compliant evaluation -- that made this gate unfailable.
#[derive(Debug, Clone)]
pub struct CedarGuardReport {
    pub status: GateStatus,
    pub summary: String,
}

/// The Cedar policy files this pull request touched.
///
/// The extension, not the word. The predicate this replaces was
/// `f.contains("api") || f.contains("route") || f.contains("handler")`, which
/// admits any Rust file with `api` in its path and no policy file that lacks
/// one; the same substring-as-scope mistake put `src/migration/registry.rs`
/// into schema-migration scope. `.cedar` is the extension the Cedar CLI and
/// every published example use, and it is what a parser can actually read.
pub fn policy_files_in_scope(changed_files: &[String]) -> Vec<String> {
    changed_files
        .iter()
        .filter(|f| f.ends_with(".cedar"))
        .cloned()
        .collect()
}

/// Reads the checker's exit into an outcome.
///
/// The exit code carries the distinction that matters. `cedar` is a clap
/// program: clap exits `2` when it cannot parse its own command line, and the
/// checker exits `1` when it can and the *policy* is bad. Collapsing the two
/// would let a flag rename inside Anvil accuse every pull request in the fleet
/// of writing invalid authorization policy. A death by signal leaves no code at
/// all, which is likewise not the pull request's doing.
pub fn interpret_cedar_outcome(exit_code: i32, stdout: &str, stderr: &str) -> CedarToolOutcome {
    let said = {
        let joined = format!("{}\n{}", stderr.trim(), stdout.trim());
        let trimmed = joined.trim().to_string();
        if trimmed.is_empty() {
            format!("exit status {exit_code}, no output")
        } else {
            trimmed
        }
    };
    match exit_code {
        0 => CedarToolOutcome::Accepted,
        2 => CedarToolOutcome::Unavailable(format!(
            "the cedar CLI rejected Anvil's own invocation: {said}"
        )),
        _ => CedarToolOutcome::Rejected(said),
    }
}

/// The finding when this pull request touched no Cedar policy file at all.
///
/// Not a pass. The scope a parser can read -- files ending in the Cedar
/// extension -- is read exhaustively off the diff, so an empty one really is an
/// observation about the policy corpus. But this gate's name is coverage, and
/// coverage of the actions the change *does* touch is the half nothing here
/// measures: deciding it needs a schema, and there is none. An empty scope is
/// therefore "did not look", which is I1's absent evidence.
pub fn no_policy_in_scope() -> CedarGuardReport {
    let summary = "This pull request touched no Cedar policy file (*.cedar), so no policy set \
                   was parsed. Whether the actions it does touch are covered by a policy is not \
                   decidable here: the validate and symcc subcommands each take --schema as a \
                   required argument, and this repository carries no Cedar schema and no entity \
                   store to decide a request against."
        .to_string();
    CedarGuardReport {
        status: GateStatus::NotMeasured {
            gate_id: CEDAR_GATE_ID.to_string(),
            reason: summary.clone(),
        },
        summary,
    }
}

/// The gate status a scope and a checker answer are between them entitled to.
///
/// Total over both, and deliberately so: every way this gate can end is one arm
/// of this match, so no exit can be added that quietly certifies. An empty
/// scope ignores the outcome entirely -- a checker that ran and was happy has
/// said nothing about a pull request whose policy files it never saw.
///
/// `policy_files` is the scope: what the pull request touched. `parsed_files`
/// is what the checker actually read, which is the smaller list whenever the
/// change deleted a policy -- its path is in the diff and nothing is on disk.
/// A pass is over `parsed_files` and names only those. The two were the same
/// list once, and a pass over one file published "accepted 2 Cedar policy
/// file(s)" naming a file that did not exist: the same vacuously-clean shape
/// `combine_outcomes` guards at zero, at one-of-two instead of none-of-any.
pub fn verify(
    policy_files: &[String],
    parsed_files: &[String],
    outcome: &CedarToolOutcome,
) -> CedarGuardReport {
    if policy_files.is_empty() {
        return no_policy_in_scope();
    }

    let named = policy_files.join(", ");
    match outcome {
        CedarToolOutcome::Accepted => {
            let named = parsed_files.join(", ");
            let summary = format!(
                "cedar check-parse accepted {} Cedar policy file(s): {named}. That is a parse \
                 of the policy set and nothing further -- no schema exists here, so no policy \
                 was type-checked and no request was decided against an entity store.",
                parsed_files.len()
            );
            CedarGuardReport {
                status: GateStatus::Passed,
                summary,
            }
        }
        CedarToolOutcome::Rejected(diagnostics) => {
            let summary =
                format!("cedar check-parse rejected the policy set in {named}: {diagnostics}");
            CedarGuardReport {
                status: GateStatus::Failed(summary.clone()),
                summary,
            }
        }
        CedarToolOutcome::Unavailable(why) => {
            let summary = format!(
                "cedar check-parse could not judge the {} Cedar policy file(s) this pull \
                 request touched ({named}): {why}",
                policy_files.len()
            );
            CedarGuardReport {
                status: GateStatus::NotMeasured {
                    gate_id: CEDAR_GATE_ID.to_string(),
                    reason: summary.clone(),
                },
                summary,
            }
        }
    }
}

#[derive(Default)]
pub struct CedarGuard;

impl CedarGuard {
    pub fn new() -> Self {
        Self
    }

    /// Verifies the Cedar policies this pull request touched.
    ///
    /// Returns a report rather than a `Result`. The previous signature
    /// propagated its error through `certify.rs`, so an absent `agy` binary --
    /// `run_bounded` bails with "failed to run: No such file or directory" --
    /// did not fail gate 2, it ended the certification run and every other
    /// gate never executed. A probe that could not run is
    /// `NotMeasured`; it is not an outage, and the type now says so.
    pub async fn evaluate_cedar_policies(
        &self,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> CedarGuardReport {
        let in_scope = policy_files_in_scope(&diff_ctx.changed_files);
        info!(
            "Running CedarGuard over {} Cedar policy file(s) on {}#{}...",
            in_scope.len(),
            diff_ctx.repo,
            diff_ctx.pr_number
        );

        // Not `verify(&[], ...)`: there is no checker answer to hand it here,
        // and inventing one so the call site type-checks is how a fabricated
        // input gets into a gate.
        if in_scope.is_empty() {
            return no_policy_in_scope();
        }
        let (parsed, outcome) = check_parse_all(repo_dir, &in_scope).await;
        verify(&in_scope, &parsed, &outcome)
    }
}

/// Runs the checker over every policy file in scope and combines the answers.
///
/// A rejection outranks an unavailability: a diagnostic the checker really
/// produced is evidence, and discarding it because a later file timed out would
/// lose the only finding there was. An unavailability short-circuits the loop,
/// because a checker that could not be spawned once will not be spawned twice.
async fn check_parse_all(repo_dir: &Path, in_scope: &[String]) -> (Vec<String>, CedarToolOutcome) {
    let mut rejections: Vec<String> = Vec::new();
    let mut unavailable: Option<String> = None;
    let mut parsed: Vec<String> = Vec::new();

    for file in in_scope {
        // A policy file this pull request *deleted* is in the changed list and
        // not on disk. There is nothing to parse, and nothing to hold against
        // the change either -- and nothing to claim a pass over, which is why
        // the paths that were read are returned rather than counted.
        let Ok(source) = tokio::fs::read_to_string(repo_dir.join(file)).await else {
            continue;
        };
        parsed.push(file.clone());
        match check_parse(&source).await {
            CedarToolOutcome::Accepted => {}
            CedarToolOutcome::Rejected(diagnostics) => {
                rejections.push(format!("{file}: {diagnostics}"));
            }
            CedarToolOutcome::Unavailable(why) => {
                unavailable = Some(why);
                break;
            }
        }
    }

    let outcome = combine_outcomes(&rejections, unavailable.as_deref(), parsed.len());
    (parsed, outcome)
}

/// Folds what the checker said about each file into one answer for the set.
///
/// Three decisions live here, and each of them can send a pull request the
/// wrong way, so none of them belongs inside a loop no test can reach:
///
/// - **A rejection outranks an unavailability.** A diagnostic the checker
///   really produced is evidence; dropping it because a later file timed out
///   would lose the only finding there was.
/// - **Nothing parsed is not a pass.** Every file in scope can be unreadable --
///   a change that *deletes* its policies leaves their paths in the diff and
///   nothing on disk. Zero rejections out of zero files parsed is vacuously
///   clean, which is the shape that made four other gates in this corpus
///   publish greens over corpora they never had.
/// - **Silence from a checker that ran is an acceptance**, and only then.
pub fn combine_outcomes(
    rejections: &[String],
    unavailable: Option<&str>,
    checked: usize,
) -> CedarToolOutcome {
    if !rejections.is_empty() {
        return CedarToolOutcome::Rejected(rejections.join("; "));
    }
    if let Some(why) = unavailable {
        return CedarToolOutcome::Unavailable(why.to_string());
    }
    if checked == 0 {
        return CedarToolOutcome::Unavailable(
            "none of the Cedar policy files in scope could be read from the workspace".to_string(),
        );
    }
    CedarToolOutcome::Accepted
}

/// One policy set, fed to `cedar check-parse` on stdin.
///
/// stdin rather than `--policies <path>`: with no arguments the subcommand
/// reads the policy set from stdin, which is documented behaviour and does not
/// depend on a flag name that could be renamed under us -- and a renamed flag
/// is exactly the usage error that must not read as a policy defect.
pub async fn check_parse(source: &str) -> CedarToolOutcome {
    let mut cmd = Command::new("cedar");
    // `cedar` renders diagnostics through miette's graphical handler and does
    // not gate it on a TTY; neither NO_COLOR nor TERM=dumb suppresses it, so
    // raw ANSI escapes were reaching `GateStatus::Failed` and the scorecard.
    // `--error-format` is a top-level flag, so it does not disturb the stdin
    // fallback the subcommand takes when it is given no policy path.
    cmd.arg("--error-format").arg("plain");
    cmd.arg("check-parse");
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    match run_bounded_with_stdin(
        cmd,
        source,
        ExecClass::Quick.timeout(),
        "cedar check-parse (cedar guard)",
    )
    .await
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            match output.status.code() {
                Some(code) => interpret_cedar_outcome(code, &stdout, &stderr),
                // Killed by a signal: no exit code, so no verdict.
                None => CedarToolOutcome::Unavailable(format!(
                    "cedar check-parse was terminated by a signal without an exit code: {}",
                    stderr.trim()
                )),
            }
        }
        Err(e) => CedarToolOutcome::Unavailable(format!(
            "{e}. The checker is `cedar-policy-cli`; install it with \
             `cargo install cedar-policy-cli`."
        )),
    }
}
