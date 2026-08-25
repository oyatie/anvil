//! Where each Anvil component goes when oyatie absorbs it.
//!
//! # Why this exists
//!
//! 40% of Anvil is superseded by code that already exists in oyatie -- 46 of
//! 114 components, against roughly a quarter-million lines of counterpart. That
//! fact was established once, by audit, and would have decayed into folklore the
//! moment it left the transcript.
//!
//! It is recorded here rather than in a document for the same reason
//! [`crate::fidelity`] exists: a claim that lives only in prose drifts silently
//! from the code it describes, and nothing fails when it does. As data, the
//! verdict is testable -- a component with no entry is a build failure, not an
//! oversight.
//!
//! # Why nothing is deleted yet
//!
//! Anvil is a running daemon and the superseded components are live gates. They
//! are deleted when their counterpart is proven live and bound, not when this
//! table is written. `Superseded` is a destination, not an instruction.
//!
//! # Why `Superseded` and `Standalone` are not the same thing
//!
//! `Superseded` means oyatie already does this, better, and the code is deleted.
//! Code that exists only because Anvil ships as its own repo -- the CLI, the
//! daemon bootstrap, the cross-repo pairing machinery -- is a separate fate:
//! it is also deleted at absorption, but for the opposite reason (nothing
//! replaces it; the need disappears). Collapsing the two loses the distinction
//! that decides whether a replacement must be found first.

/// What happens to a component when oyatie absorbs Anvil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Pure logic with no infrastructure dependency: moves as source.
    Migrating,
    /// The port survives; today's adapter is swapped for an oyatie-backed one.
    Rewired,
    /// oyatie already implements this. Deleted once the counterpart is bound.
    Superseded,
    /// Exists only because Anvil ships as its own repository. Also deleted at
    /// absorption, but for the opposite reason to `Superseded`: nothing replaces
    /// it, the need itself disappears. Kept distinct because the difference
    /// decides whether a replacement must be found before deletion.
    Scaffolding,
}

/// How well established the verdict is. Recorded because an unverified verdict
/// that deletes working code is the expensive kind of wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    /// Counterpart located and inspected, or absence established by search.
    Verified,
    /// Consistent with the evidence, but not confirmed by inspection.
    Probable,
    /// Could not be established. Must not drive a deletion.
    Unresolved,
}

/// One component's migration destiny.
#[derive(Debug, Clone)]
pub struct MigrationEntry {
    pub component: &'static str,
    pub verdict: Verdict,
    pub confidence: Confidence,
    /// oyatie path(s) that supersede or receive this component. Empty when none
    /// was found -- which for a `Migrating` verdict is the expected case.
    pub oyatie_counterpart: &'static str,
    /// Lines of the counterpart, where established. Zero means not measured,
    /// never "small".
    pub counterpart_loc: usize,
    pub evidence: &'static str,
}

impl MigrationEntry {
    /// Whether this component may be deleted on the strength of this entry
    /// alone. An unverified verdict never authorises deletion.
    pub fn deletion_is_authorised(&self) -> bool {
        self.verdict == Verdict::Superseded
            && self.confidence == Confidence::Verified
            && self.counterpart_loc > 0
    }
}

/// The audited ledger. Produced by inspecting each Anvil component against the
/// oyatie tree; see PLAN.md section 38 for method and honest limits.
pub const MIGRATION_LEDGER: &[MigrationEntry] = &[
    MigrationEntry {
        component: "pre_merge_guard/report",
        verdict: Verdict::Migrating,
        confidence: Confidence::Verified,
        oyatie_counterpart: "",
        counterpart_loc: 0,
        evidence: "Split out from the pre_merge_guard entry after the migration-boundary gate found \
                   seven Migrating modules depending on it. Every one of those imports exactly \
                   `GateStatus` and nothing else. report.rs owns the admission vocabulary -- \
                   Errored, NotMeasured, is_admissible -- and a search of oyatie's \
                   governance/check/honest-claims (1878 lines) and aspirational-enforcement (692) \
                   found aspiration tracking but ZERO not-measured, unmeasured, or abstain \
                   vocabulary. The distinction between a gate that failed and a gate that could not \
                   measure has no upstream equivalent. The evaluator and matrix around it ARE \
                   superseded; the vocabulary is not.",
    },
    MigrationEntry {
        component: "account_pool",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "intelligence/core/provider-pool-kernel + provider-pool-app",
        counterpart_loc: 1773,
        evidence: "Recorded under its own module name as well as under the self_governance \
                   subtree entry, because the exemption lists that cite it name it directly. \
                   provider-pool-kernel implements pick_account_with_cooldown, in_cooldown, \
                   window_for, populate_quarantine_from_changes and with_tos_ack; Anvil's pool \
                   is in-memory only, is never persisted, and has no consecutive-failure count.",
    },
    MigrationEntry {
        component: "brand_absence",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "ci/facade/cloud-name-ratchet (partial: vendor names only)",
        counterpart_loc: 0,
        evidence: "Flags aspiration and category stamps in names and PR-visible strings. \
                   oyatie's cloud-name-ratchet covers vendor naming but was not inspected for \
                   aspiration stamps, so overlap is unestablished and the verdict stays \
                   conservative.",
    },
    MigrationEntry {
        component: "migration",
        verdict: Verdict::Scaffolding,
        confidence: Confidence::Verified,
        oyatie_counterpart: "",
        counterpart_loc: 0,
        evidence: "This ledger. It records where Anvil's components go when oyatie absorbs \
                   them, so it has no purpose after absorption -- nothing replaces it, the \
                   question stops being asked.",
    },
    MigrationEntry {
        component: "bin/occupancy.rs",
        verdict: Verdict::Scaffolding,
        confidence: Confidence::Verified,
        oyatie_counterpart: "",
        counterpart_loc: 0,
        evidence: "The command `.github/workflows/ci.yml` runs to turn `admit_spawn` into a \
                   check. It holds no rule of its own: it reads two path lists and a \
                   merge-base answer, calls `change_delivery::core::shard::admit_spawn`, and \
                   maps the result onto `pre_merge_guard::report::GateStatus`. Every input is \
                   bound to this repository -- Anvil's own workflow, Anvil's own hub set in \
                   `anvil_hubs()`, which names this crate's Cargo.toml, src/main.rs and \
                   ci.yml. At absorption the absorbing repository runs its own presubmit over \
                   its own hub set, so nothing has to replace this file: the question it \
                   answers stops being asked here. Scaffolding, not Superseded -- no \
                   counterpart is needed before it goes.",
    },
    MigrationEntry {
        component: "adr_drift_ratchet.rs",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "libs/oya-governance-adr-shape-kernel + libs/oya-check-adr-index + \
                            libs/oya-check-adr-placeholders + governance/check/adr-citation-closure",
        counterpart_loc: 4555,
        evidence: "152 lines, PURE: regex-scans PrDiffContext for 5-field ADR entries. oyatie enforces the \
                  same ADR corpus invariant across three dedicated kernels plus a citation-closure gate, \
                  ~4547 lines total, operating on the real ADR index rather than a diff regex.",
    },
    MigrationEntry {
        component: "ai_driver",
        verdict: Verdict::Rewired,
        confidence: Confidence::Verified,
        oyatie_counterpart: "intelligence/adapters/cli-session-driver + \
                            intelligence/core/model-routing-{kernel,usecase} + route-policy-kernel + \
                            provider-pool-app",
        counterpart_loc: 11741,
        evidence: "2017 lines. The executor half IS superseded: intelligence/adapters/cli-session-driver \
                   spawns vendor CLIs and model-routing-{kernel,usecase} plus route-policy-kernel \
                   and provider-pool-app do routing and failover. But 827 lines have no live \
                   counterpart: task_classifier.rs (206), telemetry_ledger.rs AdaptiveRoutingBandit \
                   (431), cross_model_validator.rs (190). CORRECTED 2026-08-20: the original \
                   evidence claimed a repo-wide grep for bandit/thompson/epsilon-greedy returned \
                   zero hits. That grep was scoped to *.rs and so was never repo-wide. \
                   repos/oyatie/.grok/python/mm_ml/bandit.py is 90 lines of UCB1 bandit for \
                   model-routing suggestions -- same algorithm, same domain. It is referenced by no \
                   Rust, BUCK or toml file and nothing under intelligence/ mentions it, so it is an \
                   unwired Python sidecar in an agent dot-directory rather than a live counterpart. \
                   The classify_task half of the claim holds: zero hits anywhere in oyatie.",
    },
    MigrationEntry {
        component: "api_contract_guard.rs",
        verdict: Verdict::Superseded,
        confidence: Confidence::Probable,
        oyatie_counterpart: "governance/check/openapi-rest-route-parity + ci/facade/contract-slice-conformance",
        counterpart_loc: 4241,
        evidence: "203 lines, shells out to validate OpenAPI/route parity. oyatie has a dedicated \
                  route-parity gate plus a 4085-line contract-slice conformance gate wired to \
                  contracts/openapi.",
    },
    MigrationEntry {
        component: "attestation_guard.rs",
        verdict: Verdict::Superseded,
        confidence: Confidence::Probable,
        oyatie_counterpart: "governance/check/slsa-l3-evidence-grounded + ci/facade/artifact-accountability + \
                            ci/facade/parity-claim-evidence + intelligence/core/evidence-domain",
        counterpart_loc: 3458,
        evidence: "373 lines, of which 92 are module doc; records lane receipts in Anvil's own \
                  `.anvil/receipts` directory (module doc \
                  explicitly names that path). The receipt store exists only because Anvil is its own \
                  repo; oyatie carries evidence/ledger plus a SLSA-L3 evidence gate and \
                  artifact-accountability gate.",
    },
    MigrationEntry {
        component: "auto_rollback",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "156 lines, PURE (no NET/PROC/FS). Health evaluation + postmortem bundle generation. grep \
                  -rilE 'blue.green|blue_green' over oyatie *.rs returned ZERO hits; no rollback \
                  controller found.",
    },
    MigrationEntry {
        component: "automated_canary",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "165 lines, PURE. StatisticalCanaryEngine over MetricDistribution. No canary-analysis \
                  crate in oyatie's 844-crate index; 'canary' hits are incidental prose in \
                  build/port-engine and ci/facade gates.",
    },
    MigrationEntry {
        component: "canary_rollout",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "197 lines, PURE. CanaryCircuitBreaker over CanaryMetricsSnapshot. Same negative as \
                  automated_canary.",
    },
    MigrationEntry {
        component: "carbon_aware",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "106 lines, PURE. Carbon-intensity compute ratchet. grep -rilE 'carbon|gCO2|emissions' \
                  hit only compute/core/dcops and audit-tap crates as incidental prose; no carbon-aware \
                  scheduling capability located.",
    },
    MigrationEntry {
        component: "cedar_guard (dir)",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "iam/core/policy-cedar-domain + iam/adapters/pdp-cedar + libs/oya-shared-pdp-kernel + \
                            governance/check/cedar-fragment-coverage",
        counterpart_loc: 8408,
        evidence: "58 lines, PURE; CedarPdpEngine evaluating synthetic authorization tuples. oyatie has a \
                  real Cedar PDP stack: policy-cedar-domain (3732), pdp-cedar adapter (2508), shared \
                  pdp-kernel (1779) plus a cedar-fragment-coverage gate (389).",
    },
    MigrationEntry {
        component: "cedar_guard.rs",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "iam/core/policy-cedar-domain + iam/adapters/pdp-cedar + libs/oya-shared-pdp-kernel + \
                            governance/check/cedar-fragment-coverage",
        counterpart_loc: 8408,
        evidence: "317 lines; evaluate_cedar_policies shells to a Cedar CLI, evaluate_offline_pdp is the \
                  fallback. Same oyatie Cedar stack supersedes both paths with a real in-process PDP.",
    },
    MigrationEntry {
        component: "cell_isolation_guard.rs",
        verdict: Verdict::Superseded,
        confidence: Confidence::Probable,
        oyatie_counterpart: "governance/check/shardability + cell/core/capacity + cell/core/routing + \
                            tenancy/core/isolation-policy",
        counterpart_loc: 1784,
        evidence: "139 lines; flags cross-cell query scoping violations in a diff. oyatie's shardability \
                  check enforces the same invariant structurally (every per-tenant CREATE TABLE must \
                  declare tenant_id, verified at governance/check/shardability/src/lib.rs:153) and \
                  cell/core carries the real capacity+routing model.",
    },
    MigrationEntry {
        component: "chaos_injector",
        verdict: Verdict::Migrating,
        confidence: Confidence::Verified,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "140 lines. ChaosFaultInjector/FaultSimulator. grep -rilE 'chaos|fault injection' over \
                  oyatie *.rs returned 5 files, all incidental (workspace-member-coverage, \
                  os/core/network-domain); no fault-injection capability.",
    },
    MigrationEntry {
        component: "chaos_mutation_guard.rs",
        verdict: Verdict::Rewired,
        confidence: Confidence::Verified,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "PROC+ASYNC: run_cargo_mutants spawns `cargo mutants --in-diff` and reads its \
                  outcome lists -- a real mutation measurement. The chaos_mutation_guard/ directory \
                  (AstMutatorEngine, str::replace over lines nothing compiled) is deleted. Same \
                  verified negative as before: no mutation-testing capability anywhere in oyatie, \
                  so only the subprocess seam swaps.",
    },
    MigrationEntry {
        component: "ci_runner_economics",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "ci/facade/runner-disk-reclaim + infra/arc",
        counterpart_loc: 1041,
        evidence: "181 lines, PURE; scans workflow files for runner SKU allocation. oyatie's \
                  runner-disk-reclaim gate (1041) and infra/arc cover runner capacity/cost but not SKU \
                  right-sizing of workflow YAML; the scanner port survives, the runner-facts source swaps \
                  to ci/controller.",
    },
    MigrationEntry {
        component: "ci_triager.rs",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "ci/controller/kernel + ci/controller/github-adapter",
        counterpart_loc: 4323,
        evidence: "355 lines; PROC+GH+ASYNC. Fetches a failed workflow run and LLM-triages the log. \
                  oyatie's ci/controller maps K8s Job observations to commit statuses (map_job_to_status) \
                  but does no log triage; the GitHub/log-fetch adapter swaps, the diagnosis logic \
                  survives.",
    },
    MigrationEntry {
        component: "ci_wallclock_ratchet",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "401 lines. Module's own doc admits it timed nothing (four literals). No \
                  CI-duration/regression-budget ratchet found in oyatie's crate index. Pure logic; needs a \
                  real duration source.",
    },
    MigrationEntry {
        component: "clean_architecture_guard.rs",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "ci/facade/facade-core-layering + core-dependency-isolation + port-placement + \
                            layer-dependency-acyclicity + governance/check/layered-architecture-discipline",
        counterpart_loc: 10813,
        evidence: "186 lines of regex over a diff for Core->Ports->Adapters->Facade. oyatie enforces the \
                  identical doctrine over the REAL cargo+buck dependency graph across five gates: \
                  facade-core-layering (683, 'facade reaches core only through ports'), \
                  core-dependency-isolation (4132, kernel purity), port-placement (1645, port traits must \
                  live in core/ports), layer-dependency-acyclicity (3771), layered-architecture-discipline \
                  (582).",
    },
    MigrationEntry {
        component: "cli",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "marketplace/facade/dev-cli",
        counterpart_loc: 78263,
        evidence: "1523 lines (args 342, handlers 746, server 457). Anvil's own clap CLI and daemon \
                  bootstrap; exists only because Anvil ships as its own binary. oyatie's operator CLI is \
                  marketplace/facade/dev-cli.",
    },
    MigrationEntry {
        component: "cloud_native_guard",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "ci/facade/cloud-name-ratchet + governance/check/vendor-lockin-discipline + \
                            ci/facade/automation-language-policy",
        counterpart_loc: 7341,
        evidence: "176 lines; flags proprietary cloud SDKs in core, hardcoded endpoints, and non-Rust \
                  tooling. oyatie has all three as separate mature gates: cloud-name-ratchet (640), \
                  vendor-lockin-discipline (1280, tiered vendor classification with seam adapters), \
                  automation-language-policy (5421, Rust-first automation).",
    },
    MigrationEntry {
        component: "cluster_state_auditor",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "368 lines. Module doc states evaluate_cluster_parity compared a literal against itself. \
                  No live-cluster readback in oyatie: ci/controller/k8s-adapter (515) observes Jobs only, \
                  not desired-vs-live manifest parity. Diff logic is pure and would need a real kube \
                  reader.",
    },
    MigrationEntry {
        component: "compile_time_profiler",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "181 lines, PURE. HeavyDependencyScanner + compile-profile report. grep -rilE 'compile \
                  time|build time|timings' matched only build/port-engine and billing prose; no \
                  compile-time budget capability.",
    },
    MigrationEntry {
        component: "compliance_guard",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "compliance/core/{dlp,retention,trust-portal} + \
                            libs/oya-shared-compliance-evidence-kernel + libs/oya-check-compliance-evidence-coverage",
        counterpart_loc: 4155,
        evidence: "777 lines; a temporal regulatory-rule registry \
                  (RegulatoryRule/TemporalValidity/GeographicScope) that scans diffs for statutory \
                  violations and syncs upstream rules from a directory. oyatie's compliance axis is \
                  runtime services (DLP 681, retention 995, trust-portal 1649) plus an evidence-coverage \
                  check; the rule registry has no counterpart, so the port survives and the rule source \
                  swaps to packs/ + compliance/catalog.",
    },
    MigrationEntry {
        component: "config.rs",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "libs/oya-ci-config",
        counterpart_loc: 2129,
        evidence: "102 lines; Config::from_env for the Anvil binary. Repo-own bootstrap; oyatie carries \
                  libs/oya-ci-config (2129) for its own configuration surface.",
    },
    MigrationEntry {
        component: "consistency_guard",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "138 lines, PURE. ActiveActiveConsistencyGuard + ConflictDetector over a diff. oyatie's \
                  collab-crdt-portability-kernel is a runtime CRDT substrate, not a consistency-invariant \
                  gate; no counterpart gate found.",
    },
    MigrationEntry {
        component: "constant_work_guard",
        verdict: Verdict::Migrating,
        confidence: Confidence::Verified,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "170 lines, PURE. Scans for unbounded channels/collections. grep -rilE 'constant \
                  work|constant_work' over all oyatie *.rs returned ZERO hits.",
    },
    MigrationEntry {
        component: "corpus_auditor",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "governance/corpus/{core,extract,doc-parser} + ci/adapters/corpus-census + \
                            ci/facade/corpus-index-coverage",
        counterpart_loc: 10259,
        evidence: "379 lines; repository freshness ledger + hygiene batching. oyatie has a full corpus \
                  substrate: governance/corpus/core (1489, content-addressed fact graph), extract (2297), \
                  doc-parser (4050), plus ci/adapters/corpus-census (445) and \
                  ci/facade/corpus-index-coverage (1978).",
    },
    MigrationEntry {
        component: "cosign_signer",
        verdict: Verdict::Superseded,
        confidence: Confidence::Probable,
        oyatie_counterpart: "governance/check/image-signing-discipline + libs/oya-governance-image-discipline-kernel \
                            + governance/check/slsa-l3-evidence-grounded",
        counterpart_loc: 1073,
        evidence: "105 lines, PURE — infra scan shows no PROC, so it invokes no signing binary and reaches no Rekor; \
                  the fabricated signature bundle it used to emit is deleted and the gate now reports \
                  that nothing signed. oyatie enforces the real posture: \
                  image-signing-discipline (178) + oya-governance-image-discipline-kernel (431) + \
                  slsa-l3-evidence-grounded (464) + supply-chain-audit (2675, VEX/cosign/SBOM).",
    },
    MigrationEntry {
        component: "coverage_guard.rs",
        verdict: Verdict::Rewired,
        confidence: Confidence::Verified,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "1428 lines, PROC+ASYNC: run_llvm_cov + parse_lcov + added_lines_by_file — a real \
                  differential-coverage measurement. grep -ril 'llvm-cov' over oyatie *.rs returned ZERO \
                  hits and 'lcov' matched only incidental prose. Anvil is the only implementation; only \
                  the subprocess seam swaps.",
    },
    MigrationEntry {
        component: "criterion_bench_ratchet.rs",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "governance/check/benchmark + governance/check/perf-budget",
        counterpart_loc: 608,
        evidence: "189 lines, PURE. oyatie has governance/check/benchmark (264) and perf-budget (344) \
                  covering the same 'benchmark regression must not pass silently' class, but neither \
                  parses Criterion output; the parser survives, the budget policy swaps.",
    },
    MigrationEntry {
        component: "cross_service_impact",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "ci/facade/affected-target-set + ci/facade/service-catalog-parity",
        counterpart_loc: 11203,
        evidence: "164 lines, PURE; RequiredFieldChecker over a diff. oyatie's affected-target-set (10456) \
                  derives the exact buck2 target set from a merge-base diff, and service-catalog-parity \
                  (747) validates the service graph itself — both strictly more capable.",
    },
    MigrationEntry {
        component: "dashboard",
        verdict: Verdict::Superseded,
        confidence: Confidence::Probable,
        oyatie_counterpart: "intelligence/core/dashboard-{kernel,app,api} + console/facade/workspace-shell-app",
        counterpart_loc: 1608,
        evidence: "1463 lines across 6 files (ssr_renderer 341, styles 401, panel_formatters 236, \
                  client_scripts 136, escape 89, mod 260) — an axum-served SSR cockpit over Anvil's own \
                  AppState. Exists only because Anvil runs its own daemon; oyatie's console axis owns the \
                  operator UI.",
    },
    MigrationEntry {
        component: "deadlock_analyzer",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "LockOrderGraph: a lock-order graph over diff text, cycles reported via reachability. \
                  grep -rilE 'deadlock|lock order|lock_order' matched only incidental prose in ci/facade \
                  and os/core; no static lock-order analysis in oyatie.",
    },
    MigrationEntry {
        component: "debt_shrink_guard.rs",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "ci/facade/baseline-ratchet + governance/check/placeholder-debt",
        counterpart_loc: 9647,
        evidence: "196 lines; net-growth ratchet forbidding additions to deprecated targets. oyatie's \
                  baseline-ratchet (9154) is exactly the shrink-only ratchet mechanism with a gate-owned \
                  frozen baseline, plus placeholder-debt (493) for the debt registry.",
    },
    MigrationEntry {
        component: "doc_archival_sweeper",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "ci/facade/stale-artifact-detection",
        counterpart_loc: 583,
        evidence: "255 lines; sweeps stale docs and consolidates issue docs. oyatie's \
                  stale-artifact-detection gate is functionally identical and explicitly safer: 'report -> \
                  git mv -> _archive/, second-verifier-gated, NEVER rm' \
                  (ci/facade/stale-artifact-detection/src/lib.rs header).",
    },
    MigrationEntry {
        component: "doc_guard",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "governance/check/{doc-catalog,documentation-system,readme-coverage} + \
                            libs/oya-check-doc-axis + libs/oya-governance-doc-freshness-kernel + \
                            libs/oya-governance-doc-style-kernel",
        counterpart_loc: 3021,
        evidence: "637 lines; frontmatter validation + docs-as-code parity. oyatie covers each half: \
                  doc-catalog (369), documentation-system (382), oya-check-doc-axis (1612), \
                  doc-freshness-kernel (183, 'blocks PRs that change a source-of-truth file without \
                  regenerating dependent docs'), doc-style-kernel (316), readme-coverage (159).",
    },
    MigrationEntry {
        component: "dual_track_build_guard",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "ci/facade/build-target-parity + libs/oya-buck-syntax-kernel + \
                            tools/oya-buck-test-wiring-app",
        counterpart_loc: 484,
        evidence: "127 lines; cargo/Buck dual-track parity. oyatie's build-target-parity gate enforces \
                  precisely this: 'every member must carry a tracked BUCK file, and every member with Rust \
                  test code must declare a rust_test target' (ci/facade/build-target-parity/src/lib.rs \
                  header), backed by oya-buck-syntax-kernel and oya-buck-test-wiring-app.",
    },
    MigrationEntry {
        component: "early_exit_cascade",
        verdict: Verdict::Superseded,
        confidence: Confidence::Probable,
        oyatie_counterpart: "governance/check/pre-push + libs/oya-governance-pre-push-kernel",
        counterpart_loc: 410,
        evidence: "138 lines, PURE; FastChecksProber ordering cheap invariants before expensive lanes. \
                  oyatie owns the same pre-push cascade as pre-push-contract (governance/check/pre-push \
                  299 + oya-governance-pre-push-kernel 111) plus ci/facade/affected-target-set for \
                  scoping.",
    },
    MigrationEntry {
        component: "ephemeral_sandbox",
        verdict: Verdict::Migrating,
        confidence: Confidence::Unresolved,
        oyatie_counterpart: "not established (candidates: intelligence/adapters/supervisor-security-adapter, \
                            libs/oya-shared-wasm-runtime-kernel)",
        counterpart_loc: 580,
        evidence: "137 lines, PURE. SandboxPool allocating ephemeral sandbox instances. oyatie has \
                  supervisor-security-adapter (92) and oya-shared-wasm-runtime-kernel (488) but neither is \
                  a sandbox pool allocator; counterpart NOT established.",
    },
    MigrationEntry {
        component: "ephemeral_secrets",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "ci/facade/operator-secret-rbac + secrets/core/domain",
        counterpart_loc: 3958,
        evidence: "171 lines, PURE; OidcPolicyValidator over workflow secret declarations. oyatie's \
                  operator-secret-rbac (1955) enforces least-privilege secret RBAC + ESO/OpenBao scope \
                  isolation and secrets/core/domain (2003) owns the model, but neither validates GitHub \
                  Actions OIDC workflow policy; the validator survives, the secret backend swaps.",
    },
    MigrationEntry {
        component: "exec",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "intelligence/adapters/cli-session-driver",
        counterpart_loc: 232,
        evidence: "222 lines, PROC+ASYNC. run_bounded/run_bounded_with_stdin — timeout+kill_on_drop \
                  wrapper; module doc records 118 subprocess spawns of which one had a timeout. oyatie's \
                  cli-session-driver (232) is the equivalent spawn seam but is provider-CLI-specific; \
                  Anvil's generic bounded-exec port survives with the process backend swapped.",
    },
    MigrationEntry {
        component: "feature_flag_ratchet.rs",
        verdict: Verdict::Superseded,
        confidence: Confidence::Probable,
        oyatie_counterpart: "flags/core/evaluation-domain + flags/core/server",
        counterpart_loc: 948,
        evidence: "183 lines; flags stale toggles and dead fallback branches. oyatie owns a flags axis: \
                  flags/core/evaluation-domain (832) + flags/core/server (116) plus \
                  flags/{capabilities,cedar,policy,release} governance, which carries flag lifecycle as a \
                  first-class capability.",
    },
    MigrationEntry {
        component: "fidelity",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "governance/check/honest-claims + governance/check/aspirational-enforcement + \
                            libs/oya-governance-claim-ceiling-kernel",
        counterpart_loc: 2686,
        evidence: "564 lines; a machine-readable declaration of how much of each Anvil gate is real \
                  (Fidelity enum, honesty_ratio, GapReport). oyatie has this exact defect class as \
                  first-class gates: honest-claims (1878), aspirational-enforcement (692, 'fails only \
                  explicit binding claims'), claim-ceiling-kernel (116), and gate ids 'vacuous-green' + \
                  'claim-ceiling' in the gate catalog. The Anvil-gate-specific registry data does not s...",
    },
    MigrationEntry {
        component: "finops_ratchet",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "libs/oya-check-cost-budget + governance/check/tenant-cost-labels-coverage + \
                            billing/core/finops",
        counterpart_loc: 1056,
        evidence: "190 lines, PURE. Two distinct things: AllocationScanner (hot-path heap allocations) and \
                  a unit-cost ratchet. oyatie has libs/oya-check-cost-budget (571, provider invocation \
                  cost ceilings) and tenant-cost-labels-coverage (485) for the cost half; no hot-path \
                  allocation scanner found. Cost policy rewires, allocation scanner survives.",
    },
    MigrationEntry {
        component: "fixer",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "intelligence/core/subagent-runtime-{kernel,app,usecase}",
        counterpart_loc: 2016,
        evidence: "657 lines, PROC+GH+ASYNC. apply_code_fixes / attempt_self_correction / \
                  run_test_verification_gate. oyatie's subagent-runtime-kernel header names \
                  'tools/oya-vcs-ci-fix-loop-dispatcher-app (IP-005)' as the fix-loop consumer, and \
                  subagent-runtime-app supplies AnthropicSubagentPort; the model+GitHub adapters swap, the \
                  fix/verify loop survives.",
    },
    MigrationEntry {
        component: "flake_bisector",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "175 lines, PURE. bisect_historical_commits over test history. grep -ril 'bisect' over \
                  oyatie *.rs matched only ci/facade/scm-facts-snapshot incidentally; no bisection \
                  capability.",
    },
    MigrationEntry {
        component: "flake_cost_dampener",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "146 lines, PURE. QuarantineLogManager + flake risk scoring. No flake-management \
                  capability in oyatie (grep 'flaky' matched one file, scm-facts-snapshot).",
    },
    MigrationEntry {
        component: "flake_quarantine",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "106 lines, PURE. Quarantine lifecycle state machine for tests. Same verified negative: \
                  no test-quarantine capability in oyatie.",
    },
    MigrationEntry {
        component: "fleet_observer",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found for aggregation; ci/controller/github-adapter for the forge seam",
        counterpart_loc: 1624,
        evidence: "261 lines, GH+ASYNC. Polls N repos and aggregates a fleet overview. No fleet-aggregation \
                  crate in oyatie; the aggregation logic survives, the GitHub polling adapter swaps to \
                  ci/controller/github-adapter.",
    },
    MigrationEntry {
        component: "formal_verification",
        verdict: Verdict::Migrating,
        confidence: Confidence::Verified,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "142 lines, PURE. PolicyPatternScanner + scan_policy_text. grep -rilE \
                  'z3|smt.solver|satisfiab' over oyatie *.rs matched only incidental hits \
                  (os/core/init-app, image_cache); no SMT capability in oyatie.",
    },
    MigrationEntry {
        component: "ghost_migration_harness.rs",
        verdict: Verdict::Migrating,
        confidence: Confidence::Verified,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "234 lines, PURE. Ghost-table / online schema migration validation. grep -rilE \
                  'expand.contract|ghost table|online schema' over oyatie *.rs returned ZERO hits.",
    },
    MigrationEntry {
        component: "git_manager",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "tools/oya-checkout-guard-app + ci/adapters/path-resolver (partial)",
        counterpart_loc: 5120,
        evidence: "653 lines, PROC+FS+ASYNC. ensure_repo_cloned, create_ephemeral_worktree, \
                  clean_abandoned_worktrees, prepare_pr_diff, install_repo_hooks. oyatie's \
                  checkout-guard-app (4234) is a guard over checkout state, not a worktree manager; \
                  ci/adapters/path-resolver (886) resolves paths. The PrDiffContext producer survives; the \
                  git backend rewires.",
    },
    MigrationEntry {
        component: "github",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "ci/controller/github-adapter + ci/tide/github-adapter + \
                            oya-ci-webhook-gateway-github-adapter",
        counterpart_loc: 2463,
        evidence: "1842 lines (reviews.rs 1016, mod.rs 595, graphql.rs 176, fork_guard.rs 55). REST+GraphQL \
                  client with review-thread resolution, inline review submission, DORA fetch, fork push \
                  guard. oyatie has three GitHub adapters (ci/controller 1624, ci/tide 588, \
                  ci-webhook-gateway 251) but none does review threads or comment-vs-diff validation. Port \
                  survives, transport swaps.",
    },
    MigrationEntry {
        component: "gitops_drift_reconciler",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "ci/facade/policy-deploy-parity + libs/oya-governance-orphan-detection-kernel + dev-cli \
                            cloud_iac_gitops_evidence_gate",
        counterpart_loc: 2217,
        evidence: "154 lines, PURE. OrphanSweeper over manifests. oyatie has cloud-iac-gitops-evidence \
                  (dev-cli gate) + policy-deploy-parity (1944) + oya-governance-orphan-detection-kernel \
                  (273); the drift/orphan concept is present but scoped to oyatie's own IaC registry, not \
                  an arbitrary repo's manifests.",
    },
    MigrationEntry {
        component: "gitops_promotion",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "ci/facade/policy-deploy-parity + ci/facade/helm-chart-shape",
        counterpart_loc: 2267,
        evidence: "409 lines, PURE. Environment promotion tiers + DigestPinner (unpinned image detection). \
                  oyatie's policy-deploy-parity (1944) and cloud-iac-helm-chart-signed-image-wiring gate \
                  cover digest pinning and deploy parity; promotion-tier state machine has no counterpart.",
    },
    MigrationEntry {
        component: "harness",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "pipeline/core/admission (rules) + no counterpart (the harness itself)",
        counterpart_loc: 0,
        evidence: "PURE. The rule-running harness: Evaluated makes a measurement over zero subjects \
                  unconstructible, and every rule proves itself against a seeded defect before its \
                  verdict is trusted. oyatie's admission crate holds equivalent RULES but no \
                  equivalent harness -- its checks are two binaries each re-implementing the runner, \
                  which is the N+1 this replaces. Moves as source; the rules move with it.",
    },
    MigrationEntry {
        component: "hermetic_build",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "ci/facade/embedded-asset-hermeticity + ci/facade/generated-artifact-freshness",
        counterpart_loc: 9183,
        evidence: "125 lines, PURE. ReproducibilityChecker (byte-for-byte artifact comparison). oyatie's \
                  embedded-asset-hermeticity (2975) enforces hermetic build inputs and \
                  generated-artifact-freshness (6208) enforces regeneration, but neither does \
                  byte-for-byte reproducibility verification of built binaries.",
    },
    MigrationEntry {
        component: "hyperscaler_consensus_guard",
        verdict: Verdict::Superseded,
        confidence: Confidence::Probable,
        oyatie_counterpart: "libs/oya-shared-hyperscaler-metrics-kernel + dev-cli \
                            hyperscaler_{maturity_claims,arch_invariants}_gate + registry/hyperscaler-scorecards",
        counterpart_loc: 1189,
        evidence: "277 lines, PURE. Simulates per-cloud-provider review checklists and requires unanimity — \
                  no external provider is consulted. oyatie owns the hyperscaler-claim surface as real \
                  gates: registry/hyperscaler-scorecards plus dev-cli hyperscaler_maturity_claims_gate and \
                  hyperscaler_arch_invariants_gate (both in the 232-id gate catalog) and \
                  oya-shared-hyperscaler-metrics-kernel (1189).",
    },
    MigrationEntry {
        component: "idempotency_guard",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "governance/check/idempotency-key-coverage + libs/oya-shared-idempotency-key-kernel + \
                            libs/oya-shared-outbox-pattern-kernel",
        counterpart_loc: 993,
        evidence: "183 lines; scans mutating endpoints for idempotency keys. oyatie's \
                  idempotency-key-coverage gate (726) enforces the identical contract against every \
                  microservice OpenAPI 3.2.0 doc ('every POST/PUT/PATCH/DELETE MUST declare the canonical \
                  Idempotency-Key header'), backed by oya-shared-idempotency-key-kernel (267) and the \
                  outbox-pattern kernel.",
    },
    MigrationEntry {
        component: "incident_healer",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found for reversion; ci/facade/action-item-accounting for action-item closure",
        counterpart_loc: 1066,
        evidence: "153 lines, PURE. execute_incident_revert + PostmortemStamper. grep -rilE \
                  'postmortem|incident' matched only ci/facade prose. Closest concept is \
                  ci/facade/action-item-accounting (1066) which implements the SRE postmortem action-item \
                  model, but not incident reversion.",
    },
    MigrationEntry {
        component: "incident_sentry",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "169 lines, PURE. LiveGoldenSignals + circuit breaker over production health. \
                  observability/core/domain (1998) owns the telemetry vocabulary and \
                  k8s/core/sla-observability-kernel (1560) owns burn rate, but no incident circuit-breaker \
                  gate exists.",
    },
    MigrationEntry {
        component: "issue_reconciler",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "ci/facade/action-item-accounting",
        counterpart_loc: 1066,
        evidence: "270 lines, PROC+FS+GH+ASYNC. IssueAuditor classifies and reconciles open issues. \
                  oyatie's action-item-accounting (1066) is the same closed-loop model — 'every action \
                  item must have a declared disposition and, once terminal, verifiable closure' — but \
                  sourced from the friction ledger, not GitHub issues. Auditor survives, issue source \
                  swaps.",
    },
    MigrationEntry {
        component: "jittered_backoff",
        verdict: Verdict::Migrating,
        confidence: Confidence::Verified,
        oyatie_counterpart: "none found (libs/oya-shared-protocol-transport-retry-app is a runtime retry, not a gate)",
        counterpart_loc: 504,
        evidence: "147 lines. BackoffScanner flags retries lacking jitter in a diff. oyatie USES jitter at \
                  runtime (intelligence/core/wire-kernel, tenancy/core/tenant-lifecycle-kernel, \
                  openai-subscription-adapter) and has protocol-transport-retry-app (504), but has NO gate \
                  that detects unjittered retries in source. The gate is unique.",
    },
    MigrationEntry {
        component: "kani_guard",
        verdict: Verdict::Migrating,
        confidence: Confidence::Verified,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "211 lines, PURE: two regexes over the diff text, asking whether an added `unsafe` item \
                  carries a `// SAFETY:` comment. No subprocess and no model checker — the run_kani_proofs \
                  seam this entry once recorded went with proof_runner.rs. grep -ril 'kani' over all \
                  oyatie *.rs returned ZERO hits, so nothing there receives it either; it moves as source.",
    },
    MigrationEntry {
        component: "lib.rs",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "n/a — repo scaffolding",
        counterpart_loc: 0,
        evidence: "117 lines; crate root and module declarations for the Anvil library. Pure repo \
                  scaffolding, disappears at absorption.",
    },
    MigrationEntry {
        component: "local_inner_loop",
        verdict: Verdict::Superseded,
        confidence: Confidence::Probable,
        oyatie_counterpart: "governance/check/pre-push + libs/oya-governance-pre-push-kernel + dev-cli \
                            pre_push_contract_gate",
        counterpart_loc: 410,
        evidence: "176 lines, PURE. FastValidator + validate_pre_commit. oyatie owns the pre-push contract \
                  as a catalog gate ('pre-push-contract') implemented by governance/check/pre-push (299) + \
                  oya-governance-pre-push-kernel (111) + dev-cli pre_push_contract_gate.",
    },
    MigrationEntry {
        component: "lockfile_reconciler.rs",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "libs/oya-cargo-lock-transform-kernel + tools/oya-cargo-lock-merge-driver-app",
        counterpart_loc: 760,
        evidence: "209 lines, PROC+GH. Reconciles Cargo.lock conflicts on a PR. oyatie owns this as \
                  oya-cargo-lock-transform-kernel (760) plus a registered git merge driver \
                  (tools/oya-cargo-lock-merge-driver-app), which handles the conflict at merge time rather \
                  than after the fact.",
    },
    MigrationEntry {
        component: "main.rs",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "n/a — repo scaffolding",
        counterpart_loc: 0,
        evidence: "329 lines; the `anvil` binary entrypoint and daemon bootstrap. Exists only because Anvil \
                  is its own repo.",
    },
    MigrationEntry {
        component: "mainline_ci_healer",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "ci/controller/{kernel,app,github-adapter}",
        counterpart_loc: 5964,
        evidence: "169 lines, GH. check_and_heal_mainline_branches + analyze_failed_job_log. oyatie's \
                  ci/controller (kernel 2699 + app 1641 + github-adapter 1624) owns mainline job \
                  observation and commit-status reconciliation but does not heal; the healer survives with \
                  the forge/job source rewired.",
    },
    MigrationEntry {
        component: "merge_enlister.rs",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "ci/tide/{kernel,app,github-adapter}",
        counterpart_loc: 2256,
        evidence: "297 lines, PROC+GH. enlist_into_merge_queue + ensure_approving_review + \
                  reconcile_pr_title_and_scope. oyatie's ci/tide is the merge-queue capability: \
                  tide/kernel (1224) owns is_mergeable with the author!=reviewer approval policy, plus app \
                  (444) and github-adapter (588).",
    },
    MigrationEntry {
        component: "metrics",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "libs/oya-shared-hyperscaler-metrics-kernel + adapter-prometheus + adapter-otlp",
        counterpart_loc: 2337,
        evidence: "145 lines; a hand-rolled Prometheus text-exposition registry. oyatie owns metric \
                  emission as a mandated substrate: oya-shared-hyperscaler-metrics-kernel (1189, 'the \
                  canonical surface every oyatie microservice must implement') with prometheus (597) and \
                  OTLP (551) adapters, plus governance/check/metric-cardinality (170).",
    },
    MigrationEntry {
        component: "microbenchmark_ratchet",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "governance/check/benchmark + governance/check/perf-budget",
        counterpart_loc: 608,
        evidence: "149 lines, PURE. CriterionDiffAnalyzer over benchmark samples. Same partial as \
                  criterion_bench_ratchet: oyatie has benchmark (264) + perf-budget (344) policy but no \
                  Criterion output parser.",
    },
    MigrationEntry {
        component: "migration_orchestrator",
        verdict: Verdict::Rewired,
        confidence: Confidence::Unresolved,
        oyatie_counterpart: "libs/oya-check-saga-shape + governance/check/shardability (partial)",
        counterpart_loc: 881,
        evidence: "191 lines, PURE. MigrationPhase lifecycle + validate_migration_sql. oyatie's \
                  oya-check-saga-shape (597) validates saga definitions and shardability (284) validates \
                  migration SQL for tenant_id, but neither models an expand/backfill/contract migration \
                  lifecycle. Counterpart NOT established for the lifecycle half.",
    },
    MigrationEntry {
        component: "modularization_guard.rs",
        verdict: Verdict::Migrating,
        confidence: Confidence::Verified,
        oyatie_counterpart: "gate id 'buildability-line-count' (shell-implemented, ADR-0523 violation)",
        counterpart_loc: 0,
        evidence: "155 lines, PURE; MAX_RECOMMENDED_LINES=300 file-length gate. oyatie HAS this gate id \
                  ('buildability-line-count') but implements it in shell: \
                  libs/oya-governance-gate-catalog-domain/src/lib.rs:259 registers it as `bash \
                  tools/governance/adr-0221-governance-gates.sh buildability-line-count`, and \
                  ci/facade/baseline-ratchet/tests/gate_registration.rs:1892 flags it as violating \
                  ADR-0523's zero-shell posture. Anvil's Rus...",
    },
    MigrationEntry {
        component: "monorepo_guard",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "ci/facade/module-membership + governance/check/{authority-cohesion,cohesion,no-grouping} \
                            + libs/oya-governance-cohesion-kernel",
        counterpart_loc: 2992,
        evidence: "631 lines, NET+PROC+FS+ASYNC. ComponentDispositionClassifier + SSOT authority location + \
                  harness quarantine. oyatie owns exactly this as module-membership (2080, 'the \
                  anti-junk-drawer authority: every crate maps to EXACTLY ONE registered capability') plus \
                  authority-cohesion (416), cohesion (208), no-grouping (181) and \
                  oya-governance-cohesion-kernel (107).",
    },
    MigrationEntry {
        component: "pr_self_healer.rs",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found; git seam rewires to ci/tide + merge drivers",
        counterpart_loc: 0,
        evidence: "175 lines, PROC+FS+ASYNC. auto_heal_pr_branch (rebase/conflict resolution). oyatie \
                  references an IP-005 fix loop and carries merge-driver tooling, but no PR-branch healer \
                  crate exists; the git and forge adapters swap.",
    },
    MigrationEntry {
        component: "pre_merge_guard",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "libs/oya-governance-gate-catalog-domain + libs/oya-governance-pr-merge-gate-kernel + \
                            libs/oya-ci-gate-contract + libs/oya-ci-materializer-kernel",
        counterpart_loc: 4778,
        evidence: "1771 lines (evaluator 877, report 546, matrix 246, scanner 91). The 68-row certification \
                  matrix (matrix.rs header: 'The 68-row certification table'; the published header string \
                  still says 70). oyatie owns gate orchestration as \
                  libs/oya-governance-gate-catalog-domain (1541, 232 registered gate ids), \
                  oya-governance-pr-merge-gate-kernel (830), oya-ci-gate-contract (415), \
                  oya-ci-materializer-kernel (1992), over 61 ci/...",
    },
    MigrationEntry {
        component: "predictive_test_selector",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "ci/facade/affected-target-set",
        counterpart_loc: 10456,
        evidence: "428 lines, PROC+ASYNC. WorkspaceDagSelector + calculate_pruning_ratio. oyatie's \
                  affected-target-set (10456) is the same capability done properly: 'the pure decision \
                  kernel that turns a merge-base diff into the buck2 target set a PR MUST build and test', \
                  explicitly citing Bazel target determination.",
    },
    MigrationEntry {
        component: "preview_env_reaper",
        verdict: Verdict::Migrating,
        confidence: Confidence::Verified,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "160 lines, PURE. sweep_stale_previews over PreviewEnvironmentInfo. grep -rilE 'preview \
                  environment|ephemeral env' over oyatie *.rs returned ZERO hits. \
                  (ci/facade/stale-artifact-detection reaps repo artifacts, not deployed environments.)",
    },
    MigrationEntry {
        component: "progressive_rollout",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "247 lines, PURE. DeploymentRing + RingScheduler + validate_bake_window + \
                  validate_geo_paired_exclusion. grep -rilE 'progressive|deployment ring' matched only \
                  flags/core/evaluation-domain; no ring-based rollout orchestrator in oyatie.",
    },
    MigrationEntry {
        component: "psa_admission_guard",
        verdict: Verdict::Superseded,
        confidence: Confidence::Probable,
        oyatie_counterpart: "dev-cli cloud_iac_kubewarden_admission_gate + infra/kyverno + ci/facade/k8s-program-docs",
        counterpart_loc: 2884,
        evidence: "172 lines, PURE. Pod Security Admission manifest rules. oyatie owns admission policy as \
                  the cloud-iac-kubewarden-admission-policy gate \
                  (dev-cli/src/cloud_iac_kubewarden_admission_gate.rs: proves the repo carries a \
                  Kubewarden PolicyServer + ClusterAdmissionPolicy) plus infra/kyverno and \
                  ci/facade/k8s-program-docs (2884).",
    },
    MigrationEntry {
        component: "publish",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "Reclassified Migrating -> Rewired by the migration-boundary gate. The renderer \
                   itself is pure, but it reads two data sources oyatie supersedes: the fidelity \
                   registry (governance/check/honest-claims, 1878 lines) and the certification \
                   report. Its PORT -- produce a signed, findings-only scorecard from gate \
                   results -- survives absorption; the source behind it swaps. That is the \
                   definition of Rewired, and calling it Migrating asserted it could move as \
                   source when it cannot.",
    },
    MigrationEntry {
        component: "queue_healer (dir)",
        verdict: Verdict::Migrating,
        confidence: Confidence::Verified,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "136 lines, PURE. MergeTrainBisector::bisect_batch — binary-search over a speculative \
                  merge batch to find the culprit PR. grep -ril 'bisect' over oyatie *.rs matched only \
                  ci/facade/scm-facts-snapshot incidentally; ci/tide has no batch bisection.",
    },
    MigrationEntry {
        component: "queue_healer.rs",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "ci/tide/{kernel,app,github-adapter} (partial)",
        counterpart_loc: 2256,
        evidence: "332 lines, PROC+GH+ASYNC. heal_ejected_pr + extract_pr_number_from_merge_ref. oyatie's \
                  ci/tide (2256) owns merge eligibility and merging but not ejection healing; forge \
                  adapter swaps, healing logic survives.",
    },
    MigrationEntry {
        component: "recovery",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "373 lines (blue_green_supervisor 152, reconciliation_sweep 216), PROC+FS+GH+ASYNC. \
                  execute_atomic_binary_swap + spawn_green_and_drain_blue + run_full_sweep over open PRs. \
                  grep -rilE 'blue.green|blue_green' over oyatie *.rs returned ZERO hits; the self-upgrade \
                  supervisor is Anvil-daemon-specific and the PR sweep rewires onto oyatie's forge seam.",
    },
    MigrationEntry {
        component: "remote_cache_optimizer",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "367 lines, FS. CacheHitRateRatchet + CacheKeyGenerator + CasBitRotScrubber. Module doc \
                  admits the ratchet ran over four literals. oyatie has a toolchains/cache directory but \
                  no remote-build-cache crate in the 844-crate index; no counterpart established.",
    },
    MigrationEntry {
        component: "replay_harness",
        verdict: Verdict::Superseded,
        confidence: Confidence::Probable,
        oyatie_counterpart: "gate 'audit-chain-replay' + audit/core/{verification-domain,chain-domain}",
        counterpart_loc: 0,
        evidence: "105 lines, PURE. TraceReplayer + evaluate_replay_parity. oyatie registers \
                  'audit-chain-replay' as a catalog gate and owns audit/core/verification-domain + \
                  audit/core/chain-domain + audit/ports/sealing-* as the real deterministic-replay \
                  substrate.",
    },
    MigrationEntry {
        component: "review_memory",
        verdict: Verdict::Migrating,
        confidence: Confidence::Unresolved,
        oyatie_counterpart: "not established (candidate: libs/oya-governance-mistakes-ledger-kernel)",
        counterpart_loc: 527,
        evidence: "159 lines, PURE. ReviewMemoryStore recalling prior architectural findings and author \
                  feedback. Closest oyatie concept is libs/oya-governance-mistakes-ledger-kernel (527), \
                  but I could not establish that it recalls prior review findings for a diff — the \
                  function is unverified. Conservative: keep.",
    },
    MigrationEntry {
        component: "reviewer (dir)",
        verdict: Verdict::Rewired,
        confidence: Confidence::Verified,
        oyatie_counterpart: "intelligence/core/pr-review-dispatcher-app (fanout + rollup)",
        counterpart_loc: 1404,
        evidence: "235 lines. CanonicalLens is a CLOSED 16-lens taxonomy \
                  (CartesianDoubt..ZeroTrustDefenseInDepth, lens_feedback_engine.rs:7-24) with \
                  reconcile_lens_findings. oyatie's pr-review-dispatcher-app (1404) has a DIFFERENT closed \
                  taxonomy — 29 facets F1..F13/M1..M2/A1..A7 (fanout.rs:18-44) — plus rollup_verdict + \
                  audit_panel_completeness. The rollup function overlaps; the taxonomies must be \
                  reconciled, they are not the same...",
    },
    MigrationEntry {
        component: "reviewer.rs",
        verdict: Verdict::Rewired,
        confidence: Confidence::Verified,
        oyatie_counterpart: "intelligence/core/subagent-runtime-kernel + subagent-runtime-app (NOT \
                            pr-review-dispatcher-app)",
        counterpart_loc: 1887,
        evidence: "504 lines. RESOLVES the flagged-unproven overlap: pr-review-dispatcher-app does NOT \
                  supersede this — its own header states the subagent runtime 'is not yet wired into a \
                  Rust binary anywhere in the workspace' and it emits APPROVE tagged \
                  subagent_runtime_pending=true. The actual superseding crates are subagent-runtime-kernel \
                  (767, FacetPromptTemplate + render_system_prompt + SubagentPort + \
                  SubagentResponse::from_mod...",
    },
    MigrationEntry {
        component: "roadmap_guard",
        verdict: Verdict::Superseded,
        confidence: Confidence::Probable,
        oyatie_counterpart: "ci/facade/planning-projection + dev-cli masterplan_drift_gate + \
                            planning_ssot_coverage_gate + adr_planning_completeness_gate",
        counterpart_loc: 918,
        evidence: "270 lines, FS. evaluate_pr_scope + verify_issue_roadmap_alignment. oyatie owns planning \
                  alignment as ci/facade/planning-projection (918) plus catalog gates 'masterplan-drift', \
                  'adr-planning-completeness', and 'planning-ssot-coverage' implemented in dev-cli.",
    },
    MigrationEntry {
        component: "rust_language_policy",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "libs/oya-governance-banned-primitives-kernel (partial)",
        counterpart_loc: 988,
        evidence: "572 lines, PROC+FS+ASYNC. UpstreamRustSkillsSyncer pulls rules from an upstream source, \
                  RustQualityEngine scans diffs. oyatie's banned-primitives-kernel (988) covers the \
                  banned-primitive subset but is scoped to fenced agent-instruction blocks, not idiomatic \
                  Rust rules, and has no upstream syncer. Engine survives, rule source rewires.",
    },
    MigrationEntry {
        component: "schema_evolution",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "governance/check/event-schema-versioning",
        counterpart_loc: 566,
        evidence: "130 lines, PURE. CompatibilityChecker over schema diffs. oyatie's \
                  event-schema-versioning gate (566) enforces the same class against every AsyncAPI 3.1.0 \
                  message ('MUST declare a version header matching the SemVer pattern').",
    },
    MigrationEntry {
        component: "self_governance",
        verdict: Verdict::Rewired,
        confidence: Confidence::Verified,
        oyatie_counterpart: "intelligence/core/provider-pool-kernel + provider-pool-app (account_pool only)",
        counterpart_loc: 10285,
        evidence: "1372 lines total. account_pool subtree (676) IS superseded (see separate entry). The \
                  remaining 696 lines have NO counterpart: deathloop_detector.rs (208), \
                  process_registry.rs (168), quota_enforcer.rs (148), resource_reaper.rs (104), mod.rs \
                  (68). grep -ril 'deathloop' and 'death_loop' over oyatie *.rs both returned ZERO hits; \
                  no orphan-subprocess reaper found. Marking standalone would delete real work.",
    },
    MigrationEntry {
        component: "self_governance/account_pool (subtree)",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "intelligence/core/provider-pool-kernel + intelligence/core/provider-pool-app",
        counterpart_loc: 10285,
        evidence: "676 lines (manager 317, types 201, quota_view 88, defaults 60, mod 10): \
                  add_account/drain_account/resume_account/lease_account_with_affinity/mark_rate_limited(cooldown). \
                  oyatie's provider-pool-kernel implements the same rotation with a stronger contract — \
                  in_cooldown() at src/lib.rs:256, window_for(FailureKind, consecutive_failures) at :274, \
                  populate_quarantine_from_changes at :312, and quota-aware cooldown rotati...",
    },
    MigrationEntry {
        component: "semantic_abi_ratchet",
        verdict: Verdict::Rewired,
        confidence: Confidence::Unresolved,
        oyatie_counterpart: "intelligence/core/api-semver-domain + libs/oya-shared-semver-check-cli",
        counterpart_loc: 521,
        evidence: "169 lines, PURE. SignatureScanner detecting breaking ABI changes in a diff. oyatie \
                  registers gate id 'api-semver' and has intelligence/core/api-semver-domain (491), but \
                  oya-shared-semver-check-cli is only 30 lines and I could not establish that the domain \
                  crate scans Rust signatures. Diff scanner likely survives.",
    },
    MigrationEntry {
        component: "shadow_traffic_harness",
        verdict: Verdict::Migrating,
        confidence: Confidence::Verified,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "191 lines, PURE. Module doc admits it mirrored no traffic (four literals). grep -rilE \
                  'shadow traffic|dark traffic|traffic mirror' over oyatie *.rs returned ZERO hits.",
    },
    MigrationEntry {
        component: "shuffle_shard_simulator",
        verdict: Verdict::Rewired,
        confidence: Confidence::Verified,
        oyatie_counterpart: "intelligence/core/shuffle-sharding + governance/check/shardability",
        counterpart_loc: 612,
        evidence: "257 lines, PURE (mod 100 + math). oyatie's intelligence/core/shuffle-sharding (328) has \
                  select_shuffle_shard at src/lib.rs:105 — superseding select_tenant_cells — but grep for \
                  'overlap|combinations|blast' inside that crate returned ZERO hits, so Anvil's \
                  ShuffleShardMath::calculate_combinations / evaluate_overlap / BlastRadiusMetrics is \
                  unique. Selection rewires, blast-radius math survives.",
    },
    MigrationEntry {
        component: "slo_canary_guard",
        verdict: Verdict::Rewired,
        confidence: Confidence::Verified,
        oyatie_counterpart: "k8s/core/sla-observability-kernel + observability/core/domain + ci/facade/slo-coverage",
        counterpart_loc: 4613,
        evidence: "293 lines, FS. parse_openslo_yaml (OpenSloSpec/Objective/TimeWindow) + burn-rate matrix; \
                  module doc admits the burn rates were literals. oyatie HAS the real burn-rate engine: \
                  k8s/core/sla-observability-kernel (1560) declares 'Multi-window multi-burn-rate policy: \
                  page (fast) and ticket (slow) tiers' at src/lib.rs:60 and observability/core/domain \
                  exports burn_rate/classify_burn_rate/error_budget_remaining_ratio. But...",
    },
    MigrationEntry {
        component: "stack_whitelist_guard",
        verdict: Verdict::Superseded,
        confidence: Confidence::Probable,
        oyatie_counterpart: "governance/check/client-stack-discipline + governance/check/vendor-lockin-discipline + \
                            libs/oya-governance-banned-primitives-kernel",
        counterpart_loc: 2878,
        evidence: "188 lines, PURE. BANNED_UNAPPROVED_STACK table (redis::, mongodb::, mysql::, actix_web, \
                  rocket::, cassandra::) tied to ADR-0700..0718. oyatie enforces the same class more \
                  broadly: client-stack-discipline (610, ADR-0185 native-per-platform), \
                  vendor-lockin-discipline (1280, tiered vendor classification), banned-primitives-kernel \
                  (988), automation-language-policy (5421).",
    },
    MigrationEntry {
        component: "stacked_diffs",
        verdict: Verdict::Migrating,
        confidence: Confidence::Verified,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "114 lines, PURE. StackedDagManager + compute_stack_plan over dependent branches. grep \
                  -rilE 'stacked|stack of branches' over oyatie *.rs returned ZERO hits.",
    },
    MigrationEntry {
        component: "state.rs",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found; store would rewire to libs/oya-shared-postgres-command-kernel + \
                            transactional-outbox-kernel",
        counterpart_loc: 0,
        evidence: "306 lines, FS+ASYNC. StateManager with WalEntry, acquire_pr_lock, record_certification, \
                  clear_reviewed_sha — durable PR state for Anvil's daemon. No WAL/PR-state store found in \
                  oyatie (WAL grep hits were audit-chain prose). The state model survives; the store \
                  rewires onto oyatie's postgres/outbox substrate.",
    },
    MigrationEntry {
        component: "supply_chain_guard (dir)",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "ci/facade/supply-chain-audit + libs/oya-advisory-mirror-kernel + \
                            governance/check/supply-chain + libs/oya-governance-supply-chain-kernel",
        counterpart_loc: 5588,
        evidence: "183 lines, PROC+ASYNC. OsvAdvisoryStream (network OSV query) + SlsaAttestor generating \
                  SLSA L2 provenance. oyatie replaced exactly this pattern: ci/facade/supply-chain-audit \
                  (2675) header records that a shell cargo-audit gate was reverted and replaced by a \
                  hermetic match of the lockfile corpus against a VENDORED RustSec snapshot distilled by \
                  oya-advisory-mirror-kernel (517), plus governance/check/supply-chain (205...",
    },
    MigrationEntry {
        component: "supply_chain_guard.rs",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "ci/facade/supply-chain-audit + governance/check/supply-chain + \
                            libs/oya-advisory-mirror-kernel",
        counterpart_loc: 5247,
        evidence: "137 lines, PURE. audit_supply_chain + generate_slsa_provenance. Same oyatie stack \
                  supersedes; oyatie additionally covers the VEX policy half \
                  (governance/check/supply-chain carries VexStatus, requires_vex_justification, \
                  block_missing_vex, vex_artifacts_signed at src/lib.rs:190-291).",
    },
    MigrationEntry {
        component: "task_orchestrator",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "workflow/core/trigger-orchestrator-{kernel,usecase} + intelligence/core/dispatch-usecase",
        counterpart_loc: 3083,
        evidence: "608 lines, PROC+FS+GH+ASYNC. TaskDagSequencer + AutonomousFixEngine + SourceDocVerifier \
                  (scan_adrs_for_work, verify_scoped_task). oyatie's \
                  workflow/core/trigger-orchestrator-{kernel 990, usecase 1432} plus \
                  intelligence/core/dispatch-usecase (661) own DAG sequencing and dispatch; the \
                  ADR-ingestion + source-doc verification half has no counterpart.",
    },
    MigrationEntry {
        component: "telemetry_store",
        verdict: Verdict::Rewired,
        confidence: Confidence::Verified,
        oyatie_counterpart: "none for DORA; store rewires to libs/oya-shared-timeseries-kernel + olap-client-kernel",
        counterpart_loc: 0,
        evidence: "287 lines, FS+ASYNC. DoraCalculator + gate-failure heatmap + PR history. grep -rilE \
                  'DoraMetric|deployment_frequency' over all oyatie *.rs returned ZERO hits — DORA is not \
                  implemented in oyatie. The calculator survives; the JSON-file store rewires onto \
                  oyatie's OLAP/timeseries kernels.",
    },
    MigrationEntry {
        component: "trace_context_guard",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "governance/check/otel-trace-propagation + libs/oya-shared-tracing-client-kernel",
        counterpart_loc: 1303,
        evidence: "203 lines. SpanTracker + scan_detached_tasks for missing trace propagation. oyatie's \
                  otel-trace-propagation gate (781) enforces the identical ADR-0145 Invariant 2 ('every \
                  inter-microservice call must propagate the W3C traceparent header') by scanning gRPC \
                  client adapters, backed by oya-shared-tracing-client-kernel (522) and the http telemetry \
                  middleware.",
    },
    MigrationEntry {
        component: "unresolved_review_guard",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "none found",
        counterpart_loc: 0,
        evidence: "207 lines, PROC+GH+ASYNC. ThreadScanner over GitHub review threads. No \
                  unresolved-review-thread gate found in oyatie's 232-id gate catalog or crate index; only \
                  the GraphQL thread-fetch adapter rewires.",
    },
    MigrationEntry {
        component: "upgrade_train",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "ci/facade/dep-freshness + ci/facade/dependency-automation + \
                            ci/facade/rust-toolchain-bump-proposer",
        counterpart_loc: 5323,
        evidence: "130 lines, PURE. DependencyUpgradeCandidate + TrainOrchestrator. oyatie owns dependency \
                  currency as three mature gates: dep-freshness (1484, hermetic gate over a committed \
                  freshness mirror with snapshot_date as the as-of clock), dependency-automation (1812), \
                  rust-toolchain-bump-proposer (2027).",
    },
    MigrationEntry {
        component: "vex_scanner",
        verdict: Verdict::Rewired,
        confidence: Confidence::Verified,
        oyatie_counterpart: "governance/check/supply-chain (VEX policy only)",
        counterpart_loc: 2055,
        evidence: "137 lines, PURE. OpenVexReachabilityScanner + CallgraphPruner. oyatie's \
                  governance/check/supply-chain (2055) already carries the VEX POLICY (VexStatus, \
                  NotAffected=>'not_affected', requires_vex_justification, block_missing_vex, \
                  vex_statuses_checked), but grep -rilE 'callgraph|call graph' over oyatie *.rs matched \
                  only ci/facade/endpoint-authorization-coverage — reachability pruning has no \
                  counterpart. Policy rewire...",
    },
    MigrationEntry {
        component: "wasm_sandbox",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "libs/oya-shared-wasm-runtime-kernel + governance/check/wasm-runtime-discipline",
        counterpart_loc: 780,
        evidence: "120 lines, PURE. WasmPolicySandbox + run_sandboxed_bytecode_checks. oyatie mandates \
                  Wasmtime through one substrate — oya-shared-wasm-runtime-kernel (488) — and enforces it \
                  with wasm-runtime-discipline (292: 'no microservice may import wasmtime, wasmer, or \
                  wasmedge directly').",
    },
    MigrationEntry {
        component: "watchdog",
        verdict: Verdict::Rewired,
        confidence: Confidence::Probable,
        oyatie_counterpart: "tools/oya-lane-supervisor-app (partial, different function)",
        counterpart_loc: 1712,
        evidence: "314 lines, ASYNC. PipelineWatchdog with DynamicSlaProfile, compute_envelope, \
                  ActivityHandle/report_progress. oyatie's tools/oya-lane-supervisor-app (1712) parses \
                  lane state JSON — it is not a runtime watchdog. The envelope logic is pure and survives; \
                  the async supervision seam rewires.",
    },
    MigrationEntry {
        component: "webhook",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "oya/ci-webhook-gateway (kernel 1225 + app 938 + github-adapter 251 + ed25519-adapter 211 \
                            + authz-cedar-adapter 335)",
        counterpart_loc: 2960,
        evidence: "2707 lines across 7 files. Ingress half (webhook_handlers 558 + repo_guard 91: \
                  verify_github_hmac, WebhookPullRequest/Review/WorkflowRun/MergeGroup parsing) is \
                  superseded — oya-ci-webhook-gateway-kernel implements route_github_event at \
                  src/lib.rs:278 plus WebhookSignature, WebhookAuthzRequest/AuthzDecision, \
                  WebhookAuditEvent, across a 2960-line five-crate gateway. The remaining half \
                  (manual_handlers 535, admin_aut...",
    },
    MigrationEntry {
        component: "zero_day_patcher",
        verdict: Verdict::Superseded,
        confidence: Confidence::Probable,
        oyatie_counterpart: "libs/oya-advisory-mirror-kernel + ci/facade/supply-chain-audit + \
                            ci/facade/dependency-automation",
        counterpart_loc: 5004,
        evidence: "159 lines, PURE. AdvisoryListener + reconcile_advisories against SecurityAdvisory. \
                  oyatie's advisory pipeline is real and hermetic: oya-advisory-mirror-kernel (517, \
                  distills RustSec advisory markdown with a deterministic content hash) feeding \
                  supply-chain-audit (2675), with dep-freshness (1484) and dependency-automation (1812) \
                  driving the bumps.",
    },
    MigrationEntry {
        component: "zero_trust_workload",
        verdict: Verdict::Superseded,
        confidence: Confidence::Verified,
        oyatie_counterpart: "iam/core/identity-workload-svid-kernel + identity-workload-domain + \
                            iam/adapters/identity-workload-svid-{trustd,operator-k8s} + \
                            iam/facade/identity-workload-rest",
        counterpart_loc: 9000,
        evidence: "202 lines, PURE. audit_cleartext_transport + IdentityAuditor -- renamed from \
                  audit_spiffe_and_mtls, which named a property no text scan can observe. \
                  Searched by CONCEPT, not \
                  vendor name: oyatie implements SPIFFE as identity-workload-svid-*. \
                  iam/core/identity-workload-svid-kernel (586) has SpiffeId::parse, WorkloadPath, \
                  trust_domain_authority, bind_caller_tenant, X509Svid, SvidRequest; plus \
                  identity-workload-domain (1333), svid-trustd adapter (750), svid-operator-k8s (901), \
                  identity-workload-rest (5430),...",
    },
    MigrationEntry {
        component: "shape (dir)",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "ci/facade/module-membership + repo-root-hygiene + baseline-ratchet + \
                            layer-dependency-acyclicity (the shape engine generalises these over a \
                            tenant-carried spec)",
        counterpart_loc: 0,
        evidence: "Shape Program engine (plan breezy-purring-crayon, 2026-08-20): tenant-carried \
                  .anvil/shape.json spec, pure placement/measurement core, ratchet consumer. Built in \
                  core/ports/adapters/facade form from day one so it is the first conformant unit. \
                  Counterpart loc deliberately 0: the four oyatie gates were not summed because the \
                  engine is a generalisation, not a transcription of any one of them.",
    },
    MigrationEntry {
        component: "ratchet (dir)",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "ci/facade/baseline-ratchet",
        counterpart_loc: 0,
        evidence: "Generic shrink-only ratchet (plan breezy-purring-crayon G15): frozen reference at \
                  merge-base, per-rule mode as data, frozen_empty rules, one-way sign-off door with \
                  inert-entry failure. Transcribes oyatie's baseline-ratchet semantics over string keys \
                  so any gate can consume it. Counterpart loc 0: not summed, transcription not copy.",
    },
    MigrationEntry {
        component: "change_delivery (dir)",
        verdict: Verdict::Migrating,
        confidence: Confidence::Probable,
        oyatie_counterpart: "ci/controller (landing) + tools/oya-reorg-codemod-app (rewrite)",
        counterpart_loc: 0,
        evidence: "Shape Program change delivery (plan breezy-purring-crayon B1): move plan, owner-disjoint \
                  sharding, I8 purity check, landing policy and pure admission, ledger. Pure core and \
                  dry-run facade; ports/adapters that open PRs follow. Counterpart loc 0: none measured.",
    },
];
