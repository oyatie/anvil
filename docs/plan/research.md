# Roadmap research: hyperscaler monorepo practice and agentic-harness state of the art

**Date:** 2026-08-31. **Method:** eight parallel fresh-context research agents (one per topic) plus one
completeness critic, run as a dynamic workflow; every citation carries a publication or last-updated date
the researching agent verified on the source, and the consolidated set was mechanically re-checked:
**90 citations, all dated 2026-03-01..2026-08-31; 0 outside the window** (schema-enforced pattern plus a
post-hoc regex sweep). Sources older than six months were discarded per commission; where a foundational
older work matters, a 2026 treatment of it is cited instead.

**How to read this:** each finding names a practice, who runs it, the concrete mechanism, and the
implication for Anvil. Implications are inputs to `anvil-roadmap.md`; the roadmap, not this file, decides
sequencing.


## Build graph and hermeticity at hyperscalers and in open source, state of 2026: Bazel/Buck2, remote execution, CAS caching, target-level CI, hermetic merge-blocking CI, and Cargo<->Buck2 hybrids for Rust monorepos

By mid-2026 the field has consolidated around a stable stack: Bazel 9 LTS is bzlmod-only and governed by the Linux Foundation's BUILD Foundation, while Meta ships Buck2 on a biweekly open-source cadence with reindeer (Cargo->Buck2 translation) and buck2-change-detector (btd/supertd target determination) under daily active development through August 2026. The frontier moved from "have a remote cache" to sub-blob efficiency (content-defined chunking landed in Bazel 8.7/9.1+ and BuildBuddy, cutting transfers ~40%) and resilience (action rewinding on cache eviction, remote repo contents caches for hermetic external deps). The most Anvil-relevant shift: vendors and researchers now treat build/test infrastructure explicitly as the validation substrate for AI coding agents — BuildBuddy ships snapshot-warmed remote runners plus an MCP endpoint for agents, TDAD shows deterministic source-to-test dependency maps cut agent regressions 70% where TDD prompting alone made things worse, and SWE-CI benchmarks agents on longitudinal CI-loop maintenance rather than one-shot fixes. Merge-queue telemetry (Mergify, 200k merges) quantifies the economics: broken-main scales 16x with team size, batching+bisection is underused, and AI-assisted PRs already break main less than half as often as human ones — direct evidence for Anvil's agents-with-merge-authority endgame, gated by deterministic machinery.


### Bzlmod-only Bazel 9 LTS under neutral foundation governance

**Who:** Google and the Bazel community; BUILD Foundation operating as a Linux Foundation Directed Fund


**Mechanism:** Bazel 9.0 (Jan 2026) removed WORKSPACE entirely: external dependencies resolve solely through MODULE.bazel + the Bazel Central Registry (modernized UI at bcr.stack.build), giving one canonical, lockfile-backed channel for third-party code. The BUILD Foundation, formalized in 2026, funds the community roadmap, docs, and rulesets; the 9.x line ships quarterly minors (9.1.0 April, 9.2.0 July).


**Anvil implication:** The ecosystem Anvil would interoperate with is now governance-stable and single-format for dependency policy. If Anvil ever adds Bazel repos to its fleet, deny-by-default dependency gates can key off MODULE.bazel/BCR alone; more broadly, it confirms that policy-as-code over a declared dependency manifest (not scanned imports) is the industry-standard enforcement point Anvil's gates should mirror for Cargo/Buck2.


**Sources:** [Bazel Q1 2026 Community Update](https://blog.bazel.build/2026/04/08/bazel-q1-2026-community-update.html) (Bazel Blog (Google/BUILD Foundation), 2026-04-08)


### Content-defined chunking and action rewinding in the remote CAS

**Who:** BuildBuddy (vendor, shipped in production) and Bazel core (flags in 8.7.0/9.1+, parallel chunk transfers in 9.2.0)


**Mechanism:** The remote cache splits large blobs (linker outputs, archives) into content-defined chunks instead of monolithic digests; only novel chunks transfer. BuildBuddy reports 40% less data uploaded, a 40% smaller disk cache, and 300 TiB of duplicate uploads skipped in two weeks (vendor-reported numbers). Bazel exposes this as --experimental_remote_cache_chunking (8.7.0, 2026-05-07) with parallel chunk transfers in 9.2.0 (2026-07-13); --rewind_lost_inputs lets actions re-run to recover from cache evictions instead of failing the build, and an experimental remote repo contents cache extends CAS coverage to repository rules (external dep fetches).


**Anvil implication:** State-of-the-art CAS is sub-artifact and eviction-resilient. Anvil's cache layer for the ~700-crate oyatie monorepo should key on chunked content, not whole rlib/binary artifacts, and its gates must tolerate cache eviction via rewind-style recovery rather than treating a lost blob as a red gate — otherwise cache churn becomes false certification failures.


**Sources:** [Remote Cache CDC: Reusing Bytes](https://www.buildbuddy.io/blog/content-defined-chunking) (BuildBuddy (vendor engineering blog), 2026-05-01); [Releases · bazelbuild/bazel (8.7.0 and 9.2.0 release notes)](https://github.com/bazelbuild/bazel/releases) (Bazel project, GitHub, 2026-07-13)


### Target determination as the CI front door (btd/supertd)

**Who:** Meta, open-sourced as facebookincubator/buck2-change-detector; bazel-contrib/target-determinator is the Bazel-side equivalent


**Mechanism:** Three binaries: `targets` dumps the unconfigured Buck2 target graph at base and changed revisions; `btd` diffs the two graphs plus the changed-file list to compute the impacted target set, with depth ranking so CI can trim how far down the reverse-dependency cone it builds; `supertd` unifies them. CI builds/tests only that set instead of `buck2 build ...`. Actively developed through 2026-08-28 (most recent commit adds a version-delta pipeline step as a supertd subcommand).


**Anvil implication:** This is the exact primitive Anvil's 73-gate certification matrix needs for oyatie: run gates per impacted target, make the merge-blocking set = btd output, and log the graph diff so every skipped gate is deterministically auditable. Anvil should wrap supertd rather than reimplement change detection, and treat the impacted-target set as the unit of certification.


**Sources:** [facebookincubator/buck2-change-detector (commit history, active through late August 2026)](https://github.com/facebookincubator/buck2-change-detector/commits/main) (Meta, GitHub, 2026-08-28)


### Cargo->Buck2 dependency translation as checked-in build rules (reindeer)

**Who:** Meta (facebookincubator/reindeer), used for Rust third-party deps in the Meta monorepo and by external Buck2+Rust monorepos; Buck2 itself ships biweekly dated open-source releases


**Mechanism:** reindeer buckify translates the transitive Cargo.toml graph into generated BUCK rules committed to source control (vendored or non-vendored); at build time Cargo is absent and Buck2 invokes rustc directly, with fixups handling build scripts. The repo shows daily maintenance through 2026-08-27: platform symlink management, feature resolution and public-alias handling, and a continuous stream of crate version bumps — i.e., Meta keeps the Cargo<->Buck2 bridge production-grade rather than legacy. Buck2 releases on a ~biweekly YYYY-MM-DD cadence (latest visible 2026-08-22).


**Anvil implication:** oyatie's Buck2+Cargo hybrid is on the supported upstream path, not a dead end. Anvil should automate `reindeer buckify` as a codemod pipeline and add a drift gate: BUCK files must be a deterministic function of Cargo.lock, with divergence merge-blocking. The checked-in-rules model also suits Anvil's determinism thesis — agents edit Cargo.toml, machinery regenerates BUCK, gates verify the fixpoint.


**Sources:** [facebookincubator/reindeer (commit history, active through late August 2026)](https://github.com/facebookincubator/reindeer/commits/main) (Meta, GitHub, 2026-08-27); [facebook/buck2 releases (biweekly dated releases)](https://github.com/facebook/buck2/releases) (Meta, GitHub, 2026-08-22)


### Snapshot-warmed remote runners as the validation substrate for coding agents

**Who:** BuildBuddy (vendor; 'Remote Bazel' product) — marketing-adjacent but mechanism-specific


**Mechanism:** Thesis stated outright: 'the bottleneck has shifted from writing code to validating code written by agents.' The CLI mirrors local git state including uncommitted changes to cloud runners colocated (sub-ms) with the cache; runners are snapshotted and cloned after each build so every agent iteration starts with a warm Bazel analysis cache; parallel agents each get their own runner, avoiding output-base conflicts; cross-platform via --os/--arch flags. A companion MCP endpoint lets agents query build/test metadata, logs, artifacts, and target statuses read-only.


**Anvil implication:** Anvil's core premise is now a commercial product category, which validates the direction and sets the baseline. The roadmap should give each reviewer/fixer agent an isolated, snapshot-warmed execution environment (analysis-cache reuse is the key latency win for iterative gate re-runs) and expose gate results/build metadata to agents over a typed machine interface (MCP-shaped), not log scraping.


**Sources:** [Remote Bazel: The missing piece for AI coding agents](https://www.buildbuddy.io/blog/remote-bazel-with-agents) (BuildBuddy (vendor engineering blog), 2026-03-02)


### Speculative, scoped merge queues with measured broken-main economics

**Who:** Mergify (vendor telemetry report across 477 organizations, ~200k merges, rolling 90-day window); GitHub merge queue as the commodity baseline


**Mechanism:** Queues test each batch against the projected state of main after its dependencies merge (speculation); on batch failure, bisection finds the culprit in O(log n) full CI runs; scoped parallel queues partition a monorepo so unrelated areas do not serialize. Measured data (vendor-collected, mark accordingly): broken-main rises from 0.77% at 2-5 engineers to 12.5% at 40+; only 6% of private merges use batching despite 50-75% CI-cost cuts; bot PRs clear in ~0.4 min vs ~12 for humans; AI-assisted PRs are 14.4% of private merges and break main at 1.9% vs 4.4% for non-AI PRs.


**Anvil implication:** Anvil's merge-queue engine should ship batch+bisect and per-crate-cluster scoped queues (scope derivable from the btd impact set — a combination nobody has published yet). The AI-PR data is early external evidence for the merge-authority endgame: agent-authored changes already break main less than human ones when gated, so Anvil can frame agent merge authority as risk-reducing, with broken-main rate as the tracked safety metric.


**Sources:** [State of Merge Queues 2026](https://mergify.com/reports/state-of-merge-queues-2026) (Mergify (vendor report; self-collected telemetry), 2026-07-27); [The Merge Queue Is the New Bottleneck](https://tianpan.co/blog/2026-07-02-the-merge-queue-is-the-new-bottleneck) (TianPan.co (independent engineering blog), 2026-07-02)


### Longitudinal CI-loop benchmarking of agent maintainership (SWE-CI)

**Who:** Academic (Chen, Xu, Wei, Chen, Zhao; arXiv preprint)


**Mechanism:** 100 real-world repository tasks, each spanning ~233 days of development history and ~71 consecutive commits; agents are evaluated through iterated CI loops on how functional correctness evolves over dozens of analysis/coding rounds, rather than one-shot patch correctness. Explicitly reframes evaluation from static short-term correctness to dynamic long-term maintainability.


**Anvil implication:** Anvil's graduation criteria for granting agents merge authority should be longitudinal, SWE-CI-shaped: track an agent's gate-pass and regression trajectory across sequences of merges into oyatie, not per-PR review accuracy. Anvil's fleet history is effectively a private SWE-CI; instrumenting it turns the merge-authority decision into a measured ratchet instead of a judgment call.


**Sources:** [SWE-CI: Evaluating Agent Capabilities in Maintaining Codebases via Continuous Integration](https://arxiv.org/abs/2603.03823) (arXiv (v1 2026-03-04, v4 2026-04-01), 2026-04-01)


### Deterministic source-to-test impact maps as an agent tool, not a prompt (TDAD)

**Who:** Academic (Alonso, Yovine, Braberman; arXiv preprint), evaluated with open-weight models (Qwen3-Coder 30B class) on consumer hardware


**Mechanism:** A pre-built dependency graph maps source code to the tests that exercise it; before finalizing a change the agent queries the map as a lightweight skill to select exactly the impacted tests and self-correct. Results: regression rate 6.08% -> 1.82% (70% reduction) and resolution rate 24% -> 32%; notably, giving the agent TDD instructions without the deterministic map made regressions worse than baseline (9.94%).


**Anvil implication:** Direct experimental support for Anvil's 'deterministic machinery steering LLM agents' thesis: a queryable graph beats prompting, and prompting alone can be net-negative. Anvil should hand reviewer/fixer agents the btd impact set and a source-to-test map as first-class tools, and treat 'agent ran the impacted tests before proposing' as a certifiable, gate-checkable precondition.


**Sources:** [TDAD: Test-Driven Agentic Development — Reducing Code Regressions in AI Coding Agents via Graph-Based Impact Analysis](https://arxiv.org/abs/2603.17973) (arXiv, 2026-03-18)


**Open questions:** No in-window primary source describes Google's or Meta's internal hermetic merge-blocking CI end-to-end (TAP / Meta's CI); the 2026 view is assembled from open-sourced components (btd, reindeer, Buck2 releases) and vendor writing — worth watching BazelCon 2026 (Oct 13-15, Amsterdam) talks for a primary treatment. · No published practice yet combines merge-queue scoping with target determination (queue scope = btd/target-determinator impact cone); Anvil building this would be ahead of the public state of the art — is that a differentiator worth the maintenance cost? · rules_rust maturity on Bazel 9 (bzlmod-only) was not verified inside the recency window; if Anvil ever weighs Bazel vs Buck2 for Rust, that needs a fresh check. · Remote-execution backend choice for a Buck2 fleet (BuildBarn vs BuildBuddy vs EngFlow vs NativeLink) has no 2026-window neutral benchmark; all comparative claims found were vendor-authored. · Mergify's AI-vs-human broken-main numbers (1.9% vs 4.4%) are vendor telemetry with unknown selection effects (teams using merge queues are already disciplined); independent replication would strengthen the merge-authority argument before Anvil cites it externally. · How Bazel's experimental remote repo contents cache and Buck2's approach to hermetic external fetches converge (or don't) on a shared standard for hermetic third-party ingestion remains unclear.


## Codemod-first large-scale-change (LSC) pipelines, state of 2026

By mid-2026 the field has converged on the pattern Anvil bet on: deterministic, compiler-aware transforms do the majority of a large-scale change, and LLM agents are reserved for the tail — authoring the transform, fixing residual breakage, and reviewing loop outputs. Codemod.com ships this as a persistent agent skill (npx codemod ai) so agents default to building a codemod instead of brute-forcing file edits; Moderne runs 10,000+ OpenRewrite recipes over Lossless Semantic Trees with Gartner explicitly endorsing rule-based-tools-called-by-AI over pure LLM refactoring; Anthropic's published migration playbook is a Rosie-shaped pipeline (rulebook, dependency-sharded mechanical work queue, cheap implementer agents, adversarial reviewer agents, compiler/test referees, "review loop results, not code"). Verification practice centers on before/after recipe tests, parity harnesses against the original code, and distilled human-editable playbooks that shrink LLM variance. The landing side is now the bottleneck: agent-inflated PR volume is forcing batched, tiered, flake-quarantining merge queues with agent rate limits. Research adds that static structure (call graphs, config deps) injected into agent context halves run-to-run variance, and that agent configurations themselves need a content-addressed, permission-tiered, audit-logged supply chain — directly validating Anvil's deny-by-default authorization direction.


### Rulebook-driven migration with a mechanical work queue and adversarial review agents (Rosie-shaped LSC, 2026 form)

**Who:** Anthropic (internal large-scale migrations run with Claude Code)


**Mechanism:** Six-step pipeline: (1) write a translation rulebook and dependency map so work shards into independent units; (2) a 'shakedown cruise' mini-migration validates the rules before scale-out; (3) a batch script — not an LLM — decides what is done by checking whether the translated file exists on disk and slices pending files into batches for cheap implementer agents, with larger 'adversarial reviewer' models flagging issues as TODO comments that become the next queue items; (4-6) fixer-agent loops drive compile, test, and behavioral-parity verification against the original code ('a built-in referee'), with a build daemon serializing expensive operations. Human review is front-loaded into the rulebook; afterwards the principle is 'review loop results, not code'.


**Anvil implication:** This is the closest public blueprint to what Anvil's codemod pipeline should become: the harness owns the deterministic work queue, sharding, and referees (compilers, parity harnesses, diffs), and gates — not humans — judge each shard. Anvil should make the rulebook a first-class reviewed artifact, add a mandatory shakedown-cruise stage before any fleet-wide codemod, and encode 'review loop results, not code' as the review model for thousand-file changes across oyatie's ~700 crates.


**Sources:** [How Anthropic runs large-scale code migrations with Claude Code](https://claude.com/blog/ai-code-migration) (Anthropic, 2026-07-16)


### Codemod-as-default agent skill: 'npx codemod ai'

**Who:** Codemod.com (open-source CLI + skill/MCP package for Claude Code, Cursor, OpenCode, and other harnesses)


**Mechanism:** One command installs (a) a persistent skill teaching the agent to recognize when a change warrants a codemod instead of file-by-file edits, (b) MCP tools for AST inspection, tree-sitter node-type lookup, and test execution inside the conversation, (c) a /codemod entry point, and (d) auto-updates. The agent authors a deterministic JSSG codemod, iterates against red/green before/after test feedback (their debarrel example: 58 test assertions across five re-export patterns), then runs the compiler-aware transform repo-wide in seconds. Vendor-authored post, but the mechanism is concrete and the tooling is open source.


**Anvil implication:** Anvil should adopt the same inversion for its own agents: a harness-level policy/skill that makes 'build a verified transform, then run it' the default for any change matching more than N sites, with the codemod's before/after test corpus becoming a typed certification gate. The pattern of exposing AST inspection and codemod test execution as deterministic tools to the agent maps directly onto Anvil's gate/tool architecture.


**Sources:** [npx codemod ai: Fast, Reliable Migrations for Coding Agents](https://codemod.com/blog/npx-codemod-ai) (Codemod.com, 2026-04-09)


### Deterministic recipe catalogs called by AI as tools over compiler-accurate code models (LST)

**Who:** Moderne / OpenRewrite (enterprise fintech and Java-estate customers; Gartner 2026 MQ Leader)


**Mechanism:** Code is parsed into Lossless Semantic Trees — compiler-accurate representations with resolved types and dependencies — and 10,000+ versioned, testable OpenRewrite recipes execute over them across thousands of repos at once; AI agents interpret intent and select/compose recipes rather than emitting edits, with every change tracked, auditable, and attributable. Gartner's 2026 assessment (quoted in a vendor post — treat framing as marketing, the Gartner quote as analyst finding): 'rule-based, deterministic tools used by an AI service can outperform a purely LLM-based approach in codebase refactoring due to gains in time and resources needed for verification and testing.'


**Anvil implication:** Independent analyst consensus now backs Anvil's core thesis: the expensive part of LLM refactoring is verification, and determinism amortizes it. For a Rust fleet, Anvil's equivalent of the LST is its build/code graph plus rustc/rust-analyzer semantics; the roadmap item is a versioned, tested, auditable recipe catalog (syn/ast-grep/rustfix-based) that agents invoke as typed tools, with recipe version pinned in the certification record.


**Sources:** [Gartner Names Moderne a Leader in the 2026 Magic Quadrant for AI-Augmented Code Modernization Tools](https://moderne.ai/blog/moderne-leader-gartner-magic-quadrant-ai-code-modernization) (Moderne, 2026-08-14)


### Structural search/rewrite primitives built for agents (ast-grep line, 2026)

**Who:** ast-grep (open-source, Rust; used as the AST layer by agent harnesses and codemod tooling)


**Mechanism:** 2026 releases target agent consumption directly: 0.43 (May) adds ESQuery-style structural selectors on the CLI and Markdown support; 'Outline' (June) provides a fast syntax-aware table of contents for source files 'without building an index', explicitly aimed at coding agents needing cheap structural orientation. In July the project shipped an AI-assisted migration of tree-sitter's C core to Rust (30% faster, compatibility preserved) — a live example of the LLM-for-the-tail pattern applied to systems code with deterministic compatibility/performance verification.


**Anvil implication:** ast-grep is the natural fast structural layer for Anvil's codemod pipeline (Rust-native, YAML rules, machine-checkable), sitting below full-semantic syn-based transforms; Outline-style structural orientation is cheap context Anvil's harness can feed agents deterministically. The tree-sitter C-to-Rust rewrite is also a directly citable precedent for agent-driven Rust migrations gated on parity benchmarks.


**Sources:** [ast-grep 0.43 - Search Code and Markdown with Structure](https://ast-grep.github.io/blog) (ast-grep, 2026-05-25); [Introducing ast-grep Outline](https://ast-grep.github.io/blog) (ast-grep, 2026-06-22); [Rewriting Tree-sitter's C Core in Rust: Migration and Compatibility](https://ast-grep.github.io/blog) (ast-grep, 2026-07-21)


### AI-generated, human-editable migration playbooks that constrain planner variance

**Who:** AWS Migration & Modernization (production migration tooling)


**Mechanism:** A four-phase multi-agent pipeline distills prior migrations' artifacts (commit histories, diffs, error logs, decision rationales) into a readable playbook; frequency-based filtering emphasizes patterns seen across many migrations and deprioritizes one-offs. The playbook then constrains the planner's solution space — 'guided toward a narrower set of proven approaches rather than sampling freely' — yielding measured 4.93-15.79% consistency improvements across three LLM judges. Because playbooks are plain text (unlike vector stores), experts review and correct them, and corrections propagate to subsequent runs.


**Anvil implication:** Anvil should treat migration knowledge as a reviewed, versioned artifact: after each fleet codemod, a distillation pass turns the loop's logs and fix patterns into a playbook that seeds the next one, with the founder reviewing the playbook (cheap) instead of the diffs (expensive). This is the same 'front-load human review into the rulebook' move as Anthropic's, plus a concrete distillation mechanism and a variance metric to ratchet.


**Sources:** [Reproducible Code Migration at Scale with AI-Generated Playbooks](https://aws.amazon.com/blogs/migration-and-modernization/reproducible-code-migration-at-scale-with-ai-generated-playbooks/) (AWS, 2026-05-13)


### Merge-queue throughput engineering for agent-era change volume (batching, tiered CI, flake quarantine, agent rate limits)

**Who:** Industry practice synthesized from GitHub's merge-queue operations and agent-heavy teams (practitioner analysis)


**Mechanism:** With agents multiplying PR arrival rates, a serial queue at 30-minute CI lands ~2 PRs/hour and wait times explode nonlinearly. Documented countermeasures: batch multiple PRs per validation run (GitHub: ~5x throughput, time-to-ship cut by a third); tier tests into pre-queue (lint/typecheck/build), in-queue (cross-change integration), and post-merge (E2E, benchmarks) with fast rollback for post-merge failures; quarantine flaky tests so they report without blocking; and rate-limit agents to two queue entries per change before escalating to a human.


**Anvil implication:** Anvil's merge-queue engine is already the strategic asset; this names the specific mechanisms to build: batch speculation across the fleet, a typed split of the 73 gates into pre-queue/in-queue/post-merge tiers, first-class flake quarantine, and — critically for agent merge authority — a deterministic policy that an agent gets N queue attempts before the change is kicked to human triage. Post-merge gates require an automated revert path, which is itself a pre-action-authorized operation.


**Sources:** [The Merge Queue Is the New Bottleneck](https://tianpan.co/blog/2026-07-02-the-merge-queue-is-the-new-bottleneck) (TianPan.co (practitioner blog), 2026-07-02)


### Deterministic anchoring: injecting static code structure to make agent behavior reproducible

**Who:** Academic (ISSTA 2026 paper), evaluated on LLM repo-navigation agents


**Mechanism:** Lightweight static analysis emits stable structural facts — call graphs, inheritance hierarchies, configuration dependencies — injected as plain-text anchors that 'constrain probabilistic exploration'. Measured effect: +2.2pp function-level localization and 1.6 fewer interaction rounds, but the headline result is stability — run-to-run variance halved and single-run reliability +3.4pp; structure 'helps less by making agents smarter and more by making their navigation disciplined and reproducible'. Optimal anchor density varies with repo size (large projects benefit from sparse inverse-only links).


**Anvil implication:** Anvil already computes build and code graphs for gating; this is evidence they should also be piped into agent context as deterministic anchors, and that the payoff to measure is variance reduction, not raw accuracy. For codemod-tail work across oyatie's 700 crates, reproducible agent trajectories are what make certification gates meaningful — the same input should yield the same investigation.


**Sources:** [How Much Static Structure Do Code Agents Need? A Study of Deterministic Anchoring](https://arxiv.org/abs/2606.26979) (arXiv (accepted ISSTA 2026), 2026-06-25)


### Agent configuration as a managed, deny-by-default supply chain

**Who:** Academic (arXiv, June 2026), with the Rel(AI)Build reference implementation; prevalence study over 10,008 public GitHub repos


**Mechanism:** Study found agent configs propagate as undeclared shared components (10.1% of tracked paths exact duplicates across independent repos; 75.5% of clone pairs cross org boundaries; <1% declare permission boundaries; 58% never change after first commit). Rel(AI)Build enforces, before any LLM invocation: SHA-256 content addressing of agent definitions, HMAC-stamped lockfiles, hash-chained audit logs, tiered permissions with attack-derived blocklists, prompt-drift detection (Jaccard similarity), and a phase state machine with requirement-to-file-to-test traceability bounding what the agent may do.


**Anvil implication:** Direct external validation of Anvil's end-state: agents holding merge authority under deterministic deny-by-default pre-action authorization. The concrete additions for the roadmap: content-address and lockfile Anvil's own agent/skill/prompt definitions (they are supply chain, not config), hash-chain the authorization audit log, and add prompt-drift detection as a certification gate so the thing holding merge authority is itself verified — consistent with the 'verify oyatie too' principle.


**Sources:** [A Deterministic Control Plane for LLM Coding Agents](https://arxiv.org/abs/2606.26924) (arXiv, 2026-06-25)


**Open questions:** No public 2026-window primary source describes Google's Rosie/LSC pipeline as currently operated with LLM agents in the loop — the canonical descriptions (SWE-at-Google ch.22, Chromium LSC process docs, the warehouse-scale ISA-migration paper from Oct 2025) all predate the window; a current-state account of how Google shards/approves agent-generated LSCs remains unverified. · Codemod rollback practice is thinly documented: sources cover pre-merge verification (before/after recipe tests, parity harnesses, tiered merge-queue CI with 'fast rollback' named but not specified); no in-window source details automated revert of a landed thousand-file change (shard-level revert vs. inverse-transform vs. full rollback), which Anvil will need to design largely from first principles. · OpenRewrite's ecosystem is Java-centric; whether the LST-style compiler-accurate transform layer exists or is emerging for Rust (beyond ast-grep's syntactic layer and rustfix) needs deeper investigation — Anvil may have to build the Rust recipe catalog itself. · Moderne's agent-facing tools (Prethink, Trigrep, local MCP server) surfaced in search results but their detailed mechanism could not be tied to an in-window dated primary page. · Quantified field data on the deterministic-majority/LLM-tail split ratio (what fraction of sites the deterministic transform covers in practice, and tail cost per site) is not published by any 2026-window source; Anvil should instrument and publish its own.


## Merge queues and trunk health, state of 2026: speculative batching, bisection, flake handling, scope-aware lanes, stack-aware queueing, and hyperscaler submit-queue patterns

In 2026 the merged-state rehearsal (GitHub's merge_group speculative merge commit) is table stakes, but GitHub's native queue remains serial-ish, flake-blind, and scope-unaware; the vendor tier (Mergify, Trunk, Graphite) differentiates on speculative parallel batching with bisect-on-failure, build-graph-derived parallel lanes (Bazel/Nx/Buck impacted targets mapped to per-lane speculative budgets), queue-integrated flake quarantine with governance (thresholds, tickets, TTLs), and stack-aware landing through GitHub's async Merge API. Uber open-sourced SubmitQueue (Aug 2026), putting a production speculation-tree engine with build-target conflict analysis and ML-ordered speculation in the public domain — the closest public artifact to Google TAP / Meta land-queue internals, which still lack in-window first-party 2026 publications. The discourse framing shifted: with AI agents inflating PR volume (and defect density ~1.7x), the merge queue, not review, is named the binding constraint on shipping. For Anvil: run all 73 gates on the merge-group SHA, shard the queue by Buck2 affected-target overlap, type flake-quarantine as a first-class gate outcome, treat stacks as atomic landing units, and tier gates presubmit/queue/post-merge — the TAP flow rebuilt deterministically.


### Merged-state rehearsal via merge_group speculative merge commits

**Who:** GitHub native merge queue (GA), used by GitHub's own monorepo and broadly across OSS/industry


**Mechanism:** Queueing a PR creates a temporary gh-readonly-queue branch combining target branch + all PRs ahead + this PR; required checks re-run against that speculative merged state via the merge_group workflow trigger (checks keyed only to pull_request never run in the queue); target fast-forwards only on pass. Batches up to a configured group size (default ~5); on failure the queue ejects the culprit and rebuilds speculative commits for everything behind it. Known limit: GitHub can rebuild the merge commit after CI, so the merged SHA is not always the SHA CI signed off on.


**Anvil implication:** Anvil's 73-gate certification matrix must execute against the merge-group SHA (the MERGED state), never the branch head — any gate wired only to pull_request events silently tests the wrong state, a fleet-wide 'verify the instrument' failure. Anvil should also assert tested-SHA == merged-SHA as its own gate, since GitHub does not guarantee it.


**Sources:** [GitHub Merge Queue in 2026: How It Works & Handling Flaky Required Status Checks](https://tenki.cloud/blog/github-merge-queue-setup) (Tenki (last modified date verified on page), 2026-04-16); [When to Outgrow GitHub's Merge Queue](https://mergify.com/blog/when-to-outgrow-github-merge-queue) (Mergify (vendor blog — marketing-adjacent but mechanism-specific), 2026-06-24)


### Speculative parallel batching with automatic bisection-on-failure

**Who:** Mergify (speculative/parallel checks), Trunk.io, Graphite merge queues; absent-by-default in GitHub native


**Mechanism:** The queue builds N draft combinations (PR1, PR1+2, PR1+2+3, ...) and runs CI on them in parallel; a passing prefix merges as a batch, and a failing batch is binary-searched by re-running CI on subsets to isolate the culprit, which is ejected while the rest continue — avoiding head-of-line stalls and full-queue restarts. Mergify additionally skips CI on intermediate batches whose superset already passed (skip_intermediate_results) and reports the CI runs saved.


**Anvil implication:** Anvil's merge-queue engine needs batch+bisect as the baseline algorithm, but its typed gates change the economics: a typed gate failure often names the offending change directly (per-crate, per-target attribution), letting Anvil replace CI-time binary search with deterministic attribution and reserve bisection for genuinely ambiguous failures.


**Sources:** [When to Outgrow GitHub's Merge Queue](https://mergify.com/blog/when-to-outgrow-github-merge-queue) (Mergify (vendor), 2026-06-24); [GitHub Merge Queue in 2026: How It Works & Handling Flaky Required Status Checks](https://tenki.cloud/blog/github-merge-queue-setup) (Tenki, 2026-04-16)


### Scope-aware parallel lanes from build-graph impacted targets (queue-aware path/occupancy scheduling)

**Who:** Mergify (dynamic scopes with per-lane speculative budgets), Trunk.io (parallel queues from Bazel/Nx/Gradle impacted targets)


**Mechanism:** The build tool emits the set of targets a PR actually impacts; the queue partitions PRs into independent lanes whose target sets are disjoint, and each lane batches, speculates, and merges concurrently. Mergify's 2026-08-25 change adds scopes.default_capacity: a single speculative-check budget applied to every dynamically generated scope, with each lane running its own speculative checks independently under a global max_parallel_checks cap. Trunk surfaces missing impacted-target metadata directly on the PR (June 2026) because the scheduling degrades without it.


**Anvil implication:** This is the queue-aware occupancy scheduling item, and Anvil is unusually well-placed: oyatie's Buck2 target graph already yields affected-target sets, so the queue can shard by target-set overlap natively rather than via vendor integration. It also operationalizes the 'measure overlap before claiming disjoint' memory — disjointness must be computed from the build graph per pair of queued changes, and missing impact metadata must fail closed into the serial lane.


**Sources:** [Per-lane check capacity for dynamic scopes](https://docs.mergify.com/changelog/2026-08-25-per-lane-check-capacity-for-dynamic-scopes/) (Mergify docs changelog, 2026-08-25); [Trunk changelog (impacted-targets tooltip on PR details)](https://docs.trunk.io/changelog) (Trunk.io docs changelog, 2026-06-11)


### Queue-integrated flake quarantine as a governed, first-class state

**Who:** Trunk.io Flaky Tests (quarantine on by default since June 2026), Mergify test-quarantine guidance; GitHub native queue has no flake concept


**Mechanism:** Queues amplify flakes: a test failing 5% of the time has a 5% chance per cycle of ejecting a good PR and cascading speculative rebuilds behind it. The 2026 pattern: detect nondeterministic failure above a threshold (~2% of executions, configurable monitors), auto-quarantine — the test still runs and records results but leaves the required-check set — and govern the quarantine with an auto-filed ticket (Linear/Jira/Asana), a named owner, and a 2-4 week TTL after which the test is fixed or formally retired. Trunk added distinct 'broken' vs 'flaky' classification (March 2026) and pass-on-retry monitors.


**Anvil implication:** Flakiness must be a typed gate outcome in Anvil (pass / fail / flaky-quarantined), not a binary — exactly the missing distinction behind the 'anvil metrics need M4' memory. Quarantine policy belongs in policy-as-code: threshold, owner, and TTL as a ratchet that expires, so quarantine cannot become a permanent bypass of deny-by-default gating.


**Sources:** [Test Quarantine: Stop Flaky Tests From Blocking Merges](https://mergify.com/learn/test-quarantine) (Mergify (vendor learn page; updated date verified on page), 2026-05-19); [Trunk changelog (quarantine default-on; broken-vs-flaky classification; threshold monitors)](https://docs.trunk.io/changelog) (Trunk.io docs changelog, 2026-06-08); [GitHub Merge Queue in 2026: How It Works & Handling Flaky Required Status Checks](https://tenki.cloud/blog/github-merge-queue-setup) (Tenki, 2026-04-16)


### The merge queue as the binding constraint in the AI-agent era (theory-of-constraints framing)

**Who:** Industry analysis (TianPan.co, July 2026), citing GitHub's monorepo and Uber data


**Mechanism:** A strictly ordered queue with 30-minute merge-group CI lands at most ~2 PRs/hour (~48/day); batching five PRs per run buys ~5x throughput. GitHub's monorepo pushes ~2,500 PRs/month through its queue, with dynamic merge groups cutting time-to-ship by a third. Failure probability inside the queue is rising: AI-co-authored PRs carry roughly 1.7x more issues than human-only ones, and Uber's iOS mainline was green only 52% of the time before dedicated flake management (~1,000 flaky tests among 600,000).


**Anvil implication:** As Anvil's agents multiply PR volume across the fleet, queue throughput — batch size vs. per-change failure probability vs. CI latency — becomes the roadmap's binding constraint before merge authority does. Anvil should model queue capacity explicitly (expected merges/day as a function of gate latency, batch size, and gate failure rate) and treat it as a tracked SLO, since agent-authored changes empirically fail more often inside the queue.


**Sources:** [The Merge Queue Is the New Bottleneck](https://tianpan.co/blog/2026-07-02-the-merge-queue-is-the-new-bottleneck) (TianPan.co, 2026-07-02)


### Speculation-tree submit queues with build-graph conflict analysis, now open source (hyperscaler pattern made public)

**Who:** Uber (SubmitQueue, open-sourced August 2026; gates all landings across Uber's monorepos); discussed on HN alongside Google Piper/TAP and Meta internals, and Airbnb's internal 'Evergreen' equivalent


**Mechanism:** SubmitQueue speculatively rebases and validates many pending changes in parallel against predicted future states of HEAD; a conflict analyzer declares two changes independent when their build-target sets are disjoint, pruning the speculation tree so independent changes validate concurrently; ML-predicted change-success probability and build time order the speculation. Written in Go with fake/git/GitHub provider modes and a full request log. HN discussion (Aug 2026) notes it predates GitHub's queue, originated partly in Uber's AV division where builds ran ~6 hours, and that Google/Meta equivalents remain unpublished.


**Anvil implication:** A production-grade, auditable reference implementation of the hyperscaler submit-queue pattern now exists to study or fork — including the exact conflict-analysis-over-build-targets mechanism Anvil would build on Buck2. Its request log recording the full trail matches Anvil's deterministic pre-action authorization needs; its ML ordering layer is the one component Anvil should replace with deterministic gate-cost heuristics first.


**Sources:** [Uber SubmitQueue: a high-performance speculative merge queue (HN discussion of the open-sourcing)](https://news.ycombinator.com/item?id=49138084) (Hacker News (date verified as ~21 days before 2026-08-31), 2026-08-10)


### Stack-aware queueing as a mainstream queue capability

**Who:** Mergify (GitHub-native stacked PRs, Aug 2026) and Trunk.io (native stack testing/merging, Jul 2026); Graphite pioneered it earlier


**Mechanism:** The queue treats a stack as one ordered unit: queueing any member evaluates queue rules and CI on the stack, then lands the whole stack in order through GitHub's asynchronous Merge API. Concrete constraint Mergify documents: the queue bot's bypass mode on repository rulesets must be set to Exempt — the only bypass mode GitHub's Merge API honors — for sequenced landing to work. Trunk shipped native testing and merging of entire PR stacks together on 2026-07-31.


**Anvil implication:** Agent-produced work (codemod pipelines, large-scale changes sharded per-crate) naturally arrives as stacks of small typed changes; Anvil's queue must land stacks atomically and in order, not as N independent PRs racing each other. The Exempt-bypass requirement is a design warning: GitHub ruleset bypass semantics will interact with Anvil's deny-by-default authorization layer and must be modeled explicitly rather than granted broadly.


**Sources:** [Merge GitHub stacked pull requests with the merge queue](https://docs.mergify.com/changelog/2026-08-12-merge-github-stacked-pull-requests-with-the-merge-queue/) (Mergify docs changelog, 2026-08-12); [Trunk changelog (stacked pull request support in Merge Queue)](https://docs.trunk.io/changelog) (Trunk.io docs changelog, 2026-07-31)


### Tiered verification: light presubmit always, heavy checks only in the merge group, broad suites post-merge (the 2026 rendering of Google's TAP presubmit-to-submit flow)

**Who:** Prescribed by Mergify and Tenki for teams outgrowing single-stage queues; descends from Google TAP's presubmit/postsubmit split (no in-window first-party Google source exists)


**Mechanism:** Two-stage CI: cheap, reliable checks run on every PR push; the expensive suite runs once per merge group inside the queue; slow or broad integration tests move post-merge and report asynchronously, with the required-check surface kept deliberately minimal so only fast deterministic checks can block the queue. This trades a small post-merge revert risk for order-of-magnitude queue throughput, mirroring TAP's model of fast presubmit (95%+ predictive of full-suite pass) plus asynchronous post-submit testing of all affected targets.


**Anvil implication:** Anvil should partition its 73 typed gates into three tiers by cost and determinism: presubmit gates (fast, always), merge-group gates (the certification matrix against the merged state), and post-merge ratchet gates (expensive fleet-wide invariants that trigger auto-revert rather than block the queue). The tier assignment itself belongs in policy-as-code so moving a gate between tiers is a reviewed, typed change.


**Sources:** [When to Outgrow GitHub's Merge Queue](https://mergify.com/blog/when-to-outgrow-github-merge-queue) (Mergify (vendor), 2026-06-24); [GitHub Merge Queue in 2026: How It Works & Handling Flaky Required Status Checks](https://tenki.cloud/blog/github-merge-queue-setup) (Tenki, 2026-04-16)


**Open questions:** No in-window (2026-03..2026-08) first-party Google TAP or Meta land-queue publication was found; the best 2026 treatment is secondary (HN discussion of Uber's open-sourcing noting Google/Meta internals remain unpublished). Graphite's canonical stack-aware-queue and Bors/TAP posts are 2024-2025 and were excluded by the recency rule. · TianPan's claim that GitHub's monorepo uses 'dynamic merge groups' cutting time-to-ship by a third could not be verified against a primary GitHub changelog entry in the window; worth confirming before citing in roadmap docs. · Uber's DPE Summit session ('Merge Queue at Uber Scale': ~53% CI resource reduction, 44% CPU, 37% P95 wait reduction) is listed under a 2027 summit page with no verifiable in-window date; numbers are promising but unverified. · The exact license and maintenance commitment of uber/submitqueue (fork-viability for Anvil) was not verified — the GitHub page fetch showed no release dates. · Whether any 2026 queue implements occupancy-aware scheduling beyond disjoint-lane parallelism (e.g. reordering by predicted gate cost within a lane) is unclear — Uber's ML-ordered speculation is the only observed instance.


## Ratchets and policy-as-code for engineering quality, and deterministic pre-action authorization for AI agents (state of March-August 2026)

By mid-2026 "deterministic pre-action authorization" is a named, converging field, not a metaphor. The arXiv line opened by Uchibeke's "Before the Tool Call" (Mar 2026, Open Agent Passport) defines the shape Anvil is already building: intercept every tool call synchronously, evaluate a declarative deny-by-default policy the agent cannot read or modify, emit a cryptographically signed audit record, and only then execute. Measured results are consistent across independent work: adversarial attack success drops from ~75% to 0% under deny-by-default, with decision latencies from sub-microsecond (Agent Control Protocol, TLA+-verified) to ~53 ms (OAP). Industry shipped the same architecture: AWS put Cedar (deny-by-default, forbid-wins) in front of Bedrock AgentCore Gateway tool calls with automatic CloudWatch decision logging; Microsoft's open-source Agent Governance Toolkit is a control plane between MCP clients and tool servers accepting Rego or Cedar policies with hash-chained audit logs; OPA-as-sidecar PDP fronting the tool gateway is the most-cited pattern. Ratchets remain the quality mechanism of record — Notion's eslint-seatbelt (per-file baseline counts that can only decrease, CI-enforced) is actively maintained in 2026 — and SIG showed in-loop deterministic quality gates strictly dominate post-hoc review for agent-written code. Anvil's direction is the consensus direction; its differentiator is applying it to merge authority.


### Open Agent Passport (OAP) — deterministic pre-action authorization spec

**Who:** Uchi Uchibeke; open Apache-2.0 specification with reference implementation, the anchor of the arXiv 'Deterministic Pre-Action Authorization' line (arXiv:2603.20953, submitted 2026-03-21)


**Mechanism:** A shim intercepts every agent tool call synchronously before execution, evaluates it against a declarative policy (spending limits, capability scoping, quality gates, operational contracts), and emits a cryptographically signed audit record; median enforcement overhead 53 ms (N=1,000). In a live adversarial testbed, deny-by-default policy yielded 0/879 successful adversarial actions versus 74.6% success under permissive policy — the enforcement layer, not model alignment, carries the safety property.


**Anvil implication:** Anvil's endgame (agents holding merge authority under deterministic deny-by-default pre-action authorization) now has a citable spec and an empirical result to benchmark against. Adopt OAP's decomposition — synchronous interception, declarative policy the agent cannot see or edit, signed per-decision audit record — as the contract for Anvil's authorization layer, and reproduce the permissive-vs-deny-by-default adversarial measurement against Anvil's own 73-gate matrix before granting any agent merge authority.


**Sources:** [Before the Tool Call: Deterministic Pre-Action Authorization for Autonomous AI Agents](https://arxiv.org/abs/2603.20953) (arXiv, 2026-03-21)


### Cedar-gated agent tool calls in Amazon Bedrock AgentCore Gateway

**Who:** AWS (first-party product engineering blog — vendor content, but describing shipped GA behavior)


**Mechanism:** Cedar policies evaluate every (principal, action=tool invocation, resource=gateway) triple with deny-by-default semantics: 'if no policy explicitly permits a request, it is denied', and forbid rules always beat permit rules, so restrictions compose safely on top of a baseline permit. Lambda REQUEST interceptors run before Cedar to do token exchange and context enrichment; RESPONSE interceptors redact outputs. Every Cedar allow/deny decision is automatically logged to CloudWatch with full context — audit is a property of the engine, not manual instrumentation.


**Anvil implication:** Cedar's semantics (default-deny, forbid-wins-over-permit, order-independent, side-effect-free, formally analyzable) are exactly what Anvil wants for merge-authority policy: gate definitions become permit rules, ratchets and freezes become forbid rules that provably override. The interceptor-then-policy pipeline maps onto Anvil's gate matrix (enrich context deterministically, then decide), and 'audit trail for free from the decision engine' should be a hard requirement of whatever engine Anvil embeds — cedar-policy is a Rust crate, so it embeds natively in Anvil's stack.


**Sources:** [Secure AI agents with Policy and Lambda interceptors in Amazon Bedrock AgentCore Gateway](https://aws.amazon.com/blogs/machine-learning/secure-ai-agents-with-policy-and-lambda-interceptors-in-amazon-bedrock-agentcore-gateway/) (AWS Machine Learning Blog, 2026-06-01)


### OPA-as-sidecar Policy Decision Point fronting the agent tool loop

**Who:** Tian Pan (TianPan.co engineering blog), documenting the most-cited 2026 deployment pattern for OPA/Rego in agent runtimes


**Mechanism:** The tool gateway is the Policy Enforcement Point; it queries an OPA sidecar (the PDP) before invoking any tool, and OPA answers in well under a millisecond. Rego's Datalog-like joins let policies combine request context with external data (e.g. principal.tenant == resource.tenant && actor in agent_class.support), distinguishing the human principal from the agent actor. Every decision is logged as discrete structured fields — agent identity, principal, tool name, data class, operation, decision, policy version, timestamp — streamed to SIEM. Core thesis: YAML manifests and model prompts lack enforcement authority; only a dedicated PDP has it.


**Anvil implication:** Validates Anvil's 'deterministic machinery steers LLM agents' thesis with the standard architecture vocabulary (PEP/PDP) reviewers and auditors already know. Two takeaways: (1) log policy VERSION with every decision — Anvil's gate results should be joinable to the exact policy commit that produced them; (2) model principal (Jason) and actor (agent) as separate identities in every decision from day one, since merge-authority handoff is precisely a change in which actor a policy permits under the same principal.


**Sources:** [Policy-as-Code for Agents: OPA, Rego, and the Decision Point Your Tool Loop Doesn't Have](https://tianpan.co/blog/2026-04-25-policy-as-code-agent-permissions-opa-rego) (TianPan.co, 2026-04-25)


### Microsoft Agent Governance Toolkit (AGT): a control plane between MCP clients and tool servers

**Who:** Microsoft (open-source toolkit, announced on Microsoft's developer blog; covers the OWASP Agentic Top 10)


**Mechanism:** A runtime layer that evaluates tool calls 'deterministically before every tool invocation' against declarative rules written in YAML, OPA/Rego, or Cedar; a four-tier privilege-ring model enforces least privilege; tool definitions are scanned for poisoned instructions and typosquatting before the model ever sees them; responses are inspected before returning to the agent. Agents get Ed25519 + ML-DSA-65 cryptographic identities with trust scoring, kill switches terminate non-compliant agents, and every tool-call attempt, policy decision, and execution outcome lands in append-only, hash-chained audit logs. SDKs include Rust.


**Anvil implication:** The strongest industrial template for Anvil's authorization layer: it shows the full control-plane inventory beyond allow/deny — pre-flight scanning of tool definitions, response inspection, per-agent cryptographic identity, revocation/kill-switch, and tamper-evident (hash-chained) decision logs. Anvil should treat 'hash-chained append-only decision log' as the audit standard for gate and merge decisions, and note that policy-language pluralism (YAML/Rego/Cedar behind one engine interface) is where the ecosystem landed — design Anvil's policy interface so the language is swappable.


**Sources:** [Securing MCP: A Control Plane for Agent Tool Execution](https://developer.microsoft.com/blog/securing-mcp-a-control-plane-for-agent-tool-execution/) (Microsoft Developer Blog, 2026-04-22)


### eslint-seatbelt: per-file baseline-and-ratchet files that only shrink

**Who:** Notion Engineering (open-sourced; npm package actively maintained, last published 2026-03-08)


**Mechanism:** A TSV baseline file records the allowed violation count per (file, lint rule) pair. Fixing a violation auto-decrements the allowed count via pre-commit hook; introducing a new violation fails CI. One line per file+rule means parallel work rarely merge-conflicts, and the baseline data feeds dashboards (Datadog/Notion DBs via GitHub Actions) so debt burn-down is observable, not just enforced. The invariant: counts move in one direction without requiring a dedicated migration project.


**Anvil implication:** This is the reference mechanism design for Anvil's ratchets, and it confirms two of Anvil's memory lessons at industrial scale: baselines must be per-unit (file/crate) not global (a global count lets debt shift silently between files — the 'measure overlap' failure mode), and auto-decrement matters (a stale baseline is a ratchet with slack). For the oyatie 700-crate monorepo: store per-crate baselines in a conflict-free line-per-key format, auto-tighten on every merge through the queue, and export ratchet state as metrics so M4 can distinguish 'ratchet holding' from 'ratchet slack'.


**Sources:** [eslint-seatbelt (npm) — version 0.1.3](https://www.npmjs.com/package/eslint-seatbelt) (npm registry / Notion, 2026-03-08)


### aiAuthZ: off-host, identity-bound authorization the agent cannot tamper with

**Who:** Sai Varun Kodathala (arXiv:2607.05518, submitted 2026-07-06)


**Mechanism:** Authorization runs on an external gateway, not the agent host, so a compromised or jailbroken agent cannot bypass or rewrite policy. Each message carries a per-message HMAC-SHA256 signature bound to a single-use nonce and timestamp window (identity binding + replay resistance); the gateway enforces role-based, argument-level policy 'that the agent can neither read nor modify'; every decision joins a SHA-256 hash-chained audit log; accepted actions get HMAC-authenticated receipts. Result: 0% attack success across 15 LLMs, ≤0.03 ms added latency.


**Anvil implication:** Argument-level policy is the key upgrade for Anvil: gating 'git push' is too coarse — the policy must bind to arguments (which remote, which branch, which SHA range), matching Anvil's merge-queue reality where 'push to dev after certification' and 'push to main' are different authorities. The off-host property translates to: Anvil's authorization engine must live outside the agent's writable filesystem/process (the agent must not be able to edit its own gate definitions — same class as 'verify the instrument'), and per-decision receipts give every merge a verifiable provenance token.


**Sources:** [aiAuthZ: Off-Host, Identity-Bound Authorization for AI Agents](https://arxiv.org/abs/2607.05518) (arXiv, 2026-07-06)


### Agent Control Protocol (ACP): temporal admission control with formally verified determinism

**Who:** Marcelo Fernandez, TraslaIA (arXiv:2603.18829, v1 2026-03-19, revised through 2026-04-30)


**Mechanism:** Extends pre-action authorization from stateless per-call checks to sequence-aware admission control: deterministic, history-aware risk scoring (static risk + anomaly accumulation + cooldowns, explicitly not ML anomaly detection) keyed by PatternKey(agentID, capability, resource) so signals never mix across contexts. In a test where 500 individually-valid requests formed a harmful pattern, stateless checking approved all 500; ACP admitted 2 (0.4%). Decisions in 739-832 ns p50 at 1.72M req/s, with TLA+ model checking showing zero invariant violations across 4.3 billion states.


**Anvil implication:** Two lessons for Anvil's roadmap. First, per-call gates miss sequence attacks: an agent making 500 individually-certifiable merges can still execute a harmful campaign, so the merge queue needs stateful policy — rate/cooldown/accumulation keyed per (agent, capability, repo). Second, ACP sets the verification bar: because the policy engine is deterministic, its invariants were checked in TLA+ — Anvil should aim to state its authorization invariants ('no merge without all 73 gates green AND explicit permit AND no standing forbid') formally, which is only possible because the machinery is deterministic. That is the concrete payoff of Anvil's determinism-first bet.


**Sources:** [Agent Control Protocol: Admission Control for Agent Actions](https://arxiv.org/abs/2603.18829) (arXiv, 2026-04-30)


### Sigrid Guardrails: in-loop deterministic quality gates during agent code generation

**Who:** Software Improvement Group (SIG) — vendor-run experiment marketing their Sigrid product; treat effect sizes as vendor-reported, but the design (10 guided vs 10 unguided builds of the same system) is stated


**Mechanism:** An MCP server makes the coding agent (Claude Sonnet 4.6) run quality and security analysis on the code it just changed, fix findings within the touched scope, and report back before proceeding — a gate inside the generation loop rather than a post-hoc review. Reported results over 20 builds: ~97% fewer high-risk security findings, ~24% better maintainability, and strict dominance — the worst guided build beat the best unguided build — plus fewer costly regeneration cycles.


**Anvil implication:** Evidence for moving some of Anvil's 73 gates from pre-merge (certification matrix) to in-loop (agent-facing MCP tools): the same deterministic checks, exposed to the agent during generation, cut rework and produce uniformly better output than gate-at-the-end. Architecture consequence: each Anvil gate should be callable in two modes — advisory in-loop (agent self-checks while working) and authoritative pre-merge (the ratchet that actually blocks) — with the authoritative run never trusting the in-loop run ('prove a check before trusting it').


**Sources:** [Claude Sonnet 4.6 code quality: we tested it with and without AI guardrails](https://www.softwareimprovementgroup.com/blog/claude-sonnet-4-6-guardrails-experiment/) (Software Improvement Group, 2026-07-03)


### Cryptographically bound, verifiable capability grants for agent tool use

**Who:** Ziling Zhou (arXiv:2603.14332, v1 2026-03-15, v2 2026-03-19), representative of the 2026 capability-token research wave


**Mechanism:** Formalizes 'capability-context separation' — the grant of a capability is cryptographically bound so it cannot drift from the context it was issued for — and derives three governance requirements: capability integrity, behavioral verifiability, interaction auditability. Two instantiations: a basic Ed25519+SHA-256 chain verifying in 97 microseconds, and a BBS+ selective-disclosure / Groth16 DV-SNARK variant at 13.8 ms, with formal theorems on verification-chain properties and reproducibility across models. The broader 2026 wave (Biscuit/macaroon-lineage tokens with offline attenuation) shares the core idea: a holder can only narrow a capability, never widen it, and every delegation is verifiable.


**Anvil implication:** Attenuation-only tokens are the ratchet principle applied to authority itself, and the natural credential format for Anvil's endgame: issue an agent a capability token scoped to (repo, branch, gate-set, time window); sub-agents receive only attenuated copies; the merge-queue verifies the chain offline in microseconds. This gives Anvil a cryptographic answer to 'green is not merge authority' — a passing matrix produces a signed certification, and merge authority is a separate, attenuable, revocable token that references it.


**Sources:** [Governing Dynamic Capabilities: Cryptographic Binding and Reproducibility Verification for AI Agent Tool Use](https://arxiv.org/abs/2603.14332) (arXiv, 2026-03-19)


**Open questions:** Which policy engine should Anvil embed for merge-authority decisions: cedar-policy (native Rust crate, formally analyzable, deny-by-default/forbid-wins semantics) or OPA via sidecar (richer data joins, mature decision-log pipeline)? The 2026 ecosystem uses both behind one interface (Microsoft AGT accepts Rego and Cedar), suggesting an abstraction layer, but that costs Cedar's analyzability. · Has the 'Deterministic Pre-Action Authorization' arXiv line (OAP, ACP, aiAuthZ — all single-author papers) produced a standards-track spec or major-lab adoption yet, or does industry consolidate on AgentCore Policy / AGT instead? Worth re-checking every quarter; Anvil should track OAP's Apache-2.0 spec for interop rather than inventing a wire format. · What is the state of the art for RATCHETING POLICY ITSELF — mechanisms where the permitted-action set for an agent can only shrink (or widen only via a human-signed, audited grant)? The capability-attenuation literature implies it, but no source found in the window describes a production 'authority ratchet' tied to agent track record; this may be greenfield for Anvil. · Independent replication of the vendor-reported in-loop guardrail effect (SIG's 97%/24% numbers) on Rust codebases specifically — clippy/miri/nextest in-loop vs pre-merge — before Anvil invests in exposing gates as agent-facing MCP tools. · How the 2026 argument-level policy work handles arguments that are themselves generated artifacts (a diff, a codemod script): hashing the artifact into the decision record (aiAuthZ receipts, Zhou's binding) is established, but semantic constraints on diffs ('touches only crate X') appear to still require Anvil-style typed gates, not generic policy engines.


## Agentic coding harness state of the art, mid-2026: harnesses, loop engineering, context engineering, sandboxing, verification, and spec-driven development — implications for Anvil's roadmap

By mid-2026 the field converged on a layered doctrine: kernel-level sandboxes with default-deny egress as the only real guarantee, model-based controls explicitly treated as probabilistic, and verification stacked in layers of increasing cost (fast critic, typed/static gates, execution-based QA agents, model judges, human review of high-risk verdicts only). Anthropic shipped and then defaulted-on classifier-based pre-action authorization (auto mode: tiered policy, 20+ static block rules, deny-and-continue) — but its 17% false-negative rate on overeager actions is the published gap Anvil's deterministic deny-by-default authorizer targets. Research moved from harness engineering to loop engineering: loop specifications (trigger/goal/verification/stopping-rule/memory), dependency-DAG benchmarks with standing regression obligations (best harness: 25% — long-horizon autonomy remains weak), and harness self-evolution gated by falsifiable per-edit predictions, which independently validates Anvil's falsification engine. Practice consensus: spec-driven pipelines (constitution→plan→tasks→implement), context files kept small and failure-derived (auto-generated ones measurably hurt), writer/evaluator role separation with negotiated sprint contracts and execution-based grading, and parallel best-of-N agent fleets with automatic judging and mechanical PR splitting. Anvil's deterministic-machinery bet is directionally confirmed; its gaps are an execution QA layer, a cheap pre-push critic, and spec-as-gated-artifact.


### Model-classified pre-action authorization (Claude Code auto mode)

**Who:** Anthropic, shipped in Claude Code March 2026, turned on by default August 2026


**Mechanism:** Replaces per-action permission prompts with a two-stage transcript classifier: a fast single-token allow/deny filter tuned to over-block, then chain-of-thought reasoning on flagged actions. Three policy tiers (allowlisted read tools; in-project file edits unreviewed because git makes them reversible; classifier-gated shell/network/out-of-tree ops) plus 20+ static block rules (force-push, mass delete, exfiltration, review bypass, prod pushes). Classifier sees only user messages and tool calls, never assistant text, to resist being talked into approvals. Denials return 'find a safer path' instructions; escalates to human after 3 consecutive / 20 total denials. Measured: 0.4% false-positive but 17% false-negative on real overeager actions; baseline problem was users rubber-stamping 93% of prompts.


**Anvil implication:** Direct validation of Anvil's endgame and its differentiator: the industry leader's pre-action authorization is probabilistic and misses 17% of overeager actions, while Anvil bets on deterministic deny-by-default policy. Anvil should mirror the tier structure (cheap static allow/deny rules first, expensive judgment last), adopt deny-and-continue with escalation budgets so denials are recoverable, and position its deterministic authorizer as the layer that closes the classifier's false-negative gap before agents get merge authority.


**Sources:** [How we built Claude Code auto mode: a safer way to skip permissions](https://www.anthropic.com/engineering/claude-code-auto-mode) (Anthropic Engineering, 2026-03-25); [Anthropic is turning Claude Code's auto mode on by default](https://techcrunch.com/2026/08/09/anthropic-is-turning-claude-codes-auto-mode-on-by-default/) (TechCrunch, 2026-08-09)


### Layered OS-level containment with credential exclusion ('blast radius reduction')

**Who:** Anthropic across claude.ai, Claude Code, and Cowork; OpenAI Codex CLI uses the same pattern (Seatbelt on macOS, Landlock+seccomp on Linux, no network by default)


**Mechanism:** Three separated layers: environment containment (process sandboxes, VMs, egress controls — the only layer with real guarantees), model behavior shaping (prompts, classifiers, training — explicitly treated as probabilistic), and external content controls. Per-product primitives: gVisor containers for claude.ai, OS-native Seatbelt/Bubblewrap for Claude Code, full VMs for non-technical Cowork users. Battle-tested primitives (hypervisors, seccomp) preferred over custom components. Key principle: keep credentials out of the sandbox entirely rather than containing them after entry.


**Anvil implication:** Anvil's merge-queue and reviewer agents should run under kernel-level sandboxes with default-deny egress as a hard floor beneath the 73-gate matrix — gates are model/output checks, not containment. Adopt the credential-exclusion pattern for Anvil's GitHub tokens and Buck2 remote-cache keys: agents should never hold merge credentials; the deterministic authorizer holds them and executes approved actions on the agent's behalf. This is also the memory note 'a proxy is not the thing' applied to security: classifiers are proxies, sandboxes are the thing.


**Sources:** [The Blast Radius Problem: How Anthropic Sandboxes Its Own Models](https://ai-beat.github.io/news/2026/05/containing-claude-sandbox-engineering/) (AI Beat, 2026-05-31)


### Two-layer verification stack with an /iterate convergence loop

**Who:** OpenHands (All Hands AI), in production in OpenHands Cloud and as GitHub Actions workflows


**Mechanism:** Layer 1: a small fast critic model scores agent work pre-push, killing obviously broken runs before they reach review. Layer 2, on the PR: a code-review skill applying an impact-ranked checklist (data structures, security, tests, dependency risk) that ends in a risk verdict — high-risk PRs route to a human architect instead of auto-merge — plus a QA agent that actually executes the software in a sandbox (understand changes, set up deps, exercise modified functionality, report with evidence). The /iterate skill opens a draft PR so auto-merge cannot fire early, runs all verification layers, fixes what they flag, and repeats until green before marking ready.


**Anvil implication:** Closest published analog to Anvil's reviewer + certification matrix, and it names two gaps: Anvil's gates are mostly static/typed but lack an execution-based QA agent that runs the changed behavior, and lack a cheap pre-push critic to stop doomed runs before they consume gate-matrix compute. The draft-PR /iterate pattern maps cleanly onto Anvil's merge queue: agents converge on draft PRs against dev, and 'ready' is a machine-issued state transition. Their risk-verdict routing (auto-merge low-risk, human for high-risk) is a plausible intermediate step on the path from founder-reviews-everything to agent merge authority.


**Sources:** [The Verification Stack](https://www.openhands.dev/blog/20260506-the-verification-stack) (OpenHands / All Hands AI, 2026-06-22)


### Loop engineering: loop specifications and long-horizon loop benchmarks

**Who:** Academic consensus forming (arXiv, incl. a Microsoft-affiliated LoopsBench team); practiced on top of Claude Code / Codex-class harnesses


**Mechanism:** A loop specification is a reusable artifact with five parts — trigger, goal, verification step, stopping rule, memory — handed to a harness in place of step-by-step prompting; a survey of 50 real-world loops found 70% run in the fully 'autonomous zone' of a five-level verification ladder and 74% name terminal states, with triggers and persistent memory the weak spots. LoopsBench operationalizes the evaluation: 112 tasks / 5,300+ development units structured as dependency DAGs of separately testable units, tests released along the ready frontier while completed nodes persist as regression obligations. Best config (Opus-4.7 + Claude Code + outer continuation loop) resolves only 25% of tasks.


**Anvil implication:** Two lessons. First, Anvil's ratchets and gates already are loop specifications — formalizing each as {trigger, goal, verification, stopping rule, memory} would make them composable and auditable, and Anvil's weak spots will match the field's (automated triggering, durable memory). Second, 25% on realistic long-horizon work is the ceiling argument for Anvil's whole thesis: deterministic scaffolding (DAG-ordered work, regression obligations that never expire — exactly Anvil's ratchet semantics) is what carries agents through multi-week arcs, and merge authority should be scoped per-loop-spec, not per-agent.


**Sources:** [Stop Hand-Holding Your Coding Agent: Engineering the Loops that Replace Step-by-Step Prompting](https://arxiv.org/abs/2607.00038) (arXiv, 2026-06-28); [LoopsBench: From Harness Engineering to Loop Engineering in Benchmarking Coding Agents](https://arxiv.org/abs/2608.00267) (arXiv, 2026-07-31)


### Observability-driven harness self-evolution under falsifiable contracts

**Who:** Fudan University-led research group (arXiv); harness-agnostic, demonstrated against Codex-CLI and Terminal-Bench 2


**Mechanism:** An evolving agent edits its own harness under three observability pillars: every harness component is a file (explicit, reversible action space); raw trajectories are distilled into a layered drill-down evidence corpus; and every harness edit carries a self-declared prediction that is later verified against task outcomes — a falsifiable contract that prevents trial-and-error collapse. Ten iterations lifted Terminal-Bench 2 from 69.7% to 77.0%, beating the human-designed Codex-CLI harness (71.9%); ablations attribute gains to tools, middleware, and memory structures, not system prompts.


**Anvil implication:** This is Anvil's closed-loop falsification engine applied to the harness itself, with independent evidence it works. Concretely: keep every Anvil harness component (gate definitions, policies, context files) as reviewable files; require any gate/policy change to declare a prediction ('this gate will cut class-X regressions by N') that a later measurement can falsify — the same discipline as the 'prove a check before trusting it' memory note, mechanized. Also a warning: prompt tweaks are the lowest-yield harness edits; invest in tools and memory structures.


**Sources:** [Agentic Harness Engineering: Observability-Driven Automatic Evolution of Coding-Agent Harnesses](https://arxiv.org/abs/2604.25850) (arXiv, 2026-04-28)


### Planner/generator/evaluator triads with negotiated sprint contracts

**Who:** Anthropic (Claude Agent SDK harness for multi-hour autonomous app development)


**Mechanism:** Three specialized agents: a planner expands a short prompt into a full spec (scope + high-level design only); a generator implements with git checkpoints and self-evaluation; an evaluator drives the running application through Playwright MCP — navigating, screenshotting, finding broken handlers — and grades against hard per-criterion thresholds; any criterion below threshold fails the sprint and returns remediation feedback. Before each sprint, generator and evaluator negotiate a 'sprint contract' pinning testable done-criteria. Notably, Opus 4.6 removed the need for context-reset/handoff machinery that Sonnet 4.5 required — model progress deletes harness code.


**Anvil implication:** Anvil's reviewer should split roles the same way: the agent that writes a PR must never grade it, and grading should mean exercising the built artifact (run the console, drive the dashboard), not reading the diff — distinct evaluator agents measurably outperform self-evaluation. Sprint contracts are a per-PR, negotiated complement to Anvil's fixed 73 typed gates: the gate matrix enforces invariants, the contract pins task-specific 'done'. Also plan for harness attrition: design gates and contracts (durable semantics) over context-management machinery (evaporates with each model generation).


**Sources:** [Harness design for long-running application development](https://anthropic.com/engineering/harness-design-long-running-apps) (Anthropic Engineering, 2026-03-24)


### Spec-driven development via a Constitution → Plan → Tasks → Implement pipeline (Spec Kit)

**Who:** GitHub (MIT-licensed Spec Kit, v0.11.0 June 2026, 30+ supported agents incl. Claude Code, Codex CLI, Cursor); broader SDD practice across teams in 2026


**Mechanism:** The spec, not the code, is the reviewed artifact: a repo-level 'constitution' fixes non-negotiable engineering principles; each feature flows through specify (behavioral spec) → plan (technical approach) → tasks (dependency-aware units) → implement, with human/agent agreement checkpoints before code is written; the spec then serves as the source of truth agents generate, test, and validate against. Community reports (self-reported, treat as anecdotal) claim 60-80% fewer rework cycles vs prompt-driven work. The complementary 'mise en place' methodology (arXiv) formalizes the prep phase: externalize tacit knowledge into documents, co-author specs through dialogue, decompose into dependency-aware task records before any agent runs.


**Anvil implication:** Anvil should make specs first-class gated artifacts: a PR without a machine-checkable spec reference fails a gate, and the certification matrix verifies implementation-against-spec, not just implementation-against-tests. Anvil's policy-as-code layer is precisely a 'constitution' — publishing it in Spec Kit-compatible form would let any of the 30+ harnesses Anvil might drive inherit it. Dependency-aware task records align with LoopsBench DAGs: one planning format can feed both execution and evaluation.


**Sources:** [GitHub Spec Kit Documentation](https://github.github.com/spec-kit/) (GitHub, 2026-08-21); [Mise en Place for Agentic Coding: Deliberate Preparation as Context Engineering Methodology](https://arxiv.org/abs/2605.05400) (arXiv, 2026-05-06)


### Context-file restraint: AGENTS.md/CLAUDE.md as a failure-derived pilot's checklist

**Who:** Practitioner consensus (Addy Osmani's widely-cited synthesis; Augment Code and others publishing 2026 guides); applies to every AGENTS.md-compatible harness


**Mechanism:** Keep the context file under ~60 lines, every rule traceable to a specific observed failure — a checklist, not a style guide. 2026 measurements: frontier models reliably follow only ~150-200 standing instructions before compliance degrades, and auto-generated context files are worse than none (task success down 0.5-2%, inference cost up 20%+). Complements: lifecycle hooks where success is silent and failure is verbose (typecheck output only appears on error, triggering self-correction); tool-output offloading to the filesystem; skills with progressive disclosure instead of front-loaded instructions.


**Anvil implication:** Anvil manages context files across a fleet (oyatie's ~700 crates, console, itself), so it should generate them the one way that works: mine gate failures and review findings per-repo, emit only rules with a traceable failure behind them, and enforce a line/instruction budget as its own certification gate. The 'auto-generated files are net-negative' result means Anvil must A/B its generated context files against gate-failure rates — measure the instrument (per the memory notes) rather than assume the file helps.


**Sources:** [Agent Harness Engineering](https://addyosmani.com/blog/agent-harness-engineering/) (Addy Osmani, 2026-04-19)


### Parallel agent fleets with automatic multi-agent judging and PR splitting

**Who:** Cursor (Agents platform, changelog May 2026); Cognition's Devin runs the same fleet pattern in full sandboxed VMs


**Mechanism:** From a plan, the harness identifies independent sub-tasks and runs them concurrently as async subagents ('Build in Parallel'); when N agents attempt the same task, an automatic judge evaluates all completed runs after the fact and recommends the best with a written rationale. Built-in PR review runs in the same platform, and oversized changes are mechanically split into reviewable PRs. Vendor changelog — capabilities are real shipped features, throughput claims are marketing.


**Anvil implication:** Anvil's task orchestrator (already hexagonal-face/N-wide-lane shaped) should treat best-of-N with a deterministic-then-model judge as a standard execution mode for risky changes: run k attempts, let the certification matrix eliminate non-compliant ones, and only then apply model judging among survivors — cheaper and more auditable than judging raw outputs. Mechanical PR splitting matters directly for the founder-reviews-everything bottleneck: smaller certified PRs raise review throughput now and shrink the blast radius per merge decision later.


**Sources:** [PR Review, Build Plan in Parallel, and Split PRs](https://cursor.com/changelog/05-07-26) (Cursor, 2026-05-07)


**Open questions:** Can a deterministic deny-by-default authorizer measurably beat auto mode's 17% false-negative rate on overeager actions without exceeding its 0.4% false-positive rate — and what benchmark of real agent action traces would prove it (per the prove-a-check-before-trusting-it discipline)? · Nobody found in this window ships full agent merge authority in production — OpenHands' risk-verdict routing (auto-merge low-risk only) is the closest; what risk taxonomy and evidence threshold should Anvil require before crossing from 'human reviews all' to 'human reviews high-risk verdicts only'? · LoopsBench's best harness resolves 25% of long-horizon tasks — how does Anvil's gate-matrix + ratchet machinery score on a LoopsBench-style DAG-with-regression-obligations eval of its own fleet work, as a falsifiable baseline for the roadmap? · The context-file findings (auto-generated files net-negative; ~150-200 instruction compliance ceiling) are from small studies — Anvil should run its own A/B across the ~700-crate monorepo before investing in fleet-wide CLAUDE.md generation. · Model progress deletes harness code (Anthropic dropped its context-reset machinery in one model generation): which of Anvil's 73 gates encode durable engineering invariants vs compensations for current-model weaknesses that should be tagged for retirement review? · Verification-stack economics: what is the right ordering of cheap critic → typed gates → execution QA agent → model judge so that gate-matrix compute scales with PR risk rather than PR count?


## Autonomy tiers, trust ratchets, and human-in-the-loop escalation for software agents (state of 2026)

By mid-2026 the industry has converged on tiered agent autonomy ladders (Swarmia's five levels; Unblocked's four rungs) that match Anvil's suggestion->fleet roadmap, with a shared finding: verification capacity, not model capability, is the binding constraint, and most teams ceiling at the bounded-task rung. Trust is ratcheted, not declared — AWS's graduated-autonomy pattern gives the concrete mechanics (windowed trust scores with hysteresis, safety as an independent floor, instant demotion, adversarial-test deployment gates), and Anthropic data shows auto-approve trust accrues over hundreds of sessions. Merge authority to production trunks remains almost exclusively human (29,585-PR academic study, May 2026); where autonomy exists it is per-change-class, not per-agent: narrow auto-merge lanes for docs/deps/generated code under canary, auto-revert, and policy-as-code routing, with auth/payments/infra/schema changes human-gated everywhere. Merge queues are being re-architected for agent volume: tiered test placement, flake quarantine, agent retry budgets with escalation-carrying-evidence. Decision registries are standardizing — hash-chained typed audit records (IETF AAT draft), event-sourced decision provenance with reversibility and authorization as schema fields, and a documented actor-vs-decider logging gap Anvil's registry must close. HITL escalation is becoming a typed, resumable pause/approve/resume protocol with blocking-vs-audit modes enabling gradual gate relaxation.


### Five-level coding-agent autonomy ladder (Assistive -> Conversational -> Task Agent -> Autonomous Teammate -> Agentic Avalanche)

**Who:** Swarmia (engineering-effectiveness vendor); framework adapted from Feng et al. and in wide 2026 industry use


**Mechanism:** Placement is measured by 'how much work the agent completes autonomously before returning for feedback.' Levels 1-2 keep the human in the execution loop; levels 3-4 substitute automated CI plus human code review as the gate; level 5 uses orchestrator agents supervising subagent fleets. Swarmia's central empirical claim: most teams ceiling between level 2 and 3, and the binding constraint is verification capacity, not model capability.


**Anvil implication:** Anvil's roadmap (suggestion -> approved-action -> bounded-task -> goal -> fleet) maps almost one-to-one onto the 2026 consensus ladder; the differentiator is not the ladder itself but building the verification machinery (73 typed gates, ratchets) that lets a fleet break through the level-2/3 ceiling most teams hit. Treat gate coverage as the promotion prerequisite, not model quality.


**Sources:** [Five levels of AI coding agent autonomy, and why higher isn't always better](https://www.swarmia.com/blog/five-levels-ai-agent-autonomy/) (Swarmia, 2026-03-19)


### Two-axis autonomy setting: context quality x verification maturity, with 'set autonomy to the weaker axis'

**Who:** Unblocked (context-platform vendor; partially vendor marketing) synthesizing Anthropic Feb-2026 earned-trust data, METR task-horizon data, and an arXiv role taxonomy


**Mechanism:** Four rungs (Operator, Collaborator, Approver, Observer), each with a minimum context quality and team maturity plus a mandatory guardrail (step-level approval at Collaborator; auto-approve only for low-complexity changes at Approver; spot-check with audit trail at Observer). Cites Anthropic data that full auto-approve usage rises from ~20% under 50 sessions to 40%+ by 750 sessions — trust is earned through accumulated sessions, not declared.


**Anvil implication:** Anvil should encode the weaker-axis rule as policy-as-code: an agent's permitted tier is min(context-quality score for the target repo, gate-coverage score for the change class). The Anthropic session-count curve suggests promotion thresholds denominated in successful reviewed actions (hundreds, not dozens) per repo per change class.


**Sources:** [AI Coding Agent Autonomy: A Decision Framework](https://getunblocked.com/blog/ai-coding-agent-autonomy/) (Unblocked, 2026-06-22)


### Graduated autonomy: numeric trust score with tier promotion/demotion (T1 Probation -> T4 Autonomous)

**Who:** AWS (Architecture Blog reference pattern, implemented on Bedrock AgentCore + DynamoDB + CodePipeline)


**Mechanism:** Trust score 0-100 from five weighted dimensions (accuracy 25%, safety 20% as an independent floor, consistency 20%, compliance 20%, efficiency 15%) over a 50-action rolling window. All agents start at T1 (read/list only) regardless of test performance. Promotion requires holding the threshold for the whole window plus a 5-point buffer to prevent oscillation; demotion is immediate on safety-floor breach or detected injection. CodePipeline blocks deployment on a single unauthorized tool call in adversarial tests. Audit follows a Think/Plan/Act/Observe/Score chain with pre-action state capture for reversibility.


**Anvil implication:** This is the closest published blueprint to Anvil's trust-ratchet direction: asymmetric ratchet (slow windowed promotion, instant demotion), safety as a non-tradeable floor rather than a weighted term, and hysteresis buffers. Anvil should store per-agent per-repo trust state as first-class data the merge queue reads, and gate autonomy tier changes on adversarial/seeded-defect suites — consistent with Anvil's existing 'prove a check before trusting it' doctrine.


**Sources:** [Closing the AI agent trust gap with graduated autonomy](https://aws.amazon.com/blogs/architecture/closing-the-ai-agent-trust-gap-with-graduated-autonomy/) (AWS Architecture Blog, 2026-08-26)


### Risk-tiered auto-merge lanes: change classes A-D routed by blast radius, autonomous merge for class A only

**Who:** Enterprise practice synthesized by First AI Movers (industry guide, partially vendor-flavored) citing Stripe's 'Minions' agents, Shopify, Uber's SubmitQueue, Microsoft internal data, Renovate


**Mechanism:** PRs are classified into risk tiers and routed by policy-as-code: class A (docs, formatting, generated snapshots, well-tested dependency patches) gets 'auto-label, AI summary, queue, auto-merge after deterministic checks'; auth, payments, secrets, infrastructure, schema migrations, deletion paths, and regulated-data code stay under mandatory human review ('AI must not be the only reviewer'). Four required controls on the autonomous lane: canary validation, minutes-scale auto-revert, post-merge observability, and audit logs recording who approved and which gates ran.


**Anvil implication:** The realistic 2026 shape of agent merge authority is per-change-class, not per-agent: Anvil's first agent-held merge authority should be a narrow class-A lane over the oyatie monorepo (codemod outputs, generated files, dep patches) backed by the certification matrix plus auto-revert, while the deny-by-default authorizer keeps every other class human-gated. Blast radius and reversibility are the tiering keys, matching Anvil's planned pre-action authorization.


**Sources:** [AI Pull Request Auto-Merge: Enterprise Guide 2026](https://radar.firstaimovers.com/ai-pull-request-auto-merge-enterprise-guide-2026) (First AI Movers, 2026-05-03)


### Merge-queue re-architecture for agent PR volume: tiered testing, agent retry budgets, escalation-with-evidence

**Who:** Tian Pan (infra engineer, ex-hyperscaler practice writeup) drawing on GitHub's dynamic merge groups (~2,500 PRs/month) and Uber iOS mainline flake data


**Mechanism:** Three-tier test placement (pre-queue: lint/typecheck/build/affected-target unit tests; in-queue: cross-change integration; post-merge batched: E2E and perf), flaky-test quarantine plus optimistic validation so the queue proceeds past suspected flakes, affected-target skipping from build-graph analysis, and an explicit agent retry budget: 'two queue entries, then escalate to a human with the failure evidence attached' — converting agent retry loops into triage signal instead of queue pressure.


**Anvil implication:** Anvil's merge-queue engine should treat agents as rate-limited queue citizens with typed escalation: bounded retries, then a structured hand-off carrying the failure evidence into the human review inbox. The pre/in/post-queue split tells Anvil where its 73 gates belong — cheap deterministic gates pre-queue, cross-change gates in-queue, expensive falsification post-merge with auto-revert — and Buck2 affected-target analysis on oyatie is the enabling substrate.


**Sources:** [The Merge Queue Is the New Bottleneck](https://tianpan.co/blog/2026-07-02-the-merge-queue-is-the-new-bottleneck) (TianPan.co, 2026-07-02)


### Empirical baseline: merge authority remains almost exclusively human across 29,585 agent-involved PR lifecycles

**Who:** Chung & Hassan (academic study of OpenAI Codex, Copilot, Devin, Cursor, Claude Code workflows; AIWare 2026)


**Mechanism:** An Initiator x Approver taxonomy over 29,585 PR lifecycles: 'Collaborator' tools (Cursor, Devin, Copilot) initiate >=96% of their PRs, yet agent-classified approvers appear on only a small fraction; terminal merge authority stays human across every tool studied. The paper also identifies a transparency gap: when automation executes a merge, systems log who performed the action but not who made the decision.


**Anvil implication:** Anvil's end-state (agents holding merge authority under deterministic authorization) is genuinely ahead of measured 2026 practice, so it must manufacture its own evidence base rather than import one. The logged actor-vs-decider gap is a concrete design requirement: Anvil's decision registry must record the authorizing policy, the evidence bundle, and the deciding principal separately from the executing bot identity.


**Sources:** [Collaborator or Assistant? How AI Coding Agents Partition Work Across Pull Request Lifecycles](https://arxiv.org/abs/2605.08017) (arXiv (AIWare 2026), 2026-05-08)


### Standardized tamper-evident agent audit trail (AAT): hash-chained decision records with typed events and trust levels

**Who:** Raza Sharif / CyberSecAI Ltd — individual IETF Internet-Draft (draft-sharif-agent-audit-trail-01), early standardization signal, not yet IETF-endorsed


**Mechanism:** JSON records with mandatory fields (record_id, agent_id/version, session_id, action_type from a seven-type registry — tool_call, tool_response, decision, delegation, escalation, error, lifecycle — outcome including 'denied' and 'escalated', trust_level L0-L4, record_phase pre/post-execution) chained via SHA-256 over RFC 8785 canonicalization with optional ECDSA signatures, so any record modification invalidates all subsequent prev_hash values.


**Anvil implication:** Anvil's decision registry should adopt or track this shape now — typed action registry with first-class 'escalation' and 'denied' outcomes, pre-execution records for deny-by-default authorization, and hash chaining — so agent merge decisions are auditable against an emerging external standard rather than a bespoke log format. Cheap to align with while the draft is young.


**Sources:** [Agent Audit Trail: A Standard Logging Format for Autonomous AI Systems (draft-sharif-agent-audit-trail)](https://datatracker.ietf.org/doc/draft-sharif-agent-audit-trail/) (IETF Datatracker (individual draft), 2026-08-19)


### Decision provenance via event sourcing: append-only decision events with reversibility and authorization as schema fields

**Who:** Tian Pan (practitioner writeup synthesizing OpenTelemetry span conventions, W3C PROV-AGENT, LangSmith/Langfuse/AgentOps practice, EU AI Act Aug-2026 logging obligations)


**Mechanism:** Six-category decision-event schema: identity/lineage (decision_id, parent_agent_id, parent_decision_id), timing/context (model version, sampling params), ordered reasoning trace, tool invocations with a side-effect boolean, data lineage with freshness indicators, and reversibility + authorization metadata per event. Append-only event stream (Kafka-class broker plus schema registry) enables replaying system state at any point; logging must happen before irreversible actions execute.


**Anvil implication:** Reversibility belongs in the record schema, not just the policy: every Anvil gate decision and agent action should carry a machine-readable reversible/irreversible flag and its authorization pointer, making 'irreversible and unauthorized' a statically deniable combination in the pre-action authorizer. Parent-decision lineage is what makes fleet-tier (multi-agent) actions attributable to the originating goal.


**Sources:** [Decision Provenance in Agentic Systems: Audit Trails That Actually Work](https://tianpan.co/blog/2026-04-19-decision-provenance-agentic-systems) (TianPan.co, 2026-04-19)


### Runtime HITL escalation primitives: pause/approve/resume with typed requirements and blocking vs audit approval modes

**Who:** Agno (agent-framework vendor; vendor marketing for their AgentOS product, but the patterns are representative of 2026 production practice)


**Mechanism:** Three oversight layers: tool-level (requires_confirmation pauses before a specific call, presenting a structured RunRequirement describing the pending action), workflow-level (pause between pipeline steps), and approval-level (an @approval decorator with type='required' for blocking administrator sign-off via an Approvals API, or type='audit' for execute-now-record-always). Agent state is checkpointed so confirm()/reject() plus continue_run() resumes exactly where execution paused; three interaction modes distinguish confirmation, mid-run input requests, and external-tool handoffs.


**Anvil implication:** Anvil's HITL escalation should be a typed, resumable protocol rather than a chat message: gates that fail closed should emit a structured requirement object (action, evidence, blast-radius class) into an approvals queue, and agent runs must checkpoint so the founder's approval resumes work without a re-run. The required-vs-audit distinction gives Anvil a graceful downgrade path as tiers are earned — the same gate moves from blocking to audit-only without code changes.


**Sources:** [How to add human-in-the-loop controls to AI agents that actually run in production](https://www.agno.com/blog/how-to-add-human-in-the-loop-controls-to-ai-agents-that-actually-run-in-production) (Agno, 2026-04-23)


**Open questions:** No verified 2026 primary source shows a named company granting agents standing merge authority on a production trunk beyond narrow class-A lanes (docs/deps/generated code); Stripe's 'Minions' and Safeguard's auto-merge-on-green are the closest and deserve direct primary-source confirmation. · Anthropic's Feb-2026 earned-trust dataset (auto-approve ~20% -> 40%+ over 750 sessions) was seen only via a secondary citation (Unblocked); locating the primary publication would firm up Anvil's promotion-threshold calibration. · The AWS graduated-autonomy trust score treats a whole agent as the trust unit; the auto-merge guides treat the change class as the unit. Which unit (agent, agent x repo, agent x change-class) Anvil ratchets on is an open design decision with no published evidence either way. · The IETF AAT draft is an individual -01 draft; whether it gains adoption or is superseded (e.g. by OpenTelemetry GenAI semantic conventions or W3C PROV-AGENT) should be re-checked before Anvil commits to the wire format. · Demotion triggers are underspecified in every framework found: immediate demotion on safety breach is stated (AWS), but no source quantifies how post-merge incidents (auto-reverts, escaped defects) should decay an earned tier over time. · EU AI Act high-risk logging obligations reached full force in August 2026; whether autonomous code-merging agents at a small company fall in scope is unresolved in the sources reviewed.


## Code graph and code intelligence infrastructure for agentic development, state of 2026

By mid-2026 code intelligence is treated as the cost, latency, and accuracy control plane for coding agents: the dominant pattern is precompute structure (SCIP symbol graphs, tree-sitter graphs, Glean-style fact DBs), expose it as typed MCP tools, and have agents query narrow facts instead of reading files. SCIP moved to independent open governance in March 2026 with Uber and Meta on its steering committee, cementing it as the neutral symbol substrate. Blast-radius computation became a named product category — deterministic graph services agents must query, because Claude Code/Cursor/Copilot cannot resolve cross-repo impact themselves. Codemods went graph-targeted and agentic: Sourcegraph's Agentic Batch Changes (public beta July 2026) enumerates affected repos from the code graph, canaries, and reacts to CI; Gartner's 2026 MQ found deterministic recipe engines driven by LLMs (Moderne/OpenRewrite's 10k recipes over lossless semantic trees) outperform pure-LLM refactoring. The open/proprietary split: local single-repo graphs are commoditized OSS; always-fresh, type-precise, cross-repo graphs with history remain the proprietary moat. At Meta, agents author ~50% of changes but everything still flows through industrialized review — validating Anvil's gates-before-merge-authority sequencing.


### SCIP as the neutral symbol-graph substrate (moved to open governance)

**Who:** Sourcegraph, with a Core Steering Committee including engineers from Uber and Meta


**Mechanism:** SCIP is a language-agnostic protobuf index format encoding symbol definitions, references, implementations, and cross-repo links; indexers run per-language at build time and the resulting graph powers deterministic go-to-def/find-refs. In March 2026 Sourcegraph transitioned SCIP from a company-owned project to an independent, openly governed one (scip-code.org) as it neared its fourth anniversary, explicitly to make it the shared indexing substrate multiple vendors and in-house platforms can build on.


**Anvil implication:** Standardize Anvil's symbol layer on SCIP now: emit SCIP indexes for oyatie's ~700 crates (rust-analyzer has a SCIP emitter) in the certification matrix, store them per-merge-queue-head, and treat the SCIP graph as the deterministic input to gate logic. Open governance de-risks the bet that this format outlives any one vendor.


**Sources:** [The future of SCIP](https://sourcegraph.com/blog/the-future-of-scip) (Sourcegraph, 2026-03-27); [The Future of SCIP (discussion, submission timestamp verifies date)](https://news.ycombinator.com/item?id=47544238) (Hacker News, 2026-03-27)


### Precompute structure, expose it as typed tools: code intelligence as the agent cost/accuracy control plane

**Who:** Sourcegraph (SCIP-powered MCP server); pattern echoed across enterprise agent deployments surveyed in 2026


**Mechanism:** Instead of letting agents grep and read whole files, organizations precompute SCIP/graph indexes centrally and expose narrow deterministic queries (exact definition, all callsites, implementers, cross-repo deps) over MCP. Agents query facts instead of burning context on file reads; Sourcegraph reports fewer retries and lower inference spend, and the mid-2026 survey literature calls hybrid retrieval (precise graph + embeddings + syntax) the 'serious default', with pure embeddings or syntax alone insufficient.


**Anvil implication:** Anvil's roadmap item 'code graph as agent context scoper' is the 2026 mainstream pattern. Build one central index service per fleet repo, front it with typed MCP tools, and make the reviewer/gate agents consume facts, not files — it is simultaneously a token-cost, latency, and determinism win, which matters for reproducible gate verdicts.


**Sources:** [Agentic Coding in 2026: A Practical Guide for Big Code](https://sourcegraph.com/blog/agentic-coding) (Sourcegraph, 2026-05-21); [Code Intelligence & Code-Graph Indexing for AI Agents](https://anthonywest.co.uk/research/code-intelligence-indexing-2026-openai) (Anthony West (independent research synthesis), 2026-06-03)


### Deterministic blast-radius services queried by agents before merge

**Who:** Open-source code-review-graph (MIT, tree-sitter based); Riftmap (vendor, cross-repo infrastructure edges) — note both sources are self-published and the Riftmap one is vendor marketing


**Mechanism:** code-review-graph parses a repo into a structural graph with tree-sitter and serves an agent only the blast radius of a diff over MCP — the callers, dependents, and tests actually affected — reporting ~82x median token reduction per review question. Riftmap parses declared infra edges (Terraform source blocks, Dockerfile FROM, CI includes, Helm deps) org-wide into a queryable dependency graph agents hit via HTTP pre-merge; its July 2026 analysis documents that Claude Code, Cursor, and Copilot all lack independent cross-repo blast-radius resolution — the human must name the affected repos, which is the blast-radius question itself.


**Anvil implication:** Blast radius must be an Anvil-owned deterministic gate, not an agent skill: derive it from Buck2's target graph plus cargo metadata plus SCIP references, expose it as a typed query, and key review-depth/test-scope/merge-queue-batching decisions off it. The stock agents will not do this for you in 2026.


**Sources:** [code-review-graph — Token-Efficient AI Code Review](https://explainx.ai/blog/code-review-graph-token-efficient-ai-code-review-2026) (explainx.ai, 2026-07-22); [Which AI coding assistants can see blast radius before they change code?](https://riftmap.dev/blog/which-ai-coding-assistants-see-blast-radius/) (Riftmap (vendor), 2026-07-27)


### Code-graph-targeted agentic codemods with staged rollout and CI feedback (Agentic Batch Changes)

**Who:** Sourcegraph (public beta since 2026-06-30); Mercari as named early adopter — source is a company press release


**Mechanism:** One prompt drives a fleet-wide change: the agent uses Sourcegraph search + Deep Search over the code graph to enumerate affected repositories, validates the change in one repo first, then scales the rollout, adapting to per-repo variation and reacting to CI signals; every changeset still requires human review and approval before merge. Mercari used the preview to find and patch a GitHub Actions env-var injection vulnerability across 80+ repos. Shipped self-hosted in Sourcegraph 7.5 (2026-07-08).


**Anvil implication:** This is exactly Anvil's codemod-pipeline direction, productized: targeting set computed from the graph, canary-first rollout, CI-signal-driven iteration, human merge authority retained. Anvil's differentiator should be replacing 'engineer approves each changeset' with the 73-gate certification matrix plus deny-by-default pre-action authorization — i.e., machine-checkable rollout policy rather than eyeballs.


**Sources:** [Sourcegraph Launches Agentic Batch Changes in Public Beta](https://finance.yahoo.com/technology/ai/articles/sourcegraph-launches-agentic-batch-changes-140000070.html) (Yahoo Finance (Sourcegraph press release), 2026-07-01)


### Deterministic transformation engines as agent tools: compiler-accurate LSTs plus versioned recipes

**Who:** Moderne (commercial) on OpenRewrite (open source); validated by Gartner's 2026 Magic Quadrant for AI-Augmented Code Modernization


**Mechanism:** OpenRewrite builds a Lossless Semantic Tree — type-attributed, formatting-preserving, dependency-resolved — and applies 10,000+ deterministic, versioned recipes; Moderne runs the same recipes across thousands of repos and exposes them, plus deterministic context tools (Prethink emits precomputed context tables, Trigrep for search), to LLM agents as callable tools so agents select transformations instead of generating diffs. Gartner's 2026 Critical Capabilities finding: rule-based deterministic tools driven by an AI service outperform purely LLM-based refactoring because verification/testing cost collapses.


**Anvil implication:** Anvil's 'deterministic machinery steering LLM agents' thesis now has analyst-validated precedent. For the Rust fleet, invest in the equivalent: syn/rust-analyzer-based typed rewrites (or cargo-fix-style machine-applicable lints) packaged as versioned, replayable recipes that agents invoke — agent chooses intent, recipe guarantees the edit — rather than free-form agent diffs certified after the fact.


**Sources:** [Gartner Names Moderne a Leader in Code Modernization](https://moderne.ai/blog/moderne-leader-gartner-magic-quadrant-ai-code-modernization) (Moderne (vendor, citing Gartner), 2026-08-14)


### The 2026 open/proprietary split: local tree-sitter/LSP graphs are commoditized; fleet-scale precise graphs are the moat

**Who:** Open source: tree-sitter parsers, Aider repomap, Serena (LSP-to-MCP bridge, 30+ languages), Kuzu embedded graph DB, SCIP protocol, Meta Glean, code-review-graph. Proprietary: Sourcegraph's hosted SCIP infrastructure, Cursor's Merkle-tree index, Augment Code's Context Engine, Meta's internal precomputed context engine, GitHub's MCP server


**Mechanism:** Surveys through mid-2026 converge: single-repo syntactic graphs (tree-sitter symbol maps, LSP bridges, SQLite/Kuzu-backed local graphs) are freely available and good enough for agent repo-maps; what stays proprietary is the always-fresh, cross-repository, type-precise graph with history — the expensive indexing pipeline, not the format. Retrieval mechanics documented include one-hop structural neighborhoods (IMPORTS/INHERITS/INSTANTIATES edges) and PageRank-style ranking of dependency graphs to surface core abstractions first; Meta's precomputed context reportedly cut agent tool calls per task ~40%.


**Anvil implication:** Anvil can assemble the open layer (SCIP + tree-sitter + Glean-style fact DB + embedded graph store) cheaply; the compounding asset to build is the pipeline that keeps a precise cross-repo graph current at merge-queue speed for the fleet. That freshness pipeline — incremental reindex per queue head — is where to spend engineering, since it is exactly what is not on GitHub.


**Sources:** [Codebase Intelligence: How AI Agents Navigate, Understand, and Reason About Large Repositories in 2026](https://zylos.ai/research/2026-04-19-codebase-intelligence-repository-understanding-ai-agents/) (Zylos Research, 2026-04-19); [Code Intelligence & Code-Graph Indexing for AI Agents](https://anthonywest.co.uk/research/code-intelligence-indexing-2026-openai) (Anthony West (independent research synthesis), 2026-06-03)


### Agent-authored change at ~50% of volume, with review as the universal chokepoint

**Who:** Meta (DevMate internal agent); comparable AI-native claims across large tech companies surveyed


**Mechanism:** Per a mid-2026 industry breakdown, Meta's DevMate submits roughly half of all code changes, and both agent- and human-authored diffs flow through the same review-before-merge pipeline; the survey notes the steering infrastructure (graphs, routing, test selection) exists at these companies but is mostly undocumented publicly. Meta's earlier-published Glean practice (diff indexing producing a 'diff sketch' that drives static analysis and reviewer-facing code navigation) is the known substrate, though its primary write-ups predate this research window.


**Anvil implication:** The frontier organizations did not remove the merge gate as volume shifted to agents — they industrialized it. Anvil's sequencing (deterministic certification first, merge authority for agents later, deny-by-default authorization) matches how the 50%-agent-authored world actually operates; the roadmap bet to make is on gate throughput and diff-sketch-style mechanical summaries per queue entry, not on skipping review.


**Sources:** [What 11 big tech companies actually do with AI in 2026](https://dev.to/kanywst/what-11-big-tech-companies-actually-do-with-ai-in-2026-a-layered-numbers-first-breakdown-h58) (DEV Community (kanywst), 2026-05-09)


**Open questions:** Graph-driven test selection for agent PRs has no strong 2026-window public treatment: bazel-diff/target-determinator style target diffing and Meta/Google predictive test selection are established practice but their write-ups predate March 2026 — verify current state before designing Anvil's test-selection gate, and decide deterministic target-diffing (Buck2-native, fits Anvil's determinism thesis) vs ML-ranked selection. · Review routing via ownership graphs remains publicly under-documented in the window: 2026 sources show agents inheriting CODEOWNERS/branch protections, but no primary source describes graph-computed reviewer routing for agent-authored diffs — likely still internal-only at Meta/Google, so Anvil building it is differentiation rather than catch-up. · Meta Glean's in-window primary sources are thin (the incremental-indexing post is 2026-02-06, just outside the window; the diff-sketch post is Dec 2024) — confirm Glean's current OSS velocity and whether its Rust story (via SCIP import) is production-grade before adopting it as Anvil's fact store. · Does SCIP's new open governance produce a first-class, maintained Rust indexer (rust-analyzer's SCIP emitter has historically lagged), and will Buck2 target-graph facts get a standard encoding alongside SCIP symbol facts, or does Anvil need its own fact schema joining the two? · Independent (non-vendor) benchmarks of blast-radius-scoped agent context vs full-context agents: the 82x token-reduction and retry-reduction numbers all come from the vendors/projects themselves.


## AI-era code review and test fabric at scale, state of 2026

By mid-2026 AI code review is production infrastructure, and the frontier has shifted from "can a bot find bugs" to governing bot verdicts. GitHub ships effort-tiered reviews with severity grading, ~20% measured cost reduction at equal quality, review of bot-authored PRs, and audit-trail resolution reasons. Independent side-by-side benchmarks show reviewer precision is codebase-dependent and vendor numbers are unstable, so serious teams score reviewers on their own seeded corpora. Check validation is now quantified: mutation-guided augmentation (STING, ASE 2026) shows 77% of SWE-bench Verified instances accept semantically wrong patches their suites cannot catch, and hardening suites drops agent resolved rates 4-9% — checks must provably kill seeded defects before they count. SEVRA-BENCH shows all tested review agents can be socially engineered by PR narratives, so verdicts must derive from diffs and executed evidence, not prose. Practitioner-scale evidence (3,100 coded accounts) finds AI PRs get thinner, faster review unless governance forces otherwise; Monperrus argues mandatory human review is a dead end. The emerging synthesis — risk-tiered routing, ownership-keyed policy, flake-quarantine state machines with SLAs, and Proof-of-Execution-style contract+replay authorization — is precisely Anvil's roadmap: retire human authority gate-by-gate as each gate is certified falsifiable.


### Effort-tiered AI review with severity grading and resolution audit trails

**Who:** GitHub Copilot code review (production, GA across GitHub.com)


**Mechanism:** Reviews run at selectable effort levels (Lite/Balanced GA Aug 2026) so depth matches PR risk; comments carry High/Medium/Low severity and are grouped; June 2026 efficiency work cut per-review cost ~20% at measured-equal quality (offline and online eval); Aug 2026 added machine-readable resolution reasons when comments are dismissed, plus review of bot-authored and very large PRs — i.e., the reviewer now reviews agent output and every dismissal leaves an audit record.


**Anvil implication:** Anvil's reviewer should expose effort as a typed, per-PR parameter driven by a diff risk score, and its gate matrix should require a recorded resolution reason for every dismissed finding — dismissals without evidence become a gate failure. Reviewing bot-authored PRs is now table stakes.


**Sources:** [Copilot code review effort levels are generally available](https://github.blog/changelog/2026-08-07-copilot-code-review-effort-levels-are-generally-available/) (GitHub Changelog, 2026-08-07); [Copilot code review: Resolution reasons and expanded capabilities](https://github.blog/changelog/2026-08-27-copilot-code-review-resolution-reasons-and-expanded-capabilities/) (GitHub Changelog, 2026-08-27); [Copilot code review: Analysis depth and efficiency updates](https://github.blog/changelog/2026-06-25-copilot-code-review-analysis-depth-and-efficiency-updates/) (GitHub Changelog, 2026-06-25)


### Mutation-guided validation of test suites via surviving variants (STING)

**Who:** Academic (Concordia/SUSTech-affiliated authors; accepted at ASE 2026), applied to SWE-bench Verified


**Mechanism:** Generate semantically-altered variants of a reference patch, find variants that survive the existing suite (77% of SWE-bench Verified instances admit at least one survivor), then synthesize tests accepted only if they pass the reference patch, kill at least one survivor, and stay stable under behavior-preserving transformations. 1,014 validated tests over 211 instances raised patch-region line/branch coverage 10.8/9.5 points and dropped top-10 repair agents' resolved rates 4.2-9.0% — suites that looked adequate were provably weak.


**Anvil implication:** This is the published, quantified form of Anvil's 'prove a check before trusting it' memory. The certification matrix should require each gate to demonstrably kill a seeded defect (a surviving-variant census per gate) before its verdict counts, and agent-passed rates should be treated as inflated until the suite is mutation-hardened.


**Sources:** [Are Benchmark Tests Strong Enough? Mutation-Guided Diagnosis and Augmentation of Regression Suites](https://arxiv.org/abs/2604.01518) (arXiv (accepted at ASE 2026), 2026-04-02)


### Adversarial-narrative red-teaming of review agents (SEVRA-BENCH)

**Who:** Academic security benchmark evaluating 8 production AI review agents


**Mechanism:** ~1,000 adversarial PRs built by reversing publicly disclosed vulnerability fixes (MITRE 2025 top-10 CWEs), each wrapped in one of 15 social-engineering framings — fabricated supporting evidence, urgency, claimed prior approval, appeals to authority. Result: review agents across the board are susceptible to narrative manipulation and approve vulnerable code they reject when it is presented neutrally.


**Anvil implication:** Anvil's reviewer must treat PR titles, descriptions, and comments as untrusted input — verdicts should derive only from the diff, executed evidence, and the deterministic gate results, never the author's narrative. Add a reversed-fix adversarial corpus to Anvil's own gate-certification suite, since agents will eventually author PRs into their own merge queue.


**Sources:** [SEVRA-BENCH: Social Engineering of Vulnerabilities in Review Agents](https://arxiv.org/abs/2606.13757) (arXiv, 2026-06-11)


### Review as the control point: causal theory from 3,100 practitioner accounts

**Who:** Academic grey-literature study (38,709 documents, 3,100 coded with an LLM-assisted pipeline)


**Mechanism:** Builds a causal model (26 constructs, 67 relationships) of code review in the AI era. Observed pattern: AI-authored PRs get fewer reviews, merge multiple times faster, and draw less discussion — but the pattern reverses under different governance, so outcomes are moderated by team expertise and deliberate review-process structure, not by AI per se. Central claim: review is the mechanism through which an agent's effect on the codebase is decided.


**Anvil implication:** Validates Anvil's thesis that governance machinery, not model quality, decides outcomes. Anvil should instrument review-depth telemetry (comments, discussion, time-in-review per AI PR) as first-class metrics, because thinning review of agent PRs is the measured failure mode to detect and block.


**Sources:** [3100 Opinions on Code Review in an AI World: Building Causal Theory from Practitioner Discourse](https://arxiv.org/abs/2607.07980) (arXiv, 2026-07-08)


### The 'mandatory human review is a dead end' position

**Who:** Martin Monperrus (academic position paper, June 2026)


**Mechanism:** Argues every stated goal of code review (defect finding, knowledge transfer, standards enforcement) can be served by agents at lower cost and higher throughput, and that keeping humans as mandatory reviewers of agent output 'neither provides meaningful assurance nor scales with AI-assisted throughput' — assurance must migrate from human inspection to executable machinery.


**Anvil implication:** Directly argues for Anvil's endpoint: agents holding merge authority under deterministic deny-by-default authorization. But paired with SEVRA-BENCH and the 3,100-opinions study, the migration path matters — human authority should be retired gate-by-gate as each gate is mutation-certified, not wholesale. This is a position paper, not empirical evidence.


**Sources:** [The End of Code Review: Coding Agents Supersede Human Inspection](https://arxiv.org/abs/2606.13175) (arXiv, 2026-06-11)


### Closed-loop flake quarantine lifecycle with SLAs and automatic re-promotion

**Who:** Tenki (CI infrastructure vendor; pattern documented for GitHub Actions — vendor content, but mechanism-level)


**Mechanism:** Four-stage state machine: detect via pass-on-retry and variance across identical-commit runs (e.g. 3 failures in 5 identical-commit runs triggers); quarantine into a separate non-blocking job (continue-on-error) while excluding from the required suite; auto-file a tracking issue with a git-blame-derived owner and a 14-day SLA with escalation; re-promote automatically after 10 consecutive passes, delete after 45 days of abandonment. 'A quarantined test without an owner and a deadline is worse than a red test.'


**Anvil implication:** Anvil's merge queue should implement quarantine as a typed gate state (BLOCKING -> QUARANTINED(owner, deadline) -> REPROMOTED | DELETED), not a skip-list — the lifecycle transitions themselves become gates, and an over-full quarantine (>10% of suite) is a fleet-level infrastructure alarm.


**Sources:** [Flaky Test Quarantine in GitHub Actions](https://tenki.cloud/blog/flaky-test-quarantine-github-actions) (Tenki, 2026-05-22)


### Risk-tiered five-layer review of agent output with hard structural limits

**Who:** Practitioner framework (Codex Knowledge Base, Daniel Vaughan) for teams where agents produce most PRs


**Mechanism:** Measured problem: AI PRs wait 4.6x longer for reviewer pickup though review itself runs 2x faster; a reviewer sustains only 200-400 meaningful lines/hour against 5-6 agent PRs/day/dev. Fix: (1) automated gates as required status checks, (2) 2-minute intent-vs-spec verification, (3) P0-P3 risk triage where the riskiest 20% of PRs absorb ~69% of review effort, (4) human structural review for hallucinated APIs/security/test quality, (5) author-explains-approach knowledge transfer; enforced by a 250-changed-line PR cap and stacked PRs.


**Anvil implication:** Anvil should compute a diff risk score per PR (Meta-style) and let it route: low-risk certified changes flow through the queue on gates alone, high-risk changes summon the founder. The 250-line cap and stacked-PR discipline are enforceable today as deterministic gates and directly attack the pickup-latency bottleneck.


**Sources:** [The Human Review Bottleneck: Practical Code Review Strategies for Agent Output](https://codex.danielvaughan.com/2026/05/24/human-review-bottleneck-code-review-strategies-agent-output/) (Codex Knowledge Base (Daniel Vaughan), 2026-05-24)


### Ownership-encoded review routing, extended to configuring the AI reviewer per team

**Who:** GitHub platform features plus independent practitioner documentation


**Mechanism:** CODEOWNERS maps path patterns to accountable owners so review requests route automatically and compliance rules attach to paths; in 2026 the same ownership layer started configuring the AI reviewer itself — GitHub's June 2026 release lets organizations shape Copilot code review behavior around team-specific instructions and controls, so the bot's review policy is owned and versioned the way human review policy is.


**Anvil implication:** Anvil's fleet (700-crate oyatie monorepo especially) needs machine-readable ownership as the routing substrate for both human escalation and per-crate reviewer policy: gate strictness, review instructions, and merge authority should all key off the same ownership map, policy-as-code style.


**Sources:** [CODEOWNERS: Automating Code Review Ownership](https://tenthirtyam.org/dispatches/2026/03/25/codeowners-automating-code-review-ownership/) (Hypertext Dispatches, 2026-03-25); [Shape Copilot code review around your team](https://github.blog/changelog/2026-06-02-shape-copilot-code-review-around-your-team/) (GitHub Changelog, 2026-06-02)


### Proof of Execution: evidence-carrying, replayable authorization for governed agent actions

**Who:** Academic framework (Rhodes & Kang, 2026) with a working TypeScript prototype


**Mechanism:** Binds a contract C, an Execution Causal Event Stream (tamper-evident history), and a replay context into one runtime-checkable object, with planning, enforcement, effects, and recordkeeping in separated authority domains. A governed action (a merge, a deploy) becomes attestable: each step provably authorized before execution, recorded effects match, trajectory deterministically replayable. Overhead measured at ~2.7ms per minimal flow, 4.4% on concurrent workloads, ~1.1KB per trace.


**Anvil implication:** This is the strongest published match for Anvil's deny-by-default pre-action authorization endpoint. The design lesson: merge authority for agents should not be a permission bit but a contract + tamper-evident event stream + replayability, so any granted merge can be audited and reconstructed after the fact at negligible overhead.


**Sources:** [Proof of Execution: Runtime Verification for Governed AI Agent Actions](https://arxiv.org/abs/2607.05397) (arXiv, 2026-04-26)


### Independent parallel benchmarking of review bots on live PR traffic

**Who:** Independent practitioner study (four commercial reviewers run side-by-side on one team's real PRs)


**Mechanism:** CodeRabbit, Greptile, Sentry Seer, and Cursor BugBot ran in parallel on 146 merged PRs over 3.5 weeks (679 findings, 446 review events), scoring precision, actionability, and latency: CodeRabbit 2.3% false-positive rate with 68.3% one-click-applicable diffs but highest volume (3.4 findings/PR, two-thirds mechanical); Greptile lowest density with zero false positives in this run — versus other published runs showing 11 FPs/run — demonstrating that reviewer quality is benchmark- and codebase-dependent. Note: contradicts vendor-published catch-rate marketing; treat all single-number claims as unstable.


**Anvil implication:** Anvil cannot pick or trust a reviewer model on published benchmarks; it should maintain its own seeded-defect PR corpus (from its fleet's real reverted bugs) and continuously score any reviewer — including its own — on precision, one-click-fix actionability, and latency, applying 'prove a check before trusting it' to the reviewer itself.


**Sources:** [Best AI Code Reviewer in 2026? We Ran 4 in Parallel for 3 Weeks (146 PRs, 679 Findings)](https://dev.to/_vjk/best-ai-code-reviewer-in-2026-we-ran-4-in-parallel-for-3-weeks-146-prs-679-findings-1c0f) (DEV Community, 2026-05-12)


**Open questions:** Meta's MetaMateCR production numbers (19.7% ActionableToApplied, review-time safety trials) appear in FSE 2026 proceedings, but the ACM page is paywalled and the arXiv preprint is dated July 2025 — outside the recency window; an in-window primary treatment of Meta's 2026 review stack was not found. · No in-window first-party Google source on Critique+AI 2026 state was found (the research.google post surfaced by search is dated June 2024); Google's current suggested-edit adoption and any agent-authored-CL review policy remain unverified. · Has any commercial reviewer shipped true evidence-carrying review — findings that cite executed test/coverage artifacts rather than static reasoning? Vendor pages gesture at 'verification tools' but no dated primary source documents a shipped mechanism. · Flake quarantine lifecycles at hyperscaler scale: 2026-window sources are vendor/practitioner patterns; no 2026 primary source from Google/Meta/Microsoft on their current quarantine SLAs and re-promotion criteria was found. · SEVRA-BENCH does not name which 8 review agents were tested (abstract-level access only); whether Anvil's candidate reviewer models are among the susceptible ones needs the full paper. · The 'diff risk score' concept for routing agent PRs is well attested secondhand, but the scoring features and thresholds Meta uses are not public in any in-window source — Anvil would need to design its own from merge-queue telemetry.


## Gaps named by the completeness critic (not researched this pass)

These are recorded so the roadmap can schedule them, not silently claim coverage:


- **Post-merge production loop: progressive delivery, automated canary analysis, and health-keyed auto-revert (Meta's Conveyor continuous-push, Google's Canary Analysis Service / Spinnaker ACA, feature-flag-gated rollout, revert-first SEV doctrine). The sweep is almost entirely pre-merge; auto-revert appears only in passing.** — Anvil's endgame is agents holding merge authority, and every hyperscaler that grants automation land authority pairs it with a deterministic deploy-observe-revert loop — trunk certification is only half the safety case. A multi-year plan that stops at the merge commit has no answer for the defect classes the 73 gates cannot see (behavioral/perf/prod-config), which is exactly where canary+auto-revert is the gate of record at Google and Meta.

- **Software supply chain and artifact provenance: SLSA levels, sigstore/in-toto attestation, reproducible builds, and Rust dependency-audit machinery (cargo vet as used by Mozilla/Google, cargo-audit/RUSTSEC, dependency-update automation policy for vendored crates in Buck2/reindeer trees). The sweep covered agent-config supply chain but not code/artifact supply chain.** — A 700-crate Buck2+Cargo monorepo with agents authoring and eventually landing changes is a high-value injection target; deny-by-default tool authorization does nothing about a malicious or yanked upstream crate flowing through reindeer. Provenance attestations are also the natural substrate for Anvil's signed decision registry — 'which agent, under which policy, produced this artifact' is an SLSA question.

- **Non-human identity and credential brokering for agent fleets: SPIFFE/SPIRE-style workload identity, short-lived scoped tokens, secrets isolation from agent context, and per-agent blast-radius-scoped GitHub/App credentials (the 2025-26 'NHI governance' wave). The sweep covered what an agent may do (pre-action authz) but not who an agent is and what credentials it holds.** — Merge authority is exercised through credentials. A deterministic authorizer fronting tool calls is bypassable if every agent shares one long-lived PAT with org-wide write scope; conversely, per-agent attested identity is what makes the hash-chained audit log's actor field trustworthy. This is the identity layer under Anvil's actor-vs-decider logging gap and it was not researched.

- **VCS substrate and working-copy provisioning at scale: Sapling/EdenFS virtualized checkouts, Jujutsu (jj) as an agent-native VCS, git partial clone/sparse checkout/Scalar, and cheap ephemeral workspace provisioning for parallel agent fleets (BuildBuddy-style snapshot warm-start applied to source trees, not just runners).** — Every best-of-N fleet run, speculative merge-queue lane, and codemod shard needs a working copy; at hyperscalers the virtualized VCS layer (EdenFS, Piper/CitC) is what makes thousand-way parallelism affordable. Anvil's plan covered stacked-diff queueing but not the storage/checkout substrate whose cost curve will dominate once agent volume scales — and jj's first-class conflict/operation-log model is becoming the default agent workflow in 2026.

- **Rust-native certification machinery: Crater-style fleet-wide ecosystem regression runs, cargo-semver-checks as a typed API-evolution gate, public-API diffing, MSRV/toolchain rollout policy, and miri/sanitizer/fuzzing (cargo-fuzz, OSS-Fuzz) tiers for unsafe-code-bearing crates.** — The sweep treated gates generically; a 73-gate matrix for a multi-crate Rust fleet needs Rust-specific gates or agents will land semver-breaking, MSRV-breaking, or UB-adjacent changes that compile green. Crater is the closest existing artifact to 'run the whole fleet against one change' and is the model for Anvil's cross-repo blast-radius verification of toolchain and shared-crate bumps.

- **ML-based predictive test selection and CI cost economics: Meta's predictive test selection (landed-in-production since 2018, still SOTA), TAP's culprit-finding and postsubmit-tiering internals, probabilistic flakiness scoring, and explicit CI compute budgeting/showback per agent.** — Buck2 target determination (btd) gives sound but conservative selection; hyperscalers layer probabilistic selection on top and accept measured escape rates, moving the caught-regressions to post-merge culprit finders. Anvil must decide where soundness is negotiable, because agent-inflated PR volume makes exhaustive target-level CI the dominant cost line of the whole program — and the sweep quantified merge-queue economics but not CI-selection economics.

- **Eval infrastructure for the harness itself and model lifecycle management: regression eval suites for reviewer/implementer agents (seeded-defect corpora as CI for the harness), certification gates on model-version upgrades, multi-model routing/fallback, and planning for provider API deprecation churn over a multi-year horizon.** — Anvil's falsification engine certifies gates per-edit, but the sweep never covered how to re-certify the whole harness when the underlying model changes — the single largest uncontrolled variable in a multi-year plan. The review-benchmark finding that vendor precision is codebase-dependent implies Anvil needs a standing owned eval corpus, run on every model bump the way CI runs on every commit, before any autonomy tier survives an upgrade.

- **Operational governance of the automation itself: big-red-button kill switches and per-actor rate limiters with tested disable paths, freeze windows, SEV/incident process and blameless postmortems for automation-caused breakage (Google SRE's automation-incident practice, GitHub's own Copilot-agent circuit breakers), and who is on-call for the merge queue.** — Trust ratchets in the sweep cover demotion of one agent; they do not cover halting the whole machine when the queue, a codemod pipeline, or the authorizer itself is the defective component. Hyperscalers treat 'automation can be stopped in one action, and the stop path is exercised' as a launch gate for any system with land authority — Anvil granting merge authority without a rehearsed kill path is the failure mode a postmortem-driven plan would most regret.


## Limitations

- Vendor engineering blogs report their own numbers; findings flag these as vendor-reported where the
  researching agent noticed. Treat quantitative claims from vendors as directional.
- Recency was enforced by verified page dates; an agent can still misread a date. The window check is
  mechanical, the date extraction is not.
- The critic's gaps above were deliberately not back-filled in this pass (single research round per the
  commission's iteration cap); each gap is scheduled in the roadmap's research backlog.
