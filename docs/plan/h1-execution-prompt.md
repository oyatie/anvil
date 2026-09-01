# Commission: execute one H1 milestone

> **Revision 2 (2026-09-01).** Revision 1 failed its first adversarial review with 19 findings, 16
> verified. Three were fatal: the milestone slot could name only 8 of the plan's 43 H1 rows; the
> in-scope path list — the entire authorization boundary — was invented by the agent it constrains,
> so the scope check could not fail; and `Depends-on`, which §5 ordered pasted verbatim, appears
> nowhere in the plan. The review is recorded in the decision log. Everything below is rewritten
> against those findings and re-pre-flighted against `origin/dev`.

**Fill in two things before each run** — the milestone, and the paths it may write. Both are below,
and both are refused by the agent if left blank.

---

## 0. Preconditions (the agent verifies these first, and stops if any fails)

```
git fetch origin --prune
git cat-file -e origin/dev:docs/plan/anvil-roadmap.md   # the plan is on dev
git status --porcelain                                   # must be empty before any work
```

A non-empty working tree is a stop: the baseline measurements below are meaningless on a dirty
tree, and staging becomes ambiguous.

## 1. Authorization

You have my approval, **for this run only**, to:

- read anything in the repository;
- create, modify and delete files **only** under the paths listed in `IN-SCOPE PATHS` in §4;
- run these exact verbs: `cargo build`, `cargo check`, `cargo test`, `cargo nextest`, `cargo fmt`,
  `cargo clippy`, `cargo metadata`, `cargo tree`;
- run these git verbs: `status`, `diff`, `log`, `show`, `fetch`, `rev-parse`, `ls-tree`,
  `switch -c`, `add <path>`, `commit`, `stash list`;
- run `gh` **read** commands (`gh pr view/list/checks`, `gh issue view/list`, `gh run view/list`,
  `gh api` with GET).

Everything else is refused, including by omission. In particular you do **not** have approval to:

- `cargo run` — the binary's default subcommand is `Serve`
  (`src/cli/handlers.rs:14`, `cli.command.unwrap_or(Commands::Serve)`), so `cargo run` with no
  argument **boots the production webhook daemon against live pull requests**;
- `cargo publish`, `cargo install`, or any command that reaches the network to write;
- `git push`, `git clean`, `git reset --hard`, `git checkout -- .`, `git rm`, `git branch -D`,
  `git rebase`, `git stash drop`, `git reflog expire`, or any history rewrite;
- open, approve, merge or auto-merge a pull request; create or edit an issue, ruleset or secret;
- run the daemon, `serve`, or any model-spawning subcommand.

If a task appears to require one of these, that is a **stop** under §8, not a judgement call.

**Do not run `git add -A` or `git add .`.** The working tree carries untracked paths that are not
yours to commit — `.claude/` (the operator's local settings) and `devtree/` among them. Stage only
the files you were authorized to write, by explicit path.

**Single-lane hotspots.** These serialize every change that touches them. Touch one only if §4
names it explicitly:

| Path | Why |
|---|---|
| `src/lib.rs` | every module is declared here (115 `pub mod` at `origin/dev`; count it yourself) |
| `src/fidelity/registry.rs` | `AUDITED_GATES` is one flat const array every new gate appends to |
| `tests/evaluator_preserves_gate_verdicts_test.rs` | every gate change touches it |
| `.github/workflows/**` | CI definition |

This is a single-crate repository. Write `src/<module>/**` and `tests/<file>.rs`; there is no
`crates/**` and no workspace until Phase B. `Cargo.toml` carries **zero** `[[test]]` declarations
on `dev` — integration tests auto-discover, so adding a test file does not touch the manifest.

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

**If one milestone id appears in two `ws-*` files** (`H1-7d` is in both `ws-09` and `ws-12`;
`H1-13` spans `ws-01`/`ws-02`), it is one milestone with two owners. Satisfy **both** criteria, or
stop and say which you cannot.

Branch from `origin/dev` (not `main`, which is diverged legacy). State the exact base SHA in your
first message; that is the pin for every measurement you report.

**Never restate, paraphrase, renumber or "tidy" an exit criterion, owner, ratchet or non-goal.**
Quote verbatim or cite by id.

## 3. Invariants — each ends in the detector that catches its violation

1. **Measure, never quote counts.** Every number in your report is immediately preceded by the
   command that produced it and its raw stdout.
2. **Prove a check before trusting it.** Every new or modified check ships with the seeding patch,
   an assertion the seed diff was non-empty, the red against unfixed code, and the green after.
3. **Fix the class, not the instance.** Census the siblings (show the search), and state how the
   next instance is made unwritable — a type, a ratchet, or a meta-test.
4. **Absent evidence is never a pass** (invariant I1, `src/doc_guard/mod.rs:34`) — including for
   your own instruments.
5. **Verify the instrument.** Any command whose output you report as a pass must be shown
   expressing the corresponding failure once. Anchoring bugs are the common form: `git branch -r`
   indents its output, so `grep -c '^origin/'` silently returns 0.
6. **Channel and MSRV are separate promises**; equality is a finding, not a tidy-up
   (`tests/toolchain_msrv_test.rs`).
7. **Green is not merge authority.** You do not push, approve, or merge.

## 4. THE RUN BLOCK — the human fills both fields; the agent refuses a blank

```
ACTIVE MILESTONE: <id exactly as the ws-* table writes it>
    Accepted forms, because the plan uses all of them: H1-3 · H1-8a · WS14-H1b ·
    or, for a row whose id cell is bare "H1", the ws file plus the row's milestone
    text, e.g.  ws-05 "Enlist doors driven behaviourally (#52)".
    Of 43 H1 rows across the ws files, only 8 have a bare H1-<n> id — so a bare
    number is usually NOT enough to identify one.

IN-SCOPE PATHS: <explicit list, or the single token PROPOSE>
```

**Why this is the human's field.** The plan's `ws-*` tables are `| ID | Milestone | Exit criterion
| Owner |` — they carry no path list (`grep -i in-scope` over the ws files returns 0). Revision 1
told the agent to derive the list "from the plan" and then checked the diff against it, so the
agent authored the constraint it was audited by and the check could not fail. It is supplied from
outside, or it is not a boundary.

**If the field says `PROPOSE`:** read the milestone, propose a path list with one line of
justification each, and **stop**. Do not write anything. The run resumes when a human pastes the
list back. A proposal is not an authorization.

Then, before editing, fill in from the plan:

- **Id and title**, quoted from the `ws-*` file.
- **Exit criterion**, pasted **verbatim** (both, if two files own it).
- **Owner** and **non-goals**, quoted.
- **Ordering.** The plan declares no per-milestone dependency field — `grep -i depends-on` over the
  plan returns 0, and the only stated sequence is `docs/restructure-plan.md` §Sequence, which
  covers Phases A/B/C (H1-8a/b/c) only. So: if your milestone appears in that sequence, state its
  position and confirm its predecessors have landed. **Otherwise state "no ordering is declared
  for this milestone" and proceed.** Do not invent a dependency graph.
- **Target**, as exact `path::symbol` — and resolve it to the *definition*, not a re-export. For
  example `enum GateStatus` is defined at `src/pre_merge_guard/status.rs:14`;
  `src/pre_merge_guard/report.rs:4` is `pub use super::status::GateStatus`, so naming `report.rs`
  would point you at a 199-reference aggregation file instead of the enum.
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
5. Record the baseline: `cargo nextest run --all-targets --locked --profile ci -E 'not test(/^test_live_/)'`
   and paste the summary line. The `-E` filter is load-bearing: `tests/subscription_driver_live_test.rs`
   gates `test_live_*` only on `agy` being installed, so an unfiltered run makes **live, billable
   model API calls** and its pass/fail depends on the network.

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

`DONE(<milestone>)` is the conjunction of, all at the pin:

- every exit criterion its `ws-*` file states (both files, if two own it), discharged by the
  command named in §5.2;
- `cargo fmt --all -- --check` → 0
- `cargo clippy --all-targets --locked -- -D warnings` → 0
- `cargo nextest run --all-targets --locked --profile ci -E 'not test(/^test_live_/)'` → 0 failed,
  count stated against the §5.5 baseline, any change explained
- `git status --porcelain` → empty (the suite must be green on *the committed bytes*, not on a
  dirty tree)
- `git diff --name-only origin/dev...HEAD` → every path within `IN-SCOPE PATHS`
- the seeded-defect proof for every new check

The first three commands are the repository's own CI commands, verbatim from
`.github/workflows/build-and-test.yml:28,58,60` — do not run weaker variants; `--locked` is
required by ADR-0005.

**You do not decide that a milestone is done.** Launch a fresh-context reviewer whose input is the
milestone's `ws-*` section, your diff, and your evidence rows, instructed to find reasons the
conjunction does **not** hold. Then, as an evidence row like any other: **the launch invocation,
and the reviewer's raw returned text.** A pasted verdict with no invocation behind it is
indistinguishable from prose you wrote, which would make this entire section unenforceable.

Fix what it finds and launch a **new** reviewer (not the same one — it has already accepted your
reasoning). **At most three rounds.** If round three is not clean, stop and report `PARTIAL` with
every outstanding finding; do not keep editing a real repository against an adversary primed to
always return something.

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
7. the run block is blank, or `IN-SCOPE PATHS` says `PROPOSE`.

Trigger 6 is not hypothetical. The plan's own external review found a milestone built on a count
that measured string literals rather than reads, and one whose cited mechanism had already been
retired. **A criterion wrong at the pin is a PLAN-DEFECT.** Report it in your final message with
the evidence and stop. Do **not** edit the plan to fix it — `docs/plan/**` is out of scope unless
§4 names it, and amending a normative document is a decision, not an implementation step.

**Retry budget:** two attempts at a failing approach, then stop and report with the failure
evidence. Do not retry a third time in a contaminated context. (Reviewer rounds in §7 are counted
separately and capped at three.)

## 9. Report

1. Base SHA, branch name, milestone id.
2. `git diff --stat`.
3. The DONE predicate copied verbatim, each line marked pass/fail with its command and output.
4. Seeded-defect proofs (patch, applied-assertion, red, green).
5. Sibling census with its search command and output, and the mechanism that makes the next
   instance unwritable.
6. The reviewer launch invocation and raw verdict for each round, with your disposition of every
   finding.
7. Conflicts / omissions / ambiguities from §5.3 and how each was resolved.
8. Anything deliberately not done, one line each. Opportunistic refactors go here, not in the diff.
9. What is left for me: the review, and any PLAN-DEFECT or decision needing ratification.

Commit to a feature branch off `dev`, staging only your authorized paths by name. Do not push. Do
not open a pull request. Stop after one milestone.
