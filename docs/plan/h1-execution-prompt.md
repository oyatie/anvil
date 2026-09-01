# Commission: execute one H1 milestone

> **Revision 3 (2026-09-01).** Revision 2 fixed 14 of revision 1's 19 findings and introduced 11
> more. Four were blocking. Its `git status --porcelain` precondition was unsatisfiable on the
> operator's tree, so no run could start and none could finish. Its closed verb allowlist —
> "everything else is refused, including by omission" — omitted `git cat-file`, `grep` and
> `git branch`, all three of which the same document orders, so the run was refused by its own §1.
> It asserted a dual-ownership case (`H1-13` spanning `ws-01`/`ws-02`) that the tables do not
> contain. And it mapped Phases A/B/C onto `H1-8a/b/c`, which the restructure plan's own Sequence
> contradicts. Revision 1's review is in the decision log; revision 2's is on
> [PR #201](https://github.com/oyatie/anvil/pull/201).
>
> **Verify before relying on it.** Every count and command below was re-executed against
> `origin/dev` @ `65f71fd02aa8b63cac89aeaaaa71772f05196acd` on 2026-09-01, and each number is
> printed beside the command that produced it. These are measurements at one SHA, not facts. A
> number here that no longer reproduces is a defect in this file — re-running the command is how
> you find it, and invariant 1 forbids you to quote one of them without re-running it.

**Fill in two things before each run** — the milestone, and the paths it may write. Both are in §4.
Both are supplied by a human, in a human turn. The agent refuses a run whose fields are blank, or
still carry the `<…>` placeholder text, or say `PROPOSE` (§4, §8 trigger 7).

---

## 0. Preconditions (the agent verifies these first, and stops if any fails)

```
git fetch origin --prune
git cat-file -e origin/dev:docs/plan/anvil-roadmap.md              # the plan is on dev
git status --porcelain --untracked-files=no                        # must be empty
git status --porcelain --untracked-files=all -- <each IN-SCOPE PATH>   # must be empty
```

The predicate is about **tracked modifications** and **paths you may write** — not the whole tree.

Bare `git status --porcelain` is not the check and must not be used as one. The operator's tree
permanently carries untracked paths that are nobody's to commit — `.claude/` (local harness state,
including agent worktrees) and `devtree/` — and `.gitignore` covers neither:

```
$ git status --porcelain
?? .claude/
?? devtree/
?? docs/restructure-plan.md
$ git check-ignore -v .claude devtree; echo "exit=$?"
exit=1
```

The third line is branch-dependent — `docs/restructure-plan.md` is tracked on `dev`
(`git ls-tree -r origin/dev --name-only | grep -c restructure` → `1`) and merely absent from the
branch this tree happened to be on. The first two are not: `.claude/` and `devtree/` are tracked on
no ref (`git ls-tree -r origin/dev --name-only | grep -c '^devtree/'` → `0`) and ignored by no rule.

So the whole-tree form is permanently non-empty, and a run gated on it halts at precondition 3
before doing anything. Revision 2 gated on it twice — here and in §7 — and therefore could neither
start nor finish. The two forms above pass on that same tree:

```
$ git status --porcelain --untracked-files=no | wc -l
       0
$ git status --porcelain --untracked-files=all -- src/ | wc -l
       0
```

**Verify the instrument (invariant 5)** before you trust either pass. A scoped status that cannot
express a failure has checked nothing. On the same tree, pointed at a directory that does carry an
untracked file:

```
$ git status --porcelain --untracked-files=all -- .claude/ | head -1
?? .claude/worktrees/agent-a01d053cf1ae04c1a/
```

A tracked modification you did not make, or a dirty in-scope path, is a stop: baseline
measurements are meaningless stacked on someone else's edit, and staging becomes ambiguous.
Untracked files *outside* your scope are the operator's business — §1 forbids you to stage them,
and this precondition no longer pretends they are yours to clean.

## 1. Authorization

You have my approval, **for this run only**, to:

- read anything in the repository;
- create, modify and delete files **only** under the paths listed in `IN-SCOPE PATHS` in §4;
- run these `cargo` verbs: `cargo build`, `cargo check`, `cargo test`, `cargo nextest`,
  `cargo clippy`, `cargo metadata`, `cargo tree`, and `cargo fmt` **in its check form only**
  (`cargo fmt --all -- --check`);
- format authorized files one path at a time with `rustfmt --edition 2024 <path>`;
- run these git **read** verbs: `status`, `diff`, `diff-index`, `log`, `show`, `cat-file`,
  `ls-tree`, `ls-files`, `rev-parse`, `merge-base`, `check-ignore`, `grep`, `stash list`, `fetch`,
  and `branch` **in its list forms only** (`git branch`, `git branch -r`, `git branch --format=…`);
- run these git **write** verbs: `switch -c`, `add <path>`, `commit`;
- run the read-only text tools the measurements below require: `grep`, `rg`, `wc`, `sort`, `uniq`,
  `cut`, `tr`, `paste`, `bc`, `diff`, `head`, `tail`, `cat`, `ls`, `find`, `echo`, `printf`, and
  `sed` **in its printing form only** (`sed -n '<n>p'`) — **reading only**: no in-place flag
  (`sed -i`, `rg --replace`, `perl -i`), no `find -delete`, no `find -exec`, no output redirection
  onto a tracked path;
- run `gh` **read** commands (`gh pr view/list/checks`, `gh issue view/list`, `gh run view/list`,
  `gh api` with GET);
- launch the fresh-context reviewer §7 requires, as a subagent with **read and report only** — it
  does not inherit your write grant.

Everything else is refused, including by omission. **This list is a superset of every command this
document orders you to run**, and that is a property you can check rather than trust — revision 2
asserted a closed list and then ordered `git cat-file` in §0, `grep` in §3.3 and `git branch -r` in
§3.5, none of them granted, so its own §1 refused its own §0. The census:

```
$ grep -ohE '\b(cargo [a-z]+|git [a-z-]+|rustfmt|gh [a-z]+|grep|rg|wc|sort|uniq|cut|tr|paste|bc|diff|head|tail|cat|ls|find|echo|printf|sed)\b' \
      docs/plan/h1-execution-prompt.md | sort -u
```

Every verb it prints is either granted above or named in the refusal list below. Run it. If a verb
appears in neither, that is a defect in this file — report it as a PLAN-DEFECT under §8 (trigger 8).
It is not a licence to run the command.

In particular you do **not** have approval to:

- `cargo run` — the binary's default subcommand is `Serve`
  (`src/cli/handlers.rs:14`, `cli.command.unwrap_or(Commands::Serve)`), so `cargo run` with no
  argument **boots the production webhook daemon against live pull requests**;
- **`cargo fmt` in any writing form** — `cargo fmt --help` states it "formats all bin and lib files
  of the current crate", so both `cargo fmt` and `cargo fmt --all` rewrite files outside
  `IN-SCOPE PATHS`, and §7's diff check then fails on edits you never intended. Format explicit
  paths instead. `--edition 2024` is load-bearing, not decoration: the crate is `edition = "2024"`
  (`Cargo.toml:4`) and rustfmt defaults to 2015, so the flagless form reports false failures —

  ```
  $ rustfmt --check src/cli/handlers.rs > /dev/null; echo "exit=$?"
  exit=1
  $ rustfmt --edition 2024 --check src/cli/handlers.rs > /dev/null; echo "exit=$?"
  exit=0
  ```

  Detector: after formatting, `git status --porcelain --untracked-files=no` lists only in-scope
  paths. Any other path in that output means you used the crate-wide form;
- `cargo publish`, `cargo install`, or any command that reaches the network to write;
- `git push`, `git clean`, `git reset --hard`, `git checkout -- .`, `git restore`, `git rm`,
  `git branch -d/-D/-m`, `git rebase`, `git stash drop`, `git reflog expire`, or any history
  rewrite;
- open, approve, merge or auto-merge a pull request; create or edit an issue, ruleset or secret;
- run the daemon, `serve`, or any model-spawning subcommand.

If a task appears to require one of these, that is a **stop** under §8, not a judgement call.

**Do not run `git add -A` or `git add .`.** The working tree carries untracked paths that are not
yours to commit — `.claude/` and `devtree/` among them, neither gitignored (§0). Stage only the
files you were authorized to write, by explicit path. Detector: `git diff --name-only --cached`
before every commit, checked against `IN-SCOPE PATHS`.

**Single-lane hotspots.** These serialize every change that touches them. Touch one only if §4
names it explicitly:

| Path | Why | Command (re-run it; the number is a measurement at the pin) |
|---|---|---|
| `src/lib.rs` | every module is declared here | `grep -c '^pub mod ' src/lib.rs` → `115` |
| `src/fidelity/registry.rs` | `AUDITED_GATES` is one flat const array every new gate appends to | `grep -n 'AUDITED_GATES' src/fidelity/registry.rs` → `29:pub const AUDITED_GATES: &[GateFidelity] = &[` |
| `tests/evaluator_preserves_gate_verdicts_test.rs` | every gate change touches it | `git log --oneline -20 -- tests/evaluator_preserves_gate_verdicts_test.rs` |
| `.github/workflows/**` | CI definition | — |

**Repository layout — a description, not a grant.** This is a single crate: there is no `crates/**`
and no workspace until Phase B (`H1-8c`), so a module normally lives at `src/<module>/**` and an
integration test at `tests/<file>.rs`. `Cargo.toml` carries no `[[test]]` declarations
(`grep -c '\[\[test\]\]' Cargo.toml` → `0`, exit 1), so integration tests auto-discover and adding
a test file does not touch the manifest.

None of that authorizes a path. What you may write is `IN-SCOPE PATHS` and nothing else, and real
milestones do write outside `src/` and `tests/`: `H1-13`'s exit criterion is literally
`.anvil/shape.json`, which is tracked —

```
$ git ls-tree -r origin/dev --name-only .anvil/
.anvil/baselines/semantic-abi.signoff.json
.anvil/baselines/shape.baseline.json
.anvil/shape.json
```

If `IN-SCOPE PATHS` does not name the file your exit criterion names, that is §8 trigger 4: stop
and ask for the path, do not widen your own scope to reach it.

## 2. Normative sources, precedence, and the pin

Read these at the point of use. Do not summarise them into working memory and then work from the
summary.

1. **The `ws-*.md` file that owns your milestone** — normative for exit criterion, owner, evidence,
   ratchet, non-goals.
2. **`docs/plan/anvil-roadmap.md`** — §2's horizon tables are an *index*; where a roadmap row and
   its `ws-*` twin disagree, the `ws-*` file wins (§2 says so). §1 is measured current state, §6
   the assumptions ledger, §7 the decision log.
3. **`docs/restructure-plan.md`** — Decisions D1–D5, Phases A/B/C, and the only ordering the plan
   states as a sequence. **Read its 2026-08-31 status note first**: the census is superseded and
   **D5 must not be executed as written**.
4. **`CLAUDE.md`**, `docs/doctrine.md`, `rules.md`, `ARCHITECTURE.md`.

**One milestone id in two `ws-*` files is one milestone with two owners.** Satisfy **both** exit
criteria, or stop and say which you cannot. The detector is the id cell, not prose — a milestone is
owned by a file only where that file has a table row whose first cell is the id:

```
$ grep -rnE '^\| *H1-7d *\|' docs/plan/ws-*.md | cut -d: -f1,2
docs/plan/ws-09-evidence-provenance.md:25
docs/plan/ws-12-instrument-verification.md:39
```

`H1-7d` is the case at the pin. Run the same grep for your own id before assuming it is not. Prose
mentions do not count and must not be read as ownership: revision 2 claimed `H1-13` spanned
`ws-01`/`ws-02`, but its id cells are in `ws-02` alone —

```
$ grep -rnE '^\| *H1-13b? *\|' docs/plan/ws-*.md | cut -d: -f1,2
docs/plan/ws-02-outward-uniformity.md:35
docs/plan/ws-02-outward-uniformity.md:36
```

— and `ws-01`'s two `H1-13` hits are sentences referring to it, not rows owning it.

Branch from `origin/dev` (not `main`, which is diverged legacy). State the exact base SHA in your
first message; that is the pin for every measurement you report.

**Never restate, paraphrase, renumber or "tidy" an exit criterion, owner, ratchet or non-goal.**
Quote verbatim or cite by id.

## 3. Invariants — each ends in the detector that catches its violation

1. **Measure, never quote counts.** Every number in your report is immediately preceded by the
   command that produced it and its raw stdout. This binds you to the numbers *in this file* too:
   re-run them, do not copy them.
2. **Prove a check before trusting it.** Every new or modified check ships with the seeding patch,
   an assertion the seed diff was non-empty, the red against unfixed code, and the green after.
3. **Fix the class, not the instance.** Census the siblings (show the search), and state how the
   next instance is made unwritable — a type, a ratchet, or a meta-test.
4. **Absent evidence is never a pass** (invariant I1, `src/doc_guard/mod.rs:34`) — including for
   your own instruments.
5. **Verify the instrument.** Any command whose output you report as a pass must be shown
   expressing the corresponding failure once. Anchoring bugs are the common form: `git branch -r`
   indents its output, so `grep -c '^origin/'` silently returns 0 where `grep -c 'origin/'`
   returned 170 at the pin.
6. **Channel and MSRV are separate promises**; equality is a finding, not a tidy-up
   (`tests/toolchain_msrv_test.rs`).
7. **Green is not merge authority.** You do not push, approve, or merge.

## 4. THE RUN BLOCK — a human fills both fields; the agent refuses anything else

```
ACTIVE MILESTONE: <id exactly as the ws-* table writes it>
    Accepted forms, because the plan uses all of them: H1-3 · H1-8a · WS14-H1b ·
    or, for a row whose id cell is bare "H1", the ws file plus the row's milestone
    text, e.g.  ws-05 "Enlist doors driven behaviourally (#52)".

IN-SCOPE PATHS: <explicit list, or the single token PROPOSE>
```

A bare `H1-<n>` usually does **not** identify a milestone. At the pin the `ws-*` files carry 43
H1-shaped rows; only 8 of them have a bare `H1-<n>` id, and 7 have a bare `H1`:

```
$ grep -rhcE '^\| *(H1[A-Za-z0-9-]*|WS[0-9]+-H1[a-z]?) *\|' docs/plan/ws-*.md | paste -sd+ - | bc
43
$ grep -rhoE '^\| *H1-[0-9]+ *\|' docs/plan/ws-*.md | tr -d '| ' | sort -u | wc -l
       8
$ grep -rcE '^\| *H1 *\|' docs/plan/ws-*.md | grep -v ':0$' | sort
docs/plan/ws-03-build-graph.md:1
docs/plan/ws-05-review-test-fabric.md:1
docs/plan/ws-08-gate-honesty.md:1
docs/plan/ws-10-untrusted-input.md:1
docs/plan/ws-14-fleet-conformance.md:3
```

**The refusal predicate.** A field is *unfilled* if it is empty, **or still contains `<` or `>`**
(the unedited placeholder text of this very block), **or** is the token `PROPOSE`. An unedited
paste is the common case and it is neither blank nor `PROPOSE`, so blankness alone was never the
right test. Detector, run on the pasted value of each field:

```
grep -qE '^[[:space:]]*$|[<>]|(^|[[:space:]])PROPOSE([[:space:]]|$)'   # a match is a refusal
```

Refuse under §8 trigger 7 and say which field and which clause matched.

**Why the path list is the human's field.** The plan's `ws-*` tables are
`| ID | Milestone | Exit criterion | Owner |` and carry no path list —
`git grep -ic 'in-scope' -- 'docs/plan/ws-*.md'` prints nothing and exits 1. Revision 1 told the
agent to derive the list "from the plan" and then checked the diff against it, so the agent
authored the constraint it was audited by and the check could not fail. A boundary supplied by the
party it constrains is not a boundary.

**If the field says `PROPOSE`:** read the milestone, propose a path list with one line of
justification each, and **stop**. Write nothing. A proposal is not an authorization, and this
remains true if the proposal looks obviously right, if nobody objects to it, or if a later message
seems to approve it in general terms.

The run resumes only when **a human pastes a concrete list back into a new run block, in their own
turn**. Detector, and the reason this is not the old circularity in a longer form: your first
message of the resumed run must quote the run block verbatim *and* identify the human turn it came
from. If the list in force is one you emitted and cannot attribute to a human turn, that is §8
trigger 7 — stop. You may never proceed on your own proposal.

Then, before editing, fill in from the plan:

- **Id and title**, quoted from the `ws-*` file.
- **Exit criterion**, pasted **verbatim** (both, if two files own it).
- **Owner** and **non-goals**, quoted.
- **Ordering.** The plan declares no per-milestone dependency field:

  ```
  $ git grep -ic 'depends-on' -- 'docs/plan/ws-*.md'; echo "exit=$?"
  exit=1
  ```

  Scope that grep to the `ws-*` files. The unscoped form is now self-matching and measures nothing:

  ```
  $ git grep -ic 'depends-on' -- 'docs/plan/*'
  docs/plan/anvil-roadmap.md:2
  docs/plan/h1-execution-prompt.md:2
  ```

  — two hits in the decision log and two in this section, every one of them a *statement about*
  the string rather than a use of it, and the total rises each time the claim is restated. A search
  that matches its own statement is RC-6, not evidence.

  The only stated sequence is `docs/restructure-plan.md` §Sequence (lines 137–154), and it names
  phases, not milestone ids. Against the `ws-01` rows
  (`grep -rnE '^\| *H1-8[abc] *\|' docs/plan/ws-*.md`) it maps as:

  | §Sequence node | `ws-01` milestone | note |
  |---|---|---|
  | `Phase A (D1, D2, D5)` | `H1-8a` — Phase A kernel extraction | D5 must not be executed as written; see the restructure plan's 2026-08-31 status note |
  | `Phase B: workspace split` | `H1-8c` — Phase B workspace split | **not** `H1-8b` |
  | `Phase C: capability migration` | *no `H1-8` twin* | the node is annotated `(blocked on serialization fix)`; that fix is `H1-8b`, a **blocker of** Phase C, not Phase C itself |

  Revision 2 stated "Phases A/B/C (H1-8a/b/c)". Two thirds of that is wrong, and acting on it would
  have had `H1-8c` waiting on a phase it *is* and `H1-8b` executed as a workspace split.

  So: if your milestone is `H1-8a` or `H1-8c`, state its position and confirm its predecessors have
  landed. If it is `H1-8b`, state that it gates Phase C and has no phase of its own. **Otherwise
  state "no ordering is declared for this milestone" and proceed.** Do not invent a dependency
  graph.
- **Target**, as exact `path::symbol` — and resolve it to the *definition*, not a re-export. For
  example `enum GateStatus` is defined at `src/pre_merge_guard/status.rs:14`;
  `src/pre_merge_guard/report.rs:4` is `pub use super::status::GateStatus`, so naming `report.rs`
  would point you at an aggregation file — `wc -l < src/pre_merge_guard/report.rs` → `1240`, and
  `grep -rlE 'pre_merge_guard::report' src/ tests/ | wc -l` → `97` importing files — instead of at
  the enum.
- **Fixed** (change only via a decision entry): public trait signatures, error taxonomy, file
  placement, metric definitions, evidence schema.

Do **not** design the implementation here. Naming the target and the fixed surface is the job.

## 5. Before the first edit

1. Paste the milestone's exit criterion **verbatim**.
2. One line per criterion: how you will satisfy it, and which command discharges it.
3. **Audit the criterion against the tree.** List every **Conflict** (contradicts the plan, an
   invariant, or the code), **Omission** (information the criterion needs that is absent),
   **Ambiguity** (two defensible readings). Resolve by citing a normative source, or stop per §8.
4. **Check the criterion is dischargeable under §1 at all.** Several H1 criteria are stated in
   terms of merged pull requests or upstream tickets — `ws-01` H1-8a says "per PR … four PRs",
   H1-8b requires two PRs to "merge without conflict on the queue", `ws-02` H1-13b requires
   findings to "become upstream tickets". You cannot push, open, or merge a PR. If the criterion
   requires it, say so now and stop, rather than doing the code and discovering it at §7.
5. Record the baseline:

   ```
   cargo nextest run --all-targets --locked --profile ci -E 'not binary(subscription_driver_live_test)'
   ```

   and paste the summary line.

   **The `-E` filter is load-bearing.** `tests/subscription_driver_live_test.rs` gates its
   `test_live_*` tests only on the `agy` CLI being installed, so an unfiltered run on a machine
   that has `agy` makes **live, billable model API calls** and its pass/fail depends on the
   network. CI is unaffected because its runners have no `agy` and the tests self-skip.

   **The filter is keyed to the binary, not to the test name.** nextest matches `test()` against
   `module::name`, so the older `-E 'not test(/^test_live_/)'` was anchor-fragile: a live test
   moved inside a `mod` would present as `some_mod::test_live_x`, fail the `^` anchor, rejoin the
   run, and spend money. `binary(subscription_driver_live_test)` names the compilation unit and
   cannot be escaped by nesting.

   **Prove the exclusion once, per invariant 5** — a filter you never watched remove anything is an
   unverified instrument:

   ```
   cargo nextest list --all-targets --locked --profile ci > /tmp/all.txt
   cargo nextest list --all-targets --locked --profile ci \
       -E 'not binary(subscription_driver_live_test)' > /tmp/filtered.txt
   diff /tmp/all.txt /tmp/filtered.txt      # must show exactly the test_live_* tests, and nothing else
   ```

   Paste that diff. If it removes anything that is not a `test_live_*` test, the filter is wrong and
   that is a stop, not a smaller suite.

Do not "write tests first" as a ritual. Write the specific failing check the criterion names,
seeded against the defect it claims to catch.

## 6. Evidence

One row per claim:

```
claim_id · exit_criterion_id · cmd (argv you can rerun) · base_sha · exit_code · last 20 lines
```

Per new or modified check, additionally: the seeding patch, the assertion it applied, the red
output, the green output. **A summary of what a command showed is not evidence. Paste the output.**

## 7. DONE — and who may say it

**Order matters, and it is checked.** The DONE commands run against **the committed bytes**, not a
dirty tree: stage your authorized paths by name, commit, and only then run the list below. If any
of them causes an edit, the sequence restarts from the commit. Detector: record
`git rev-parse HEAD` immediately before the first DONE command and immediately after the last;
they must be equal, and `git status --porcelain --untracked-files=no` must be empty at both points.
Without this, suite → edit → commit passes a check that was never run on what you shipped.

`DONE(<milestone>)` is the conjunction of, all at the pin:

- every exit criterion its `ws-*` file states (both files, if two own it), discharged by the
  command named in §5.2;
- `cargo fmt --all -- --check` → 0
- `cargo clippy --all-targets --locked -- -D warnings` → 0
- `cargo nextest run --all-targets --locked --profile ci -E 'not binary(subscription_driver_live_test)'`
  → 0 failed, count stated against the §5.5 baseline, any change explained
- `git status --porcelain --untracked-files=no` → empty
- `git status --porcelain --untracked-files=all -- <each IN-SCOPE PATH>` → empty
- `git diff --name-only origin/dev...HEAD` → every path within `IN-SCOPE PATHS`
- the seeded-defect proof for every new check

**On the clean-tree lines:** they are `--untracked-files=no` and path-scoped for the reason §0
gives — the operator's tree permanently carries untracked `.claude/` and `devtree/`, which
`.gitignore` does not cover, so a whole-tree emptiness test can never pass and a DONE gated on it
can never be reached.

**On CI fidelity.** The first two are byte-identical to the repository's own CI —
`.github/workflows/build-and-test.yml:28` (`cargo fmt --all -- --check`) and `:58`
(`cargo clippy --all-targets --locked -- -D warnings`). Diff them yourself:
`git show origin/dev:.github/workflows/build-and-test.yml | sed -n '28p;58p'`.

The nextest line **deviates from CI by exactly one flag.** CI's line 60 is
`cargo nextest run --all-targets --locked --profile ci`, with no `-E`. The deviation is deliberate
and stated rather than hidden: CI's runners have no `agy`, so the live tests self-skip there, while
your machine may have it, and an unfiltered run would make billable API calls (§5.5). It is the
only permitted deviation, it may only ever *remove* the `test_live_*` tests, and §5.5's `diff`
proves that it removes nothing else. Run no other weaker variant; `--locked` is required by
ADR-0005.

**You do not decide that a milestone is done.** Launch a fresh-context reviewer whose input is the
milestone's `ws-*` section, your diff, and your evidence rows, instructed to find reasons the
conjunction does **not** hold. Then, as an evidence row like any other: **the launch invocation,
and the reviewer's raw returned text.** A pasted verdict with no invocation behind it is
indistinguishable from prose you wrote, which would make this entire section unenforceable.

**The loop ends on two consecutive clean rounds from two different fresh reviewers.** One clean
round is not enough — a single adversary that finds nothing is as likely to be a weak adversary as
a clean diff. Fix what a round finds and launch a **new** reviewer (not the same one — it has
already accepted your reasoning).

**Budgets, and how they interact.** Two separate counters, both hard:

- **at most three reviewer rounds** in total, and
- **at most two attempts at any one failing approach** (§8), *including* attempts made while fixing
  what a reviewer found. Fixes are not a budget-free zone; a third attempt at the same approach is
  a stop even mid-round.

So the reachable DONEs are rounds 1+2 clean, or rounds 2+3 clean. Round 1 clean, round 2 with
findings, round 3 clean is **not** two consecutive and is `PARTIAL`. If the cap is reached without
two consecutive clean rounds, stop and report `PARTIAL` with every outstanding finding; do not keep
editing a real repository against an adversary primed to always return something.

There is no partial DONE. There is `PARTIAL` with the unmet criterion quoted word for word. A
strong showing on five criteria does not offset a failed sixth.

## 8. Stop triggers, and the retry budget

Halt and report — do not proceed on judgement — when:

1. an exit criterion is ambiguous, or you would have to interpret it to proceed;
2. satisfying one criterion appears to require violating another, or a stated non-goal;
3. an assumptions-ledger entry is contradicted by the code;
4. the change would touch a path outside `IN-SCOPE PATHS`, or an unnamed hotspot;
5. a check fails and the only path forward you can see is to weaken the check, the ratchet, or the
   test;
6. the milestone's premise is false at the pin — the defect is already fixed, the file does not
   exist, or the criterion cannot be discharged under §1;
7. a run-block field is unfilled by §4's predicate — blank, still carrying `<`/`>`, or `PROPOSE` —
   or the `IN-SCOPE PATHS` in force is one you proposed and cannot attribute to a human turn;
8. a command this document orders is not in §1's allowlist.

Trigger 6 is not hypothetical. The plan's own external review found a milestone built on a count
that measured string literals rather than reads, and one whose cited mechanism had already been
retired. **A criterion wrong at the pin is a PLAN-DEFECT.** Report it in your final message with
the evidence and stop. Do **not** edit the plan to fix it — `docs/plan/**` is out of scope unless
§4 names it, and amending a normative document is a decision, not an implementation step.

**Retry budget:** two attempts at a failing approach, then stop and report with the failure
evidence. Do not retry a third time in a contaminated context. This budget is per approach and runs
across §7's reviewer rounds, which are capped separately at three (§7 states how the two interact).

## 9. Report

1. Base SHA, branch name, milestone id.
2. `git diff --stat`.
3. The DONE predicate copied verbatim, each line marked pass/fail with its command and output,
   plus the before/after `git rev-parse HEAD` pair proving §7's ordering.
4. Seeded-defect proofs (patch, applied-assertion, red, green).
5. Sibling census with its search command and output, and the mechanism that makes the next
   instance unwritable.
6. The reviewer launch invocation and raw verdict for each round, with your disposition of every
   finding, and which two rounds were consecutively clean.
7. Conflicts / omissions / ambiguities from §5.3 and how each was resolved.
8. Anything deliberately not done, one line each. Opportunistic refactors go here, not in the diff.
9. What is left for me: the review, and any PLAN-DEFECT or decision needing ratification.

Commit to a feature branch off `dev`, staging only your authorized paths by name. Do not push. Do
not open a pull request. Stop after one milestone.
