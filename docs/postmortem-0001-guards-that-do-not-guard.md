# Postmortem 0001: the guards that did not guard

**Status:** accepted
**Scope:** one working session, 12 open pull requests, 4 adversarial review rounds
**Defects found:** 33 confirmed in round 3 alone, after refutation

This is not a list of bugs. The bugs are fixed. This is the account of *why the
same defect kept being written*, because five of the six root causes below
produced more than one instance, and three of them produced an instance **inside
the fix for the previous instance**.

## What happened

A session spent hardening Anvil's pre-merge guards produced, in order:

* a door rule that enumerated three verbs and so could not see the fourth door,
  which ran a model turn and filed a public issue while Anvil was paused;
* a hub set naming `.github/workflows/ci.yml`, a file this repository does not
  have, so the rule meant to serialise CI changes serialised nothing;
* a recovery blocklist listing three combinators and not `.or_else(`, the one
  that fabricates a clean report *and* keeps the `?`;
* a scan that stripped double-quoted spans and then read its own explanatory
  comment as an adoption of Redis, refusing the pull request that introduced it;
* a scan that read `without_commentary`, which keeps string literals by
  documented design, so `warn!("…pause…")` counted as a guard;
* an assertion satisfied by a different call site — twice, the second time one
  paragraph below the comment explaining the first;
* a scan matching `"return "` with a trailing space, which `bail!` walks past;
* a rule that reported zero violations because its parse found no files.

Every one of these passed its own test suite. Several were introduced by the
commit that fixed the one before it.

## Root causes

### RC-1 — Invariants are enforced by scanning source text, not by types

Eight of the defects above are the same defect: a guard that greps Rust source
for a pattern, in a language whose source contains comments and string literals
that look exactly like the thing being searched for. The scan cannot tell the
difference between code that does something and prose that describes it.

This is not a bug in any one scan. Scanning is a heuristic over a representation
that deliberately contains mimics, and *the author of the guard is also the
author of the prose beside it*, so the two drift into agreement.

`ARCHITECTURE.md` in this repository already states the fix and it has not been
done:

> `scan_tree(&SubjectRoot)` cannot be handed `env!("CARGO_MANIFEST_DIR")`. It
> does not compile. No test, no ratchet, no exemption list.

**Corrective action.** For every text-scanning guard, the question is not "is the
regex right" but "what type would make this unspellable". Where a scan must
remain — wiring assertions have no type — it is subject to three rules, and a
scan that cannot meet them is not a guard:

1. it reads `source_scan::code_only`, never raw text and never
   `without_commentary`, unless it is *locating* a literal, in which case it must
   say so;
2. it refuses to answer rather than guessing when its parse is ambiguous
   (`code_only` does not model raw strings; a scan that was fooled must not vote);
3. it is seeded in both directions before it is trusted — the defect it names
   must fail it, and the correct code must pass it.

### RC-2 — `GateStatus::Passed` cannot distinguish "measured nothing" from "measured, found nothing"

`Passed` is a unit variant carrying no count. So a gate that examined an empty
corpus, a gate whose parse found no files, and a gate that examined 400 files and
found nothing all publish the same word.

This is the root cause of the three numbers that have not moved:
`NOT_PROVISIONED_COUNT = 26`, `GATES_WITHOUT_PROOF = 23`,
`honesty_ratio() = 0.0`. Thirty-four of seventy-odd gates needed an
absence-exemption policy *to be admissible at all* — that policy is not a
feature, it is the shape of the missing distinction. No amount of per-gate
patching moves those numbers, because each patch re-establishes the same
ambiguity in a new place.

**Corrective action.** `ARCHITECTURE.md` M4, and it is the highest-leverage work
in the tree:

```rust
Evaluated::Measured { subjects_seen: NonZeroUsize, findings: Vec<Finding> }
Evaluated::Withheld(Withheld)
```

"Examined nothing, found nothing" has no spelling. `NonZeroUsize` is the whole
mechanism; `ABSENCE_POLICY` is deleted rather than migrated.

### RC-3 — No boundary between contributor-controlled data and control-plane decisions

A pull request title containing `[skip review]` caused the webhook handler to
return before every auto-merge withdrawal in the pipeline — so a contributor
could edit their own title, push, and keep a merge armed on a head no report
measured. Separately, comment authorship was decided by
`author.contains("bot")`, and Anvil's own login matched neither marker, so every
comment it posted spawned a clone, a model turn and a push that could post more.

Both are the same shape: **data plane deciding control plane.** Titles, labels,
comment bodies and author strings are written by the party the control plane
exists to constrain.

**Corrective action.** A `ContributorSupplied<T>` newtype at the webhook
boundary, whose inner value cannot reach an authority decision without a named
unwrap — the same escape-hatch-with-a-reason shape as
`SubjectRoot::asserted(dir, Uncloned::TestFixture)`. Routing may read it;
admission may not.

### RC-4 — Fallible external calls default to lossy handling

`disarm_auto_merge` is fail-closed and its own `bail!` says a head this run did
not certify may still merge. All four call sites downgraded that to `warn!`. One
of them then stamped the head and returned `Ok`, after which the
already-reviewed guard turned every later delivery into a no-op — so the only
thing that could have withdrawn the arming never ran again.

The same shape produced the stranding class: three exits after the reviewed-SHA
stamp, each needing a hand-rolled rollback, one of them a bare `?` that stranded
a pull request permanently when the forge was rate-limited *after every gate had
passed*.

**Corrective action.** The pipeline needs one primitive — "abort in a way that
retries" — rather than three hand-rolled rollbacks and a convention. Anything
that holds merge authority and can fail must return a type whose `Err` cannot be
dropped by writing `warn!`.

### RC-5 — Admission is decided on per-branch state, not on the merged state

Occupancy admits a pull request when its exact-path write-set is disjoint from
every lower-numbered open pull request's. #140 retypes
`PrDiffContext.repo_working_dir` from `PathBuf` to `SubjectRoot`; #146 adds a
test constructing that struct with a `PathBuf`. **The two share no file.** Both
are admissible, both are green in isolation, and the merged tree does not
compile.

Path-disjointness is a scheduling heuristic that cannot see type coupling,
semantic coupling, or trait coherence. Treating it as a correctness gate is the
error.

**Corrective action.** State in `occupancy`'s own documentation that it orders
work and does not prove correctness; the merged tree is what must build. Land a
merge-train rehearsal — merge every open pull request in queue order onto trunk
locally and run the suite — as a required check, so a cross-cutting break is a
local failure instead of a queue ejection.

### RC-6 — The verification instruments were not themselves verified

A grep that counted `^test result: ok` and reported "0 failures" — a pattern that
cannot express a failure. A monitor whose `grep -c … || echo 0` emitted `0\n0`,
so its "not finished" test was true immediately and it reported two empty
results as success. A `for` loop where zsh read `$branch:src/…` as a history
modifier and silently produced measurements of mangled revisions.

Each of these *reported a pass*. Two of them nearly became statements to a human.

**Corrective action.** Any command whose output will be reported as a pass must
be able to express the corresponding failure, and must be shown doing so at least
once. This is RC-1 applied to the tooling: the instrument is a guard, and a guard
that has not failed on purpose has not been measured.

## The pattern under the pattern

Five of these six are the same mistake at different altitudes: **a proxy was
trusted as if it were the thing.** Source text as a proxy for behaviour. `Passed`
as a proxy for evidence. A path set as a proxy for compilability. A grep as a
proxy for a test run. In each case the proxy was cheap, mostly right, and silent
when wrong — and *silent when wrong* is the property that lets a defect survive
its own fix.

The discipline that follows is not "write better regexes". It is: **prefer the
construction that cannot express the defect; where a proxy is unavoidable, make
it loud when it cannot answer.**

## Corrective actions, ordered by leverage

| # | Action | Moves |
|---|--------|-------|
| 1 | M4 `Evaluated` with `NonZeroUsize`; delete `ABSENCE_POLICY` | `NOT_PROVISIONED_COUNT` 26→0, `GATES_WITHOUT_PROOF` 23→0, `honesty_ratio` 0.0→ |
| 2 | Merge-train rehearsal as a required check | RC-5, and every future cross-PR break |
| 3 | `ContributorSupplied<T>` at the webhook boundary | RC-3 |
| 4 | One retriable-abort primitive in the review pipeline | RC-4, the stranding class |
| 5 | Scanner rules 1–3 above, enforced by a meta-guard over `tests/` | RC-1 |
| 6 | Scanner signatures take `&SubjectRoot` | RC-1, the compile-time half |

Items 1 and 6 are already written down in `ARCHITECTURE.md` and have been for
some time. That they are still open is the finding this postmortem exists to
record: the tree diagnosed itself correctly and then kept patching instances.
