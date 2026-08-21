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

### Discover
1. Product. Job: the bet and the acceptance bar. Artifact: written problem + done-when. Measurement: Quality cannot sign off without it. Today: nothing.
2. Program. Job: sequence the work. Artifact: spec, plan, phase-sequence milestones. Measurement: stale or contradictory plan fails closed. Today: nothing.
3. Research. Job: experiment before a bet is fact. Artifact: rerunnable result, including negatives. Today: nothing.
4. UX. Job: evidence that changed a decision. Artifact: task / a11y / dark-pattern findings, not a mock. Measurement: pre-ticked or biased checkout fails. Today: nothing.
5. Data science. Job: define the metric and prove it. Artifact: rerunnable eval. Today: nothing.
6. Architecture. Job: shape and boundaries. Artifact: living ADR, spec, contracts. Today: adr, clean_arch, modularization, cell_isolation, api_contract, schema_*. Must draft and keep honest, not only detect a file.

### Build
7. Implementation. Job: the change in the right module. Artifact: small owned diff. Today: rust_skills, constant_work, idempotency, deadlock, feature_flag, debt_shrink.
8. Builder tools. Job: the inner loop. Artifact: hermetic build, locked toolchain, local probe. Today: hermetic_build, remote_cache, compile_profile, ci_wallclock, runner_economics, sandbox, local_probe.
9. Docs. Job: published truth. Artifact: pages that match the live corpus. Today: doc_parity. First close.

### Prove
10. Test infrastructure. Job: harnesses, selection, flake lifecycle. Artifact: measured coverage / mutation / replay, not "a test exists." Today: coverage, test_suite, predictive_test, mutation, shuffle, flake_quarantine, kani, formal_verification, replay_harness, bench.
11. Quality sign-off. Job: go/no-go against Product's done-when plus `is_admissible`. Artifact: certification that names what was unmeasured. Today: unresolved_review + `is_certified_ready` / `is_admissible`.
12. Security. Job: threat, supply chain, IAM, policy. Artifact: cedar / attest / VEX that actually ran. Today: cedar, supply_chain, security_scan, ephemeral_secret, psa, zero_day, openvex, cosign, attestation, zero_trust.
13. Legal. Job: it-legal fabric, not counsel. Artifact: status map, field catalog, processor registry, retention / destruction, consent ledger. Today: `compliance_status` is a name. That is not representation until it measures the catalog. First corpus: Korean IT law (`jclab-joseph/it-legal` / `legalize-kr`). Jurisdiction-pluggable later. Does not file registrations, does not decide "are we a PG," does not invent 고시 detail.

### Ship
14. Release. Job: rings, rollback, trains. Artifact: a rollout that can reverse. Today: canary, automated_canary, progressive_ring, auto_rollback, upgrade_train, ghost_migration, migration_orch.
15. GitOps. Job: desired state is source. Artifact: digest pin + no drift. Today: gitops_promo, gitops_drift.

### Run
16. Production. Job: operate what you shipped. Artifact: cluster / cross-service audit. Today: cluster_audit, cross_service.
17. Observability. Job: SLOs that can page. Artifact: live SLO + traces. Today: slo, trace. NotMeasured blocks admission.
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
1. Intake: GitHub webhook (`pull_request`, `issue_comment`, `workflow_run`) while `serve` is up, or CLI (`review`, `fix`, `certify`, `triage`, `enlist`, `heal-queue`, `reconcile`). CLI is the same loop, not a bypass.
2. Review: 16-lens adversarial review posted on the PR.
3. Certify: live `TOTAL_GATES`. `is_admissible()` is certified and every gate measured. NotMeasured and Errored block. Scorecard is posted and amended in place.
4. Close the shortfall: `fix` and `heal-queue` on that same PR.
5. Enlist: if `is_admissible()`, Anvil enlists. Manager does not merge or approve.
6. Feed Product: Observability, Support repros, and Research results become the next Discover bet. That is the closed loop. Discover and Support are still uninstrumented. Naming the feed is not claiming those seats exist.

Runtime: standing path is `cargo run -- serve` plus webhooks on the three. Do not start the daemon, serve process, or webhooks unless Jason asks. Until then, manager drives Anvil and fleet PRs through the CLI of this loop first. A cloud agent is only for when that loop is stuck.

Dogfood is proven when a PR on that repo actually went through review → certify → (fix/heal if needed) → enlist or honest block, and the scorecard matches the live corpus. It is not proven because README says "Anvil reviews its own pull requests."

## What needs to be done (sequence)
Do not add twenty named gates. Give each hole a real artifact and a measurement. A seat is not done while its measurement is NotMeasured.

1. Close DocGuard. Published docs match `TOTAL_GATES` or the gate fails. In flight: [PR #12](https://github.com/oyatie/anvil/pull/12) (`fix/docguard-honest-corpus`). rustfmt fix pushed (`21948e9`). Do not merge until Jason reviews. [PR #11](https://github.com/oyatie/anvil/pull/11) is Copilot plan-only theater. Close it. Do not treat it as the close. README `.env` must list `oyatie/anvil` with the other two.
2. Prove dogfood. A real PR on each watched repo, including Anvil, goes through this loop. Serve stays off until asked. CLI is enough to prove it. Do not claim self-loop from the config default.
3. Architecture pages. ADR-0001 filename still says sixty-gate on purpose (founding name). Live count is `TOTAL_GATES`. Keep founding names historical. Repair doctrine and this ADR if they drift.
4. Legal seat. Encode the it-legal fabric as schema + fail-closed measurement. Do not ship a `legal_status` rename of `compliance_status`.
5. Discover seats. Product done-when, Program plan, Research result, UX evidence, Data science eval. Quality sign-off must fail if Product's bar is missing.
6. Support seat. Repro artifact that feeds Product. That closes the loop this manager was asked to watch.
7. [PR #10](https://github.com/oyatie/anvil/pull/10) is a separate migration / self-gate track. Do not pile DocGuard onto it.

## Snapshot 2026-08-21 (this session, will go stale)
- Live corpus on main: `TOTAL_GATES = 68`. PR #10 claims 70 after two new fields. Count authority is the field list, not this sentence.
- Cloud agent cannot launch: Cursor GitHub app is not installed on `oyatie/anvil`. Fixes go through the GitHub connector until that is installed.
- Manager weekday watch: 8:32 America/New_York.
- Enlist lock: Anvil enlists when admissible. Manager does not merge or approve.

## What this page is not
Not a legal opinion. Not a claim that Discover or Support exist. Not permission to merge. Not permission to start the daemon. Not a claim that dogfood already ran.
