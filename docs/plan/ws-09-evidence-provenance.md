# WS-09 — Evidence provenance and isolation

**The class (issues #149, #151):** measurements taken on trees that are not the certified head. The
fixer and certifier mutate one shared clone per repository with no repo-level lock, so one PR's tree
can be committed and pushed onto another's branch (#149); the gate corpus reads the shared clone's
working tree, which is never checked out at the certified head — the dependency audit audits the
base branch, added policy files are silently skipped (#151). A build of a different commit is not
this pull request's evidence. The same class sits one layer lower (#200): `github::fetch_merge_queue_depth`
converts a failed process into `Ok(0)`, so even an absence-aware caller receives a measured zero —
the fetch boundary must carry its own absence, or every consumer re-derives RC-2's missing
distinction at its own call site.

`SubjectRoot` exists and is landed (built only by cloning; worktree-at-head verified by
`rev-parse`, per ADR-0002 loop step 1). `TrunkRev` does **not** exist yet — `git grep TrunkRev
6128284 -- src/ tests/` finds nothing; it is designed prose in `ARCHITECTURE.md` §0 only
(external review caught this file claiming it existed) — so building it is in H1-4a's scope, not
assumed. This workstream finishes the half that is still a convention: **a gate can still ignore the subject and read
`CARGO_MANIFEST_DIR` directly** (ARCHITECTURE.md: "the compile-time refusal arrives when scanner
signatures take `&SubjectRoot`").

## Milestones

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H1-4a | Every gate read goes through an ephemeral worktree at the certified head; the shared clone becomes fetch-only | seeded wrong-head fixture: certification refuses with the head mismatch named; `git checkout -B` on the shared clone gone (`grep` = 0 outside fetch paths) | Implementation |
| H1-4b | Per-repo write lock (or per-PR worktree ownership) for fixer/certifier concurrency | concurrency test: two PRs fixed+certified in parallel, zero cross-branch writes; discarded-`Result` fetch/checkout sites (`let _ =`) eliminated | Implementation |
| H1-4c | Push honesty: a failed push is a failed fix — no "✅ Addressed in commit" for a commit that exists only locally (#149 part 3) | seeded push-rejection fixture: no comment posted, error propagated as a typed retriable abort (postmortem RC-4 primitive) | Implementation |
| H1-7d | Scanner signatures take `&SubjectRoot` (the compile-time half; one milestone shared with WS-12 H1-7d, scheduled H1 in both) | a scanner accepting `&Path`/`env!` no longer compiles at the gate boundary (API-level, checked by compile-fail test) | Architecture |
| H2 | Receipts carry provenance: every gate result binds `{subject_root_digest, head_sha, instrument, duration}`; signing lands in WS-15 | a report field without provenance fails the report schema; enlist door refuses a report whose head ≠ door-read head (already; extended to every consumer) | Security |

## Ratchets

- **A fetch that failed may not return a value a caller can read as measured.** `github::fetch_merge_queue_depth` (`src/github/mod.rs:482-484`) converts a non-zero exit into `Ok(0)`, so an absence-aware consumer still receives a measured zero and RC-2's missing distinction is re-derived at every call site (#200). Seeded both directions: a failing `gh` invocation must surface absence, and a real zero must still read as zero. This is the fetch-boundary twin of WS-08's `Evaluated` -- absence gets a spelling at the boundary that produces it, not only at the gate that consumes it.
- Wrong-head refusal is a standing fixture in CI (red on a seeded stale worktree).
- The retriable-abort primitive (RC-4) replaces per-site rollbacks: anything holding merge authority
  returns an `Err` that cannot be discarded by `warn!` — enforced by type (`#[must_use]` +
  no-`warn!`-swallow meta-check seeded both directions).
- Provenance fields are non-optional in the report schema — absent provenance is a schema error,
  not a default.

## Non-goals

No distributed build of evidence (single-host worktrees suffice at current fleet size; VCS-substrate
research is backlog item 4 before H2-1's parallel-move week); no re-litigating occupancy — it
schedules, the rehearsal (WS-05) proves the merged state.
