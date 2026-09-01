# WS-16 — Evaluating the harness itself (model lifecycle and the falsification engine)

**Why (research gap 7 — "the single largest uncontrolled variable"):** Anvil's falsification engine
certifies gates per-edit, but nothing re-certifies the *whole harness* when the underlying model,
prompts, or harness version change. A model bump can silently regress review quality, fixer
behaviour, and injection resistance at once — and every autonomy rung (WS-06) was earned under the
old model. Research: leading harnesses treat observability-driven self-evolution under
falsifiability constraints as the pattern; review-bot precision is codebase-dependent and vendor
numbers are unstable, so serious teams **score reviewers on their own seeded corpora**.

## The instrument

A pinned, versioned eval corpus assembled from artifacts this tree already produces — they are the
eval set precisely because each was once a real defect with a red proof:

- gate fixtures (WS-08's red/green pairs) run end-to-end through review/certify;
- the injection corpus (WS-10) — resistance is an eval axis, not only a gate;
- postmortem-derived cases (RC-1..RC-6 shapes) as regression probes;
- door/decision cases (#52's behavioural suite);
- a held-out set not shown to any prompt (mutation-guided additions, WS-05), because a corpus the
  prompts were tuned on measures tuning, not capability.

## Milestones

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H2-9a | Eval corpus v1 + runner: one command produces a scored report per (model, prompt-set, harness rev) | report is `Evaluated`-shaped (subjects_seen, findings); corpus and scores versioned; baseline recorded for the current model | Test infrastructure |
| H2-9b | Model/prompt bumps gated: a change to model id, system prompts, or agent instructions must attach the eval delta; regression past threshold blocks (default pin: any axis regressing >2% absolute vs the corpus-v1 baseline; thresholds live in the registry row that admits corpus v1) | seeded prompt-degradation (a deliberately weakened reviewer prompt) is caught by the gate (red proof archived) | Test infrastructure |
| H2-9c | Autonomy coupling: a model change freezes rung promotions until the eval is green at baseline (roadmap §5 tripwire made mechanical) | registry records the freeze/unfreeze pair automatically on model-id change | Human ticket queue |
| H3 | Continuous: scheduled eval runs (weekly + on every harness release); trend charted on the cockpit; held-out set rotated with red proofs | a quarter of weekly runs exists with zero silent skips (standing-red tripwire covers the eval job itself) | Observability |

## Ratchets

- The eval gate is proven like any check: a seeded regression must fail it before it is trusted.
- Corpus is append-only with red proofs; removing a case requires a registry ticket (same law as
  test deletion in ADR-0003: a deletion is safe only when a wrong implementation still fails).
- No self-tuning on the held-out set: the held-out list is hashed in the registry; a prompt PR
  touching held-out cases is refused.

## Non-goals

No public-benchmark chasing (SWE-bench-style scores don't gate anything here; our corpus measures
*this* fleet's failure modes); no auto-rollback of models (freeze + ticket — model choice stays a
human-ratified decision until the ladder says otherwise).
