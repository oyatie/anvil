# Commission: execute one H1 milestone

**Precondition (verify, do not assume):** PR #197 must be merged, so `docs/plan/`,
`docs/restructure-plan.md` and `CLAUDE.md` exist on `dev`. Confirm with
`git cat-file -e origin/dev:docs/plan/anvil-roadmap.md` before anything else. If it fails, stop and
say so — every path below is wrong without it.

**Edit exactly one line before each run:** `ACTIVE MILESTONE: H1-<n>`.

---

## 1. Authorization and scope

You have my approval to create, modify and delete files under the in-scope paths below, to run
`cargo`, `git` and `gh` read commands locally, and to commit to a feature branch. You do **not**
have approval to push, open a PR, merge, rewrite history, enable auto-merge, run the Anvil daemon
or `serve`, or invoke any model-spawning subcommand against a live pull request.

In-scope paths for this run: **only** those named in the ACTIVE MILESTONE block below. This is a
single-crate repo — write `src/<module>/**` and `tests/<file>.rs`; there is no `crates/**` and no
workspace until Phase B.

Four single-lane hotspots serialise every change that touches them. Do not touch one unless your
milestone block names it:
- `src/lib.rs` — every module is declared here (115 `pub mod` lines at the time of writing; count it
  yourself), so any module move touches this one file
- `src/fidelity/registry.rs`
- `tests/evaluator_preserves_gate_verdicts_test.rs`
- `.github/workflows/**`

`Cargo.toml` carries **zero** `[[test]]` declarations on `dev` — integration tests auto-discover, so
adding a test file does not touch the manifest. Verify before relying on it.

## 2. Normative sources, precedence, and pin

Read these at the point of use. Do not summarise them into this session's working memory and then
work from the summary.

1. **`docs/plan/ws-*.md`** — the workstream file owning your milestone. Its row is **normative** for
   exit criterion, owner, evidence and ratchet.
2. **`docs/plan/anvil-roadmap.md`** — §2 horizon tables are an *index*; where a roadmap row and its
   `ws-*` twin disagree, the `ws-*` file wins and the roadmap is the defect (§2 says so explicitly).
   §1 is the measured current state; §6 the assumptions ledger; §7 the decision log.
3. **`docs/restructure-plan.md`** — Decisions D1–D5, the sequence, Phases A/B/C. Read its
   2026-08-31 status note first: its census is superseded and **D5 must not be executed as written**.
4. **`CLAUDE.md`**, **`docs/doctrine.md`**, **`rules.md`**, **`ARCHITECTURE.md`**.

Branch from `origin/dev` (not `main`, which is diverged legacy). State the exact base SHA in your
first message and use it as the pin for every measurement you report.

**Never restate, paraphrase, renumber or "tidy" an exit criterion, owner, ratchet or non-goal.**
Quote it verbatim or cite it by id. Two copies drift, and the one you internalise is the lossy one.

## 3. Invariants — each one ends in the detector that catches its violation

1. **Measure, never quote counts.** Every number in your report is immediately preceded by the
   fenced command that produced it and its raw stdout. A number without its command is a defect.
2. **Prove a check before trusting it.** Every new or modified check ships with: the seeding patch,
   an assertion that the seed diff was non-empty, the recorded red against unfixed code, and the
   green after. A check that has never failed on purpose has not been measured.
3. **Fix the class, not the instance.** Before fixing, census the siblings (`grep`/`rg` with the
   search shown) and state how the next instance is made unwritable — a type, a ratchet, or a
   meta-test. Paste the census output.
4. **Absent evidence is never a pass** (invariant I1, `src/doc_guard/mod.rs:34`). A check that
   examined nothing reports withheld, never green — including your own instruments.
5. **Verify the instrument.** Any command whose output you report as a pass must be able to express
   the corresponding failure, and you must show it doing so once. (A grep that cannot match a
   failure once reported a pass in this repo. Anchoring bugs are the common form: `git branch -r`
   indents its output, so `grep -c '^origin/'` silently returns 0.)
6. **Channel and MSRV are separate promises** in opposite directions; equality is a finding, not a
   tidy-up. `tests/toolchain_msrv_test.rs` enforces it.
7. **Green is not merge authority.** You do not push, approve, or merge. A human reviews first.

## 4. ACTIVE MILESTONE

```
ACTIVE MILESTONE: H1-<n>
```

Before editing anything, fill this block from the plan — do not let me pre-fill it, and do not
accept my paraphrase if I do:

- **Id and title:** quoted from the `ws-*` file.
- **Exit criterion:** pasted **verbatim**.
- **Owner** and **non-goals:** quoted.
- **Depends-on:** which H1 milestones must land first, from the plan's sequence. If an unmet
  dependency exists, stop and say so.
- **Target:** exact `path::symbol` — e.g. `src/pre_merge_guard/report.rs :: enum GateStatus`, that
  enum only, not its importers.
- **In-scope paths:** literal paths for this milestone.
- **Fixed (change only via a decision-log entry):** public trait signatures, error taxonomy, file
  placement, metric definitions, evidence schema.

Do **not** design the implementation in this block. Naming the target and the fixed surface is the
whole job here; internal structure is yours.

## 5. Start-of-milestone protocol

Before the first edit, in order:

1. Paste the milestone's exit criterion and depends-on **verbatim**.
2. One line per criterion: how you will satisfy it, and which command will discharge it.
3. **Audit the spec against the tree** and list every: **Conflict** (the criterion contradicts the
   plan, an invariant, or the code), **Omission** (information the criterion needs that is absent),
   **Ambiguity** (two defensible readings). For each, either resolve it by citing a normative source
   or stop per §8.
4. Record the pre-change suite count: `cargo nextest run` and paste the summary line. This is your
   baseline; a pure refactor that changes it has changed behaviour.

Do not "write tests first" as a ritual. Write the specific failing check the criterion names, seeded
against the defect it claims to catch — that is what invariant 2 requires and what a procedural TDD
instruction does not give you.

## 6. Evidence protocol

For every claim you make, one row:

```
claim_id · exit_criterion_id · cmd (argv, no shell pipeline you cannot rerun) · base_sha ·
exit_code · the last 20 lines of stdout
```

For every check you add or modify, additionally:

```
check_id · seeding patch (the diff) · assertion that the seed applied (diff non-empty) ·
red output (unfixed code) · green output (fixed code)
```

A summary of what a command showed is not evidence. Paste the output.

## 7. DONE — and who is allowed to say it

`DONE(H1-<n>)` is the conjunction of, all at the pin:

- every exit criterion the `ws-*` file states, discharged by the command you named in §5.2;
- `cargo fmt --check` → exit 0;
- `cargo clippy --all-targets -- -D warnings` → exit 0;
- `cargo nextest run` → 0 failed, with the count stated against the §5.4 baseline and any change
  explained;
- `git diff --name-only origin/dev...HEAD` → every path within the milestone's in-scope list;
- the seeded-defect proof for every new check.

**You do not decide that a milestone is done.** When you believe the conjunction holds, launch a
fresh-context reviewer that sees only: the milestone's `ws-*` section, your diff, and your evidence
rows — and is told to find reasons the conjunction does **not** hold. Paste its verdict verbatim,
including findings you disagree with, and say what you did about each. If it finds a gap, fix and
re-run it. Two consecutive clean passes, or it is not done.

A strong showing on five criteria does not offset a failed sixth. There is no partial DONE; there is
`PARTIAL` with the unmet criterion quoted word for word.

## 8. Stop triggers, and the retry budget

**Halt and report — do not proceed on judgement — when:**

1. an exit criterion is ambiguous, or you would have to interpret it to proceed;
2. satisfying one criterion appears to require violating another, or a stated non-goal;
3. an assumptions-ledger entry is contradicted by the code;
4. the change would touch an out-of-scope path or an unnamed hotspot;
5. a check fails and the only path you can see is to weaken the check, the ratchet, or the test;
6. the milestone's premise is false at the pin (the defect it fixes is already fixed, or the file it
   names does not exist).

Trigger 6 is not hypothetical: the plan's own external review found a milestone built on a count
that measured literals rather than reads, and one whose cited mechanism had already been retired.
**A criterion that is wrong at the pin is a PLAN-DEFECT.** Write it to the decision log with the
evidence and stop. Never edit the criterion, never reinterpret it, never narrow it to what you
achieved.

**Retry budget:** two attempts at a failing approach, then stop and report with the failure evidence
attached. Do not retry a third time in a contaminated context.

## 9. Report — the template, filled

1. Base SHA, branch name, ACTIVE MILESTONE id.
2. `git diff --stat`.
3. The DONE predicate copied verbatim, each line marked pass/fail with its command and output.
4. Seeded-defect proofs (patch, applied-assertion, red, green).
5. Sibling census with the search command and its output, plus the mechanism that makes the next
   instance unwritable.
6. Reviewer verdict, pasted, with your disposition of each finding.
7. Conflicts / omissions / ambiguities found in §5.3 and how each was resolved.
8. Anything deliberately not done, as a one-line follow-up each. Opportunistic refactors go here,
   not into the diff.
9. What is left for me: the review, and any PLAN-DEFECT or decision that needs my ratification.

Commit to a feature branch off `dev` with a message stating what changed and why. Do not push. Do
not open a PR. Stop after one milestone.
