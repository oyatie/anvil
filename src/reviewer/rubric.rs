//! What the review must examine, and how it is asked to look.
//!
//! # Two different things, kept apart on purpose
//!
//! The rubric carried sixteen lenses — Cartesian doubt, Socratic, Red Team,
//! Contrarian — and every one is a *stance*: a way of looking. Google's
//! reviewer guidance names ten *aspects*: design, functionality, complexity,
//! tests, naming, comments, style, consistency, documentation, every line.
//! Those are what must be looked **at**.
//!
//! A reviewer can hold all sixteen stances and never once ask whether the
//! tests are adequate. That is not hypothetical: this review passed fifteen of
//! sixteen lenses on a change carrying five real defects, and four of those
//! defects were about the tests being wrong. Tests, naming, comments and
//! documentation had no lens between them.
//!
//! # Why these are constants rather than prose in a prompt
//!
//! A rubric written inline is a rubric nothing can check. As data, a test can
//! assert every named aspect is actually asked for — which is the deterministic
//! half of a judgement that is otherwise irreducibly semantic. The stances
//! stay stances; the aspects become a checklist the prompt cannot quietly drop.

/// The aspects a review must examine, each with what it is being asked.
///
/// Named after Google's reviewer guidance, so the list can be compared to the
/// source it came from rather than drifting into whatever seemed reasonable.
pub const REVIEW_ASPECTS: &[(&str, &str)] = &[
    (
        "Design",
        "Does this change belong here, and does it fit the system it is joining?",
    ),
    (
        "Functionality",
        "Does it do what the author intended, and is that what a user needs? \
         Consider edge cases, concurrency, and failure paths.",
    ),
    (
        "Complexity",
        "Could this be simpler? Would another engineer understand it on first \
         reading, and be able to change it safely?",
    ),
    (
        "Tests",
        "Are there tests, are they the RIGHT tests, and would they FAIL if the \
         code were wrong? A test that passes against the unfixed code proves \
         nothing. Name any assertion that cannot fail.",
    ),
    (
        "Naming",
        "Does each name say what the thing is, precisely and without \
         overclaiming? A name that promises more than the code delivers is a \
         defect, not a style preference.",
    ),
    (
        "Comments",
        "Do comments explain WHY rather than restate what the code does? Is \
         anything stale, or describing behaviour the code no longer has?",
    ),
    (
        "Style",
        "Does it follow the project's conventions? Style disagreements the \
         formatter already settles are not review findings.",
    ),
    (
        "Consistency",
        "Is this consistent with how the rest of this codebase solves the same \
         problem, or is it a second way of doing an existing thing?",
    ),
    (
        "Documentation",
        "If this changes how something is built, run or understood, is the \
         documentation updated in the same change?",
    ),
    (
        "Every line",
        "Read every changed line. Say plainly which parts you could not review \
         and why, rather than implying whole coverage.",
    ),
];

/// The stances: how to look, once you know what to look at.
pub const REVIEW_STANCES: &[&str] = &[
    "Cartesian doubt: Question foundational assumptions. Is this change actually solving the real root problem?",
    "Essentialism / YAGNI: Is this code minimal and necessary, or over-engineered with speculative abstractions?",
    "Chesterton's Fence: Understand why the existing code was written before approving alterations or deletions.",
    "Contrarian / Outside-the-box: Is there an unorthodox, dramatically simpler 10x architectural alternative?",
    "Socratic: Challenge interfaces, boundary contracts, and invariants with clarifying inquiries.",
    "Pragmatism: Balance theoretical purity against operational velocity, simplicity, and maintainability.",
    "Red Team: Actively probe for security vulnerabilities, injection vectors, TOCTOU race conditions, unauthenticated endpoints.",
    "Systems Thinking: Trace non-obvious cascade effects, coupling across microservices, and hidden feedback loops.",
    "Operability / Day-2: Are logs structured? Are metrics emitted? How will on-call engineers debug this at 3 AM?",
    "Opportunity Cost: Does this change introduce long-term maintenance burdens that outweigh its immediate benefit?",
    "Blast-radius / Cell-based: Can a failure in this component propagate across cell boundaries or bring down unrelated tenants?",
    "Constant-work / Anti-fragility: Does latency degrade under heavy load? Are queues and static worker pools bounded?",
    "Shared-nothing / Eventual consistency: Are distributed components decoupled? Are operations idempotent?",
    "FinOps / Unit-cost: Does this increase memory allocations, cloud egress, or unbudgeted compute hotpaths?",
    "Telemetry-first: Are OpenTelemetry traces, spans, and metrics instrumented across critical execution paths?",
    "Zero-trust / Defense-in-depth: Validate all inputs, enforce least privilege, and sanitize external data boundaries.",
];

/// The rubric as the prompt states it: aspects first, because they decide what
/// gets examined; stances after, because they decide how.
pub fn rubric_prompt() -> String {
    let mut out = String::new();
    out.push_str(
        "## What you MUST examine (report on every one; say so if an aspect \
         does not apply):\n",
    );
    for (i, (aspect, ask)) in REVIEW_ASPECTS.iter().enumerate() {
        out.push_str(&format!("A{}. {aspect}: {ask}\n", i + 1));
    }
    // The heading keeps its exact wording. A false-red guard in
    // `api_auth_and_prompt_delimiting_test` pins this string to catch the
    // prompt silently losing its rubric, and that guard is right — renaming
    // the heading while adding to the rubric would have looked identical to
    // deleting it.
    out.push_str(&format!(
        "\n## Canonical {}-Lens Adversarial Review Rubric — how to look:\n",
        REVIEW_STANCES.len()
    ));
    for (i, stance) in REVIEW_STANCES.iter().enumerate() {
        out.push_str(&format!("{}. {stance}\n", i + 1));
    }
    out.push('\n');
    out
}

/// Appends the rubric without turning dynamically assembled text into a
/// generic trusted-string escape hatch. Every prose fragment comes from the
/// static tables above; only numeric indexes are generated at runtime.
pub(crate) fn append_to(builder: &mut crate::model_prompt::ModelPromptBuilder) {
    use crate::model_prompt::HarnessText;

    builder.push_harness(HarnessText::ReviewerRubricHeading);
    for (i, _) in REVIEW_ASPECTS.iter().enumerate() {
        builder.push_harness(HarnessText::ReviewerAspect(i));
    }
    builder.push_harness(HarnessText::ReviewerRubricLensHeading);
    for (i, _) in REVIEW_STANCES.iter().enumerate() {
        builder.push_harness(HarnessText::ReviewerStance(i));
    }
    builder.push_harness(HarnessText::ReviewerRubricEnd);
}
