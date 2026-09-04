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
| H2 | Admission at target: `admission_refusal()` the only door (diagnostic `is_admissible()` retired or renamed per its weaker semantics) | one admission door (grep-level assertion + behavioural test); weekly admissibility count ≥ 1 sustained | Quality sign-off |

## Ratchets

- Proxy census: monotone decreasing, merge-blocking on increase.
- Fixture-or-no-registration (H1-2) makes the *next* proof-less gate unwritable.
- Fabricated-literal meta-test: production timers constructing their own inputs from literals are
  unrepresentable once inputs flow through typed measured-state providers.
- Delete list is shrink-only (`REORG-DRAIN`-style): a deleted module returning is a red.

## Non-goals

No new gates during the drain unless they arrive as M2 rows with fixtures; no per-gate absence
exemptions ever again; no renaming-down to dodge the census (ADR-0002's honesty law: "do not rename
down or drop a gate to make the corpus look clean" — re-labeling under (b) must *narrow the claim to
the measurement*, and the census counts it until it invokes its instrument).
