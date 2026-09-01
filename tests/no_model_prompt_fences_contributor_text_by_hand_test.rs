//! ADVISORY, NOT A RATCHET. What this scan measures is narrower than its name.
//!
//! Proven holes, each demonstrated by seeding the shape against a compiled
//! replica. Until they are closed this file is a backstop for one spelling of
//! one defect, and a green result here is not evidence that contributor text is
//! fenced:
//!
//! 1. It flags a fence only when the span between two ``` markers contains `{`,
//!    so a fence built with `push_str` is invisible -- which is how
//!    `fixer/evaluator.rs`, the module this was written for, builds its prompt.
//! 2. A module with no ``` at all is reported clean by iterating it and finding
//!    nothing. `src/queue_healer` had two raw contributor interpolations and
//!    zero backticks, so this scan called it green by absence (#199).
//! 3. `match_indices("```").chunks(2)` pairs positionally across the
//!    concatenation of a module's files, so one unpaired marker desynchronises
//!    every later pair and the check fails open.
//! 4. `preceding.contains("Regex::new(")` is an unconditional 60-char skip over
//!    raw source, so a comment near a fence disables the check for it.
//! 5. `the_list_of_prompt_builders_is_every_module_that_spawns_one` discovers
//!    modules by `exec::agent(`, but `src/reviewer` reaches a model through
//!    `ai_driver` -- so the module `Untrusted` exists for is never scanned.
//!
//! The replacement is a type on the prompt-assembly API, so contributor text
//! is unspellable in a prompt without passing the seam rather than searched for
//! afterwards. Scheduled as H1-5 in `docs/plan/ws-10-untrusted-input.md`.

//! Contributor text reaches a model through `Untrusted`, or it does not reach
//! one at all.
//!
//! A markdown code block is not a fence. Three backticks in any added file
//! close one, and everything after is unmarked prompt text the model reads in
//! the harness's own voice. `reviewer::untrusted` says so in its own module
//! doc, and four prompt sites in three other modules did it anyway -- including
//! the fixer's, which steers a turn holding write access to the tree.
//!
//! The type was already correct and already `pub`. What was missing is this:
//! nothing made using it the only way.

use anvil::source_scan::paths::module_source;
use std::path::Path;

/// Modules that spawn a model turn, and therefore build a prompt.
///
/// Enumerated rather than discovered, and the enumeration is checked below
/// against `exec::agent`'s real callers -- a module that starts spawning model
/// turns without being listed here fails, rather than being silently unscanned.
const PROMPT_BUILDERS: &[&str] = &[
    "src/ci_triager",
    "src/queue_healer",
    "src/fixer",
    "src/ai_driver",
    "src/doc_guard",
];

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Every module that reaches `exec::agent` is on the list above.
///
/// The scan's own subject, checked. Without this the list goes stale the day a
/// new module spawns a turn, and a stale list reports a clean tree it never
/// read -- the failure this whole file is about, one level up.
#[test]
fn the_list_of_prompt_builders_is_every_module_that_spawns_one() {
    let mut spawners = Vec::new();
    let mut stack = vec![repo().join("src")];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().is_some_and(|x| x == "rs")
                && std::fs::read_to_string(&p)
                    .map(|b| anvil::source_scan::without_commentary(&b).contains("exec::agent("))
                    .unwrap_or(false)
            {
                let rel = p
                    .strip_prefix(repo())
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .to_string();
                // The seam itself, and the environment lists that name it, are
                // not prompt builders.
                if !rel.starts_with("src/exec/") {
                    spawners.push(rel);
                }
            }
        }
    }
    let unlisted: Vec<&String> = spawners
        .iter()
        .filter(|f| !PROMPT_BUILDERS.iter().any(|m| f.starts_with(m)))
        .collect();
    assert!(
        unlisted.is_empty(),
        "these spawn a model turn and are not in PROMPT_BUILDERS, so nothing \
         checks how they fence contributor text: {unlisted:#?}"
    );
}

/// Fences that are harness-authored, with the reason each is not a channel.
///
/// A COUNT per module, not a name per module. Excusing a module excuses every
/// fence it will ever hold: a seeded hand-rolled fence added to `src/fixer`
/// passed this check while the excuse was per-module, which is the defect the
/// whole file is about, committed in its own allowlist.
///
/// Each entry says how many harness-authored fences that module has and why.
/// One more, and the count no longer matches.
const HARNESS_AUTHORED: &[(&str, usize, &str)] = &[
    (
        "src/ci_triager",
        2,
        "two: the JSON output schema the model is asked to return, whose \
         substitutions are the workflow name and branch shown so the model \
         copies them; and `formatted_markdown`, which is a GitHub comment body \
         for a person to read rather than a prompt for a model to obey — a \
         contributor's logs land in it, which is a markdown-injection concern \
         in a posted comment and not this file's subject",
    ),
    (
        "src/fixer",
        1,
        "the JSON output schema, inside a raw string literal pushed with \
         `push_str` — a brace there is literal text, not a substitution",
    ),
    (
        "src/doc_guard",
        1,
        "the JSON output schema, same shape as the two above",
    ),
];

/// No prompt builder interpolates a value into a bare markdown fence.
///
/// The subject is a fence with something SUBSTITUTED into it, not any fence. A
/// prompt that shows the model its output schema is harness-authored text and
/// closes nothing a contributor can reach; a fence with `{binding}` inside it
/// is a channel, and a channel needs a delimiter the content cannot close.
///
/// `{{` is a literal brace in a format string, so those are removed before
/// looking for a substitution -- otherwise every JSON schema example reads as
/// an interpolation and the check flags the safe case.
#[test]
fn no_prompt_builder_interpolates_into_a_hand_rolled_fence() {
    let mut offenders = Vec::new();
    for module in PROMPT_BUILDERS {
        let src = module_source(module, repo());
        // The TOKEN, not a line that opens with it. A fence written across
        // several source lines and one written inside a single-line format
        // string are the same fence to a model; scanning line starts sees only
        // the first, and a seeded `format!("```diff\n{}\n```", d)` in the
        // fixer passed this check until it counted tokens instead.
        let marks: Vec<usize> = src.match_indices("```").map(|(i, _)| i).collect();
        for pair in marks.chunks(2) {
            let [open, close] = pair else { continue };
            // A regex that PARSES a fence is not one that builds a prompt.
            // `(?s)```(?:json)?\s*(\{.*?\})\s*``` ` has backticks with a
            // brace between them and matches model OUTPUT.
            // Char-safe: a byte offset back from the fence can land inside a
            // multi-byte character, and these prompts carry emoji.
            let mut preceding: Vec<char> = src[..*open].chars().rev().take(60).collect();
            preceding.reverse();
            let preceding: String = preceding.into_iter().collect();
            if preceding.contains("Regex::new(") {
                continue;
            }
            let between = &src[*open..*close];
            if between.replace("{{", "").replace("}}", "").contains('{') {
                let line = src[..*open].matches('\n').count() + 1;
                offenders.push(format!("{module} line {line}"));
            }
        }
    }

    let mut unexcused = Vec::new();
    for module in PROMPT_BUILDERS {
        let here = offenders.iter().filter(|o| o.starts_with(module)).count();
        let excused = HARNESS_AUTHORED
            .iter()
            .find(|(m, _, _)| m == module)
            .map(|(_, n, _)| *n)
            .unwrap_or(0);
        if here > excused {
            unexcused.push(format!(
                "{module}: {here} interpolated fence(s), {excused} excused"
            ));
        }
    }
    assert!(
        unexcused.is_empty(),
        "{} prompt-building module(s) fence a substituted value by hand:\n  {:#?}\n\
         Three backticks in a contributor's diff close one, and everything \
         after it is prompt text in the harness's own voice. Build the segment \
         with `reviewer::untrusted::Untrusted`, whose delimiter the content \
         cannot close.",
        unexcused.len(),
        unexcused
    );
}

/// Every channel `Untrusted` knows about carries all three of its parts.
///
/// `UntrustedLabel::ALL` exists so a channel cannot be added while forgetting
/// one. This asserts the three are actually distinct per channel rather than
/// defaulting, which is what a copied match arm looks like.
#[test]
fn every_channel_declares_its_own_cap_and_heading() {
    use anvil::reviewer::untrusted::UntrustedLabel;
    let mut labels = Vec::new();
    let mut headings = Vec::new();
    for c in UntrustedLabel::ALL {
        assert!(c.max_chars() > 0, "{c:?} has no cap");
        labels.push(c.label());
        headings.push(c.heading());
    }
    labels.sort_unstable();
    let before = labels.len();
    labels.dedup();
    assert_eq!(before, labels.len(), "two channels share a delimiter label");
    headings.sort_unstable();
    let before = headings.len();
    headings.dedup();
    assert_eq!(before, headings.len(), "two channels share a heading");
}

/// Every excused module still holds a fence, and still builds a prompt.
///
/// An excuse that outlives its fence stops bounding anything, and would let a
/// module keep a standing exemption it no longer needs.
#[test]
fn no_excuse_outlives_the_fence_it_excuses() {
    for (module, count, reason) in HARNESS_AUTHORED {
        assert!(*count > 0, "`{module}` is excused for zero fences");
        assert!(
            PROMPT_BUILDERS.contains(module),
            "`{module}` is excused but is not a prompt builder"
        );
        assert!(
            module_source(module, repo()).contains("```"),
            "`{module}` is excused for a fence it no longer has; remove the entry"
        );
        assert!(
            reason.len() > 40,
            "`{module}` is excused with no reason a reviewer can weigh"
        );
    }
}
