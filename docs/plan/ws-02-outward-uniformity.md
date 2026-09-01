# WS-02 — Outward managed-repo uniformity (the codemod-first LSC pipeline)

**Obligation:** outward — with one deliberate, labeled inward exception. Managed repos are written
and rewritten into one uniform hyperscaler-monorepo shape with clean architecture — defined by the
shape spec each tenant carries, **not** by oyatie's internal tree and not by anvil's. ADR-0006
already made the right cut: anvil ships only the engine; the layout lives in per-tenant
`.anvil/shape.json`, and `tests/shape_no_hardcoded_layout_test.rs` keeps tenant names out of the
engine. The exception: the engine's **first tenant is anvil itself** (H1-13) — doctrine §5 forbids
enforcing outward a rule not applied inward through the same entrypoints, so H1-13 is
inward-obligation work (the roadmap labels it "WS-02 inward half") and it gates every outward
enforcement milestone below. That is self-application as a precondition, not a conflation: the
*shape each tenant takes* remains the tenant's spec.

## The LSC doctrine (hard constraint, verbatim into practice)

Every large-scale change is **codemod-first**: a deterministic transform carries the mechanical
majority; agents handle only the enumerated tail; a ratchet holds the ground so the old pattern is
unwritable afterward. Research (2026): this is the industry-endorsed pattern — rulebook-driven
migration with a dependency-sharded mechanical work queue and adversarial reviewer agents
(Anthropic's playbook), deterministic recipe catalogs called by AI as tools (Moderne/OpenRewrite,
Gartner-endorsed), `npx codemod ai` as a default agent skill, ast-grep-class structural rewrite
primitives. "Review loop results, not code."

## Typed I/O contract (codemod is a named capability)

- **Input:** `LscSpec { id, tenant, transform (recipe/ast-grep program or Rust codemod), scope
  (path/target set from WS-04 graph), ratchet_rule_id, rollback }` — schema-versioned JSON.
- **Output:** `LscReport { edits_mechanical, edits_tail: [{path, agent, evidence}], parity: red/green
  recipe tests, ratchet_baseline_ref }`. Unattributed edits fail the schema (roadmap §4).

## Milestones

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H1-13 | Self-application first (**inward obligation**; the labeled exception above): anvil's own shape spec enforcing, zero self-exemptions (same engine, same entrypoints as tenants — doctrine §5: "a rule Anvil would enforce on oyatie and not on itself is a rule that does not compile") | `.anvil/shape.json` has no `advisory-until-infra`; seeded violation red | Architecture |
| H1-13b | Report-only conformance on oyatie + console using their proposal specs (`tests/fixtures/shape/`) | weekly report artifact per repo; findings become upstream tickets (WS-01 duty for oyatie; console analog) | Docs |
| H2-4 | Codemod capability lands (contract above); **first real LSC** on one managed repo: mechanical majority by transform, tail by agents, ratchet flipped | `LscReport` shows ≥80% mechanical; ratchet red on seeded reintroduction; recipe before/after tests in CI | Implementation |
| H2-4b | Shape-migration LSC catalog: dump-root dissolution, satellite placement, face moves — each a reusable recipe with parity tests | catalog entries each carry a fixture repo that goes red→green | Implementation |
| H3-4 | Fleet-scale: standing shape ratchets on every managed repo; onboarding drill = spec + codemod run + tail queue in ≤1 week | drill measured on a fresh repo, zero hand edits outside the tail queue | Implementation |

## Ratchets

- Per-tenant `baseline-block-on-new` on every shape rule (the mode already in `.anvil/shape.json`):
  the debt census is frozen; a *new* instance is merge-blocking from day one.
- LSC ground-holding: completing an LSC flips its `ratchet_rule_id` from baseline to forbid; the
  ratchet test seeds one old-pattern instance and must go red before the LSC is declared done.
- Engine purity: `shape_no_hardcoded_layout_test` stays; a tenant name in `src/shape/` is
  unwritable.

## Non-goals

No pushes to managed repos before H2-4 (report-only until the codemod capability carries typed
evidence); no oyatie-internal-shape enforcement on tenants; no per-repo bespoke gates — a new
managed-repo rule enters the engine generically or not at all (hard constraint: no N+1 gates).
