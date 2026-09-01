# WS-12 — Instrument verification and scanner discipline

**The class (postmortem RC-1 + RC-6, issues #179, #52):** the verification instruments were not
themselves verified. Eight defects in one session were the same defect — a guard grepping source
text in a language whose comments and string literals mimic code; a grep counting `^test result:
ok` that could not express a failure reported a pass; a monitor whose `|| echo 0` made "not
finished" true immediately; the path-keyed source reads across test files that go **blind, not red**, the day
their subject file is split (`unwrap_or_default()` on the read — absent evidence published as a
pass, invariant I1, inside the test suite itself).

**Corrected by external review (2026-08-31), and the correction is itself an RC-6 instance:** the
grep issue #179 headlined (`grep -rn 'src/[a-z_]*\.rs"' tests/*.rs | wc -l` — 332 on 2026-08-30,
342 at `6128284`) counts **path string literals**, not reads: at `6128284`, the matches are
overwhelmingly fixture inputs (`&["src/lib.rs"]`, `changed_files: vec![…]`), exactly one
`read_to_string("src/…` call site remains in `tests/` (`git grep -c 'read_to_string("src/'
6128284 -- tests/` → `source_scan_test.rs:1`), and an in-tree ratchet already bans the read class
(`tests/path_keyed_source_read_ratchet_test.rs`, present at the baseline). #179's *mechanism*
(blind `unwrap_or_default` reads) was real and is now substantially drained; its *headline count*
was a proxy for it — this plan initially trusted that proxy, quoted it, and built a milestone on
it. The two instruments are kept distinct below.

## The three scanner rules (postmortem RC-1, now enforced rather than remembered)

1. A scan reads `source_scan::code_only` — never raw text, never `without_commentary` — unless it is
   *locating* a literal and says so.
2. A scan **refuses to answer** when its parse is ambiguous (`code_only` does not model raw strings;
   a scan that was fooled must not vote).
3. A scan is **seeded in both directions** before it is trusted: the defect it names must fail it,
   and correct code must pass it — and the seed is asserted to have applied (a silently no-op'd
   patch makes a broken check look sound).

## Milestones

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H1-7a | Meta-guard over `tests/` and guard code enforcing rules 1–3 | seeded raw-text scan red; seeded guess-on-ambiguity red; both archived as proof runs | Test infrastructure |
| H1-7b | Path-keyed-read class closed out (#179): **prove the existing in-tree ratchet** (`path_keyed_source_read_ratchet_test`) by seeding a blind read and showing it red (rule 3 — it has never been shown failing); disposition the one remaining `read_to_string("src/…` site in `source_scan_test.rs`; the 342-literal census is kept only as a **fixture-staleness signal** (a literal naming a live module as a scan subject goes stale on split), explicitly not a read count | seeded blind read turns the ratchet red (proof archived); read-call count in `tests/` (`git grep 'read_to_string("src/' -- tests/`) is 0 or each site is dispositioned; fixture-literal census published with its predicate stated | Test infrastructure |
| H1-7c | Instrument seeding for CI tooling (RC-6): any command whose output is reported as a pass must be shown expressing the corresponding failure once | instrument registry: each entry links its red-proof run; unregistered instruments fail the census | Test infrastructure |
| H1-7d | Scanner signatures take `&SubjectRoot` (compile-time half, shared with WS-09 H1-7d — postmortem leverage item 6, front-loaded: it was "already written down and still open" once, and that is the failure mode this plan exists to prevent) | compile-fail test holds; `env!("CARGO_MANIFEST_DIR")` reachable from zero gate scanners | Architecture |
| H1-7e | #52's door scan demoted to backstop when behavioural door tests land (WS-05); refusal assertions restored | admission-decision deletion reddens the behavioural suite (measured in-CI) | Test infrastructure |

## Ratchets

- Path-keyed-read *call* count in `tests/` held at zero by the in-tree ratchet test (proven per
  H1-7b); the 342-literal census is a signal, never a ratchet — ratcheting a proxy count was this
  file's own near-miss, caught by external review.
- Prove-a-check as registration: WS-08's fixture mandate covers gates; this workstream extends the
  same mandate to *test-suite guards and CI instruments* — the instrument registry refuses an entry
  without a red-proof link.
- Both-directions seeding is itself checked: a proof run whose seed diff applied to zero files is
  invalid (assert-the-seed-applied).

## Non-goals

No "better regexes" (the postmortem's explicit non-fix); no rewrite of fixture literals for its
own sake (they are inputs, not reads); scans stay for wiring assertions
that have no type — under rules 1–3 or not at all.
