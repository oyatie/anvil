//! Which stages of the pipeline are actually invoked by production code.
//!
//! # The class this closes
//!
//! `gate_proof` makes a gate demonstrate it can FIRE. Nothing made a stage
//! demonstrate it RUNS, and the difference is not academic: three stages were
//! found dead in a single day, each complete, documented and tested.
//!
//!   * `webhook::next_phase` decides what happens after a verdict — enlist,
//!     fix, or halt. It had no caller, so the reject arm did nothing at all and
//!     a pull request anvil asked to change sat until a person noticed.
//!   * `core.hooksPath` pointed at a directory that did not exist, so every
//!     local rung ran nothing while the suite stayed green.
//!   * The enlist doors refused every input on a premise that had stopped
//!     being true, so the merge queue was unreachable.
//!
//! None of the three was a bug in the code that failed. Each was correct and
//! unreached, which no unit test can catch: the function under test was right
//! the whole time.
//!
//! # Why a declared registry rather than inference
//!
//! Reachability alone answers the wrong question. A module inside a reachable
//! parent scores reachable even when nothing calls it — `webhook::next_phase`
//! measured as reachable all session while having zero callers. So each stage
//! names the SYMBOL whose presence in production code proves it is invoked,
//! and that symbol is looked for outside the stage's own files.
//!
//! # What this is not
//!
//! It does not prove a stage ran in production, only that something outside it
//! calls it. A caller behind a condition that is never true would still pass.
//! That is a weaker claim than the name suggests and is stated here rather
//! than left for a reader to discover.

use crate::source_scan::{code_only, without_test_modules};

/// One stage, and the evidence that anything runs it.
#[derive(Debug, Clone, Copy)]
pub struct Stage {
    /// The stage, as this codebase names it.
    pub stage: &'static str,
    /// A symbol whose presence in production code proves the stage is invoked.
    ///
    /// Not the module path: a `use` line mentions the module without calling
    /// anything, and `next_phase` was imported nowhere and still compiled.
    pub invocation: &'static str,
    /// Path fragment identifying the stage's own files, which are excluded
    /// from the search. Without it a stage proves its own liveness.
    pub owns: &'static str,
    /// What is lost while nothing runs it. A row that cannot say this is a row
    /// nobody can act on.
    pub loses: &'static str,
}

/// Stages that must be reachable, and the evidence for each.
pub const STAGES: &[Stage] = &[
    Stage {
        stage: "webhook::next_phase",
        invocation: "next_phase(",
        owns: "webhook/next_phase.rs",
        loses: "a verdict reaches no decision: REQUEST_CHANGES chains into \
                nothing and APPROVE never enlists",
    },
    Stage {
        stage: "postmortem",
        invocation: "postmortem::",
        owns: "postmortem",
        loses: "no fix is classified to the rung that should have caught it, \
                so \"CI is debt\" stays a slogan rather than a measurement",
    },
    Stage {
        stage: "gate_proof",
        invocation: "gate_proof::",
        owns: "gate_proof",
        loses: "nothing in production asks whether a gate has ever demonstrated \
                it can fire; 23 have not",
    },
    Stage {
        stage: "shape::facade::sweep",
        invocation: "sweep_repo(",
        owns: "shape/facade/sweep.rs",
        loses: "no whole-repository conformance audit runs, so structural drift \
                is only ever seen through the narrow window of one diff",
    },
    Stage {
        stage: "brand_absence",
        invocation: "brand_absence::",
        owns: "brand_absence",
        loses: "aspirational naming accumulates unmeasured",
    },
    Stage {
        stage: "source_scan",
        invocation: "source_scan::",
        owns: "source_scan",
        loses: "the shared stripper is unused, and each scanner grows its own \
                copy — twelve already had the same blind spot",
    },
    Stage {
        stage: "migration::registry",
        invocation: "migration::",
        owns: "migration",
        loses: "the recorded destination of every component is consulted by \
                nothing, so it drifts from the law it encodes",
    },
    Stage {
        stage: "cloud_native_guard",
        invocation: "cloud_native_guard::",
        owns: "cloud_native_guard",
        loses: "the gate publishes no verdict on any pull request",
    },
    Stage {
        stage: "stack_whitelist_guard",
        invocation: "stack_whitelist_guard::",
        owns: "stack_whitelist_guard",
        loses: "the gate publishes no verdict on any pull request",
    },
    Stage {
        stage: "dual_track_build_guard",
        invocation: "dual_track_build_guard::",
        owns: "dual_track_build_guard",
        loses: "the gate publishes no verdict on any pull request",
    },
];

/// Stages with no production caller today.
///
/// EXACT, not a ceiling. A stage that quietly gains a caller must be noticed
/// as much as one that loses it: this number falling is the work, and a `<=`
/// bound would let a newly-dead stage hide beneath an old one.
///
/// Six, not the ten a first hand-written probe reported. That probe stripped
/// `#[cfg(test)]` by counting braces and matched string literals with a regex
/// that did not handle escapes, so it wrongly condemned four stages that do
/// have callers. Using `code_only` and `without_test_modules` — the strippers
/// the rest of this codebase already relies on — is what made the answer
/// correct, and is the argument against every scanner growing its own.
///
/// Six became four when nine branches integrated: `webhook::next_phase` gained
/// `webhook/pipelines/review.rs:338` and `postmortem` gained
/// `publish/scorecard.rs:317`. Recorded here rather than on either branch
/// because neither could see the fall alone — the count is a property of the
/// merged tree, and this merge is the change that moved it. The fall was
/// proven before it was recorded: both call sites were read, and the scanner
/// was confirmed to have correctly ignored the one mention of `postmortem::`
/// that lives in a doc comment. A count that falls without that check is how
/// a blind spot gets laundered into progress.
/// Four became three when `gate_proof` gained `publish/scorecard.rs`. It knew
/// which gates have been seeded with their own defect and which have only
/// been green, and no pull request was ever told; the qualifier on the blocked
/// scorecard is the caller. The three that remain -- `cloud_native_guard`,
/// `stack_whitelist_guard`, `dual_track_build_guard` -- publish no verdict on
/// any pull request, which is the same defect in the same shape.
pub const STAGES_WITHOUT_A_CALLER: usize = 3;

/// Stages nothing outside their own files invokes.
///
/// `sources` is `(path, contents)` for production Rust. Comments, string
/// literals and `#[cfg(test)]` modules are removed first: a stage named in a
/// doc comment is documented, not called, and `next_phase` appeared in exactly
/// one place in `src/` — a doc comment.
pub fn uninvoked(sources: &[(String, String)]) -> Vec<&'static Stage> {
    STAGES
        .iter()
        .filter(|s| {
            !sources.iter().any(|(path, text)| {
                !path.contains(s.owns)
                    && code_only(&without_test_modules(text)).contains(s.invocation)
            })
        })
        .collect()
}
