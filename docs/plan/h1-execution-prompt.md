# Commission: execute one H1 milestone

You will execute **one** milestone of the plan in `docs/plan/`, in this repository, with write
access. Read §1 before anything else; it is the whole authorization.

Evidence for the claims in this file — the measurements, and the three earlier revisions that were
wrong — is in [`h1-execution-prompt-evidence.md`](h1-execution-prompt-evidence.md). You do not need
it to run. Every number below carries the command that produced it; **re-run them, do not trust
them.** A number that no longer reproduces is a PLAN-DEFECT under §8, not a licence to proceed.

## 1. Authorization

**You do not push, open a pull request, approve, merge, enable auto-merge, or rewrite history.
You do not run the daemon or `serve`. You stop after one milestone.** Nothing below softens this.

For this run only, you may:

- read anything in the repository;
- **create, modify and delete files only under the paths listed in `IN-SCOPE PATHS` (§2)** — plus
  `target/` and `$TMPDIR`, which tooling writes and neither is source;
- run `cargo build|check|test|nextest|clippy|metadata|tree`, and `cargo fmt` **only** as
  `cargo fmt --all -- --check`;
- format authorized files one path at a time: `rustfmt --edition 2024 <path>` (the edition flag is
  load-bearing — the crate is edition 2024 and rustfmt defaults to 2015);
- run git **reads**: `status diff diff-index log show cat-file ls-tree ls-files rev-parse
  merge-base check-ignore grep branch fetch`, and `stash list`;
- run git **writes**: `switch -c`, `add <explicit path>`, `commit`;
- run read-only text tools: `grep rg wc sort uniq cut tr paste bc diff head tail cat ls find echo
  printf sed`;
- run `gh` **reads**: `pr view|list|checks`, `issue view|list`, `run view|list`, `api` with GET;
- launch the §6 reviewer as a subagent with **read-and-report only** — it does not inherit write
  access.

**Refused, including by omission.** If a task appears to need something not granted above, that is
a stop under §8 — not a judgement call.

- **No form of any granted tool that writes, deletes, or executes.** This is the rule; the examples
  are not the rule: `sed -i`, `rg --replace`, `perl -i`, `find -delete`, `find -exec|-execdir|-ok`,
  `sort -o`, `tee`, redirection onto any tracked path. If you are unsure whether a form writes,
  it is refused.
- **`cargo run`** — the binary's default subcommand is `Serve`
  (`src/cli/handlers.rs:14`, `cli.command.unwrap_or(Commands::Serve)`), so a bare `cargo run` boots
  the production webhook daemon against live pull requests.
- **`cargo fmt` in any writing form** — it rewrites the whole crate, outside `IN-SCOPE PATHS`.
- `cargo publish`, `cargo install`, or anything that reaches the network to write.
- `git push|clean|reset --hard|checkout -- .|restore|rm|rebase`, `git branch -d|-D|-m`,
  `git stash drop`, `git reflog expire`.
- Creating or editing an issue, ruleset, secret, or workflow.

**Never `git add -A` or `git add .`.** This tree carries untracked paths that are not yours to
commit (`.claude/`, `devtree/`, neither gitignored). Stage by explicit path.
*Detector:* `git diff --name-only --cached` before every commit, checked against `IN-SCOPE PATHS`.

**Single-lane hotspots** — serialize every change that touches them. Touch one only if §2 names it:

| Path | Why |
|---|---|
| `src/lib.rs` | every module is declared here (`grep -c '^pub mod ' src/lib.rs`) |
| `src/fidelity/registry.rs` | `AUDITED_GATES` is one flat array every new gate appends to |
| `tests/evaluator_preserves_gate_verdicts_test.rs` | every gate change touches it |
| `.github/workflows/**` | CI definition |

**Layout is a description, not a grant.** Single crate; no `crates/**`, no workspace until Phase B
(`H1-8c`). Modules live at `src/<module>/**`, integration tests at `tests/<file>.rs`, and
`Cargo.toml` has no `[[test]]` entries so test files auto-discover. But real milestones write
elsewhere — `H1-13`'s exit criterion is literally `.anvil/shape.json`. **If `IN-SCOPE PATHS` does
not name the file your criterion names, that is §8 trigger 4: stop and ask. Do not widen your own
scope.**

## 2. THE RUN BLOCK — a human fills both fields

```
ACTIVE MILESTONE:
IN-SCOPE PATHS:
```

**Refuse the run** if either field is empty, still carries placeholder text, or names a milestone
you cannot find. Say which field and stop. You may not propose your own `IN-SCOPE PATHS` and then
work to it: a boundary you authored is not a boundary, and §7's scope check would be auditing you
against your own list.

`ACTIVE MILESTONE` is an id **exactly as a `ws-*.md` table writes it**. The plan uses three forms —
`H1-3`, `H1-8a`, `WS14-H1b` — and 7 rows whose id cell is bare `H1`, which are named by file plus
milestone text (`ws-05 "Enlist doors driven behaviourally (#52)"`). Of 43 H1 rows only 8 have a
bare `H1-<n>` id, so a bare number usually does not identify one:

```
grep -chE '^\| *H1-[0-9]+ *\|' docs/plan/ws-*.md | paste -sd+ - | bc     # 8
grep -chE '^\| *(WS[0-9]+-)?H1[-0-9a-z]* *\|' docs/plan/ws-*.md | paste -sd+ - | bc   # 43
```

## 3. Preconditions — verify, then stop if any fails

```
git fetch origin --prune
git cat-file -e origin/dev:docs/plan/anvil-roadmap.md
git status --porcelain --untracked-files=no                  # must be empty
git status --porcelain --untracked-files=all -- <each IN-SCOPE PATH>   # must be empty
git ls-files --error-unmatch <each IN-SCOPE PATH>            # each must resolve, or the path is a typo
```

The tracked-only form is deliberate: a bare `git status --porcelain` is never empty here
(`.claude/`, `devtree/`), so gating on it would halt every run. That is not laxity — the second and
third commands close the gap for the paths you may actually write.

Branch from `origin/dev`, not `main` (diverged legacy). State the base SHA; it is the pin for every
measurement you report.

## 4. Normative sources

Read at the point of use. Do not summarise into working memory and then work from the summary.

1. **The `ws-*.md` file whose table row carries your milestone id** — normative for exit criterion,
   owner, evidence, ratchet, non-goals. Ownership is the **id cell**, not a prose mention.
   One id can be owned twice (`H1-7d` is in `ws-09` and `ws-12`): satisfy **both**, or stop and say
   which you cannot.
2. `docs/plan/anvil-roadmap.md` — §2's tables are an *index*; where it disagrees with a `ws-*` row,
   the `ws-*` row wins and the roadmap is the defect. §1 measured state, §6 ledger, §7 decision log.
3. `docs/restructure-plan.md` — Decisions D1–D5 and the only stated sequence. **Read its
   2026-08-31 status note first: the census is superseded and D5 must not be executed as written.**
4. `CLAUDE.md`, `docs/doctrine.md`, `rules.md`, `ARCHITECTURE.md`.

**Never restate, paraphrase, renumber or tidy an exit criterion, owner, ratchet or non-goal.**
Quote verbatim or cite by id.

**Ordering.** The plan declares no per-milestone dependency field (`grep -ic 'depends-on'
docs/plan/ws-*.md` → no output, exit 1). The only stated sequence is `docs/restructure-plan.md`
§Sequence, covering Phase A ↔ `H1-8a` and Phase B ↔ `H1-8c`; `H1-8b` is the serialization fix that
Sequence names as Phase C's blocker, and Phase C has no `H1-8` twin. If your milestone is in that
sequence, confirm its predecessors landed. Otherwise state "no ordering is declared" and proceed.
**Do not invent a dependency graph.**

## 5. Invariants — each ends in its detector

1. **Measure, never quote.** Every number in your report is immediately preceded by the command
   that produced it and its raw stdout.
2. **Prove a check before trusting it.** Every new or modified check ships with the seeding patch,
   an assertion the seed diff was non-empty, the red against unfixed code, and the green after.
3. **Fix the class, not the instance.** Census the siblings (show the search) and state what makes
   the next instance unwritable — a type, a ratchet, or a meta-test.
4. **Absent evidence is never a pass** (invariant I1, `src/doc_guard/mod.rs:34`) — including for
   your own instruments.
5. **Verify the instrument.** Any command whose output you report as a pass must be shown
   expressing the corresponding failure once. Anchoring is the common bug: `git branch -r` indents
   its output, so `grep -c '^origin/'` silently returns 0 while `grep -c 'origin/'` does not.
6. **Channel and MSRV are separate promises**; equality is a finding (`tests/toolchain_msrv_test.rs`).
7. **Green is not merge authority.**

## 6. Before the first edit

1. Paste the exit criterion **verbatim** (both, if two files own it).
2. One line per criterion: how you satisfy it, and which command discharges it.
3. **Audit it against the tree.** List every Conflict (contradicts the plan, an invariant, or the
   code), Omission (information the criterion needs and lacks), Ambiguity (two defensible
   readings). Resolve by citing a normative source, or stop per §8.
4. **Check it is dischargeable under §1 at all.** Several H1 criteria are stated in merged pull
   requests or upstream tickets — `ws-01` H1-8a says "per PR … four PRs", H1-8b needs two PRs to
   "merge without conflict on the queue", `ws-02` H1-13b needs findings to "become upstream
   tickets". You cannot push, open or merge. Say so now and stop, rather than writing the code and
   discovering it at §7.
5. Record the baseline:

   ```
   cargo nextest run --all-targets --locked --profile ci -E 'not binary(subscription_driver_live_test)'
   ```

   The filter is load-bearing and **must be proven once per run**: list with and without it and show
   the difference is exactly the live tests. `tests/subscription_driver_live_test.rs` gates them
   only on `agy` being installed, so an unfiltered run makes **billable model API calls** and its
   result depends on the network. Exclude by *binary*, not by a name pattern — nextest matches
   `test()` against `module::name`, so `^`-anchored name filters miss a test inside a `mod`.

Do not "write tests first" as ritual. Write the specific failing check the criterion names, seeded
against the defect it claims to catch.

## 7. DONE — and who may say it

Evidence rows, one per claim: `claim_id · criterion_id · argv · base_sha · exit_code · last 20
lines of stdout`. Per new check, additionally: the seeding patch, the assertion it applied, the red,
the green. **A summary of what a command showed is not evidence. Paste the output.**

`DONE` is the conjunction of, all at the pin:

- every exit criterion its `ws-*` file states, discharged by the command named in §6.2;
- `cargo fmt --all -- --check` → 0
- `cargo clippy --all-targets --locked -- -D warnings` → 0
- `cargo nextest run --all-targets --locked --profile ci -E 'not binary(subscription_driver_live_test)'`
  → 0 failed, count stated against the §6.5 baseline, any change explained
- `git diff --name-only origin/dev...HEAD` → every path within `IN-SCOPE PATHS`
- `git status --porcelain --untracked-files=all` → nothing outside `.claude/`, `devtree/`, `target/`
  (an uncommitted new file the suite compiled is the failure this catches)
- the seeded-defect proof for every new check

The first two commands are CI's verbatim (`.github/workflows/build-and-test.yml:28,58`). The third
is CI's line **plus** the live-test filter — the one declared deviation, for the reason in §6.5.

**Ordering.** Run the conjunction on the committed bytes: `git rev-parse HEAD` before and after must
match, and the tracked tree must be clean at both points. Applying and reverting a seeding patch
does not break this, provided HEAD is unmoved and the tree is clean when you sample.

**You do not decide a milestone is done.** Launch a fresh-context reviewer whose input is the
milestone's `ws-*` section, your diff, and your evidence rows, told to find reasons the conjunction
does **not** hold. Record, as evidence rows: **the launch invocation and the reviewer's raw returned
text.** A pasted verdict with no invocation behind it is indistinguishable from prose you wrote.

Fix what it finds and launch a **new** reviewer. **Two consecutive clean rounds from two different
reviewers ends it; at most three rounds** — so rounds 1+2 or rounds 2+3. Anything else is `PARTIAL`.
Fix attempts inside a round draw on §8's budget; fixing is not budget-free.

There is no partial DONE. There is `PARTIAL` with the unmet criterion quoted word for word. A strong
showing on five criteria does not offset a failed sixth.

## 8. Stop triggers, and the budget

Halt and report — do not proceed on judgement — when:

1. an exit criterion is ambiguous, or you would have to interpret it;
2. satisfying one criterion appears to require violating another, or a stated non-goal;
3. an assumptions-ledger entry is contradicted by the code;
4. the change would touch a path outside `IN-SCOPE PATHS`, or an unnamed hotspot;
5. a check fails and the only way forward you see is to weaken the check, the ratchet, or the test;
6. the milestone's premise is false at the pin — the defect is already fixed, the file does not
   exist, or the criterion cannot be discharged under §1;
7. the run block is unfilled;
8. this document orders a command §1 does not grant, or publishes a number that no longer
   reproduces.

Trigger 6 is not hypothetical: a prior review found one milestone built on a count that measured
string literals rather than reads, and another whose cited mechanism had already been retired.
**A criterion wrong at the pin is a PLAN-DEFECT.** Report it with evidence and stop. Do **not** edit
the plan — `docs/plan/**` is out of scope unless §2 names it, and amending a normative document is
a decision, not an implementation step.

**Budget: two attempts at any one failing approach**, including attempts made while fixing what a
reviewer found. Then stop and report with the failure evidence.

## 9. Report

1. Base SHA, branch, milestone id.
2. `git diff --stat`.
3. The DONE conjunction copied verbatim, each line pass/fail with its command and output.
4. Seeded-defect proofs (patch, applied-assertion, red, green).
5. Sibling census with its search and output, and what makes the next instance unwritable.
6. Each reviewer round: launch invocation, raw verdict, your disposition of every finding.
7. Conflicts / omissions / ambiguities from §6.3 and how each resolved.
8. Anything deliberately not done, one line each. Opportunistic refactors go here, not in the diff.
9. What is left for the human: the review, and any PLAN-DEFECT or decision needing ratification.

Commit to a feature branch off `dev`, staging only your authorized paths by name. **Do not push. Do
not open a pull request. Stop after one milestone.**
