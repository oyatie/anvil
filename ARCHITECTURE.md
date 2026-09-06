# Anvil: the architecture

Six mechanisms, one typed boundary, and a delete list. Everything below cites
code that exists today; nothing is proposed that the tree does not already
half-contain.

## Status

This document was written in a scratch directory and cited in review for weeks
without being committed — which is the defect `docs/doctrine.md` exists to
prevent, applied to the architecture itself. A design that lives only in
conversation cannot be disagreed with, cannot be measured against the tree, and
cannot go stale visibly. It is committed here so that it can do all three.

It is a record, not a plan of record: where the tree has since moved, the
document says so rather than being quietly rewritten. §0 already carries that
shape under **"Built, and what it taught"**, and that is the convention for
amending it — state what was built, and what building it taught that the design
did not anticipate.

Two places where the tree has moved since this was written, neither yet on
`dev`:

- **§0, `SubjectRoot`.** Landed across 96 files, not 92, and carries
  `Deref<Target = Path>` as well as `AsRef<Path>` — the same one-way rule, which
  keeps 31 call sites at `&repo_dir` rather than respelling them.
- **§6, the outbound seam.** Built with `Posture` carrying a workspace and the
  credentials leased for one turn, rather than `Restricted`/`FullAccess`.
  The isolation decision it actually enforces is the environment: `env_clear`
  plus an allowlist that separates the credential a turn *is* from the authority
  Anvil *has*. A per-tool permission grant is not among the flags the installed
  `agy` offers — measured, not assumed — so `Restricted { writable }` describes
  something no adapter can currently express.

## 0. The boundary types

The defect at the root is that a boundary carrying authority is an unmarked
primitive. These are the whole fix; the rest follows.

A type earns a place here only when its constructor is the act that establishes
the fact it claims. `SubjectRoot` is built by cloning; `TrunkRev` by proving
ancestry. A type whose constructor merely range-checks its argument belongs
next to the value it guards, not here.

**Built, and what it taught.** `SubjectRoot` landed across 92 files. Two
things the design above did not anticipate:

The escape hatch is the design. A type with no way out is routed around
rather than used, so `asserted(dir, Uncloned)` exists -- and the compiler,
not the plan, enumerated the four honest reasons. `SelfMeasurement` is anvil
measuring its own tree at boot, the one place subject and reviewer coincide
and now the only place that reads that way. `OperatorSupplied` is a path off
the command line, the one case where a human rather than the clone step is
the authority. `NoTreeBehindThisDiff` is a corpus of patch text. `TestFixture`
is a stand-in. The type's worth is entirely the discipline on that one
symbol, which means the guard on it is not an accessory to the mechanism --
it is half of it.

That guard took three cuts, and the first two passed their own seeded defect.
A latching `#[cfg(test)]` flag waved through anything below the test module.
Counting braces on `without_commentary`, which keeps literal bodies, let one
unbalanced brace in a fixture string hold the region open to end of file.
The third counts on `code_only` and refuses to answer when depth does not
return to zero, because `code_only` does not model raw strings and a scan
that was fooled must not guess. Seed every remedy against the defect that
beat the one before it; a check written once and trusted is a check that has
not been measured.

`AsRef<Path>` is implemented and that is not a leak: using a subject where a
path is wanted is sound, and no impl permits the reverse. It removed 20 of
the 47 mechanical edits.

What this step does NOT yet buy: a gate can still ignore the subject and read
`CARGO_MANIFEST_DIR` directly. The compile-time refusal arrives when scanner
signatures take `&SubjectRoot`. Until then that half is a test.

```rust
// git_manager: the only constructor is the clone step.
pub struct SubjectRoot(PathBuf);
impl SubjectRoot {
    pub(crate) fn cloned(dir: PathBuf) -> Self;   // ensure_repo_cloned only
    pub fn join(&self, rel: &str) -> PathBuf;
}

// A revision proven to be an ancestor of the trunk. Not any sha.
pub struct TrunkRev(String);
impl TrunkRev {
    pub async fn prove(root: &SubjectRoot, sha: &str, trunk: &str)
        -> Result<Self, NotOnTrunk>;
}
```

`scan_tree(&SubjectRoot)` cannot be handed `env!("CARGO_MANIFEST_DIR")`. It does
not compile. No test, no ratchet, no exemption list.

## 1. M1 — one subject

`Corpus` becomes the only door. It already holds `subjects`, `contents`,
`manifests`, `changeset`; what changes is that nothing else is reachable.

```rust
pub struct Corpus {
    root: SubjectRoot,
    base: TrunkRev,
    head: String,
    files: Vec<FileDiff>,          // from diffs_by_path, never raw text
    contents: BTreeMap<String, String>,
    manifests: BTreeMap<String, String>,
    commit_subjects: Option<Vec<String>>,
    capabilities: Capabilities,     // build_graph, toolchain, network
}

impl Corpus {
    pub fn root(&self) -> &SubjectRoot;
    pub fn files(&self) -> &[FileDiff];
    pub fn production(&self, path: &str) -> Option<ProductionCode>;
    pub fn raw_for_prompt(&self) -> &str;   // named exception: the reviewer
    pub fn byte_len(&self) -> usize;        // named exception: MAX_DIFF_CHARS
    pub fn satisfies(&self, needs: Requires) -> bool;
}
```

`PrDiffContext.diff_content` becomes private behind `files()`. Measured: 63
readers outside `git_manager`, of which 12 hand-roll a parse (10 `.lines(`,
2 `.split("diff --git`), 4 want only the size, and the rest pass it along.
`FileDiff` already has private `added`/`all` with accessors -- the pattern is
half-applied, and this finishes it.

**Retires:** the 18 hand-rolled diff parsers, `diff_parsing_ratchet_test` and
its `CEILING = 19`, `gates_scan_the_repo_under_review_test`, and the ~34
spellings of "is this production code".

## 2. M2 — one rule engine

`harness::Rule` unchanged; it is already correct.

`Requires` stays at eight rungs. A ninth for the code graph is the obvious next
one, and it waits: `stage()` has no production caller, `Ord` is compared only in
`rule_harness_test`, and `local_inner_loop` builds a single corpus shape, so
every rule runs at one stage whatever it declared. The ladder is declared and
nothing derives from it. Give it a consumer first -- and settle whether the
rungs are ordered or independent, because `satisfies` treats them as flags while
`Ord` calls them a cost order, and a corpus holding manifests but no contents
satisfies the higher rung and fails the lower one. When the rung does land it
carries `Option<Graph>`, not the `bool` that `build_graph` uses: a toolchain is
external and must be asserted, a graph is anvil's own artifact and its presence
can be structural.

```rust
pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;
    fn requires(&self) -> Requires;
    fn examine(&self, corpus: &Corpus) -> Evaluated;
    fn fixture(&self) -> Fixture;     // not optional, and that is the point
}
```

A check becomes a row in `rules::registered()`, not a module. Four are
registered today against ~72 hand-wired gates.

**Absorbs:** every `*_guard` whose body is a regex over changed lines --
`formal_verification` (2 rules), `kani_guard` (1), `cell_isolation` (2),
`jittered_backoff`, `idempotency_guard`, `constant_work_guard`,
`zero_trust_workload`, and the rest of the reliability cluster.

## 3. M3 — one Finding, one Fix

`shape::core::report::Finding` is the survivor; `harness::Finding` deletes
itself (its own doc at `harness/mod.rs:178` argues for it).

```rust
pub struct Finding {
    pub rule: RuleId,
    pub key: String,              // stable across edits that do not change the defect
    pub path: String,
    pub line: Option<u32>,
    pub detail: String,
    pub fix: Option<Fix>,         // Some = derived drift; None = a decision
}
```

`fix` IS the declare/derive boundary. `Some(Fix)` means a derived artifact
drifted and anvil may regenerate it; `None` means a declaration was violated
and only a human may resolve it. **Anvil auto-fixes `Some` and never `None`.**

Key derivation follows Infer: severity + rule + sanitised symbol + basename,
never line or column, so a finding survives a file moving.

**Absorbs:** 46 `*Finding`/`*Violation` structs, 92 `*Report` structs.

## 4. M4 — one outcome

`Evaluated` replaces `GateStatus`. It already makes the defect unspellable:

```rust
Evaluated::Measured { subjects_seen: NonZeroUsize, findings: Vec<Finding> }
Evaluated::Withheld(Withheld)
```

"Examined nothing, found nothing" has no spelling. `GateStatus::Passed` is a
unit variant carrying no count, which is why 34 of 72 gates needed an
absence-exemption policy to be admissible at all.

The 72 `pub <name>: GateStatus` fields become `BTreeMap<&'static str,
Evaluated>`; `GATE_LABELS` (already pinned at `matrix.rs:429`) becomes the one
registry row.

## 5. M5 — one keyed ratchet

`ratchet::core::{baseline,compare,signoff}` over key sets, never counts.
Baselines carry a `TrunkRev` and are refused when it is not on the trunk --
the shape baseline today names a commit on neither trunk and its guard checks
only that the string is 40 hex characters.

**Absorbs:** `ratchet/facade/derived.rs` (122 lines, zero production callers)
and the 17 frozen numeric ceilings in `tests/`.

## 6. M6 — one outbound seam

```rust
// exec: the only way to reach a tool.
pub fn gh() -> Command;                       // installation token, not ambient
pub fn agent(tool: Tool, cwd: &SubjectRoot, posture: Posture) -> Command;
pub enum Posture { Restricted { writable: PathBuf }, FullAccess { why: &'static str } }
```

33 `Command::new("gh")` sites and 5 agent spawn sites collapse to two
constructors. `Posture` has no `Default`, so a new spawn site cannot omit the
isolation decision.

## 7. Migration order, by dependency

1. **Boundary types** (`SubjectRoot`, `TrunkRev`). Nothing else compiles
   correctly without them; they are what make the rest unspellable rather than
   merely checked.
2. **M1 Corpus + private `diff_content`.** 63 call sites, mechanical. Retires
   four prevention-tests.
3. **M4 outcome.** Unblocks admission: today 34 of 72 gates are
   absence-exempted and no pull request has ever been admissible.
4. **M3 finding.** Needs M4's envelope to land in.
5. **M2 rule rows.** The bulk migration; needs M1 and M3 in place first.
6. **M5, M6.** Independent of the above; may land any time.

## 8. What is deleted, not moved

~10,600 lines. `wasm_sandbox` (fabricated), `ephemeral_sandbox` (unit struct),
the 8 modules built into `AppState` and never invoked, the reliability cluster
that has no subject in a pre-merge daemon, `predictive_test_selector` (446
lines re-deriving what btd computes from a real graph).

D-8 states the test: "A directory or file is allowed only if something that is
not a census gate loads it... Do not invent a destination for leftovers; many
must not exist."

## 9. What stays declared

`WATCHED_REPOS`, `.anvil/shape.json`'s `face_dependency_matrix`,
`ABSENCE_POLICY`, the D-8 root set, rule maturity, signoffs, test size. Those
are decisions. Everything else is derived from them, and anvil regenerates a
derivation that drifts.
