---
status: Accepted
date: 2026-08-20
---

# ADR-0006 — The shape specification is tenant data, not Anvil code

## Context

Anvil enforces repository shape on the repositories it manages:
`CleanArchitectureGuard` checks `core → ports → adapters → facade`,
`MonorepoGuard` checks hermeticity, `DebtShrinkGuard` reads `REORG-DRAIN.md`.
Each carries its layout rule in Rust source, so each rule is an assertion
about other people's repositories that Anvil cannot apply to itself (its own
tree has no such layout) and cannot state for a tenant whose layout differs.
Invariant I13 forbids exactly this: a rule Anvil cannot state generically
belongs in the tenant repository, not in the tool.

oyatie already expresses its shape as data — ADR-0562 §3 (a deterministic
placement rule) and per-gate `<gate>-policy.json` files. What it lacks is a
declared satellite set (runbooks, SLOs, contracts, …) and a single engine that
measures every unit, capability or app, against one skeleton.

**Amended 2026-09-10 (#198, #269).** This paragraph cited
`governance/capability-registry.json` as oyatie's closed unit set. No such path
exists there, and none can: `governance` is in that repository's
`FORBIDDEN_NAMES`. Measuring the proposal against oyatie refused outright, so
it had never produced a figure since this ADR was accepted.

The citation is not repointed, because the file is not wanted. The 21
capabilities are exactly the directories holding a declared face, so a registry
listing them restates the tree — hand-maintained, drifting, checkable against
nothing. `unit_kinds` gained `members: "faces"` and oyatie's spec names no
registry at all. §3 below still governs a spec that *does* refer to one; no
shipped proposal now does.

## Decision

1. Every tenant repository carries `.anvil/shape.json` (`schema: anvil/shape/v1`).
   It declares unit kinds and their roots, skeletons (faces, the dependency
   matrix, the satellite set with one canonical home per class), placement
   steps (ADR-0562 §3 as data), root-file allowlist, naming rules, legacy
   roots, per-unit `destination_stable`, and per-rule mode.
2. Anvil ships only the engine. `src/shape/` contains no face name, satellite
   name, crate prefix, ADR id or registry path. `tests/shape_no_hardcoded_layout_test.rs`
   scans its string literals and fails on any. The one path literal is
   `.anvil/shape.json` itself — Anvil's config location, not a tenant rule.
3. A spec that refers to a registry (`unit_registry`) reads it by JSON pointer
   and key; Anvil never copies the registry. Proposals that would change the
   registry (a satellite key, the SLO location) are opened as PRs to its
   owner. (This cited "plan §27.4", which does not exist -- the roadmap has
   §1-§8 and `27.4` appears nowhere under `docs/plan/`. Struck rather than
   repointed, since the sentence stands without a citation. #198's own class,
   found in the file amending it.)
4. Unknown keys are errors. A rule the spec does not declare is not run. A
   spec that needs a registry it was not given resolves nothing rather than
   guessing.
5. The same skeleton applies to capabilities and apps: an app is a unit whose
   placement step was `composition`, not a unit with a different shape.

## Consequences

- **Amended 2026-09-10 (#269): face-directory form is validated for every
  skeleton, not only those a `faces` kind discovers on.** Face directories drive
  `unit_missing_face` and `face_edge_denied` however a unit was enrolled, so a
  spec whose malformed face dirs sat on a skeleton reached only by a `discover:`
  kind validated before and does not now. No shipped proposal was affected.
  A skeleton no unit kind references is also refused.

- oyatie, console and Anvil are measured by one engine against three specs;
  the specs in `tests/fixtures/shape/` are the proposals until each tenant
  adopts its own.
- Anvil's own `.anvil/shape.json` reports every flat module as
  `unit_missing_face` until the tree is restructured — the honest first
  number, recorded rather than suppressed.
- `CleanArchitectureGuard`, `DebtShrinkGuard` and `MonorepoGuard`'s layout
  rules are subsumed once the shape gate blocks (G11: one owner per rule).
