---
schema: hyperscaler.doc.v1
title: "ADR-0003: The delivery lifecycle"
doc_id: adr-0003
category: adr
status: active
canonical_authority: true
owner: "@jason931225"
last_verified_at: "2026-08-21"
---

# ADR-0003: The delivery lifecycle

## Status
Accepted (Jason, 2026-08-21). Method, not machinery. This page adds no gate and no code.

## Schema
Achieves: One law, applied at every radius — a spec is written and reviewed before the thing it governs exists, and it carries a measurement that can fail.
Origin: Practice in use on this repository and not written down anywhere. `rules.md` asks only that a change "have corresponding tests"; it says nothing about order, nothing about reviewing the tests themselves, and nothing about the artifacts above and below the code. Sharpened against Addy Osmani's writing on specs, software factories, owning the outer loop, and agentic autonomy levels (2026), which names several failure modes this repository had already walked into.
Rule: Tests come before implementation, and the tests are reviewed to approval before implementation starts. What is true of a function is true of a contract, a migration, a runbook and a product bet — only the radius changes.
Ensure: Every stage names an artifact and a measurement. A stage whose artifact nobody can fail is not a stage, it is a document.
Overturn-When: A measurement shows a stage costs more than the defects it catches, or Jason cuts one.

## The constraint
Generation is cheap and verification is not. A fleet that writes faster does not ship faster; it deepens the queue at the review gate, because the narrow neck was never the writing.

Everything below follows from that. The pipeline is long because verification is the scarce thing, and each stage buys a distinct kind of assurance. The rule that governs how much work may be delegated is the same: **hand out only as much autonomy as can be cheaply and reliably verified, and not one inch more.** Where verification is cheap, run wide. Where it is expensive, run narrow — regardless of how capable the agent is.

Two costs accrue silently when that rule is broken.

**Comprehension debt** is the gap between how much code exists and how much any human still understands. Tests passing while understanding collapses is the failure, and it arrives quietly and late. A suite of several thousand lines that no human has read is debt even when it is green.

**Summary substitution** is accepting an agent's account of its work in place of the evidence. A summary is an argument, not a measurement. The remedy is that the same evidence packet crosses the boundary as for manual work — the diff, the tests, the logs, the failing output, the mutation results — and the summary points at it rather than replacing it.

## Why this page exists
The method was carried in conversation and in reviewers' heads. That is the same defect this repository exists to prevent: an unwritten rule cannot be checked, and a rule nobody can check is indistinguishable from one nobody follows.

## The pipeline
Order matters. Each stage gates the next; nothing skips ahead because a later stage looks more interesting.

1. **Spec tests.** The complete suite for the change, written first. Every test red, and red for a wrong-behaviour assertion — never a typo or a missing import. Scaffolding that lets tests compile (a signature with a `todo!()` body, a module declaration) is not implementation. Real logic in a body is.
2. **Review the tests.** Two reviewers with deliberately different lenses. One asks whether the suite covers the specification. The other assumes the suite is broken and writes the laziest wrong implementation that would satisfy it. Implementation does not begin until both approve. If they cannot converge, an adjudicator rules each open item blocking or acceptable, and every accepted gap is published rather than quietly dropped.
3. **Implement.** Against frozen tests. The implementer may not weaken, delete, skip or loosen an approved test. Convinced one is wrong? Stop and say so; do not edit it. Adding a test is always allowed.
4. **Review the code.** Green tests are a floor, not a verdict. This stage reads the implementation: correctness beyond the cases the suite covers, fail-closed discipline on every branch and early return, fit with the surrounding idiom, and whether a type could carry the invariant that a check is currently carrying.
5. **Coverage.** Regression tests naming the defect each one guards, and integration tests exercising the real wiring rather than the unit seam.
6. **Review the coverage.** Tests written after the code have a specific failure mode: they assert what the code *does* rather than what it *should do*. A test derived by reading the implementation cannot detect that the implementation is wrong. This stage looks for exactly that, and for duplicate mass.
7. **Verify.** Mutation testing, including mutations nobody in the pipeline was shown. Revert the implementation and confirm the tests go red again. A test still green with the implementation gone is vacuous and gets named.

## What the stages are for
Every stage is a different way of being wrong.

A suite can be complete and vacuous. It can be discriminating and brittle. An implementation can pass every test and still be wrong on the case nobody wrote. Coverage written after the fact can restate the code back to itself. Each stage exists because the previous one cannot see its own blind spot — which is also why reviewers must be decorrelated. Two reviewers sharing a lens are one reviewer with two names.

Deleting test mass is a legitimate outcome, and it needs the same proof as adding it: a deletion is safe only when a wrong implementation still fails without it. A fixed list of mutations is not enough to prove that. Only a pass free to invent its own mutations will find what the list missed.

## Method siblings
TDD, BDD and DDD are not alternatives. They answer different questions and compose.

- **DDD** decides the concepts and boundaries, and supplies the vocabulary — gate, seat, corpus, admissible, certify, enlist, lease, ratchet. ADR-0002 is this repository's glossary.
- **BDD** writes the specification in that vocabulary. Test names here are behaviour sentences for that reason, not decoration.
- **TDD** sequences the writing.

The strongest form is to let the model carry the invariant so the test does not have to. An invariant enforced by a type is checked everywhere, for free, at compile time; the same invariant enforced by a call is checked only where somebody remembered to call it. Prefer making a defect unrepresentable over testing that it does not occur. A test is how an invariant is defended when the type system cannot hold it.

## Contracts, schemas and data
Data outlives the code that wrote it, so contracts get their own cycle and it runs before the code cycle — a contract change is the thing that breaks other people.

A round-trip test over the current types is vacuous by construction: it tests the serialiser, not compatibility. Contract tests run against **frozen artifacts from the past** — bytes captured at a version, committed, and never regenerated. A fixture that gets regenerated when it fails has stopped being a test.

Sequence: expand, migrate producers, migrate consumers, enforce, contract. The gate's job is to know which phase a change is in and to fail a change that skips one.

Persisted and published shapes are contracts whether or not anyone declared them: the certification report on the wire, the state file, provenance receipts, the OpenAPI document. Each needs a version, a golden corpus, and a parse that fails loudly rather than defaulting. Defaulting a missing field is how absent evidence becomes a pass.

## The wider radii
The same law, further out. Each row is an artifact somebody must be able to fail.

- **Requirements.** The bet and the acceptance bar, before the work. Quality sign-off cannot certify without one.
- **Plan.** The order of the work, and which parts are stale. A plan that has gone stale must fail closed, or it silently authorises work in the wrong order.
- **High-level design.** Boundaries and contracts, before modules.
- **Low-level design.** This is the spec suite. It is not a separate document.
- **Migrations.** How live data crosses a version boundary, rehearsed rather than described.
- **Runbooks.** Run's spec test, with the same vacuity failure mode as any other: a runbook nobody has executed is a document. It needs a rehearsal that fails when the steps are wrong.
- **Documentation.** Published truth, checked against the live corpus rather than against intent.

## Calibrated autonomy
How much a task may be delegated is a property of the task, not of the agent and not of anyone's confidence. Three questions decide it:

1. **Detection.** How quickly will we know it went wrong?
2. **Reversibility.** How cleanly can it be undone?
3. **Verification.** What evidence would prove it right?

Cheap to detect, cheap to undo, cheap to verify — delegate widely. Expensive on any axis — narrow the scope, not the ambition. A refactor behind a strong suite with a clean rollback earns more autonomy than a documentation sweep with no canonical truth to check against, even though the refactor sounds riskier.

The levels, in rising order: suggestions a human acts on; actions approved one at a time; a bounded task with the human nearby; a goal with a measurable stopping condition; parallel slices; and dispatch managed by exception. Moving up should change one axis at a time.

Autonomy is not a status. The engineer's skill is choosing the right level, and settling at a lower one deserves the same credit as running wide.

Every delegated run carries a contract, stated before it starts: goal as an outcome rather than an activity; scope and non-goals; tools and permissions; a stopping condition, measurable where possible; the evidence it must produce; when to escalate rather than continue; and a budget in time and attempts. A run without a stopping condition does not terminate, it merely stops being watched.

Boundaries are stated in three tiers, not two: what may be done without asking, what requires a human first, and what is never done. This repository's standing entries are in ADR-0002 — never merge, never approve, never start the daemon, never run a model-invoking subcommand against a live pull request.

## Answerability
Someone must be able to say exactly what changed, why it was safe, and what happens if that judgement was wrong. At the delegation rates this pipeline allows, decisions multiply past the point where they can be reconstructed from inputs, so the system has to emit the signal at the time: the receipt, the scorecard, the review thread, the commit message that says why rather than what.

Answerability is why a refusal must be observable rather than silent, and why a gate that cannot say what it measured is worse than one that measures less.

## Blast radius
A change invalidates some contracts, some documents, some runbooks and some tests, and not the others. Without knowing which, a team either re-reviews everything, which nobody can afford, or re-reviews nothing, which is what actually happens.

The dependency graph is therefore the scheduler for every stage above: it decides what re-runs. Building it is not a prerequisite for the pipeline — the pipeline works without it, more expensively — but it is what makes the wider radii affordable.

## Known failure modes
Named so they can be recognised rather than rediscovered. Each has been seen on this repository.

- **Summary substitution** — reading the agent's account instead of its evidence.
- **Comprehension debt** — green tests over code nobody has read.
- **Permission laundering** — approval fatigue answered by widening permissions rather than narrowing scope.
- **Autonomy as status** — running wider because it is impressive rather than because it is verifiable.
- **Fleet cosplay** — parallel agents whose dependencies a human is still resolving by hand.
- **Vacuous assurance** — a suite, a gate or a fixture that cannot fail, publishing confidence it did not earn.

Keep a record of what agents get wrong. A single mistake is noise; the same mistake three times is a defect in the producing prompt, the harness or the spec, and should be fixed there rather than corrected downstream one output at a time.

## What this page is not
Not a gate. Not machinery. Not a claim that every stage is instrumented today: most of the wider radii have no measurement yet, and ADR-0002 names which. Naming a stage is not the same as owning it.
