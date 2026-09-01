# Anvil multi-year roadmap

**Status:** living document. Amend by PR to `dev`; every amendment adds a decision-log row.
**Written:** 2026-08-31, measured against `origin/dev` @ `6128284` (`git rev-parse origin/dev`).
**Companion files:** [`research.md`](research.md) (dated evidence base, all sources 2026-03..2026-08),
one `ws-*.md` detail file per workstream, and [`h1-execution-prompt.md`](h1-execution-prompt.md) — the
reusable session commission for executing a single H1 milestone (one `ACTIVE MILESTONE` slot per run;
grading is done by an independent verifier, never by the implementing agent), all beside this file.

Anvil's mission, restated as the two obligations this plan never conflates:

- **Inward.** Anvil is an incubator that merges into oyatie. Anvil's own tree converges on the
  hyperscaler capability layout that oyatie is itself converging toward — `{core, ports, adapters,
  facade}` faces plus satellites — so absorption is cheap. oyatie is the reference frame, **not** the
  template: oyatie has not finished its own consolidation (its live layout law ADR-0701 is frozen as
  "unclassified migration input"), so every oyatie-side discrepancy is a finding raised upstream, never
  a shape copied. The north star is the pattern, taken from measured hyperscaler practice.
- **Outward.** Anvil-as-a-tool writes and rewrites its managed repos into one uniform
  hyperscaler-monorepo shape via a codemod-first LSC pipeline held by ratchets. Managed repos take
  *that* shape — not oyatie's internal shape, and not Anvil's.

---

## 1. Current state (all numbers measured 2026-08-31; command beside each)

### 1.1 The tree

| Fact | Value | Command |
|---|---|---|
| dev head | `6128284` | `git rev-parse origin/dev` |
| Top-level modules | **115**, all declared in one `src/lib.rs` | `git show origin/dev:src/lib.rs \| grep -c '^pub mod'` |
| Source lines | **68,978** across **368** `.rs` files | `find src -name '*.rs' -exec cat {} + \| wc -l`; `git ls-tree -r origin/dev --name-only src \| grep '\.rs$' \| wc -l` |
| Gate corpus | `TOTAL_GATES = 73` | `src/pre_merge_guard/report.rs:204` |
| Ports layer | `core/ports/adapters/facade` ships in `change_delivery`, `ratchet`, `shape` | `ls src/{change_delivery,ratchet,shape}` |
| Shape self-spec | 2 rules self-exempted `advisory-until-infra` (`unit_missing_face`, `cross_unit_non_facade`) | `.anvil/shape.json` |
| Toolchain | channel `1.98.0`, `rust-version = "1.97.1"` (separate promises — correct; but ADR-0005 still says channel 1.97.1: doc drift) | `cat rust-toolchain.toml`; `git show origin/dev:Cargo.toml` |
| Not a workspace | single `[package]`; root dirs `openapi/ policies/ scripts/ src/ tests/` non-conformant to target layout | `git show origin/dev:Cargo.toml \| head`; `ls` |

### 1.2 Delivery state

| Fact | Value | Command |
|---|---|---|
| Open PRs | **2** (#196, #195), both base `dev` | `gh pr list --state open --json number,baseRefName` |
| Open issues | **11** at measurement (§3 maps every one); +1 the same day — #198, filed by this plan's own WS-01 duty, mapped to WS-14 | `gh issue list --state open` |
| CI on dev, last 40 runs | postsubmit **12/12 success**, CodeQL 13/13, nightly 1/1, toolchain-weekly 1/1, **promotion-open-next 0/13** | `gh run list --branch dev --limit 40 --json name,conclusion` |
| Promotion failure cause | `403: GitHub Actions is not permitted to create or approve pull requests` (no `PROMOTION_PAT`); a `promote(staging): 211 commits` PR can never open | `gh run view 33363340079 --log-failed` |
| main vs dev | main is **300 behind and 45 ahead** of dev — diverged both directions, not merely stale | `git rev-list --count origin/main..origin/dev` / reverse |
| Rulesets | dev: merge queue (ALLGREEN, ≤5, MERGE) + required `fast-checks` (strict), **`required_approving_review_count: 0`**; staging/canary/production: require `promotion-predecessor`; main: 1 human approval | `gh api repos/oyatie/anvil/rulesets/{21064279,21064983,21230025}` |
| Branch sprawl | **315 local / 167 origin / 78 `pr/*`** branches; **50** worktrees (167 includes this plan's own branch, pushed the same day) | `git branch \| wc -l`; `git branch -r --format='%(refname:short)' \| grep -c '^origin/'`; `git branch -r --format='%(refname:short)' \| grep -c '^pr/'`; `git worktree list \| wc -l` |

Two consequences worth stating plainly:

- **The bottom rung of the autonomy ladder is not mechanically enforced.** ADR-0002's manager
  clause ("do not merge or approve") and ADR-0003's standing boundaries say a human reviews before
  merge; the dev ruleset requires **zero** approving reviews, and ADR-0002 records that
  `MergeEnlister` submits an APPROVE and arms auto-merge — while authenticated as `jason931225`
  (issue #171). Today "Jason reviews first" is a convention wearing Jason's own credential, not a
  policy. The ladder (WS-06) starts by making today's rung real, then climbs.
- **The promotion ladder is a standing red.** `promotion-open-next` has failed on every push to dev in
  the measured window, so `staging/canary/production` starve while their rulesets guard branches that
  never advance. A permanently-red job that blocks nothing is an alarm nobody hears (WS-13).

### 1.3 Gate honesty (the incumbent finding set)

Three denominators appear below; they reconcile as: **73** = `TOTAL_GATES` at dev head
(`src/pre_merge_guard/report.rs:204`); **72** = the corpus size when `postmortem-0001` and
`ARCHITECTURE.md` were written (the corpus drifts as gates land — that drift is itself why H1-3
scripts the census); **64** = the distinct `*Report` parameters the gate evaluator takes — verified at `6128284` as
`PreMergeGuard::evaluate_pre_merge_gates` (`src/pre_merge_guard/evaluator.rs:110`); issue #59's
body names a symbol that never existed in-tree (`git log --all -S PreMergeCertificationEvaluator`
finds only this plan), a carried-citation defect caught by external review — the count survived
verification, the symbol did not.

- **59 of 64 evaluated gates decide by inspecting the diff string in-process; 5 invoke real
  tooling** — issue #59's census, dated 2026-08-30, method documented in the issue
  (`gh issue view 59`); carried from that day-old in-repo measurement, **not re-executed this
  session** — H1-3 exists precisely to make this census scripted, CI-run, and ratcheted so it can
  never be a stale quotation again. No build runs, no test executes, in the overwhelming majority
  of the certification matrix.
- **Admission's absence exemption is live policy, not a fixed defect list**: at `6128284` the
  door is `admission_refusal()`, and `is_admissible()` no longer requires an empty unmeasured set —
  its own doc says "this is not the admission decision" and it now applies the same
  `ABSENCE_POLICY` three-way absence split (`src/pre_merge_guard/report.rs:538`,
  `src/pre_merge_guard/admission.rs:79` — read this session; issue #19's premise predates this
  change and is partially overtaken). The standing finding: `ABSENCE_POLICY` still exists and
  classifies `slo_status` and friends `NotProvisioned` so their absence never blocks — that policy
  is the shape of RC-2's missing distinction, and M4 deletes it rather than curating it. 34 of 72
  gates needed absence exemptions when `postmortem-0001` was written (in-tree artifact);
  `honesty_ratio` is defined at `src/fidelity/mod.rs:145` (`grep -rn 'honesty_ratio' src/`) and
  reported ≈0.02 by the postmortem.
- **5 rules** are registered in the new typed rule engine (`grep -c 'Box::new'
  src/harness/rules/mod.rs`) against the ~72-gate hand-wired corpus — the M2 drain has barely
  begun.
- `postmortem-0001` names the pattern under the pattern: **a proxy trusted as the thing** — source
  text for behaviour, `Passed` for evidence, path-disjointness for compilability, a grep for a test
  run — *cheap, mostly right, silent when wrong*. Items 1 and 6 of its own leverage table
  (`Evaluated` with `NonZeroUsize`; scanner signatures taking `&SubjectRoot`) were "already written
  down in ARCHITECTURE.md" and still open when the postmortem was accepted. This plan front-loads
  them (WS-08, WS-12); leaving them open again is the failure mode this document exists to prevent.

### 1.4 oyatie, measured against its own predicates (inward reference frame)

oyatie is **not** presumed correct; these are measurements, and each discrepancy is a finding to raise
upstream (WS-01 carries the duty; none is a template to copy):

| Measurement | Value | Command |
|---|---|---|
| Capability roots | 24 top-level roots + `app/` | `git -C ~/Developer/oyatie ls-tree origin/dev` |
| Face grammar | ADR-0719 D-8's closed-children table mandates the same four faces for every cap and app ("this shape does not change"); strict census pinned at `1119e99` over the 21 capability roots (excluding `app/`, `packs/` (install authority, D-24), `build/` (meta root), `docs/`, `templates/`, `third-party/`): **9 carry the full quartet** (billing compute data iac iam k8s pipeline secrets tenancy), **12 lack ≥1 face** (audit −facade; bus −ports,facade; cell −adapters,facade; compliance −adapters,facade; flags −ports,adapters,facade; gateway −ports,facade; intelligence −ports; marketplace −ports,adapters,facade; network −facade; observability −ports,facade; policy −ports,facade; storage −facade). An earlier revision of this row said "8 roots break it" — wrong denominator and wrong set, caught by external review and corrected upstream on oyatie#2341 | per-root `git ls-tree --name-only 1119e99:<root>`, quartet membership tested per face |
| Live layout law | ADR-0700 apex series; **ADR-0701 (faces/layout) is Superseded — "frozen transitional migration input"**; the closed directory set lives in ADR-0719 D-8 | `docs/decisions/ADR-0701…` |
| D-8 violations | `intelligence/` carries cap-root `contracts/` and `k8s/`; ADR-0719 D-8's closed-children table (read this session, `docs/decisions/ADR-0719…` §D-8) admits exactly `core/ ports/ adapters/ facade/ cedar/ observability/ iac/` + `OWNERS`/`BUCK` — both `contracts/` and `k8s/` sit outside it (`contracts/` also named in ADR-0701's restatement; `k8s/` measured against the table itself, added to oyatie#2340 by comment) | `git ls-tree --name-only 1119e99:intelligence`; D-8 table |
| Markdown predicate | AGENTS.md: tracked markdown only at root; **119** non-root `.md` outside `docs/` remain as "frozen migration inventory" | `git ls-tree -r origin/dev --name-only \| grep '\.md$' \| grep -vE '^(README\|AGENTS\|CLAUDE\|LICENSE)\.md$' \| grep -v '^docs/' \| wc -l` |
| Dead pointer | `governance/capability-registry.json` **does not exist** on oyatie dev, yet Anvil's ADR-0006 cites it as oyatie's shape-as-data; D-8 confirms "`governance/` is gone (D-17)" | `git cat-file -t origin/dev:governance` (fatal — path absent) |
| Protection drift | oyatie's own `.github/branch-protection.yaml` opens by declaring live GitHub protection does not match it | file header |
| CI law | ADR-0700: single required admission context `presubmit`; binding verification is Rust/Buck2 gate apps; GHA is a transitional adapter | ADR-0700 §Decision |

### 1.5 What the research says (evidence base: `research.md`, 90 dated citations)

The 2026 consensus, compressed: **verification capacity, not model capability, is the binding
constraint**; merged-state rehearsal is table stakes and the merge queue is the choke point at agent
PR volume; codemod-first LSC (deterministic majority, agents for the tail, ratchet holds the ground)
is the endorsed pattern; deterministic **pre-action authorization is a named field** with measured
0%-attack-success results (OAP, Cedar-fronted tool calls, ACP); merge authority to trunks remains
almost exclusively human, and where autonomy exists it is **per change class, not per agent**;
checks must **provably kill seeded defects before they count** (mutation-guided validation); review
agents are socially engineerable, so verdicts must derive from executed evidence.

---

## 2. Horizons

Rules that apply to every milestone: exit criteria are machine-checkable (a command, a test, or a
ratchet — never "done when it feels done"); every new check is **proven by seeding the defect it
claims to catch before it is trusted**; owners are ADR-0002 seats (agent-operated) or the human
ticket queue (cockpit, WS-07; GitHub issues interim).

**Which table is normative.** 22 milestone IDs appear in both this file and a `ws-*` file
(measured: `grep -oE '^\| H[123]-[0-9]+[a-z]?'` over both sets, joined). That duplication produced
roughly half of every finding across four review passes — three iterations plus an external review
each found a roadmap row and its `ws-*` twin disagreeing on horizon, owner, evidence window, or
baseline. Duplicated normative rows are the malpractice; reconciling them by hand again would be
the point fix this plan forbids. So: **the `ws-*` row is normative for exit criterion, owner, and
evidence; this file's horizon tables are an index** carrying the milestone's title and horizon
placement only. Where the two disagree, the `ws-*` file wins and this file is the defect.
WS-14 carries the drift check that makes the next divergence unwritable (`WS14-H1b`); until it
lands, this sentence is the tie-break rule, not a promise that they agree.

Milestone IDs are stable; a ws-file row
without its own ID is addressed as `<WS>-<horizon><letter>` in file order (e.g. `WS03-H1a`), and
parameters a criterion leaves symbolic (an N, a budget, a threshold) are pinned as a registry row
when the milestone starts — the pin is part of entering the milestone, and the named defaults below
hold until repinned.

### H1 — 0–6 months (through 2027-02): make the existing claims true

The malpractice classes found in Phase 0 are **front-loaded here, non-deferrable** — every one is a
class-level workstream with a regression ratchet (§3). Nothing in H2/H3 may build on a gate, metric,
or identity that H1 has not made honest.

| ID | Milestone (detail in ws file) | Exit criterion (machine-checkable) | Owner |
|---|---|---|---|
| H1-1 | **M4 typed outcome** lands: `Evaluated::Measured{subjects_seen: NonZeroUsize}` / `Withheld`; `ABSENCE_POLICY` deleted (WS-08) | `grep -rn 'ABSENCE_POLICY' src/ \| wc -l` = 0; seeded empty-corpus fixture red-then-green in CI; `honesty_ratio` metric replaced by measured-share | Architecture |
| H1-2 | **Rule engine + fixture mandate**: new-check registration requires `Rule::fixture()`; registry refuses a rule without a red/green pair (WS-08, WS-12) | `gates-without-proof = 0` held by a test that deletes a fixture and asserts registration fails | Test infrastructure |
| H1-3 | **Proxy-gate drain schedule** locked: #59 census scripted and ratcheted (WS-08) | census runs in CI; in-process-diff-string gate count strictly decreases per quarter or the ratchet test fails | Architecture |
| H1-4 | **Evidence provenance**: every gate reads an ephemeral worktree at the certified head; per-repo write lock (WS-09; issues #149, #151) | seeded wrong-head fixture: certification refuses; concurrency test: two PRs, zero cross-writes | Implementation |
| H1-5 | **Untrusted-input seam**: one typed prompt seam for contributor text (PR #196 direction), `doc_guard` fence killed (WS-10; #192) | injection corpus (fence-escape, marker-forgery, narrative attacks) red on old code, green on new; meta-test: no `format!` builds a model prompt outside the seam | Security |
| H1-6 | **Machine identity**: Anvil runs as a GitHub App; loop-guard decides on principal, not string match (WS-11; #171) | positive predicate, not a name comparison: the daemon's authenticated principal is of type App (`gh api user --jq .type` = `Bot`, installation id resolves) and is disjoint from every human reviewer principal on the repo; seeded Jason-comment fixture reaches the fixer, seeded self-comment fixture does not. Stated as identity *type* rather than `≠ jason931225` deliberately — a negative string predicate is the exact defect WS-11 exists to delete, and it must not survive in the criterion that closes it | Security |
| H1-7 | **Instrument discipline**: scanner rules 1–3 meta-guard over `tests/`; path-keyed-read drain begins (WS-12; #179) | meta-guard red on a seeded raw-text scan; path-keyed read count ratcheted strictly downward from **342** (re-measured 2026-08-31, `grep -rn 'src/[a-z_]*\.rs"' tests/*.rs \| wc -l`; #179 measured 332 one day earlier — the class is still being written) | Test infrastructure |
| H1-8 | **Restructure Phase A+B**: kernel extraction (4 hub PRs), workspace split, serialization points removed (WS-01) | `src/lib.rs` no longer the single declaration point (per-capability `mod` decls); `[workspace]` in root manifest; suite green with counts before/after each PR | Architecture |
| H1-9 | **Ladder rung 0 made real**: dev ruleset requires 1 human approval; approve/auto-merge under machine identity only after human review recorded (WS-06) | effective state, not a ruleset id: every ruleset applying to `dev` (`gh api repos/oyatie/anvil/rulesets --jq` filtered by ref, then each ruleset's `pull_request` rule) yields `required_approving_review_count ≥ 1`; a probe PR merged with zero human approvals is refused; decision registry row per merge. Ruleset **21064279** is today's carrier — named as evidence, never as the predicate, because an id is an ephemeral handle and the criterion must survive its replacement | Human ticket queue |
| H1-10 | **Decision registry + cockpit ticket MVP** (WS-07): every human-authority decision is a ticket with an evidence packet; #19 is decided through it as the pilot | registry append-only file exists; a tier/decision change without a ticket reference fails a test; #19 closed via a registry row | Architecture (build, per WS-07); Human ticket queue (decisions through it) |
| H1-11 | **Promotion fabric unbroken** (WS-13): promotion PRs open under App token | last 5 `promotion-open-next` runs on dev conclude `success` (`gh run list`) | Builder tools |
| H1-12 | **Merge-train rehearsal** as a required check (postmortem RC-5; WS-05) | seeded cross-PR type-conflict pair: rehearsal red while both PRs green in isolation | Test infrastructure |
| H1-13 | **Shape enforcement on self**: the two `advisory-until-infra` self-exemptions retired after Phase B (WS-02 inward half) | `.anvil/shape.json` has zero `advisory-until-infra` rules; shape gate red on a seeded dump-root | Architecture |
| H1-14 | **Flake lifecycle with real inputs** (WS-05; #53, #191): quarantine fed by measured CI outcomes, not literals; wall-clock tests de-flaked | seeded flaky test enters and exits quarantine by policy; `tests::flaky_test` literal gone; #191 tests pass under contention harness | Test infrastructure |

**H1 non-goals:** no Buck2 (H2 per interview); no agent merge authority above rung 1; no managed-repo
rewriting beyond report-only shape measurement (oyatie/console get findings, not pushes); no new
gates without fixtures; no model-lifecycle work beyond pinning.

### H2 — 6–18 months (2027-03 .. 2028-02): graphs, codemods, first earned autonomy

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H2-1 | **Phase C capability migration**: 86+ dependency-free modules into capability layout (WS-01) | shape gate enforcing (`unit_missing_face` blocking) with zero exemptions; **5** parallel move-PRs (default pin; registry row at start) land in one week without a serialization conflict (measured on the queue) | Architecture |
| H2-2 | **Buck2 dual build** (interview: after Phase C starts landing): reindeer-vendored `third-party/`, BUCK generated from `cargo metadata`, drift merge-blocking (WS-03) | `buck2 build //...` green weekly; seeded BUCK/manifest drift fails the drift gate | Builder tools |
| H2-3 | **Code graph service**: SCIP index + blast-radius query for the fleet (WS-04; issue #15) | blast-radius answers for a seeded cross-module change match ground truth on a 20-case corpus; graph freshness ≤ 1 commit behind dev | Architecture |
| H2-4 | **Codemod capability** with typed I/O + first real LSC executed on a managed repo via codemod-majority/agent-tail (WS-02) | LSC report: ≥80% of edits mechanical, tail enumerated, ratchet holds (reintroduction unwritable); before/after recipe tests in CI | Implementation |
| H2-5 | **Target-keyed certification pilot**: gates scoped by impacted targets (btd/supertd-style) instead of whole-repo (WS-03) | for a seeded leaf-only change, gate set shrinks accordingly and the skip list is emitted as an auditable artifact | Builder tools |
| H2-6 | **Autonomy rung 2**: auto-merge lanes for change classes (docs-only, lockfile bumps, generated code) under canary + auto-revert; promotion via evidence ticket (WS-06) | Cedar policy encodes the lane; seeded out-of-class PR is denied pre-action; ≥30-day incident-free window recorded in registry before enabling | Human ticket queue |
| H2-7 | **Post-merge loop on self**: canary analysis + health-keyed auto-revert for anvil's own daemon deploys (WS-13; research gap 1) | seeded bad deploy auto-reverts within the drill budget; drill logged quarterly | Release |
| H2-8 | **Supply chain**: SLSA-lane provenance, sigstore attestation of receipts, agent-config (skills/MCP/instructions) treated as supply chain (WS-15) | receipts verified by an independent verifier job; seeded unsigned receipt refused; agent-config change requires review lane | Security |
| H2-9 | **Harness eval suite**: pinned seeded-defect corpus re-run on every model/prompt change (WS-16) | model bump PR must attach eval delta; regression past threshold blocks the bump | Test infrastructure |
| H2-10 | **Fleet conformance engine v1**: shape + doc-parity + harness-instruction conformance measured repeatedly on every managed repo (WS-14) | conformance report per repo per week, generated by the same entrypoints anvil runs on itself; drift opens a ticket automatically | Docs |

**H2 non-goals:** no agent-decided sandbox/allow-deny policy changes (R4 per WS-06); no whole-fleet LSC
fan-out (one repo first); Cargo remains merge path (ADR-0716 mirror); no oyatie absorption event.

### H3 — 18 months+ (2028-03 onward): the fully agentic end-state, rung by rung

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H3-1 | **Build graph as verification authority**: Buck2 target graph decides what is built/tested/certified; Cargo remains the crates-io-facing manifest; merge authority stays with queue+policy (per 2026-08-31 decision, ledger A-5) | 100% of merge-blocking gates keyed to impacted targets; whole-repo fallback only on graph-invalidation, logged | Builder tools |
| H3-2 | **Autonomy rung 3**: general code changes auto-merge on dev behind full certification + rehearsal + post-merge canary; human is exception-handler via cockpit | ≥90 days rung-**2.5** incident-free in registry (the ladder, including R2.5's own 60-day R2 prerequisite, is normative in WS-06 — no rung is skipped); every rung-3 merge carries a replayable evidence packet; demotion drill passes | Human ticket queue |
| H3-3 | **Agentic policy plane**: sandbox policy, allow/deny (destructive actions always tiered highest) proposed by agents, evaluated deterministically pre-action, deny-by-default; Jason holds constitution + veto via ticket | policy change itself passes through the decision registry with a dry-run diff of allowed/denied actions; seeded destructive action denied at every tier below the top | Security (mechanism); adoption via Human ticket queue |
| H3-4 | **Fleet-scale LSC**: managed repos held in one uniform hyperscaler shape by standing ratchets; new repo onboarding is a codemod run + spec, not a project | onboarding drill: fresh repo reaches conformance report zero-findings in ≤1 week, no hand edits outside the tail queue | Implementation |
| H3-5 | **Promotion rungs agent-driven**: dev→staging→canary→production advanced by agents on health evidence; rollback rehearsed | promotion latency and rollback drills measured; a seeded failed-canary halts promotion without a human in the loop | Release |
| H3-6 | **oyatie absorption event**: anvil's capabilities merge into oyatie's tree/build/intelligence per the then-ratified oyatie layout | absorption PR series lands with oyatie's own presubmit green; anvil repo archived or reduced to a shim — decided by ticket | Human ticket queue |

**H3 non-goals:** agents never ratify their own tier promotions (Jason does, per interview); no rung
skipped regardless of model capability (ADR-0003: autonomy is a property of the task and its
verification cost, not of the agent); `main` stays a human gate (ruleset `main-not-agent-merge`)
until a registry decision retires it.

---

## 3. Workstreams and the malpractice census

Every malpractice class found in Phase 0 has a workstream, a ratchet, and an H1 slot — none is
deferred, none gets a bespoke N+1 gate. Two workstreams (WS-15, WS-16) cover **research-identified
gaps rather than Phase-0 findings** and start in H2 per ledger A-8; they are labeled as such below.
Reviews, tests, build graph, code graph, and codemod are **named capabilities with typed,
machine-readable I/O contracts** (each ws file specifies its contract).

| WS | File | Covers (findings) | Ratchet that makes the next instance unwritable |
|---|---|---|---|
| WS-01 | [ws-01-inward-convergence.md](ws-01-inward-convergence.md) | restructure phases; serialization points; pilot absorption; oyatie findings upstream | shape gate on self; lib.rs declaration-point test; oyatie-discrepancy ticket duty |
| WS-02 | [ws-02-outward-uniformity.md](ws-02-outward-uniformity.md) | managed-repo shape; codemod-first LSC | per-tenant `.anvil/shape.json` + baseline-block-on-new; LSC ground-holding ratchet |
| WS-03 | [ws-03-build-graph.md](ws-03-build-graph.md) | hermeticity; Buck2 dual build; target selection | BUCK↔cargo-metadata drift gate; hermeticity seeded-leak test |
| WS-04 | [ws-04-code-graph.md](ws-04-code-graph.md) | issue #15; blast radius; context scoping | graph-freshness check; blast-radius ground-truth corpus |
| WS-05 | [ws-05-review-test-fabric.md](ws-05-review-test-fabric.md) | #52, #191, RC-5; review/test typed I/O | merge-train rehearsal required; door tests behavioural; flake SLA |
| WS-06 | [ws-06-autonomy-ladder.md](ws-06-autonomy-ladder.md) | unenforced rung 0; trust ratchet to agent merge authority | Cedar pre-action deny-by-default; promotion only via evidence ticket; demotion drill |
| WS-07 | [ws-07-developer-platform.md](ws-07-developer-platform.md) | cockpit as ticket surface; decision registry | decision without ticket-ref fails a test; registry append-only |
| WS-08 | [ws-08-gate-honesty.md](ws-08-gate-honesty.md) | #59 proxy gates; #19/RC-2 absent evidence; #53 fabricated inputs | `Evaluated` type; fixture-or-no-registration; proxy-census ratchet; fabricated-literal meta-test |
| WS-09 | [ws-09-evidence-provenance.md](ws-09-evidence-provenance.md) | #149, #151 shared-clone/wrong-head | `SubjectRoot`-typed reads; wrong-head refusal test; per-repo lock |
| WS-10 | [ws-10-untrusted-input.md](ws-10-untrusted-input.md) | #192, RC-3 | `ContributorSupplied<T>`/`Untrusted` seam; injection corpus in CI |
| WS-11 | [ws-11-actor-identity.md](ws-11-actor-identity.md) | #171; credential brokering | machine identity; no string-match identity predicates (meta-test) |
| WS-12 | [ws-12-instrument-verification.md](ws-12-instrument-verification.md) | #179, RC-1, RC-6; #52's scan-only coverage | scanner meta-guard; path-keyed-read ratchet (342 ↓, 2026-08-31 baseline); instrument seeded both directions |
| WS-13 | [ws-13-promotion-topology.md](ws-13-promotion-topology.md) | promotion 0/13 red; main 45/300 divergence; 315-branch sprawl | standing-red tripwire; branch TTL ratchet; post-merge loop |
| WS-14 | [ws-14-fleet-conformance.md](ws-14-fleet-conformance.md) | ADR-0005/0006 drift; harness-instruction uniformity | doc-parity fail-closed; pointer-liveness check; conformance re-run weekly per repo |
| WS-15 | [ws-15-supply-chain.md](ws-15-supply-chain.md) | research gap 2 (unsigned receipts today are honestly `NotMeasured`, not a hidden defect) — H2 start | signature-required verifier; config-change review lane |
| WS-16 | [ws-16-harness-eval.md](ws-16-harness-eval.md) | research gap 7 (model-change risk) — H2 start | pinned eval corpus gates model/prompt bumps |

Issue map (no finding left unscheduled): #15→WS-04 · #19→WS-08+WS-07 · #52→WS-05+WS-12 · #53→WS-08 (timers on fabricated input) + WS-05 (the flake-lifecycle half, H1-14) ·
#59→WS-08 · #149→WS-09 · #151→WS-09 · #171→WS-11 · #179→WS-12 · #191→WS-05 · #192→WS-10.
#198→WS-14 (its pointer-liveness seed; filed by this plan's WS-01 duty) ·
oyatie-side findings (§1.4) → WS-01 upstream-ticket duty. Doctrine/ruleset gap → WS-06. Promotion
red, main divergence, sprawl → WS-13. ADR drift → WS-14.

---

## 4. Metrics that can't be gamed

Every metric names the predicate it measures; a proxy is never the thing. Every *check* behind a
metric is trusted only after a seeded defect has made it fail (and the seed is asserted to have
applied — a silently no-op'd seed makes a broken check look sound).

| Metric | Predicate measured | Anti-gaming proof |
|---|---|---|
| Measured share | fraction of gates returning `Evaluated::Measured{subjects_seen ≥ 1}` on real PRs | seeded empty corpus must produce `Withheld`, not `Measured` |
| Proxy-gate count | #59 census (external tooling invoked vs in-process string decision), scripted | census red-teamed with a seeded fake `Command::new` in a comment — must not count |
| Gates without proof | rules registered without a red/green fixture pair | deleting a fixture must fail registration in CI |
| Admissibility reachability | count of real PRs admitted per week by `admission_refusal()` | a week of zero on nonzero merges pages WS-08 (metric watches the watcher) |
| Trunk health | broken-dev minutes/week; queue p50/p95 wait | rehearsal must catch a seeded cross-PR conflict pre-merge |
| LSC mechanical share | codemod-authored edits / total edits per LSC, from typed LSC report | tail edits carry agent attribution; unattributed edits fail the report schema |
| Autonomy evidence | incident-free days per rung; demotions; evidence packets per promotion | promotion without a registry ticket is unrepresentable (schema-refused) |
| Conformance drift | findings per managed repo per week from the same entrypoints anvil runs on itself | a rule anvil enforces outward but not inward "does not compile" (doctrine §5 mechanism) |
| Instrument honesty | scanners reading `code_only`; refusal-on-ambiguity count | every scanner seeded in both directions before trust (postmortem RC-1 rules) |

---

## 5. Risks and tripwires

| Risk | Tripwire (observable signal) | Contingency |
|---|---|---|
| Model change regresses the harness | eval-suite score drops past threshold on a model/prompt bump (WS-16) | block the bump; pin previous model; freeze tier promotions until green |
| Merge queue starves at agent PR volume | queue p95 wait > 60 min for 7 consecutive days | activate scope-aware lanes / speculative batching (research: WS-05/03); cap fleet concurrency |
| Ratchet erosion | a baseline file changes without a signoff row in the same diff | merge-blocking refusal + cockpit ticket; postmortem row |
| oyatie layout law moves | oyatie `docs/ADR-INDEX.md` (the canonical index — `docs/decisions/` holds only a redirect stub `INDEX.md`; the previously named `docs/decisions/ADR-INDEX.md` does not exist, caught by external review) or the `docs/decisions/` ADR-07xx set changes (watched weekly by WS-14) | reconcile anvil shape spec within one sprint; file upstream finding if the change contradicts D-8 |
| Identity migration stalls | `promotion-open-next` still red 30 days after H1-11 date, or daemon still authenticates as `jason931225` at H1-6 date | escalate ticket; **block all tier promotions** (identity is a prerequisite for every rung) |
| Review verdicts socially engineered | injection/SEVRA-style corpus failure, or a verdict cites no executed evidence | demote review verdict to advisory until reseeded; incident row in registry |
| Standing red normalizes | any required or scheduled workflow red > 7 days on dev | auto-ticket with owner; weekly report counts standing reds (target 0) |
| The machine itself is the defect | kill-switch drill fails, or authorizer/queue/codemod misbehaves in drill | big-red-button: halt queue, daemon, codemod fan-out; quarterly drill is the tripwire's own proof |
| Sprawl regrows | remote branch count rises above ratchet baseline | TTL sweep job + ratchet test red |
| Absorption window slips | oyatie consolidation (its own purge) unfinished at H3-6 review date | absorption re-scoped by ticket; anvil continues standalone; no silent drift of the inward target |

---

## 6. Assumptions ledger

| # | Assumption / answer | Source |
|---|---|---|
| A-1 | Inward target = oyatie live law + measured majority, **as reference frame for hyperscaler best practice** — oyatie has not completed its own consolidation/purge, so conformance is to the pattern, and oyatie discrepancies are filed upstream | Interview 2026-08-31, Q1 (answered, with correction) |
| A-2 | Ticket surface = **Anvil cockpit**; GitHub issues remain the interim surface until the cockpit queue ships (H1-10) | Interview Q2 (answered); interim default taken |
| A-3 | Tier promotions ratified by **Jason via evidence ticket**; Cedar encodes tiers; deterministic pre-action check enforces | Interview Q3 (answered = recommended default) |
| A-4 | Buck2 dual-build lands **H2, after capability migration begins** | Interview Q4 (answered) |
| A-5 | North-star build authority: Buck2 graph is the **verification substrate** (what is built/tested/certified); Cargo stays the crates-io-facing manifest; merge authority lives in queue + policy, not a build tool | Mid-session answer, 2026-08-31 |
| A-6 | Fleet scope = the three watched repos (`oyatie/oyatie`, `oyatie/console`, `oyatie/anvil`); onboarding more is an H3 drill | Default taken (measured `WATCHED_REPOS` default; ADR-0002) |
| A-7 | Anvil 1.0 versioning question (restructure plan "Open") is decided through the decision registry, not assumed here | Default taken |
| A-8 | Research is one round; the critic's 8 gaps are scheduled (WS-13/15/11/16 absorb 4; the rest sit in the research backlog, §8) rather than silently claimed covered | Default taken (iteration cap) |
| A-9 | This plan schedules work; it changes no behaviour itself. Planning commission = findings become front-loaded workstreams, not mid-planning fixes | Commission |
| A-10 | Durable doctrine (dev-not-main, nextest, prove-a-check, verify-oyatie, MSRV≠channel, green≠merge-authority) rides in `CLAUDE.md` at repo root per the commission's housekeeping note | Commission housekeeping |

## 7. Decision log

| Date | Decision | Why |
|---|---|---|
| 2026-08-31 | Plan drafted against dev @ `6128284`; measured numbers only, commands cited | Commission Phase 0 |
| 2026-08-31 | Interview held in one batch (4 questions, defaults attached); answers in ledger A-1..A-4 | Commission Phase 1 |
| 2026-08-31 | Buck2 = verification authority at H3, not merge authority (A-5) | User question mid-session; consistent with oyatie ADR-0716/0700 and Meta practice (research §build-graph) |
| 2026-08-31 | Malpractice classes all placed in H1 with ratchets; postmortem's "diagnosed correctly, kept patching instances" named as the anti-pattern this ordering prevents | Hard constraint (no deferral) |
| 2026-08-31 | **Adversarial review iteration 1** (fresh-context reviewer): 5 violations, 7 advisories. V1 `&SubjectRoot` scanner-signature milestone scheduled H2 in WS-12 vs H1 in WS-09 while §1.3 claimed it front-loaded → front-loaded as shared H1-7d in both. V2 contradictory measured claims on `promotion-predecessor` requiredness → re-checked ruleset 21064983: it IS required; WS-13 corrected, the workflow's stale "advisory" PR-body prose reclassified as a doc-drift instance for WS-14. V3 roadmap H3-2 skipped rung R2.5 → exit criterion rebound to R2.5, WS-06 ladder declared normative. V4 §1.3/ws-08/ws-12 numbers without commands → re-measured what is cheap (path-keyed reads 332→**342**, registered rules 4→**5**, `honesty_ratio` definition site cited) and marked day-old in-repo census numbers (#59, RC-2) as carried-not-re-executed with H1-3 as the permanent fix. V5 ws-07 H3 exit not machine-checkable → decidable predicate with pinned thresholds. Advisories A1–A5, A7 applied (WS-15/16 labeled research-gap H2 starts; R0 keyed; parameters pinned with defaults; #53 map reconciled; denominators 73/72/64 reconciled; upstream-duty owner named); A6 resolved by a milestone-ID addressing convention in §2 | Process loop, iteration 1 |
| 2026-08-31 | **Adversarial review iteration 2** (fresh reviewer, no knowledge of iteration 1): 4 violations, 6 advisories — all cross-file consistency. V1 stale 332 baseline in roadmap §2/§3 vs 342 in WS-12 → 342 everywhere with the command. V2 WS-02 declared "outward only" while owning the inward H1-13 self-application milestone → the inward exception is now labeled in WS-02's header as a doctrine-§5 precondition, not a conflation. V3 automated merged-branch deletion at H1 contradicted WS-06's destructive-highest-tier law → H1-11d rewritten report-and-ticket (deletions only on ratified registry ticket until R4; GitHub's delete-on-merge is a human-flipped setting). V4 H3-3 owner Security vs Human ticket queue → split: Security owns the mechanism, every policy adoption is ratified via ticket. Advisories applied: sprawl commands inline in WS-13; the weekly oyatie ADR-INDEX watch named as WS-14's H1 mechanism behind the §5 tripwire; WS-05 H3 drill pinned (quarterly, seeded high-risk PR, 100%-logged selections); honesty_ratio pinned at ≈0.02 with source in both places; R1 activation given milestone WS06-H1b; research.md citation-window claim independently re-verified by the reviewer | Process loop, iteration 2 |
| 2026-08-31 | **Adversarial review iteration 3** (final under the 3-iteration cap): 2 violations, 6 advisories. V1 two stale "332" strings survived iteration 2's fix inside WS-12 itself (§class description, §Non-goals) → corrected to the 342 baseline with attribution to #179's 2026-08-30 figure. V2 the "68 entries" drift claim in WS-14 carried no artifact → pinned to `tests/brand_absence_gate_test.rs:29` with the grep. Advisories applied: §1.4 markdown command spelled out in full; WS-01 upstream-duty denominator scoped to discrepancy rows; H3-3 owner cell carries the mechanism/adoption split; R4 gets a 180-day R3 window default pin; WS-10 template-provenance wording research-anchored; H2 non-goal parenthetical aligned to R4. **Cap status, flagged per the process rule:** the cap is reached with the two iteration-3 violations *fixed in place after the review* — the fixes are single-string substitutions verifiable by `grep -rnw '332' docs/plan/*.md` and `grep -rn '68 entries' docs/plan/*.md` (expected: only attributed historical mentions) but have not themselves been re-reviewed by a fourth fresh-context pass. **Instrument note:** the unanchored form first used here (`grep -rn '332\|68 entries' docs/plan/`) also matches arXiv ids such as `2603.14332` inside `research.md`, so it over-reports — a check whose output would be read as a pass must be able to express only what it claims, and this one could not (RC-6, caught in the same external review) | Process loop, iteration 3 — cap reached |
| 2026-08-31 | **External review (dispatched `/code-review` agent, multi-finder + adversarial verification): 15 confirmed findings, all addressed this revision.** The five that matter most, each an instance of a class this plan itself polices: (1) ws-13's branch ratchet cited `git branch -r \| grep -c '^origin/'`, which always returns 0 (bare `git branch -r` indents lines) — instrument replaced with the `--format` form, baseline re-pinned 167, and the ratchet now requires its own seeded proof; (2) ws-12's 342 "path-keyed reads" are path *literals* — one real `read_to_string("src/` site remains and `tests/path_keyed_source_read_ratchet_test.rs` already bans the class at baseline — milestone re-scoped to proving the existing ratchet, the literal census demoted to a signal (#179's headline number was a proxy; this plan trusted it); (3) §1.4's "8 roots break the face grammar" was the wrong set and denominator — strict census pinned at `1119e99`: 9/21 full quartet, 12/21 partial, D-8's closed-children table identified as the live predicate, correction posted to oyatie#2341; (4) §1.3 described `is_admissible()`'s retired empty-unmeasured mechanism as current — rewritten against `report.rs:538`/`admission.rs:79` as read at `6128284`; (5) ws-14's inward watch named `docs/decisions/ADR-INDEX.md`, which does not exist (canonical: `docs/ADR-INDEX.md`) — a dead pointer inside the pointer-liveness milestone, now that check's second seed. Also fixed: restructure-plan status note (stale census 109→115, D5 channel==MSRV erratum, dead pilot path → `~/Developer/intelligence`); ws-09's false "TrunkRev exists" (designed only, `git grep` = 0 in src); §1.3's citation of a never-existed symbol (real: `PreMergeGuard::evaluate_pre_merge_gates`, `evaluator.rs:110`); `intelligence/` `k8s/` claim now measured against D-8's table (comment added to oyatie#2340); ws-01 duty text reconciled with its own filing record; CLAUDE.md↔ADR-0002 merge-authority contradiction bridged (enlister behavior named as documented defect #171, not authorization); research.md ~700-crate erratum (measured: 471 manifests at `1119e99`); "doctrine" attributions split between `docs/doctrine.md` §-cites and `CLAUDE.md`/ADR-0002 law; #198 added to §3's issue map; `(empty)`→fatal annotation; H1-10 owner aligned; axum de-listed from the security-relevant pair. Not adopted: none — every finding verified before fixing (two reviewer numbers were themselves off by the same-day drift they flagged: origin count 167 not 166/168 under the fixed instrument; one `read_to_string("src/` site, not zero) | External review of PR #197 |
| 2026-08-31 | **External review's cut list worked through** (the reviewer confirmed 13 more findings but capped its report at 15; leaving them unscheduled would be the deferral this plan forbids). Three were structural and are now fixed as classes, not instances: (1) **22 milestone IDs are duplicated** between this file and `ws-*` files — measured, and the cause of roughly half of every finding across four review passes; §2 now declares the `ws-*` row normative and this file's tables an index, with `WS14-H1b` carrying the drift check that makes the next divergence unwritable. (2) **`docs/plan/` is outside the amendable perimeter** — `corpus_sync::OWNED` is README + doctrine + openapi (+ADR dirs) at `src/doc_guard/corpus_sync.rs:23`, so this plan's 73/72/64 gate-count claims would go stale in silence while owned pages get corrected; `WS14-H1b` brings the plan inside the perimeter, proven by seeding a `TOTAL_GATES` change. (3) **Two exit criteria were keyed to ephemeral or self-contradicting predicates** — H1-9 to ruleset id `21064279` (an id is a handle, not a state: now effective-state over every ruleset applying to `dev`, id kept as evidence) and H1-6 to `≠ jason931225`, a negative string predicate living inside the milestone that exists to delete negative string predicates (now identity *type* plus disjointness from human principals). Also: the iteration-3 verification grep over-matched arXiv ids — corrected in place as an RC-6 instance. **Measured and dismissed:** the "19 pages lack `hyperscaler.doc.v1` frontmatter" finding — `FrontmatterValidator::validate_doc_frontmatter` (`src/doc_guard/frontmatter.rs:53`) makes frontmatter mandatory only under `docs/adr/` and `docs/decisions/`, and `docs/doctrine.md` — an *owned* page — carries none either, so `docs/plan/` is conformant as written; recorded here so it is not re-raised | External review, cut list |
| 2026-09-01 | **The execution prompt failed its first adversarial review: 19 findings, 16 verified, 3 fatal.** It was added in the last commit before merge and had received no review pass of any kind — the newest and most operationally dangerous file in the plan, unreviewed. The fatal three: (1) the `ACTIVE MILESTONE: H1-<n>` slot could name only **8 of 43** H1 rows across the `ws-*` files (`grep -cE '^\| *H1-[0-9]+ *\|'` = 8 vs 43 H1-shaped rows) — the lettered and `WS<nn>-H1<x>` ids this plan itself introduced were unaddressable; (2) `IN-SCOPE PATHS` — the entire authorization boundary — was derived by the agent from a plan that contains no such field (`grep -i in-scope` over the `ws-*` files = 0), and §7 then audited the diff against the list the agent had invented, so **the scope check could not fail**; (3) §5 ordered the agent to paste `Depends-on` verbatim, and `depends-on` appears nowhere in the plan (`grep -i` = 0), so every run stalled before the first edit. Also verified: `cargo run` boots the production daemon (`src/cli/handlers.rs:14`, `unwrap_or(Commands::Serve)`) under an unqualified `cargo` grant; the DONE commands were weaker than CI's (no `--locked`, no `--all-targets`, no `--profile ci`); the mandated baseline made **live billable model API calls** (`test_live_*` gated only on `agy` being installed); the anti-self-certification mechanism was the one claim exempt from the evidence rule, so a pasted verdict was indistinguishable from prose; the reviewer loop had no round cap; and the example target named a re-export site (`report.rs:4`) rather than the definition (`status.rs:14`) — the exact error the same sentence warns against. Rewritten as revision 2 in a follow-up PR; the in-scope list is now supplied from outside or the run refuses to start. **Recorded rather than quietly fixed because the failure is the plan's own thesis turned on its author:** grading must be taken away from the implementer, and revision 1's scope check was a check whose input the checked party supplied | Adversarial review of the execution prompt |
| 2026-09-01 | **Revision 4: the prompt was cut, not patched again.** Three revisions in, the pattern was the finding: each was written by the author of the one before it, and each introduced defects of the class it had just fixed — rev 2 made a precondition that halted every run and an allowlist that refused its own §0 command; rev 3's verb census, added to stop that recurring, was a tautology (its regex's third branch copied the grant list, so seeding four ungranted verbs — three of them writing — produced byte-identical output), and its two fixes for `PROPOSE` were written against each other, leaving it both a hard refusal and a documented workflow. Rev 3 also reached **541 lines / ~185 standing obligations** against a ~150–200 compliance budget, with the stop triggers at 93% depth and "do not push" on the last line — so the controls that survived review were the ones least likely to survive a skim. Rev 4 is **250 lines** with all ten controls retained and the push prohibition at **5%**; justification moved to `h1-execution-prompt-evidence.md`. `PROPOSE` is deleted rather than repaired: the boundary is human-supplied or the run refuses to start. The refusal list now names the class ("no form of any granted tool that writes, deletes, or executes") instead of enumerating flags, which closes `sort -o` and `find -execdir` without a fourth round of enumeration. Every command re-pre-flighted at `65f71fd` — 8/43 H1 rows, `depends-on` exit 1, 115 `pub mod`, 0 `[[test]]`, `--error-unmatch` proven to fail on a bad path — and one instrument bug found and fixed in the process (`paste -sd+` needs a trailing `-` for stdin on macOS) | Cut, after the third review |
| 2026-09-01 | **Execution prompt revision 3 — and three corrections to the revision-2 row above, which is left standing because this log amends by adding.** (a) That row's "`depends-on` ... appears nowhere in the plan (`grep -i` = 0)" is now falsified by its own text: `git grep -ic 'depends-on' -- 'docs/plan/*'` returns 3 (this log once, the prompt twice). Only the scoped `git grep -ic 'depends-on' -- 'docs/plan/ws-*.md'` still exits 1, and that is the form the prompt now carries — an unscoped search that matches its own statement is the RC-6 class, same as iteration 3's arXiv over-match. (b) "19 findings, 16 verified, 3 fatal" cited no artifact; the review is [PR #201](https://github.com/oyatie/anvil/pull/201) and those counts are that PR's, not independently re-derived. (c) "the in-scope list is now supplied from outside or the run refuses to start" overstated a fix that shipped with a `PROPOSE` hatch, under which the agent authors the very list §7 audits it against — the circularity revision 2 existed to remove. Revision 3 keeps `PROPOSE` but requires the proposal to be pasted back **by a human, in a human turn**, with the agent's first resumed message quoting the run block and naming that turn; proceeding on its own proposal is a stop. **Revision 2 itself failed review: it fixed 14 of revision 1's 19 findings and introduced 11, four blocking.** All four re-executed at `origin/dev` @ `65f71fd`: (1) §0 and §7 both gated on `git status --porcelain` being empty, which is unsatisfiable — `.claude/` and `devtree/` are permanently untracked and `git check-ignore -v .claude devtree` exits 1 — so no run could start and none could finish; replaced by `--untracked-files=no` plus a path-scoped form, each shown passing on the real tree and shown expressing a failure. (2) The closed verb allowlist ("everything else is refused, including by omission") omitted `git cat-file`, ordered by its own §0; `grep`, ordered by §3.3 and §9.5; and `git branch`, ordered by §3.5 — the document refused its own preconditions. The allowlist is now a declared superset, with a census command that lists every verb the file names so the next omission is visible rather than latent. (3) "`H1-13` spans `ws-01`/`ws-02`" was false: `grep -rnE '^\| *H1-13b? *\|' docs/plan/ws-*.md` puts both id cells in `ws-02`, `ws-01`'s two hits are prose, and the real two-owner case is `H1-7d` (`ws-09`:25, `ws-12`:39) — ownership is now defined as the id cell, with the grep beside it. (4) "Phases A/B/C (H1-8a/b/c)" was two-thirds wrong against `docs/restructure-plan.md` §Sequence: Phase B is the workspace split, which is `H1-8c`; `H1-8b` is the serialization fix the Sequence names as Phase C's *blocker*; Phase C has no `H1-8` twin. Also fixed: the live-test filter `-E 'not test(/^test_live_/)'` was anchor-fragile — nextest matches `test()` against `module::name`, so a live test nested in a `mod` would rejoin the run and make billable API calls — now `-E 'not binary(subscription_driver_live_test)'`, with the run required to prove the exclusion by diffing two `cargo nextest list` outputs (1843 tests listed at the pin, 1840 under the filter, the three removed being exactly the `test_live_*` set); the anchoring hazard is demonstrable on tests already in the tree — `-E 'test(/^test_frontier_/)'` lists nothing while `-E 'test(/test_frontier_/)'` lists `ai_driver::router::tests::test_frontier_defaults`, so `^` never matches a module-qualified name; §7 claimed its commands were CI's "verbatim" while adding a flag — fmt and clippy are byte-identical to `build-and-test.yml:28,58`, and the nextest `-E` is now declared as the single deviation with its reason (CI runners have no `agy`, local machines may); bare `cargo fmt` de-granted, since `cargo fmt --help` says it formats every bin and lib file of the crate, i.e. outside `IN-SCOPE PATHS` — replaced by `cargo fmt --all -- --check` for verification and `rustfmt --edition 2024 <path>` for writing, the edition flag load-bearing (`rustfmt --check src/cli/handlers.rs` exits 1 without it, 0 with it, on a clean tree); the refusal predicate now catches an unedited `<...>` placeholder, which is neither blank nor `PROPOSE`; "two consecutive clean reviewer rounds" restored after revision 2 silently dropped it, leaving one clean round sufficient, and the three-round cap reconciled with §8's two-attempt budget, which now explicitly runs through fix attempts inside a round; DONE states the suite runs on the committed bytes, detected by an unchanged `git rev-parse HEAD` across the DONE sequence; §1's imperative "Write `src/<module>/**` and `tests/<file>.rs`" demoted to a layout description, because `H1-13`'s exit criterion is `.anvil/shape.json` — tracked (`git ls-tree -r origin/dev .anvil/`) and outside both; and every published count now carries the command that produced it, with revision 1's "verify before relying on it" restored | Adversarial review of execution prompt revision 2, PR #201 |
| 2026-08-31 | **WS-01 upstream duty executed** (first standing duty activated post-draft): oyatie discrepancies re-verified at oyatie dev `1119e99`, then filed — [oyatie#2339](https://github.com/oyatie/oyatie/issues/2339) (registry disposition), [oyatie#2340](https://github.com/oyatie/oyatie/issues/2340) (cap-root `contracts/` vs D-8), [oyatie#2341](https://github.com/oyatie/oyatie/issues/2341) (face-grammar classification request), [anvil#198](https://github.com/oyatie/anvil/issues/198) (ADR-0006 dead citation; WS-14 seed). URLs recorded in ws-01; two §1.4 rows deliberately not filed (self-recorded in oyatie's own files — reasons in ws-01) | WS-01 duty |

## 8. Research backlog (from the completeness critic, scheduled not silently absorbed)

1. Post-merge production loop depth (canary analysis, auto-revert) → WS-13 (H2-7, H3-5).
2. Supply chain / SLSA / sigstore / agent-config supply chain → WS-15.
3. Non-human identity & credential brokering (SPIFFE-style) → WS-11 H2 scope.
4. VCS substrate / working-copy provisioning at fleet scale (EdenFS/Jujutsu) → research round 2 before H2-1 parallel-move week.
5. Rust-native certification (crater-style runs, cargo-semver-checks, MSRV) → WS-15/WS-08 gate additions, each with fixture.
6. ML predictive test selection economics → WS-03 after H2-5 (btd first, probabilistic layer later).
7. Harness eval / model lifecycle → WS-16 (H2-9).
8. Operational governance of the automation itself (kill switches, per-actor rate limits) → WS-06/§5 kill-switch drill.
