---
schema: hyperscaler.doc.v1
title: "ADR-0002: Agentic roster and delivery fabric"
doc_id: adr-0002
category: adr
status: active
canonical_authority: true
owner: "@jason931225"
last_verified_at: "2026-08-21"
---

# ADR-0002: Agentic roster and delivery fabric

## Status
Accepted (Jason, 2026-08-21). Implementation is in flight. This page is the lock, not a claim that the seats already exist.

## Schema
Achieves: Anvil is the full hyperscaler product lifecycle, agentic. Not only the PR review / certify / enlist / heal loop.
Origin: Long-horizon practice at Google, AWS, Meta, Netflix, Azure, plus Jason's corrections (product ownership, Legal as it-legal fabric, design-system ownership, dogfood closed loop).
Rule: A seat is a job, an artifact, and a fail-closed measurement. A gate named after a team is not representation. Published names must match live measurement. NotMeasured and Errored are honest reporting, not a pass. Do not rename down or drop a gate to make the corpus look clean.
Ensure: Existing design pages (ADR, spec, plan, contracts, milestones) are drafted and kept honest. Stale, contradictory, or non-hyperscale pages are repaired or fail closed.
Overturn-When: Jason cuts a seat, or a measurement proves a named job is already owned by another seat without shallow wrapping.

## What Anvil is
Anvil owns Discover through Run. Planning, prep, and architectural drafting are Anvil work. The shipped PR loop is one phase, not the product.

Phase sequence (order, not calendar):
1. Discover locked
2. Build ready
3. Prove admissible
4. Ship
5. Run

Support, Observability, and Research feed Product. That is the closed loop.

Manager authority: diagnose, open a branch or PR, keep Jason posted. Do not merge or approve. Drive fixes through Anvil's own pipeline first. A cloud agent is the fallback when that loop is stuck. Do not run the Anvil daemon, serve process, or webhooks unless Jason asks.

## Honesty law
Prevent false greens and shallow checks wrapped as sophisticated gates. Close the shortfall until the published name and the actual measurement are the same. If a plan, spec, or ADR is inadequate, repair it from hyperscaler practice and the northstar. Do not leave a dishonest page.

DocGuard: auto-update owned pages when it can write an honest page. Fail closed if it cannot. The README exemption is a defect. Live authority is `TOTAL_GATES` / `PreMergeCertificationReport::all_statuses().len()`, not README or doctrine.

## Design-system artifacts Anvil drafts and keeps honest
- ADR
- Spec
- Plan
- Contracts (read as engineering contracts: API, events, SLO, interfaces; not counsel paper, unless Jason says both)
- Phase-sequence milestones

## Roster (20 seats, approved)
No DevOps seat. That work is Builder tools plus Production. Quality sign-off is a job, not a standing QA team.

Roster names are the live report fields minus the `_status` suffix, so every `Today:` line can be checked mechanically against `PreMergeCertificationReport`. Every gate in the live corpus is named by exactly one seat.

### Discover
1. Product. Job: the bet and the acceptance bar. Artifact: written problem + done-when. Measurement: Quality cannot sign off without it. Today: nothing.
2. Program. Job: sequence the work. Artifact: spec, plan, phase-sequence milestones. Measurement: stale or contradictory plan fails closed. Today: nothing.
3. Research. Job: experiment before a bet is fact. Artifact: rerunnable result, including negatives. Today: nothing.
4. UX. Job: evidence that changed a decision. Artifact: task / a11y / dark-pattern findings, not a mock. Measurement: pre-ticked or biased checkout fails. Today: nothing.
5. Data science. Job: define the metric and prove it. Artifact: rerunnable eval. Today: nothing.
6. Architecture. Job: shape and boundaries. Artifact: living ADR, spec, contracts. Today: adr, clean_arch, modularization, monorepo, cell_isolation, api_contract, semantic_abi, schema_evolution, schema_compat, consistency, shape, migration_boundary. Must draft and keep honest, not only detect a file.

### Build
7. Implementation. Job: the change in the right module. Artifact: small owned diff. Today: rust_skills, constant_work, idempotency, deadlock, performance_concurrency, feature_flag, debt_shrink, stacked_diffs.
8. Builder tools. Job: the inner loop. Artifact: hermetic build, locked toolchain, local probe. Today: hermetic_build, remote_cache, compile_profile, ci_wallclock, sandbox, local_probe.
9. Docs. Job: published truth. Artifact: pages that match the live corpus. Today: doc_parity, brand_absence. First close.

### Prove
10. Test infrastructure. Job: harnesses, selection, flake lifecycle. Artifact: measured coverage / mutation / replay, not "a test exists." Today: coverage, test_suite, predictive_test, mutation, shuffle, flake_quarantine, kani, formal_verification, replay_harness, bench, microbench.
11. Quality sign-off. Job: go/no-go against Product's done-when plus `is_admissible`. Artifact: certification that names what was unmeasured. Today: unresolved_review, review_verdict + `is_certified_ready` / `is_admissible`.
12. Security. Job: threat, supply chain, IAM, policy. Artifact: cedar / attest / VEX that actually ran. Today: cedar, wasm_sandbox, supply_chain, security_scan, ephemeral_secret, psa, zero_day, openvex, cosign, attestation, zero_trust_workload.
13. Legal. Job: it-legal fabric, not counsel. Artifact: status map, field catalog, processor registry, retention / destruction, consent ledger. Today: compliance. It scans added diff lines against five live regex rules — a sixth carries no pattern and can never fire — and filters them by a hardcoded evaluation date. It has no NotMeasured path, and the five-entry jurisdiction list it assembles, naming some seventeen regimes, is read by nothing. That is not representation until it measures the catalog. First corpus: Korean IT law (`jclab-joseph/it-legal` / `legalize-kr`). Jurisdiction-pluggable later. Does not file registrations, does not decide "are we a PG," does not invent 고시 detail.

### Ship
14. Release. Job: rings, rollback, trains. Artifact: a rollout that can reverse. Today: canary, automated_canary, progressive_ring, auto_rollback, upgrade_train, ghost_migration, migration_orch.
15. GitOps. Job: desired state is source. Artifact: digest pin + no drift. Today: gitops_promo, gitops_drift.

### Run
16. Production. Job: operate what you shipped. Artifact: cluster / cross-service audit. Today: cluster_audit, cross_service.
17. Observability. Job: SLOs that can page. Artifact: live SLO + traces. Today: slo, trace. NotMeasured blocks admission — true for slo, which reports NotMeasured with no telemetry endpoint configured. trace does not: it reports Passed on zero measurements and has no NotMeasured or Errored branch ([#14](https://github.com/oyatie/anvil/issues/14)).
18. Resilience. Job: break it on purpose. Artifact: chaos / shadow / backoff that ran. Today: chaos_injection, jittered_backoff, shadow_traffic.
19. Capacity / FinOps. Job: unit cost and capacity. Artifact: cost / carbon / runner numbers. Today: finops, carbon_compute, runner_economics.
20. Support. Job: reproduce a customer failure and feed Product. Artifact: repro that closed a loop. Today: nothing.

## Dogfood (closed loop)
Fleet is the whole oyatie org. No other public repo exists as of 2026-08-21:
- `oyatie/oyatie`
- `oyatie/console`
- `oyatie/anvil`

Anvil is a watched product, not an exemption. The same loop runs on every watched repo.

Allowlist: `WATCHED_REPOS`. Code default in `src/config.rs` is the three. README intro lists the three. README `.env` example drops Anvil. That is a lie. Published config must match the live allowlist. A string in the default is not proof the loop ran.

Loop (same path, every watched repo):
1. Intake: GitHub webhook while `serve` is up, or CLI (`review`, `fix`, `certify`, `triage`, `enlist`, `heal-queue`, `reconcile`). The handler dispatches `pull_request`, `pull_request_review_comment`, `workflow_run`, and `merge_group`; `issue_comment` is forwarded by `serve` but never dispatched, so it is not an intake path today. CLI is the same loop for review and certify, and now for enlist: every call site into the merge queue runs the certification corpus — the review verdict and the local verification gate measured on that path, not asserted absent — and hands over what it produced. The verification gate runs in an ephemeral worktree checked out at the head being certified, not in the shared clone, which is never checked out to a pull request head and which `fix` leaves parked on an unrelated branch; a build of a different commit is not this pull request's evidence. `enlist_into_merge_queue` refuses what it was handed no evidence for, and refuses a report measured against a commit that is not the pull request's head as read at the door ([#17](https://github.com/oyatie/anvil/issues/17)).
2. Review: 16-lens adversarial review posted on the PR.
3. Certify: live `TOTAL_GATES`. Admission is `PreMergeCertificationReport::admission_refusal()`: produced by a certification run, certified, and every gate measured. NotMeasured and Errored both block. `is_admissible()` is a weaker diagnostic reading that sees neither Errored nor provenance, and nothing gates a merge on it. Scorecard is posted and amended in place.
4. Close the shortfall: `fix` and `heal-queue` on that same PR.
5. Enlist: Anvil hands the report to `enlist_into_merge_queue`, which admits or refuses and says why. Manager does not merge or approve. Enlisting is not passive: `MergeEnlister` submits an APPROVE review on Anvil's behalf and then sets auto-merge on the PR. The approval body and the enlistment note are derived from the certification report, and neither is published for a pull request Anvil refuses to admit ([#18](https://github.com/oyatie/anvil/issues/18)).
6. Feed Product: Observability, Support repros, and Research results become the next Discover bet. That is the closed loop. Discover and Support are still uninstrumented. Naming the feed is not claiming those seats exist.

Runtime: standing path is `cargo run -- serve` plus webhooks on the three. Do not start the daemon, serve process, or webhooks unless Jason asks. Until then, manager drives Anvil and fleet PRs through the CLI of this loop first. A cloud agent is only for when that loop is stuck.

Dogfood is proven when a PR on that repo actually went through review → certify → (fix/heal if needed) → enlist or honest block, and the scorecard matches the live corpus. It is not proven because README says "Anvil reviews its own pull requests."

## What needs to be done (sequence)
Do not add twenty named gates. Give each hole a real artifact and a measurement. A seat is not done while its measurement is NotMeasured.

1. Close DocGuard. Published docs match `TOTAL_GATES` or the gate fails. [PR #12](https://github.com/oyatie/anvil/pull/12) merged 2026-08-21 with [PR #13](https://github.com/oyatie/anvil/pull/13): published counts are derived, and residual drift fails the gate closed. [PR #11](https://github.com/oyatie/anvil/pull/11) was Copilot plan-only theater; closed 2026-08-21, branch kept. README `.env` must list `oyatie/anvil` with the other two.
2. Prove dogfood. A real PR on each watched repo, including Anvil, goes through this loop. Serve stays off until asked. CLI is enough to prove it: `certify` drives the same gated pipeline the webhook drives. The outcome reachable today is the honest block — with no telemetry endpoint configured `is_admissible()` cannot return true ([#19](https://github.com/oyatie/anvil/issues/19)) — so the enlist half of the proof waits on telemetry. Do not claim self-loop from the config default.
3. Architecture pages. ADR-0001 keeps its founding filename on purpose: the count at inception, spelled out. Live count is `TOTAL_GATES`. Keep founding names historical, but never write the founding token on an owned page in either form — corpus sync rewrites the digit form and the word form alike, wherever they appear, including inside a filename it cannot tell apart from a claim. Name that page by its ADR id instead. Repair doctrine and this ADR if they drift.
4. Legal seat. Encode the it-legal fabric as schema + fail-closed measurement. Do not ship a `legal_status` rename of `compliance_status`.
5. Discover seats. Product done-when, Program plan, Research result, UX evidence, Data science eval. Quality sign-off must fail if Product's bar is missing.
6. Support seat. Repro artifact that feeds Product. That closes the loop this manager was asked to watch.
7. [PR #10](https://github.com/oyatie/anvil/pull/10) merged 2026-08-21, adding `shape`, `migration_boundary`, `brand_absence`, and `review_verdict`. That last one promotes the AI review verdict into the counted corpus, closing [#16](https://github.com/oyatie/anvil/issues/16).

## Snapshot 2026-08-21 (this session, will go stale)
- Live corpus on main is `TOTAL_GATES`. The authority is the field list on `PreMergeCertificationReport`, never a number written on this page.
- Cloud agent cannot launch: Cursor GitHub app is not installed on `oyatie/anvil`. Fixes go through the GitHub connector until that is installed.
- Manager weekday watch: 8:32 America/New_York.
- Enlist lock: Anvil enlists when admissible. Manager does not merge or approve. The admission decision is taken inside `enlist_into_merge_queue`, so it is no longer the webhook path's alone ([#17](https://github.com/oyatie/anvil/issues/17)). Live gap: with no telemetry endpoint configured `slo_status` reports NotMeasured, so no report is admissible and no path enlists anything ([#19](https://github.com/oyatie/anvil/issues/19)). There was a second, unconditional blockage beside it, and it is worth naming because it was published here as if #19 were the whole story: the three enlist doors ran the corpus with the review verdict hardcoded to `Errored` and the test suite to `None`, so they refused every input in every configuration and configuring telemetry would not have changed that. Both gates are measured on those paths now, which leaves #19 as the live cause.

## What this page is not
Not a legal opinion. Not a claim that Discover or Support exist. Not permission to merge. Not permission to start the daemon. Not a claim that dogfood already ran.
