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
`blocked_on: None` (promotable with work alone) and 37 are blocked. The 37 blocker strings are
pairwise distinct, but **distinct strings are not disjoint dependencies** — a keyword pass over the
same 37 shows shared capabilities: `telemetry` in 4, `deploy` in 4, and `prometheus`, `opentelemetry`,
`canary` and `signing` in 3 each. One reachable Prometheus/OTel endpoint therefore unblocks several
rows at once, and the blocked set should be scheduled by shared capability, not one ticket per gate
(doctrine: measure overlap before claiming disjoint — an earlier draft of this section claimed the
blockers were disjoint on exact-string distinctness alone, which is the same proxy error the
workstream drains).

The gap field carries a second signal, and it is softer than a count implies, so the instrument is
stated with it. Classifying the 73 `gap` strings by phrase match: under a narrow pattern
(`regex|substring|grep|keyword|string`) **26** describe a string decision standing in for the named
instrument and **10** (`hardcod|hard-cod|constant`) describe a fixed result; under a broader phrase
set those rise to ~30 and ~31. **The two sets overlap under every pattern tried** (3 gates narrow,
13 broad), so they are a signal about the drain's shape — one dominant cause rather than seventy
independent ones — and explicitly not a partition of the corpus. Any number published from this
field must carry the pattern that produced it.

A second measured item, because it changes sequencing: the drift ledger is **computed in
production and then dropped**. `gap_report` calls `drift::against_the_proof_ledger()`
(`src/fidelity/mod.rs:181`), which is a non-`cfg(test)` function — the "0 drifting by construction"
defect was fixed in `7aceff9` (2026-08-29) and `tests/the_scorecard_stops_printing_zero_drifting_test.rs:29`
now asserts `drift: Vec::new()` cannot return. What remains is that nothing publishes the result:
`src/publish/scorecard.rs:309` is the only production `gap_report` caller and reads `.unaudited`
only, and `GapReport::summary()` — the one place that renders `drift.len()` — has no production
caller. So drift is measured and unread, which is a weaker defect than a dead ledger but the same
consequence for anything downstream: a usefulness ratio or an autonomy-tier evidence packet built on
fidelity state would be built on a number no production surface displays. Publishing it is a
precondition for the milestones below rather than one of them.

*(Recorded because of how the earlier draft got this wrong: it quoted the doc comment above
`against_the_proof_ledger`, which narrates the pre-fix state in the present tense, instead of reading
the function beneath it. "Measured, not quoted" governs state claims, not only counts.)*

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
| H1-17 | **`KANI_STATUS` promoted by invoking Kani**, not by linting for `// SAFETY:`. Domain is **anvil's own tree**, not the reviewed diff: `git grep -nE 'unsafe (fn\|impl\|\{\|trait)' -- src` = 4 hits, all fixtures or prose, so the surface is empty today and the honest first outcome is `Withheld`/`NothingToMeasure` from a gate that really ran, not a green from a lint. Kani over an arbitrary contributor's repo is a separate, much larger piece of work and is **not** this milestone | Kani installed and invoked in anvil's CI; a seeded `unsafe` block with real UB fails the gate (proving the instrument on a surface deliberately created for it); empty surface reports `Withheld`, never `Passed`; registry row moves `Heuristic` → `Measured` only once a `proof.rs` entry names that seed | Implementation |
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

Concretely: WS-08 targets a `Measured`-share and usefulness-ratio budget per horizon, and when the
budget is met the drain **stops** and capacity returns to delivery, rather than continuing to promote
gates because promotion is available.

**The rule is not decidable yet, and saying so is the point.** No number is stated here, and none is
invented: per ledger A-3 thresholds are ratified by Jason on an evidence ticket, not chosen by the
drafter. Until that ticket exists a machine cannot tell whether the drain should stop, so this rule
is an aspirational claim of exactly the kind this workstream drains — recorded as such rather than
presented as a control. **H1-18** closes it: the horizon budget is set by ratified ticket, with the
baseline being the measured 2/73, and both this rule and H1-16's deferred threshold resolve to that
one number so the same operand is not deferred twice.

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H1-18 | Horizon budget ratified: a `Measured`-share target and a usefulness-ratio threshold, one ticket, one operand shared with H1-16 | registry ticket exists and names both numbers against the 2/73 baseline; the drain's stop condition is machine-checkable against them; a seeded over-budget state halts the drain in CI | Human ticket queue (Jason ratifies) |

**Evidence gap, recorded rather than papered over:** the error-budget practice is foundational SRE
(2016-vintage), and a search for a source inside the commission's six-month window returned only
undated vendor pages and two posts whose URLs date them before the window opens. No in-window citation
is therefore recorded in `research.md` for this item, and it is carried here as a design decision
with its provenance named. If it is later ratified as roadmap doctrine it needs a decision-log row,
not a silent promotion to "researched".
