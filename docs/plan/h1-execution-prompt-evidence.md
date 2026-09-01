# Evidence for the H1 execution prompt

Companion to [`h1-execution-prompt.md`](h1-execution-prompt.md). Nothing here is needed to run a
milestone; it is the record of what the prompt's claims are grounded in, and of the three revisions
that were wrong. Kept separate because the commissioning prompt is read by an agent under an
instruction budget, and justification competes with control for that budget.

## Why the prompt is short

Revision 3 reached 541 lines and roughly 185 standing obligations. Frontier models reliably follow
~150–200 standing instructions before compliance degrades, so the file had become its own defect:
its stop triggers sat at 93% depth and "do not push" was the last line. Revision 4 is 250 lines
with every control retained and the push prohibition at 5%.

## The three revisions that were wrong

**Revision 1** — added in the last commit before #197 merged, with no review pass of any kind.
Adversarial review returned 19 findings, 16 verified. Fatal: the `ACTIVE MILESTONE: H1-<n>` slot
could name 8 of 43 H1 rows; `IN-SCOPE PATHS` was derived by the agent and then used to audit that
same agent's diff, so the scope check could not fail; and §5 ordered `Depends-on` pasted verbatim,
a field that appears nowhere in the plan.

**Revision 2** — fixed 14, introduced 11. It made `git status --porcelain` empty a precondition on
a tree that permanently carries untracked `.claude/` and `devtree/`, so every run halted before
reading its milestone; its closed verb allowlist omitted `git cat-file`, which its own §0 ordered
as the second command; it asserted `H1-13` spans two files when it spans one; and it mapped Phase B
to `H1-8b` when Phase B is `H1-8c`.

**Revision 3** — fixed all 19, introduced 10. The verb census it added to prevent the allowlist
defect recurring was a tautology: its regex's third branch was a copy of the grant list, so it could
only print verbs already granted. Seeding four ungranted verbs — three of them writing — produced
byte-identical output. `PROPOSE` became simultaneously a hard refusal and a documented workflow,
because two fixes were written against each other. The DONE restart rule did not terminate.

**The pattern.** Each revision was written by the author of the one before it, and each introduced
defects of the class it had just fixed. Revision 4 is a cut rather than a patch for that reason.

## Measurements, at `origin/dev` @ `65f71fd`

| Claim | Command | Result |
|---|---|---|
| 8 of 43 H1 rows have a bare id | `grep -chE '^\| *H1-[0-9]+ *\|' docs/plan/ws-*.md \| paste -sd+ - \| bc` | 8 |
| total H1 rows | `grep -chE '^\| *(WS[0-9]+-)?H1[-0-9a-z]* *\|' docs/plan/ws-*.md \| paste -sd+ - \| bc` | 43 |
| no dependency field | `grep -ic 'depends-on' docs/plan/ws-*.md` | no output, exit 1 |
| modules in one file | `grep -c '^pub mod ' src/lib.rs` | 115 |
| tests auto-discover | `grep -c '\[\[test\]\]' Cargo.toml` | 0 |
| `cargo run` boots the daemon | `src/cli/handlers.rs:14` | `cli.command.unwrap_or(Commands::Serve)` |
| `GateStatus` is defined, not re-exported, here | `git grep -n 'pub enum GateStatus'` | `src/pre_merge_guard/status.rs:14` |
| `.anvil/shape.json` is tracked | `git ls-tree -r origin/dev --name-only .anvil/` | 3 files |
| CI's own commands | `.github/workflows/build-and-test.yml` | lines 28, 58, 60 |
| `H1-7d` is owned twice | `grep -rnE '^\| *H1-7d *\|' docs/plan/ws-*.md` | `ws-09:25`, `ws-12:39` |
| `H1-13` is owned once | `grep -rnE '^\| *H1-13b? *\|' docs/plan/ws-*.md` | `ws-02:35`, `:36` |

**Deliberately not pinned:** remote branch counts. `git branch -r` enumerates the local clone's
refs, and §3's first ordered command (`git fetch --prune`) mutates that set — three different values
appeared within two days. A number that cannot reproduce off one machine does not belong in a file
whose first invariant is "measure, never quote".

## Why the live-test filter excludes by binary

`tests/subscription_driver_live_test.rs` gates its three `test_live_*` tests only on `agy` being
installed, so an unfiltered suite makes billable model API calls. The filter excludes by *binary*
rather than by name pattern because nextest matches `test()` against `module::name`: an anchored
name filter such as `test(/^test_live_/)` misses a test declared inside a `mod`, and the first live
test written that way would silently rejoin the run.
