# WS-08 — Gate honesty: typed outcomes, real measurements, no fabricated inputs

**The malpractice classes this workstream exists to close (front-loaded, non-deferrable):**

1. **Absent evidence readable as a pass** (postmortem RC-2): `GateStatus::Passed` is a unit variant
   — "examined nothing" and "examined 400 files, found nothing" publish the same word. 34 of 72
   gates needed an absence-exemption policy to be admissible at all; `honesty_ratio` ≈ 0.02
   (postmortem-0001 RC-2; defined at `src/fidelity/mod.rs:145`); **no PR has ever been
   admissible** (#19).
2. **Proxy gates** (#59): 59 of 64 evaluated gates decide by inspecting the diff string in-process;
   5 invoke real tooling. Source text as a proxy for behaviour — cheap, mostly right, silent when
   wrong.
3. **Fabricated inputs** (#53, #200): daemon timers evaluating literals (`tests::flaky_test`),
   fleet summaries publishing `"HEAD"` and zeros nobody measured; `aggregate_fleet_overview`
   replacing every failed query with a healthy literal — a fabricated DORA snapshot on error
   (whose `change_failure_rate_percent: 1.4` regenerates the exact 98.6% figure this module's own
   doc comment says it removed), `unwrap_or_default()` open-PR counts, `unwrap_or(0)` queue depth —
   making the dashboard's em-dash path unreachable in production (#200, filed 2026-09-01; its sites are the H1 drain's newest
   seeds — per prove-a-check, each failing query is seeded and the surface shown reporting absence
   before the fix is trusted).

These are one root pattern (a proxy trusted as the thing), so the fix is class-level: change the
types so the defect has no spelling, then drain the instances against a ratchet. Never one more
bespoke per-gate patch — the postmortem records that per-gate patching "re-establishes the same
ambiguity in a new place."

**Registry state, measured 2026-09-04** (`cat src/fidelity/registry/entries_*.rs | grep -oE 'fidelity: Fidelity::[A-Za-z]+' | sort | uniq -c`):
73 entries — 42 `Heuristic`, 22 `Aspirational`, 7 `Partial`, **2 `Measured`**. Of the 73, 36 carry
`blocked_on: None` (promotable with work alone) and 37 are blocked — and all 37 blockers are
*distinct*, one gate each, so there is no shared external dependency to unlock a cluster. The shared
cause is in the gaps, not the blockers: **30 of 73 gap strings describe a string or regex decision
standing in for the instrument the gate names**, and 11 describe a hardcoded or constant result.
That is the drain's actual shape — one cause, not thirty tickets.

A second measured item, because it changes sequencing: `src/fidelity/drift.rs` records that
`gap_report` hardcodes `drift: Vec::new()` and that every caller of `audit_against_reality` is
inside `#[cfg(test)]` — "a measurement nobody took, published as one that was". The drift ledger is
dead in production. Nothing that consumes fidelity state (a promotion proposal, a usefulness ratio,
an autonomy-tier evidence packet) can be trusted before that ledger is live, so it is a
precondition for the milestones below rather than one of them.

## Mechanisms (ARCHITECTURE.md M2/M3/M4 — already designed, this schedules them)

- **M4 `Evaluated`:** `Measured { subjects_seen: NonZeroUsize, findings } | Withheld(Withheld)`.
  "Examined nothing, found nothing" has no spelling. `ABSENCE_POLICY` is deleted, not migrated.
- **M2 `Rule` rows:** `fn fixture(&self) -> Fixture` is **not optional** — a check that cannot
  demonstrate its own failure cannot register. 5 rules registered at dev head
  (`grep -c 'Box::new' src/harness/rules/mod.rs`) vs ~72 hand-wired gates; the drain moves gates
  into rows.
- **M3 one `Finding`:** `fix: Option<Fix>` is the declare/derive boundary — anvil auto-fixes
  `Some` (derived drift) and never `None` (a violated declaration needs a human ticket, WS-07).

## Milestones

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H1-1 | M4 lands; `ABSENCE_POLICY` deleted; admission recomputed over `Evaluated` | `grep -rn 'ABSENCE_POLICY' src/ \| wc -l` = 0; seeded empty-corpus fixture yields `Withheld` and blocks admission; measured-share metric live | Architecture |
| H1-1b | #19 decided through the registry: SLO telemetry configured **or** the gate's admission role re-scoped — by ticket, not by silent exemption | registry row exists; ≥1 real PR admissible within 30 days of M4 (admissibility-reachability metric) | Human ticket queue |
| H1-2 | Fixture mandate: registration refuses a rule without a red/green pair | CI test deletes a fixture, asserts registration failure | Test infrastructure |
| H1-3 | Proxy-gate census scripted (from #59's method: external tooling invoked vs in-process string decision) and ratcheted | census in CI; count strictly decreases quarterly; red-teamed with a `Command::new` in a comment (must not count) | Architecture |
| H1 | Fabricated-input drain (#53): every daemon timer consumes measured state or reports `Withheld`; fleet observer publishes real head SHAs and counts or nothing | seeded literal input fails a meta-test; `"HEAD"` placeholder gone (`grep` = 0) | Implementation |
| H1→H2 | Gate drain: each of the 59 in-process gates either (a) becomes an M2 rule invoking its real instrument, (b) is re-labeled to claim only what it checks (honest-names line), or (c) is deleted per ARCHITECTURE.md §8's ~10,600-line delete list | census trend + delete-list burndown, both charted from CI artifacts | Architecture |
| H1-15 | **Finding disposition captured**: every finding a gate emits carries an outcome (fixed / dismissed / ignored), on the M3 `Finding` type rather than a side table | a gate whose findings carry no disposition fails registration; disposition queryable per gate over real PRs | Architecture |
| H1-16 | **Usefulness ratio + disable threshold** (Tricorder's rule): dismissed / total per gate, measured on real PRs; a gate over threshold is **disabled**, not annotated | threshold stated in the registry; a seeded all-dismissed gate trips the disable path; disable is exercised at least once against a real gate | Quality sign-off |
| H1-17 | **`KANI_STATUS` promoted by invoking Kani**, not by linting for `// SAFETY:` — the corpus's highest-value single promotion, `blocked_on: None` | Kani runs in CI over the unsafe surface; seeded UB fails the gate; registry row moves `Heuristic` → `Measured` with a `proof.rs` entry | Implementation |
| H2 | Admission at target: `admission_refusal()` the only door (diagnostic `is_admissible()` retired or renamed per its weaker semantics) | one admission door (grep-level assertion + behavioural test); weekly admissibility count ≥ 1 sustained | Quality sign-off |

## Ratchets

- Proxy census: monotone decreasing, merge-blocking on increase.
- Fixture-or-no-registration (H1-2) makes the *next* proof-less gate unwritable.
- Fabricated-literal meta-test: production timers constructing their own inputs from literals are
  unrepresentable once inputs flow through typed measured-state providers.
- Usefulness ratio is merge-blocking on regression, and **disabling a useless gate is a legitimate
  outcome of the drain** — the corpus is allowed to shrink for cause, not only for the delete list.
- Delete list is shrink-only (`REORG-DRAIN`-style): a deleted module returning is a red.

## Non-goals

No new gates during the drain unless they arrive as M2 rows with fixtures; no per-gate absence
exemptions ever again; no renaming-down to dodge the census (ADR-0002's honesty law: "do not rename
down or drop a gate to make the corpus look clean" — re-labeling under (b) must *narrow the claim to
the measurement*, and the census counts it until it invokes its instrument).

## Stopping rule (decision, not a citation)

The drain needs a termination condition or it becomes the N+1 loop this workstream exists to avoid:
"promote every gate" is unbounded, and an unbounded improvement loop cannot be scheduled against
feature work. The rule adopted here is SRE's error-budget shape — reliability work is bounded by a
budget, not by aspiration, and stops when the budget is met.

Concretely: WS-08 targets a stated `Measured`-share and usefulness-ratio budget per horizon, and
when the budget is met the drain **stops** and the capacity returns to delivery, rather than
continuing to promote gates because promotion is available.

**Evidence gap, recorded rather than papered over:** the error-budget practice is foundational SRE
(2016-vintage), and a search for a source inside the commission's six-month window returned only
undated vendor pages and posts dated 2026-01-30 / 2026-02-20, both outside it. No in-window citation
is therefore recorded in `research.md` for this item, and it is carried here as a design decision
with its provenance named. If it is later ratified as roadmap doctrine it needs a decision-log row,
not a silent promotion to "researched".
