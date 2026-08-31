# WS-14 — Fleet conformance: docs, ADRs, and harness instructions as managed artifacts

**The commission's standing requirement:** the principles in this plan (and the prompt-engineering
research behind it) are not a one-off conformance job on this repo — Anvil must **repeatedly** check
conformance, in this repo and in any repo it manages. That covers three artifact classes:

1. **Published docs vs live corpus** — doctrine §1's law ("the field list is the authority") and
   DocGuard's fail-closed page amendment. Measured drift today: ADR-0005 says channel `1.97.1`
   while `rust-toolchain.toml` pins `1.98.0`; ADR-0006 cites `governance/capability-registry.json`,
   which no longer exists on oyatie dev; `tests/brand_absence_gate_test.rs:29` still says
   "68 entries in `all_statuses()`" against `TOTAL_GATES = 73`
   (`grep -rn '68 entries' tests/brand_absence_gate_test.rs`, measured 2026-08-31).
2. **Shape conformance** — WS-02's engine, run weekly per managed repo, same entrypoints as
   self-measurement (doctrine §5: a rule enforced outward but not inward "does not compile").
3. **Harness instructions** — `CLAUDE.md`/`AGENTS.md`/`rules.md` in every managed repo carry the
   same operating law (instruction-source discipline, data-vs-instructions, prove-a-check, typed
   evidence), templated once and drift-checked, so agent behaviour is uniform across the fleet.
   oyatie already states the strongest version of this law in its root AGENTS.md; the fleet gets
   one canonical template with per-repo deltas, not N hand-maintained copies.

## Typed I/O contract

- **Input:** `ConformanceRun { repo, rev, checks: [shape | doc_parity | pointer_liveness |
  harness_instructions | adr_freshness] }`.
- **Output:** `ConformanceReport { findings: [Finding], measured: NonZeroUsize subjects, rev,
  digest }` — WS-08's `Evaluated` envelope; an unreadable repo is `Withheld`, never clean.

## Milestones

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H1 | Pointer-liveness check: every path/URL an ADR or doctrine page cites must resolve at the cited rev — **plus the inward-target watch**: a weekly scripted diff of oyatie's `docs/decisions/ADR-INDEX.md` and ADR-07xx set (this is the H1 mechanism behind roadmap §5's "oyatie layout law moves" tripwire and WS-01's weekly re-measurement; the full conformance engine takes over at H2-10) | seeded dead pointer red; ADR-0006's dead citation caught by the check on its first run (the live defect is the seed); the watch job's weekly artifact exists and a seeded ADR-INDEX change opens a ticket | Docs |
| H1 | ADR freshness: `last_verified_at` older than policy on a `canonical_authority: true` page is a finding; ADR-0005's toolchain claim corrected through DocGuard's honest-page path | drift census = 0 on anvil after the sweep; recurrence caught weekly | Docs |
| H1 | Harness-instruction template v1: canonical `CLAUDE.md`/`AGENTS.md` law (from this commission's research: instruction-source discipline, batched-interview/defaults pattern for commissions, prove-a-check, measure-don't-trust) versioned in-repo; anvil adopts it | template exists; anvil's root files match template + declared delta (drift check green, seeded drift red) | Docs |
| H2-10 | Conformance engine v1 across the fleet: weekly `ConformanceReport` per managed repo; drift auto-opens tickets (cockpit) | three repos reporting weekly; a seeded drift in console opens a ticket end-to-end | Docs |
| H3 | Onboarding path: a new managed repo receives template + shape spec + conformance schedule as one codemod run (with WS-02) | onboarding drill produces first green report in ≤1 week | Implementation |

## Ratchets

- Doc-parity fails closed (ADR-0002 honesty law): a page DocGuard cannot amend honestly is a red,
  not a skip.
- Pointer-liveness and freshness run on self **and** tenants from the same entrypoints — the
  self-exemption spelling does not exist.
- Template drift: harness files diverging from template+delta is merge-blocking in the managed repo
  once adopted (baseline-block-on-new during rollout).

## Non-goals

No prose duplication into managed repos beyond the template's root files (oyatie's root-only
markdown law is respected outward); no auto-rewriting a tenant's ADRs (findings and PRs, tenant
owners decide — declare/derive boundary: `fix: None` means a human resolves it).
