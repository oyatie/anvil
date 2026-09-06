# WS-03 — Build graph and hermeticity

**Thesis (ledger A-4/A-5):** Cargo is the merge path today (oyatie ADR-0716 mirrored by anvil
ADR-0005). Buck2 dual-build lands in H2 after capability migration begins; at H3 the Buck2 target
graph becomes the **verification authority** — it decides what is built, tested, cached, and
certified — while Cargo remains the crates-io-facing manifest and merge authority stays with the
queue + policy engine. Research grounding: Meta keeps the Cargo↔Buck2 bridge production-grade
(reindeer, daily maintenance through 2026-08); btd/supertd target determination is the CI front
door; CAS is sub-artifact (content-defined chunking) and eviction-resilient (action rewinding).

## Typed I/O contract (build graph is a named capability)

- **Input:** `GraphQuery { repo, base_rev, head_rev }`.
- **Output:** `ImpactedSet { targets, depth_ranked, graph_digest, skipped: [target, reason] }` —
  the skip list is an auditable artifact, never implicit.

## Milestones

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H1 (with H1-8c) | Workspace split gives anvil a real Cargo package graph; `cargo metadata` becomes the graph source; `--locked` everywhere stays (ADR-0005) | graph capability answers `ImpactedSet` for a seeded leaf change correctly (ground-truth corpus, 10 cases) | Builder tools |
| H1 | Hermeticity gates stop being string checks: network-deny test harness for gate execution (a gate that fetches the network in a hermetic lane fails) | seeded network-touching gate red in the hermetic lane | Builder tools |
| H2-2 | Buck2 dual build: `.buckconfig`, reindeer-vendored `third-party/`, first-party BUCK generated from `cargo metadata`, weekly smoke (oyatie's pattern) | `buck2 build //...` green weekly; seeded drift between BUCK and manifest fails the drift gate | Builder tools |
| H2-5 | Target-keyed certification pilot: gate scoping by `ImpactedSet` instead of whole-repo | seeded leaf-only change shrinks the executed gate set; skip list emitted and archived per run | Builder tools |
| H3-1 | Buck2 graph = verification authority (roadmap H3-1); ML/predictive selection layered only after btd-sound selection, with measured escape rate | 100% merge-blocking gates target-keyed; fallback-to-full only on graph invalidation, logged | Builder tools |

## Ratchets

- Drift gate: BUCK files are a deterministic function of `Cargo.lock`/`cargo metadata`; divergence is
  merge-blocking (proven by seeding a hand-edited BUCK file first).
- Hermeticity ratchet: the set of gates allowed ambient network/filesystem access is a frozen
  baseline that only shrinks.
- Doctrine §4 stays binding: PR CI wallclock ≤ 5 min, ≥90% cache hit; heavy lanes partitioned to
  nightly/weekly — measured in CI, not asserted.

## Non-goals

Buck2 never becomes merge authority (that is the queue + policy, ledger A-5); no Bazel track (the
fleet is Buck2+Cargo; Bazel findings in research are transferable mechanics, not a migration); no
remote-execution build farm before target-keyed certification proves local+CAS insufficient
(cost tripwire, roadmap §5).
