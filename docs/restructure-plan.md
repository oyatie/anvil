# Restructure plan

Rewritten 2026-08-24 against `origin/dev` @ `769a7de`.

> **Status note, 2026-08-31 (added when this file was first committed, after external review of
> PR #197; the body below is the 2026-08-24 record and is deliberately not rewritten):**
>
> - **Census superseded.** The numbers below were true at `769a7de`: 109 modules, 52,534 src
>   lines. At `6128284` the roadmap re-measures 115 modules / 68,978 lines
>   (`docs/plan/anvil-roadmap.md` §1.1). The kernel table's derived figures (+30/+29 levers, 86
>   dependency-free modules) are `769a7de`-era and must be re-derived before Phase A executes —
>   ws-01 carries that as part of H1-8a's entry.
> - **D5 erratum.** "Toolchain converges on 1.98.0" must not be executed as written: setting
>   `rust-version = "1.98.0"` equal to the channel trips `Drift::Conflated`
>   (`src/toolchain/mod.rs:202`) and fails `tests/toolchain_msrv_test.rs` — channel and MSRV are
>   two promises in opposite directions, and equality is the exact conflation the in-tree check
>   forbids (root `CLAUDE.md`; the check postdates this document). The channel converges on
>   1.98.0 (done); the MSRV decision is re-taken separately via the decision registry.
> - **Pilot path renamed.** `~/Developer/rust-agent-lab` no longer exists; the pilot is
>   `~/Developer/intelligence` (no `BRIEF.md` — its shape is described in ws-01).

An earlier revision of this document was measured against a feature branch
rather than `dev`. Its census, its kernel table, its toolchain table, and its
claim that no ports layer exists were all wrong. Nothing here is carried over
from it unverified.

## Method

Numbers below are reproducible. Modules are the top-level `pub mod` entries in
`src/lib.rs`. Dependencies are `crate::<module>` references anywhere under that
module's directory. Run against a clean `origin/dev` worktree, not a feature
branch — that mistake is what invalidated the previous revision.

## Census

| | value |
|---|---|
| top-level modules | **109** |
| src lines | **52,534** |
| edition | **2024** (already) |
| rust-version | **1.97.1** -> target **1.98.0** |
| average sibling imports | 3.1 |
| modules importing >5 siblings | 3 |

**A ports layer already exists.** `change_delivery` (3 port traits), `ratchet`,
and `shape` each already ship `core/ports/adapters`. This work extends an
established pattern rather than introducing one — which also means the pattern
is already proven in this codebase and does not need to be argued for.

## The kernel

| kernel | modules left dependency-free |
|---|---|
| none | 17 / 109 |
| `git_manager` | 47 / 108 |
| `+ exec` | 52 / 107 |
| `+ github` | 57 / 106 |
| `+ pre_merge_guard` | **86 / 105** |
| `+ change_delivery, config, state` | 85 / 102 — *worse* |

Hubs by dependent count: `git_manager` 61, `pre_merge_guard` 36, `exec` 27,
`github` 12, `ai_driver` 5, `self_governance` 4.

**Two levers, not four.** `git_manager` yields +30 and `pre_merge_guard` +29;
`exec` and `github` buy five each. Extraction stops paying after the fourth —
adding three more hubs makes the free set *smaller*, because the kernel
consumes them.

### `git_manager` is not a leaf

It depends on `attestation_guard`, `change_delivery`, and `exec`. Any plan that
treats it as an extractable leaf is wrong, and `{change_delivery, git_manager}`
must be broken before or during extraction, not after.

### `pre_merge_guard` is a types hub, not a logic hub

36 dependents, but the overwhelming majority import `GateStatus` and nothing
else. Moving that one enum to `core` recovers most of the +29 without moving
the guard itself. Verify the exact split before extracting — this is the
cheapest large win available and it must not be guessed at.

## Serialization points — the real bottleneck

Measured on the seven-PR drain: **every pair of gate PRs conflicts**, on
`src/fidelity/registry.rs` and `tests/evaluator_preserves_gate_verdicts_test.rs`.
Seven disjoint changes required seven sequential merges and six rebases.

A third is worse for this plan specifically: **all 109 modules are declared in
the single `src/lib.rs`**, so every module move in Phase C touches one file.

This reframes an earlier conclusion. When occupancy refused 7 of 8 PRs, that was
recorded as a defect in occupancy. It was not — occupancy correctly reported
that this repository has files every change must touch. The scheduler was right;
the layout is the bug.

**Phase C cannot deliver parallel moves until these are removed.** Options,
cheapest first:

1. one file per gate under `src/fidelity/gates/`, with a `mod.rs` touched only
   when a gate is added or removed
2. registry as data — one file per gate, loaded at build time, zero Rust conflicts
3. distributed registration (`inventory` / `linkme`) — no central list exists

The same treatment applies to the test file. For `src/lib.rs`, per-capability
module declarations move into the capability, leaving the root file touched only
when a capability is added.

An interim mitigation exists and is in use: a semantic 3-way merge tool for
`registry.rs` that unions entries by `gate_id` and refuses on ambiguity rather
than guessing. It removes the risk, not the serialization.

## Decisions

| # | decision | rationale |
|---|---|---|
| D1 | Forge ports split by concern | a capability reading PR metadata should not compile against webhook code |
| D2 | Four PRs, one per hub | independently revertible and verified |
| D3 | Delete the classified checks before extracting | less to move |
| D4 | Phase A waits for one green seeded-defect fixture | a large import rewrite should not be guarded by a suite in which no gate can demonstrably fail |
| D5 | Toolchain converges on **1.98.0** | matches the pilot; one less reconciliation |

### D3 is smaller than previously recorded

Re-derived against `dev`: **26 checks**, not 34. The figure 34 was almost
certainly the count of *unaudited* gates (72 audited-corpus minus 38 registry
entries), carried forward as if it were a deletion list.

`TOTAL_GATES` is **not** deletable on this branch: it drives gate 1's verdict
through `doc_guard/mod.rs:141` and is published by `dashboard/mod.rs:129`. The
"self-policing" category is empty.

Three gates cannot fail — `shuffle_status` (overlap is exactly 2 against a >2
threshold), `canary_status` (0.2 against a 3.0 ceiling), and
`progressive_ring_status` (its input is a constant). Their fabricated *inputs*
are the defect, not the report fields.

## Sequence

    [done]  seven gate PRs merged to dev
    [done]  merge queue merge_method: SQUASH -> MERGE
              |
              +---> D3: delete the 26 classified checks  -----+
              |                                               |
              +---> pilot: first seeded-defect fixture        |
                          proven red-then-green               |
                                    |                         v
                                    +-------------> Phase A (D1, D2, D5)
                                                          |
                                                          v
                                                    Phase B: workspace split
                                                          |
                                                          v
                                                    Phase C: capability migration
                                                    (blocked on serialization fix)

Deletion is not gated on the fixture: a check that provably cannot fire cannot
regress detection by being removed. That property is what makes it deletable.

## Phase A — kernel extraction

    core/
      GateStatus                     <- from pre_merge_guard, the +29 lever

    ports/
      process.rs                     <- exec
      git.rs                         <- git_manager
      forge/{pull_request,review,webhook,metrics}.rs   <- github, split per D1

    adapters/
      process/  git/  forge/

Per PR: define the trait, re-export from the original path, move the
implementation, rewrite call sites onto the trait, delete the re-export. Step
two is what keeps every intermediate state compiling; do not skip it.

Verification per PR is `fmt`, `clippy -D warnings`, and the full suite **with
counts**, plus a statement of what behaviour was expected to be unchanged and
how that was checked. A pure import rewrite that changes a test count has
changed behaviour.

## Phase B — workspace split and pilot absorption

`Cargo.toml` becomes `[workspace]` with one capability carved: `intelligence/`,
the destination for the verdict-runtime pilot (`~/Developer/rust-agent-lab`,
see its `BRIEF.md`). New work then has a correct destination instead of a flat
`src/`.

**Absorbing the pilot is not a directory move.** Reconciliation required:

| | anvil | pilot |
|---|---|---|
| edition | 2024 | 2024 |
| rust-version | 1.97.1 | 1.98.0 (target, per D5) |
| version policy | caret ranges | exact pins (`=0.8.9`) |

Three genuinely breaking dependency conflicts:

| dep | anvil | pilot | note |
|---|---|---|---|
| `axum` | 0.7 | `=0.8.9` | 0.x bumps are breaking |
| `hmac` | 0.12 | `=0.13.0` | digest trait bump |
| `sha2` | 0.10 | `=0.11.0` | same |

`hmac` and `sha2` are the **webhook signature verification** path
(`webhook/webhook_handlers.rs`), and `sha2` additionally backs admin token
comparison in `webhook/admin_auth.rs`. Treat that bump as a security-relevant
change, not a version bump.

The pilot's exact pinning is deliberate and consistent with the determinism
goal. Converge on it rather than relaxing the pilot.

**Five duplicate concepts must collapse to one:**

| anvil | pilot | resolution |
|---|---|---|
| `exec::run_bounded` | `run_bounded` in the tool adapter | one `ProcessPort`; the pilot's is the reference |
| `ai_driver::router` — five CLI spawns, `--dangerously-skip-permissions` | `ModelPort` + typed adapters | router becomes an adapter behind `ModelPort`; the permission bypass dies here |
| `ephemeral_sandbox` + `wasm_sandbox` (257 lines of stub) | `SandboxPort` + Local and Container adapters | delete ours, adopt theirs |
| `git_manager` | `GitPort` | union behind one port |
| `GateStatus` / report | `VerificationResult` / `is_hard_verified()` | **own PR, own decision** |

Four are "delete ours, adopt theirs" — ours are stubs or shell-outs. The fifth
rewrites the certification core and needs seeded-defect coverage on the gates it
touches before it lands.

Both vocabularies were found to let absent evidence read as a pass. Anvil's is
fixed (`GateCounts`, four-way). The pilot's is not: `VerificationStatus::Inconclusive`
is declared and constructed nowhere, inconclusiveness lives on the error channel,
and `is_hard_verified()` can return true with no safety evidence. That must be
fixed there before absorption, not during.

## Phase C — capability migration

The 86 dependency-free modules move independently. **Blocked on the
serialization fix above** — until `src/lib.rs` stops being a single declaration
point, "parallel moves" is not achievable and should not be claimed.

Once unblocked this is the first genuine N-parallelism workload in this fleet,
and the honest test of the occupancy machinery.

## Release versioning — a capability, not a chore

Anvil must bump versions on release **for itself and for every repository it
manages**. Neither happens today: both Anvil and the pilot sit at `0.1.0`, and
`CHANGELOG.md` was last touched by a promotion commit rather than by anything
that shipped.

The hard input already exists: `semantic_abi_ratchet` produces
`BreakingAbiFinding`, which is the major-versus-minor decision.

| bump | deterministic input |
|---|---|
| major | `breaking_findings` non-empty |
| minor | new public API, no breakage |
| patch | neither |

Two requirements follow from this session: the same code path must run against
Anvil itself, not only managed repositories; and the published version must be
recorded **with the evidence that justified it**, or it is one more unbacked
claim.

**Open:** whether Anvil goes `1.0` before this lands. Under `0.x`, a breaking
change bumps the minor, so the rule the ratchet implements depends on the answer.

## Target layout

Only `core`, `ports`, `adapters`, `facade` (plus `cedar`, `observability`,
`iac`, `docs`) are legal inside a capability. Capability roots carry `OWNERS`,
`README.md`, `PRD.md`, `BUCK`, `Cargo.toml` — none of which exist today.

Root directories `openapi/`, `policies/`, `scripts/`, `src/`, `tests/` are
non-conformant and need dispositions during Phase C.
