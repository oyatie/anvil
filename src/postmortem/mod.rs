//! Every fix is a class we failed to catch earlier. This is the ledger of them.
//!
//! # The doctrine
//!
//! A `fix(...)` commit is not a unit of work. It is evidence that a class of
//! defect reached somewhere expensive enough to need repairing by hand, which
//! means no check refused it earlier or cheaper. So every review finding and
//! every fix is a CANDIDATE FOR ADMISSION into the pipeline, and this is where
//! that admission is recorded.
//!
//! Admitting one asks four questions, in order:
//!
//! 1. **What is the underlying issue, from first principles?** Not the symptom.
//!    Thirteen of them fabricated file paths; the issue was that thirteen places
//!    each parsed a diff.
//! 2. **How is the CLASS prevented, rather than the instance?** A fix that
//!    repairs one site leaves the other twelve and the fourteenth.
//! 3. **How is that automated reusably?** One mechanism other classes can also
//!    use, not a bespoke check per defect.
//! 4. **Which layer, and is it more than one?** Frequently more than one: a
//!    type that makes the defect unspellable AND a scan that finds the sites
//!    already written.
//!
//! # CI is the last resort, not the default
//!
//! A class caught in CI has already been written, committed, pushed and put in
//! front of a reviewer. The run only reports that it happened -- and every
//! report is another wave of fixes. CI still matters as the backstop, but a
//! class whose only remedy is a CI check has not been prevented; it has been
//! observed. [`Layer::Ci`] therefore counts as debt, and the entry must name
//! the earlier layer it should move to.
//!
//! # Deterministic work does not belong in the semantic layer
//!
//! A check that can be decided by reading the source must be
//! [`Mechanism::Mechanical`]: a type, a lint, a scan. Judgement is expensive,
//! non-deterministic, and cannot be re-run to the same answer, so it is spent
//! only where the question genuinely requires it. Every
//! [`Mechanism::Semantic`] remedy has to say why a mechanism could not decide
//! it.
//!
//! # Why a compiled table and not a document
//!
//! The pipeline is the living documentation. A post-mortem nobody executes
//! decays into prose; this one compiles, its `Live` remedies are checked to
//! exist, and the count of classes with no early remedy is a ratchet that must
//! fall. Same shape as `fidelity::registry` and
//! `pre_merge_guard::admission::ABSENCE_POLICY`.

/// Where a class is refused, cheapest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    /// Prevented. The defect has no spelling: one entry point, or a type that
    /// cannot express the wrong thing. Nothing to catch, because nothing can be
    /// written.
    Unspellable,
    /// Caught as it is written, by rustc or clippy, in the editor.
    Implementation,
    /// Refused before it becomes a commit.
    PreCommit,
    /// Refused before it becomes a pull request.
    PrePush,
    /// CI. Everything above has already been paid for, and the finding is
    /// another wave of fixes rather than a prevention.
    Ci,
}

impl Layer {
    /// Whether arriving here means the defect already happened.
    pub fn is_after_the_fact(self) -> bool {
        matches!(self, Layer::Ci)
    }
}

/// How a remedy decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mechanism {
    /// Deterministic: a type, a lint, a source scan. Same input, same answer,
    /// every time, at no inference cost.
    Mechanical,
    /// Requires judgement. Expensive, non-reproducible, and to be spent only
    /// where a mechanism genuinely cannot decide.
    Semantic {
        /// Why a mechanism could not decide this.
        why_not_mechanical: &'static str,
    },
}

/// Whether a remedy exists yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Live. `named` must exist in the tree, which is asserted -- a remedy
    /// naming something absent is worse than none, because it reads as covered.
    Live { named: &'static str },
    /// Not built. The entry is a work item, not a shrug.
    Missing,
}

/// One way a class is refused, at one layer.
#[derive(Debug, Clone, Copy)]
pub struct Remedy {
    pub layer: Layer,
    pub mechanism: Mechanism,
    /// What the remedy actually does, in one sentence.
    pub what: &'static str,
    pub status: Status,
}

/// A class of defect, why it happens, and everything that refuses it.
#[derive(Debug, Clone, Copy)]
pub struct FixClass {
    pub id: &'static str,
    /// The class, in the terms a reviewer would recognise it by.
    pub what: &'static str,
    /// The underlying issue, not the symptom.
    pub first_principles: &'static str,
    pub instances: usize,
    /// Specific enough to re-derive the count.
    pub evidence: &'static str,
    /// Often more than one: a class can be prevented going forward AND swept
    /// for existing sites.
    pub remedies: &'static [Remedy],
}

impl FixClass {
    /// The earliest layer at which a LIVE remedy refuses this class.
    pub fn earliest_live_layer(&self) -> Option<Layer> {
        self.remedies
            .iter()
            .filter(|r| matches!(r.status, Status::Live { .. }))
            .map(|r| r.layer)
            .min()
    }

    /// Whether this class is only ever caught after the fact, or not at all.
    ///
    /// The question the ledger exists to answer. A class whose earliest live
    /// remedy is CI has been observed, not prevented.
    pub fn only_caught_after_the_fact(&self) -> bool {
        match self.earliest_live_layer() {
            None => true,
            Some(l) => l.is_after_the_fact(),
        }
    }
}

/// The classes this session produced, and what refuses each of them.
///
/// Measured from twelve pull requests (#113-#124, 5304 lines added). Six
/// classes, not twelve bugs -- and one of them generated most of the others.
pub const FIX_CLASSES: &[FixClass] = &[
    FixClass {
        id: "n-copies-of-one-logic",
        what: "The same logic pasted N times, so one defect becomes N defects and one fix \
               becomes N fixes.",
        first_principles: "Copying is cheaper than extracting at the moment of writing, and the \
                           cost is paid later by whoever finds the defect -- N times over, once \
                           per copy, by which point the copies have drifted and no longer share \
                           a fix. This is the GENERATING class: the thirteen that \
                           fabricated paths and the seven that refused deletions had those \
                           defects because thirteen places each parsed a diff. Retiring the \
                           duplication retired both.",
        instances: 28,
        evidence: "Diff parsing duplicated across thirteen of them, retired by `diffs_by_path` \
                   (#117, #118); the `Fix` enum spelled twice (#113); the `unknown.rs` cursor \
                   block in three of them (#117); and NINE spellings of \
                   \"strip commentary before scanning source\" under four different behaviours \
                   and one name -- a four-line filter that drops whole comment lines, a per-line \
                   truncation with a crude quote guard, a line-wise lexer, and a whole-source \
                   state machine. That last is the only one that sees a literal spanning lines, \
                   and the weakest was reached first by a reader who assumed the strongest.",
        remedies: &[
            Remedy {
                layer: Layer::Unspellable,
                mechanism: Mechanism::Mechanical,
                what: "one parser, `diffs_by_path`, which takes the path from a header that \
                       states it and attributes nothing to a hunk naming no file",
                status: Status::Live {
                    named: "src/git_manager/diff_context.rs::diffs_by_path",
                },
            },
            Remedy {
                layer: Layer::PrePush,
                mechanism: Mechanism::Mechanical,
                what: "the ratchet runs in `pre-push`, so a fourteenth parser never reaches the \
                       remote. Not `pre-commit`: the scan reads source and needs no network, but \
                       it needs the test harness compiled, and that hook is seconds-only. \
                       Moving it earlier means extracting the scan into a binary that shares its \
                       logic with the test rather than re-spelling it",
                status: Status::Live {
                    named: "src/git_manager/hooks/pre-push",
                },
            },
            Remedy {
                layer: Layer::Unspellable,
                mechanism: Mechanism::Mechanical,
                what: "one `code_only`, in `src`, so a scan reads code rather than commentary \
                       and a caller cannot reach a weaker spelling by accident; it preserves \
                       byte offsets, so a finding can name a line without a second pass",
                status: Status::Live {
                    named: "src/source_scan/mod.rs::code_only",
                },
            },
            Remedy {
                layer: Layer::Ci,
                mechanism: Mechanism::Mechanical,
                what: "an exact count of the remaining local spellings, so a ninth cannot be \
                       written and each migration must lower it; the scan uses `code_only` \
                       itself, without which it counted its own string literals",
                status: Status::Live {
                    named: "tests/source_scan_test.rs",
                },
            },
            Remedy {
                layer: Layer::Ci,
                mechanism: Mechanism::Mechanical,
                what: "the same ratchet as a backstop, exact rather than a ceiling so removing \
                       parsers must lower the count in the same change",
                status: Status::Live {
                    named: "tests/diff_parsing_ratchet_test.rs",
                },
            },
        ],
    },
    FixClass {
        id: "absence-read-as-a-verdict",
        what: "Absence of a finding published as a verdict, in either direction: no finding read \
               as a pass, or no measurement read as a failure that blocks.",
        first_principles: "An empty result set and an unrun check produce the same value -- an \
                           empty list -- so any type that returns one cannot distinguish them, \
                           and every caller must remember to. Callers do not. The repair is not \
                           to remember harder but to make the two different values.",
        instances: 17,
        evidence: "Thirteen of them published a path never read out of the diff (#116-#120); \
                   `NotMeasured` counted as a pass in the tally; admission refused EVERY absence, \
                   so 34 of the 72 left no pull request admissible (#123); an empty subject set \
                   reported as a Warning among real findings (#123).",
        remedies: &[
            Remedy {
                layer: Layer::Unspellable,
                mechanism: Mechanism::Mechanical,
                what: "`Evaluated::measured` is the only constructor and refuses a measurement \
                       over zero subjects, so \"examined nothing, found nothing, therefore \
                       clean\" has no spelling",
                status: Status::Live {
                    named: "src/harness/mod.rs::measured",
                },
            },
            Remedy {
                layer: Layer::Unspellable,
                mechanism: Mechanism::Mechanical,
                what: "`NotApplicable` is a separate status from `NotMeasured`, so a gate cannot \
                       report an empty subject set as a defect or as a pass",
                status: Status::Live {
                    named: "src/pre_merge_guard/report.rs::NotApplicable",
                },
            },
            Remedy {
                layer: Layer::Ci,
                mechanism: Mechanism::Mechanical,
                what: "the admission policy's four properties, including that an unlisted gate \
                       still blocks so the classification cannot become an off switch",
                status: Status::Live {
                    named: "tests/admission_absence_policy_test.rs",
                },
            },
        ],
    },
    FixClass {
        id: "inversion-flagging-a-removal",
        what: "A scanner reads the whole diff instead of the lines a change ADDS, so it refuses \
               the pull request that DELETES the thing it is looking for.",
        first_principles: "A unified diff is one string containing both sides, so reading it \
                           whole is the DEFAULT and reading only additions is the deliberate \
                           act. The defect is what you get by not thinking about it, which is \
                           why it recurred even after being fixed twice.",
        instances: 7,
        evidence: "The credential scanner; gate 41's policy scan; gate 64's allocation rule; \
                   three more in #117; and `evaluate_formal_invariants` again in #115, written \
                   fresh with the same defect after the first two fixes.",
        remedies: &[
            Remedy {
                layer: Layer::Unspellable,
                mechanism: Mechanism::Mechanical,
                what: "`FileDiff` hands out `added()` and `after_change()`, neither of which \
                       contains a removed line; the only corpus that does is reached through \
                       `both_sides(BothSides::..)`, a closed set of reasons, so asking for \
                       removals is a named act that appears in review",
                status: Status::Live {
                    named: "src/git_manager/diff_context.rs::BothSides",
                },
            },
            Remedy {
                layer: Layer::Ci,
                mechanism: Mechanism::Mechanical,
                what: "assert neither ordinary corpus contains a removed line, that exactly one \
                       rule reads the removed side, and that the sanctioned-reason set stays \
                       closed -- a second variant is a design decision, not a refactor",
                status: Status::Live {
                    named: "tests/removals_are_not_reachable_by_accident_test.rs",
                },
            },
        ],
    },
    FixClass {
        id: "report-claims-what-the-code-cannot-support",
        what: "A published report states something the code does not compute: a count nothing \
               derives, a verdict nothing measured, a citation pointing at code that moved.",
        first_principles: "Prose and code are edited by different acts. A number written into a \
                           sentence is correct once, at the moment of writing, and nothing \
                           re-derives it afterwards -- so it decays silently while continuing to \
                           be read as current.",
        instances: 8,
        evidence: "Stale prose counts, including `--help` publishing a count of 21 (#114); a \
                   fabricated SMT-solver verdict over a two-substring scan (#115); line \
                   citations rotting four times in one day (#122).",
        remedies: &[
            Remedy {
                layer: Layer::Unspellable,
                mechanism: Mechanism::Mechanical,
                what: "cite a symbol rather than a line number, so the window is located by \
                       searching for the definition and moves with the code",
                status: Status::Live {
                    named: "tests/fidelity_registry_citations_test.rs::symbol_window",
                },
            },
            Remedy {
                layer: Layer::PrePush,
                mechanism: Mechanism::Mechanical,
                what: "the prose-count scan runs in `pre-push`, so a stale count never reaches a \
                       reviewer. Same reason it is not `pre-commit`: source-only, but it needs \
                       the harness built",
                status: Status::Live {
                    named: "src/git_manager/hooks/pre-push",
                },
            },
            Remedy {
                layer: Layer::Ci,
                mechanism: Mechanism::Mechanical,
                what: "the prose-count gate, over documentation and Rust doc comments alike",
                status: Status::Live {
                    named: "tests/prose_counts_test.rs",
                },
            },
        ],
    },
    FixClass {
        id: "supervisor-tighter-than-its-subject",
        what: "A timeout or watchdog bounded more tightly than the work it supervises, so it \
               truncates healthy work and then reports the failure it caused.",
        first_principles: "Two deadlines for one operation are written in two places and nothing \
                           relates them, so they drift. The supervisor wins because it fires \
                           first, and its report names a stall rather than the truncation it \
                           performed -- so the evidence points away from the cause.",
        instances: 2,
        evidence: "The doc-parity probe told agy it had 120s and was supervised with a hardcoded \
                   30s; separately, the inactivity window also governed the wait for the FIRST \
                   token, so a model was killed for thinking (#124).",
        remedies: &[
            Remedy {
                layer: Layer::Unspellable,
                mechanism: Mechanism::Mechanical,
                what: "`SupervisedTurn` is one value yielding every deadline for a turn -- the \
                       watchdog's budget, the process bound, and the tool's own `--print-timeout` \
                       derived by subtracting the margin. There were THREE numbers in three \
                       places with nothing relating them; now none can be tightened without the \
                       others",
                status: Status::Live {
                    named: "src/exec/mod.rs::SupervisedTurn",
                },
            },
            Remedy {
                layer: Layer::Ci,
                mechanism: Mechanism::Mechanical,
                what: "assert the inner deadline lands inside the outer budget, and that the \
                       probe supervises its own constant rather than a smaller literal",
                status: Status::Live {
                    named: "tests/supervisor_outlives_what_it_supervises_test.rs",
                },
            },
        ],
    },
    FixClass {
        id: "a-gate-green-for-its-own-defect",
        what: "A check that does not fire on the defect it exists to catch, and so buys down \
               scrutiny without earning it.",
        first_principles: "A check is written by asserting what it should catch, and passes from \
                           the moment it compiles -- so a green tells you nothing about whether \
                           it can fail. Nothing distinguishes a check that found nothing from \
                           one that cannot find anything, and the second is worse than no check \
                           because it occupies the slot.",
        instances: 4,
        evidence: "The diff-parsing ratchet had two sites of slack and missed a seeded parser; \
                   the same ratchet was blind to five real parsers until anvil's own review said \
                   so; `path.rs::symbol` citations were silently DROPPED rather than checked; \
                   `symbol_window` did not recognise `pub field:`. Every one was found by \
                   seeding the defect, none by reading the code.",
        remedies: &[
            Remedy {
                layer: Layer::Unspellable,
                mechanism: Mechanism::Mechanical,
                what: "`Rule::fixture` is not optional: a rule ships a defect it must flag and a \
                       twin it must spare, and the harness runs both before trusting any verdict",
                status: Status::Live {
                    named: "src/harness/mod.rs::fixture",
                },
            },
            Remedy {
                layer: Layer::Unspellable,
                mechanism: Mechanism::Mechanical,
                what: "the same obligation for the 72 hand-wired ones, which are not on the \
                       harness and so are not required to demonstrate they fire",
                status: Status::Missing,
            },
        ],
    },
];

/// Classes whose earliest live remedy is CI, or which have none at all.
///
/// EXACT, and it must fall. This is the number the doctrine is about: a class
/// counted here is observed rather than prevented, and every observation is
/// another wave of fixes.
///
/// It is ZERO. Every class recorded from this session is now refused before CI
/// -- four of them at the type level, where the defect has no spelling, and two
/// in `pre-push`, where they never reach a reviewer.
///
/// Zero here does NOT mean the work is finished, and the number must not be
/// read that way. It means no class is caught *only* after the fact.
/// `missing_remedies()` is the live backlog: remedies named, argued for, and
/// not yet built -- chiefly the obligation that the seventy-two hand-wired
/// gates demonstrate they fire, which `Rule::fixture` already makes unspellable
/// for anything on the harness and which nothing forces on the rest.
pub const CLASSES_ONLY_CAUGHT_AFTER_THE_FACT: usize = 0;

/// The classes still waiting for a remedy earlier than CI.
pub fn awaiting_early_remedy() -> Vec<&'static FixClass> {
    FIX_CLASSES
        .iter()
        .filter(|c| c.only_caught_after_the_fact())
        .collect()
}

/// Remedies named but not built, across every class.
pub fn missing_remedies() -> Vec<(&'static FixClass, &'static Remedy)> {
    FIX_CLASSES
        .iter()
        .flat_map(|c| c.remedies.iter().map(move |r| (c, r)))
        .filter(|(_, r)| matches!(r.status, Status::Missing))
        .collect()
}

/// Total instances recorded, across every class.
pub fn total_instances() -> usize {
    FIX_CLASSES.iter().map(|c| c.instances).sum()
}
