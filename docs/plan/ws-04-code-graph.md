# WS-04 — Code graph

**Seed:** issue #15's RFC asks for exactly this tier. Research (2026): precompute structure (SCIP
symbol graphs — now under neutral open governance with Uber and Meta on the steering committee —
tree-sitter graphs, Glean-style fact DBs), expose it as **typed tools agents query for narrow
facts** instead of reading files; blast-radius computation is a named product category; code-graph
targeting is what makes codemods (WS-02) and review routing (WS-05) scale.

## Typed I/O contract (code graph is a named capability)

- **Input:** `GraphFact` queries — `refs(symbol)`, `defs(path)`, `blast_radius(change_set)`,
  `owners(path_set)`, `deps(module)` — each schema-versioned.
- **Output:** typed fact sets carrying `graph_digest` + `indexed_rev`; a query against a stale graph
  says so rather than answering silently (absent evidence is not an answer — invariant I1 applied to
  the graph).

## Milestones

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H1 (prep) | Module-dependency graph for anvil from `cargo metadata` + `syn`-level imports (the restructure plan's census, mechanized and kept fresh) | census numbers (modules, hub dependents) reproduced by the capability, matching the hand measurement at the same rev | Architecture |
| H2-3 | SCIP indexing for the fleet; blast-radius service answering for oyatie/console/anvil | 20-case ground-truth corpus: blast-radius answers match; freshness ≤ 1 commit behind dev (measured by the freshness check) | Architecture |
| H2-3b | Graph consumers wired: review routing by ownership (WS-05), codemod scoping (WS-02), gate scoping input (WS-03), context scoping for agents (narrow facts, not file dumps) | each consumer has a before/after measurement (e.g. review routed to owner seat; agent context tokens per task drop and are recorded) | Architecture |
| H3 | Cross-repo graph: fleet-wide symbol/dependency queries powering fleet LSC targeting (H3-4) | fleet LSC scope enumerated from the graph matches per-repo enumeration on a drill | Architecture |

## Ratchets

- Freshness check: a graph answer carrying `indexed_rev` older than the query's base rev is a
  refusal, not an answer (seeded stale-graph fixture proves it).
- Ground-truth corpus is append-only; a blast-radius regression on any corpus case blocks the graph
  service bump.
- Path-disjointness (occupancy) is documented as a **scheduler, not a correctness gate**
  (postmortem RC-5); the graph's type-coupling answers feed the merge-train rehearsal (WS-05), and a
  test keeps occupancy's own docs saying so.

## Non-goals

No knowledge-graph prose store (oyatie AGENTS.md forbids checked-in derived views; the graph is a
regenerable index, never authority); no semantic search product surface; no graph-driven *automatic*
merge decisions — the graph informs, policy decides (WS-06).
