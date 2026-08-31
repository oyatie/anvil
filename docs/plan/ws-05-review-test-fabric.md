# WS-05 — The review/test fabric

**Scope:** the 16-lens adversarial review, the test suite as a gate, flake lifecycle, and the
merge-train rehearsal. Reviews and tests are named capabilities with typed I/O. Research (2026):
review agents are socially engineerable by PR narratives (SEVRA-BENCH), so **verdicts must derive
from executed evidence**; 77% of SWE-bench-Verified suites accept semantically wrong patches
(STING), so suites are validated by mutation and seeded defects; flake quarantine is a governed
lifecycle with SLAs, not a mute button; ADR-0003's pipeline (spec tests → review the tests →
implement → review the code → coverage → review the coverage → verify) is the method this fabric
mechanizes.

## Typed I/O contracts

- **Review:** input `ReviewRequest { repo, pr, head_sha, corpus_digest, lenses }`; output
  `ReviewVerdict { findings: [{lens, claim, evidence: ExecutedEvidence | CitedCode, severity}],
  verdict, refusals }`. A finding without evidence fails the schema; a verdict over a corpus digest
  that is not the PR head at the door is refused (ADR-0002 loop step 1 already demands this).
- **Test run:** input `SuiteRequest { subject_root, target_set }`; output `SuiteReport { counts
  {passed, failed, skipped}, duration, flake_events, evidence_path }` — counts always stated, per
  restructure-plan verification discipline ("a pure import rewrite that changes a test count has
  changed behaviour"). Runner is `cargo nextest` (doctrine; serial `cargo test` is not the suite).

## Milestones

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H1-12 | Merge-train rehearsal required: merge open PRs in queue order locally, build+test the merged state (postmortem RC-5) | seeded #140/#146-shaped pair (disjoint paths, type conflict): both green alone, rehearsal red | Test infrastructure |
| H1-14 | Flake lifecycle on measured inputs: quarantine ingest from real CI outcomes; `tests::flaky_test` literal dies (#53); #191's wall-clock tests fixed by capability-fakes or budget-aware harness | quarantine state transitions driven by a seeded flaky test end-to-end; contention harness keeps #191 green | Test infrastructure |
| H1 | Enlist doors driven behaviourally (#52): trait seam for the forge so CLI, `POST /api/enlist`, and healer re-enlist run end-to-end against a fake; refusal assertions restored | admission-decision deletion makes ≥6/7 door tests red (today: 6/7 stay green — measured in #52) | Test infrastructure |
| H2 | Review verdicts evidence-carrying: every blocking finding cites executed evidence (a run, a diff hunk, a graph fact); narrative red-team corpus in CI | SEVRA-style corpus: verdict unchanged under narrative manipulation; evidence-free finding is schema-unrepresentable | Quality sign-off |
| H2 | Mutation adequacy beyond the fixed list: mutation pass free to invent mutants on changed code (ADR-0003 §verify) | surviving-mutant report per PR; seeded vacuous test named by the pass | Test infrastructure |
| H2 | Review routing by ownership from the code graph (WS-04) | routed-lens assignment matches OWNERS for a 20-case corpus | Quality sign-off |
| H3 | Review fabric at agent volume: risk-tiered depth (light always, heavy on class/blast-radius triggers), human reviews exceptions only (feeds WS-06 rung 3) | depth-tier selection is Cedar-decided with 100% of selections logged (an audit query returning an unlogged selection is red); quarterly drill: a seeded high-risk PR must trigger the heavy tier (red proof archived) | Quality sign-off |

## Ratchets

- Prove-a-check (doctrine, and roadmap §4): a new review lens, gate, or suite addition is trusted
  only after its seeded defect fails it — and the seed is asserted to have applied.
- Door coverage: the source-text scan (#52) is demoted to backstop the day the behavioural tests
  land; a test asserts the behavioural suite exists and reddens on decision deletion.
- Flake SLA ratchet: quarantined tests carry TTL + owner; the quarantine set size is baselined and
  a stale entry past TTL is a red, not a shrug.

## Non-goals

No "AI review as the only review" while verdict authority is unproven (WS-16 gates model changes);
no coverage-percentage worship — differential coverage stays, but the metric that gates is
seeded-defect kill rate, not line percent (a proxy is not the thing).
