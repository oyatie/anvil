# WS-01 — Inward convergence (anvil's own tree → the layout oyatie is converging toward)

**Obligation:** inward only. This workstream reshapes *anvil's* tree so oyatie absorption is cheap.
It never touches a managed repo (that is WS-02) and never copies an oyatie defect (discrepancies go
upstream as findings).

## Ground truth this builds on

- `docs/restructure-plan.md` (rewritten 2026-08-24 against dev @ `769a7de`): 115 modules today
  (re-measured at `6128284`), all declared in one `src/lib.rs`; kernel analysis says two levers pay
  (`git_manager` +30, `pre_merge_guard` +29 — mostly the `GateStatus` enum) and extraction stops
  paying after four hubs.
- A `core/ports/adapters/facade` layer already ships in `change_delivery`, `ratchet`, `shape` —
  extend the proven pattern, don't argue for a new one.
- Serialization points measured on the seven-PR drain: every pair of gate PRs conflicted on
  `src/fidelity/registry.rs` and one shared test file; `src/lib.rs` is a single declaration point.
  **Phase C cannot deliver parallel moves until these are removed.**
- The pilot (`~/Developer/intelligence`) already carries the target unit shape
  (`core/ports/adapters/facade` + `OWNERS`/`BUCK`/PRD/SPEC satellites, toolchain 1.98.0).

## Milestones

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H1-8a | Phase A kernel extraction: `GateStatus`→core; `ProcessPort`/`GitPort`/forge ports split per restructure D1/D2, four PRs | per PR: fmt + clippy `-D warnings` + full suite green **with counts** stated before/after; re-export step used so every intermediate state compiles | Architecture |
| H1-8b | Serialization fix: per-capability `mod` declarations; one file per gate under `fidelity/gates/`; shared test file split | two seeded disjoint gate-PRs merge without conflict on the queue; `src/lib.rs` diff-touched only when a capability is added (test asserts) | Architecture |
| H1-8c | Phase B workspace split; pilot-absorption reconciliation ledger (axum/hmac/sha2 bumps are security-relevant, per restructure plan) | root manifest is `[workspace]`; `hmac`/`sha2` bump PR carries webhook-signature regression tests seeded red first | Architecture |
| H2-1 | Phase C capability migration of the 86+ dependency-free modules | shape gate enforcing with zero `advisory-until-infra`; a week with **5** parallel move-PRs (default pin; registry row at start) and zero serialization conflicts (queue-measured) | Architecture |
| H2-1b | Pilot absorbed as `intelligence/` capability; the five duplicate concepts collapse to one (`ProcessPort`, `ModelPort`, `SandboxPort`, `GitPort`, verdict type — the fifth needs seeded-defect coverage first, per restructure plan) | duplicate-concept census = 0; `--dangerously-skip-permissions` spelling gone from tree (`grep -rn` = 0) | Architecture |
| H3-6 | Absorption event (see roadmap H3-6) | oyatie presubmit green on the absorption series; decided by ticket | Human ticket queue |

## The upstream-findings duty (standing, starts now)

Every oyatie discrepancy measured in roadmap §1.4 is filed on oyatie as an issue, each citing the
command that measured it — dead `governance/capability-registry.json` pointer (also fix anvil
ADR-0006's citation), 8 face-less capability roots, `intelligence/` cap-root `contracts/`+`k8s/` vs
ADR-0719 D-8, the 119-file frozen markdown inventory's unclassified status, and
`branch-protection.yaml`'s self-declared drift. **Exit criterion:** every §1.4 *discrepancy* row
(the preamble's finding set — not the rows that merely record oyatie's law and counts) has an
upstream issue URL recorded in the decision registry; re-measurement runs weekly — in H1 via WS-02 H1-13b's
shape report plus WS-14's ADR-INDEX watch, from H2-10 via the full conformance engine — and new
discrepancies auto-open tickets. **Owner:** Architecture files; Human ticket queue tracks upstream
acceptance (anvil cannot close oyatie's findings for it).

## Ratchets

- Shape gate on self: `unit_missing_face` and `cross_unit_non_facade` leave `advisory-until-infra`
  at H1-13 and become blocking; a seeded dump-root (`plan/`, `libs/`) must fail CI before the mode
  flips (prove the check first).
- Declaration-point ratchet: a test fails any PR that adds a `pub mod` to `src/lib.rs` for an
  existing capability.
- Convergence direction: module count in flat `src/` only decreases (baseline 115); the ratchet
  refuses a new flat top-level module.

## Non-goals

No renaming sweep for its own sake; no adoption of oyatie satellites (`cedar/`, `iac/`) before a
capability actually carries policy or infra; no absorption before oyatie's consolidation stabilizes
(tripwire in roadmap §5).
