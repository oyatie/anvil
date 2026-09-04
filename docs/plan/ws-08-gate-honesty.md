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
`blocked_on: None` (promotable with work alone) and 37 are blocked.

**The proxy-gate count is that typed field, not a scan of prose.** `Fidelity::Heuristic` is
*defined* at `src/fidelity/mod.rs:55-58` as "a proxy signal -- a regex, a line count, a filename
match", so the count above already **is** the proxy census: **42 of 73**, from the same command, no
further derivation.

This is **not** the same number as class 2's "59 of 64", and the two must not be conflated: that
census counts *evaluated gates deciding by in-process string inspection*, and it is what H1-3
scripts and ratchets monotone-decreasing; this one counts *registry rows whose declared fidelity is
`Heuristic`*. Different populations, different methods, both real — H1-3 tracks the 59, and 42/73 is
the registry's own view of the same malpractice. Earlier drafts classified the free-text `gap` field with a regex and
published counts from it instead -- three times, all three wrong: the published pattern returned
34/11/5 while the text claimed 26/10/3 (that pair comes from an unstated word-boundary form, which
silently drops seven genuine members whose gap text pluralises "regexes"/"substrings", plus one **false** positive of the plain form -- the attestation gate, matching only via `to_string_pretty` inside `serde_json::to_string_pretty`, whose gap describes no string decision at all, so the boundary form is right to drop it); a second pair
shipped with no pattern at all; and the two sets were never disjoint under any pattern tried. The
lesson is this workstream's own -- a number derived from prose by regex is a proxy, and the typed
field beside it was the thing all along. **No count from the `gap` field is published here.**

**The blocked set schedules by shared capability, not one ticket per gate.** The 37 blocker strings
are pairwise distinct, but distinct strings are not disjoint dependencies. Counted case-insensitively over the `blocked_on`
values, **after joining Rust's `\`-continuations** -- 11 of the 37 `blocked_on: Some(` calls are
written across lines (`grep -rn 'blocked_on: Some($' src/ | wc -l`) and 9 carry a literal continuation inside the
string, so a single-line `grep -c` returns 3/2/2/2/2/1, every one of them wrong -- and the case-fold is
load-bearing too, since `prometheus` and `opentelemetry` return 0 case-sensitively: `telemetry` 4, `deploy` 4, `prometheus` 3, `opentelemetry` 3,
`canary` 3, `signing` 3. Read in full, those three Prometheus rows want different things -- one names
only "a reachable Prometheus or OpenTelemetry endpoint", while the other two also require a canary
deployment, one noting "this crate has no HTTP client to reach one with". So an endpoint is a
**shared prerequisite of three rows**, not something that unblocks them: exactly one of the three is
unblocked by the endpoint alone. Keyword co-occurrence is itself a proxy for a dependency, used here
to group the drain and never to assert that a capability closes a row.

A second measured item, because it changes sequencing: the drift ledger is **computed in
production and then dropped**. `gap_report` calls `drift::against_the_proof_ledger()`
(`src/fidelity/mod.rs:181`), which is a non-`cfg(test)` function — the "0 drifting by construction"
defect was fixed in `7aceff9` (2026-08-29) and `tests/the_scorecard_stops_printing_zero_drifting_test.rs:27-28`
asserts the literal `drift: Vec::new()` is absent from the source -- a text scan, so it proves the
literal is not *written*, not that empty cannot be *returned*; the runtime path is exercised at
`tests/fidelity_drift_ratchet_test.rs:57`, which calls `against_the_proof_ledger()` for real. What remains is that nothing publishes the result:
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
| H1-17 | **`KANI_STATUS` promoted by invoking Kani**, not by linting for `// SAFETY:`. Domain is **`src/`** -- not the reviewed diff, and not the whole tree. Over `src/` the unsafe surface is 4 hits, all fixtures or module prose, so it is empty today (command in the note below this table: pipes in an inline regex do not survive a markdown cell, and printed inside one it returns 0 -- which would agree with the conclusion, so the broken form could not have expressed a failure). Scoped to code (`-- src tests`) there are 30 hits, 26 of them in `tests/` -- of those 26, **10** are real unsafe blocks (env-var ceremony under Rust 2024) and 16 are fixture strings or prose, so none is a proof obligation and they widen the domain without adding a Kani target. **The scope is load-bearing, and the unscoped count is deliberately not published here:** at `ec1e5c9` the unscoped form returned 31 rather than 30 because its 31st hit was this row's own prose, the RC-6 class the roadmap logs elsewhere (as a `depends-on` over-match and an arXiv-id over-match, not as this one). Exactly one committed revision carries that match -- `ec1e5c9`, named above; later drafts re-created it while describing it, which is the point -- any sentence about this regex tends to contain a token the regex matches, so an unscoped count over a corpus that includes this file moves whenever this sentence is edited. Scoping to `-- src tests` is the fix; quoting an unscoped number is not and the honest first outcome is `Withheld`/`NothingToMeasure` from a gate that really ran, not a green from a lint. Kani over an arbitrary contributor's repo is a separate, much larger piece of work and is **not** this milestone | Kani installed and invoked in anvil's CI; a seeded `unsafe` block with real UB fails the gate (proving the instrument on a surface deliberately created for it); empty surface reports `Withheld`, never `Passed`; registry row moves `Heuristic` → `Measured` only once a `proof.rs` entry names that seed | Implementation |
| H1-18 | Horizon budget ratified: a `Measured`-share target **and** a usefulness-ratio threshold (two different operands, one ticket). Blocked on H1-15 | registry ticket exists and names both numbers against the measured 2/73 baseline; the stop condition is a predicate over those two numbers, evaluated in CI; a seeded over-budget state halts the drain | Human ticket queue (Jason ratifies) |
| H2 | Admission at target: `admission_refusal()` the only door (diagnostic `is_admissible()` retired or renamed per its weaker semantics) | one admission door (grep-level assertion + behavioural test); weekly admissibility count ≥ 1 sustained | Quality sign-off |

**H1-17's command**, outside the table so its regex survives:

```
git grep -nE 'unsafe (fn|impl|\{|trait)' -- src   # 4 hits, all fixtures or module prose
```

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
invented: ledger A-3 puts *tier promotions* on a Jason-ratified evidence ticket, and
this extends that mechanism to a gate-honesty threshold -- an extension of A-3, not a quotation of
it, and owed a ledger row of its own if it stands. Until that ticket exists a machine cannot tell whether the drain should stop, so this rule
is an aspirational claim of exactly the kind this workstream drains — recorded as such rather than
presented as a control. **H1-18** closes it: the horizon budget is set by ratified ticket, with the
baseline being the measured 2/73, and one ticket sets **both** operands together -- a corpus-level
`Measured`-share target and a per-gate usefulness ratio, which are different numbers -- so the two
deferrals resolve on one ratification instead of drifting apart. H1-18 cannot start before H1-15:
a usefulness ratio has nothing to measure until findings carry a disposition.


**Evidence gap, recorded rather than papered over:** the error-budget practice is foundational SRE
(2016-vintage), and a search for a source inside the commission's six-month window returned only
undated vendor pages and two posts whose URLs date them 2026-01-30 and 2026-02-20, both before the window opens. No in-window citation
is therefore recorded in `research.md` for this item, and it is carried here as a design decision
with its provenance named. If it is later ratified as roadmap doctrine it needs a decision-log row,
not a silent promotion to "researched".
