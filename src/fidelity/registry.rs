//! The declared fidelity of each audited gate.
//!
//! # Scope of this registry
//!
//! Entries exist ONLY for gates whose implementation was read directly during
//! the 2026-08-19/20 audit. Gates that were not read have no entry and are
//! counted as `unaudited` in the gap report.
//!
//! This is deliberate. Guessing a fidelity would reproduce exactly the failure
//! being corrected: a confident claim with nothing behind it. An honest
//! "not yet audited" is worth more than an invented "Heuristic".

use super::{Fidelity, GateFidelity};

/// Gates audited by direct code reading, with file:line evidence in the gap text.
pub const AUDITED_GATES: &[GateFidelity] = &[
    GateFidelity {
        gate_id: "coverage_status",
        aspiration: "Measure statement and branch coverage of the lines added by this PR, via compiler \
                     instrumentation, and block below a threshold.",
        reference: "cargo-llvm-cov; Google TAP coverage instrumentation",
        fidelity: Fidelity::Aspirational,
        gap: "Runs no compiler coverage instrumentation tool. When no executable lines are added it reports \
              `NothingToMeasure`, and with no llvm-cov tool it reports `NotMeasured` (coverage_guard.rs:85-88,135-138).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "kani_status",
        aspiration: "Discharge memory-safety proof obligations for unsafe blocks using a bounded model \
                     checker.",
        reference: "Kani / CBMC; AWS Automated Reasoning Group",
        fidelity: Fidelity::Heuristic,
        gap: "Checks whether a `SAFETY:` documentation comment appears near `unsafe` (kani_guard/mod.rs:49-50). \
              When the kani binary is absent it returns status `VERIFIED_STATIC` -- a missing verifier \
              reporting success (kani_guard/proof_runner.rs:40-44).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "slo_status",
        aspiration: "Evaluate multi-window multi-burn-rate error budget consumption against live production \
                     telemetry (14.4x/1h, 6x/6h).",
        reference: "Google SRE Workbook, multiwindow multi-burn-rate alerting",
        fidelity: Fidelity::Aspirational,
        gap: "Queries no telemetry. With no Prometheus or `OpenTelemetry` endpoint configured, the gate \
              structurally validates touched OpenSLO specs and reports `NotMeasured` \
              (slo_canary_guard/mod.rs:45-47,154-157).",
        blocked_on: Some("a reachable Prometheus or OpenTelemetry endpoint"),
    },
    GateFidelity {
        gate_id: "remote_cache_status",
        aspiration: "Report the real distributed build-cache hit rate and ratchet it upward.",
        reference: "Bazel/Buck2 remote execution CAS statistics",
        fidelity: Fidelity::Aspirational,
        gap: "With no sccache or Buck2 CAS statistics endpoint configured, reports `NotMeasured` \
              (remote_cache_optimizer/mod.rs:82-85). Cache keys use non-cryptographic FNV-1a hashing via \
              `compute_cache_key` (remote_cache_optimizer/cache_keys.rs:16).",
        blocked_on: Some("sccache or Buck2 CAS statistics"),
    },
    GateFidelity {
        gate_id: "mutation_status",
        aspiration: "Inject AST mutations, run the suite against each mutant, and require a kill rate above \
                     a threshold.",
        reference: "cargo-mutants; Meta TestInfra mutation testing",
        fidelity: Fidelity::Heuristic,
        gap: "Compiles and runs no mutants. Checks whether a changed filename contains \"test\" \
              (chaos_mutation_guard.rs:57,76).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "supply_chain_status",
        aspiration: "Resolve the dependency graph, match it against a live vulnerability database, and \
                     emit a signed SBOM.",
        reference: "osv-scanner, cargo-deny, CycloneDX",
        fidelity: Fidelity::Heuristic,
        gap: "Matches a short list of banned package names by regex. No dependency resolution, no CVE \
              database, no SBOM.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "cosign_status",
        aspiration: "Sign artefacts through Fulcio OIDC and record inclusion in the Rekor transparency log.",
        reference: "Sigstore; SLSA provenance levels",
        fidelity: Fidelity::Aspirational,
        gap: "Emits the literal string \"-----BEGIN CERTIFICATE-----\\nMIIC...\" as a certificate chain and \
              formats a fake Rekor uuid from the digest prefix (cosign_signer/sigstore_attestor.rs:26).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "formal_verification_status",
        aspiration: "Encode authorization policy into SMT and prove non-escalation with a solver.",
        reference: "AWS Zelkova; Z3",
        fidelity: Fidelity::Heuristic,
        gap: "A chain of policy_content.contains(..) tests. No solver exists. The file and its types \
              were renamed from smt_solver.rs/SmtConstraintEngine to say so \
              (formal_verification/policy_scanner.rs:28).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "deadlock_status",
        aspiration: "Build an inter-procedural lock-acquisition graph and detect cycles.",
        reference: "Tarjan strongly connected components; Meta Infer",
        fidelity: Fidelity::Aspirational,
        gap: "Matches four literal identifiers -- session_lock, user_mutex, global_state, cluster_mutex -- \
              which occur ZERO times in this repository outside the analyzer's own test fixture. It cannot \
              fire on any real code (deadlock_analyzer/lock_graph.rs:27-36).",
        blocked_on: Some("the Phase G L1 code graph, for real call edges"),
    },
    GateFidelity {
        gate_id: "semantic_abi_status",
        aspiration: "Compare the public API surface across revisions and require a semver major bump for \
                     breaking changes.",
        reference: "cargo-semver-checks over rustdoc JSON",
        fidelity: Fidelity::Heuristic,
        gap: "Whole-diff predicate: contains(\"-pub fn \") && !contains(\"+pub fn \"). Removing a public \
              function goes undetected if the same PR adds any other one, and a signature change emits both \
              lines so it can never be detected (semantic_abi_ratchet/signature_scanner.rs:33).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "ci_wallclock_status",
        aspiration: "Measure this PR's actual CI wallclock and ratchet against a trunk baseline.",
        reference: "internal CI wallclock budgets; ADR-0718",
        fidelity: Fidelity::Aspirational,
        gap: "Without GitHub Actions timing API access, reports `NotMeasured` rather than measuring real \
              build duration or cost (ci_wallclock_ratchet/mod.rs:83-86).",
        blocked_on: Some("GitHub Actions timing API"),
    },
    GateFidelity {
        gate_id: "cluster_audit_status",
        aspiration: "Compare live cluster state against the declared manifests and report drift.",
        reference: "ArgoCD drift detection; kube-rs",
        fidelity: Fidelity::Aspirational,
        gap: "With no Kubernetes API or ArgoCD access configured, reports `NotMeasured` and performs no \
              Git comparison (cluster_state_auditor/mod.rs:79-82).",
        blocked_on: Some("Kubernetes API or ArgoCD access"),
    },
    GateFidelity {
        gate_id: "shadow_traffic_status",
        aspiration: "Mirror production traffic to a shadow deployment and diff the responses.",
        reference: "Envoy request mirroring; Diffy",
        fidelity: Fidelity::Aspirational,
        gap: "Without traffic mirror infrastructure or a replay target configured, reports `NotMeasured` \
              (shadow_traffic_harness/mod.rs:76-79).",
        blocked_on: Some("traffic mirroring infrastructure and a replay target"),
    },
    GateFidelity {
        gate_id: "wasm_sandbox_status",
        aspiration: "Execute untrusted policy in a WebAssembly sandbox with a constrained ABI.",
        reference: "Wasmtime",
        fidelity: Fidelity::Heuristic,
        gap: "No WebAssembly runtime. Checks lower.contains(\"process::abort\") \
              (wasm_sandbox/policy_runner.rs:20).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "zero_trust_workload_status",
        aspiration: "Verify workload identity via SPIFFE/SPIRE issued mTLS certificates.",
        reference: "SPIFFE/SPIRE",
        fidelity: Fidelity::Heuristic,
        gap: "Checks for the substring \"http://\" in the diff \
              (zero_trust_workload/identity_auditor.rs:15).",
        blocked_on: Some("a SPIFFE/SPIRE control plane"),
    },
    GateFidelity {
        gate_id: "ghost_migration_status",
        aspiration: "Validate schema migrations against a shadow database for lock-free application and \
                     rollback parity.",
        reference: "gh-ost; pt-online-schema-change",
        fidelity: Fidelity::Heuristic,
        gap: "Regex scan for CONCURRENTLY, DROP COLUMN and NOT NULL. Connects to no database and runs no \
              migration.",
        blocked_on: Some("a shadow database"),
    },
    GateFidelity {
        gate_id: "doc_parity_status",
        aspiration: "Verify documentation parity with the change, and amend affected documents.",
        reference: "docs-as-code; Google g3doc",
        fidelity: Fidelity::Partial,
        gap: "Fails closed, creates missing ADRs, and corpus_sync amends owned pages so published \
              gate counts match TOTAL_GATES. generate_and_write_docs still writes only when a file \
              does not exist (doc_guard/mod.rs:350-375); it does not rewrite existing documents.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "predictive_test_status",
        aspiration: "Select the minimal set of affected test targets from a real dependency graph, and \
                     report the pruning ratio actually achieved.",
        reference: "Google TAP affected-targets analysis; Meta Predictive Test Selection (arXiv:1810.05286)",
        fidelity: Fidelity::Heuristic,
        gap: "Computes a package DAG, but then sets is_optimized to true (predictive_test_selector/mod.rs:64) \
              so the guard reports PASSED regardless of the subprocess outcome. run_sync_bounded handles \
              metadata subprocesses (predictive_test_selector/workspace_dag.rs:17).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "unresolved_review_status",
        aspiration: "Block on unresolved review threads using authoritative thread state.",
        reference: "GitHub GraphQL pullRequest.reviewThreads.nodes { isResolved }",
        fidelity: Fidelity::Heuristic,
        gap: "Infers resolution from comment text rather than querying isResolved, so informational comments \
              can hold a PR at 'unresolved' indefinitely.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "automated_canary_status",
        aspiration: "Compare canary and baseline metric distributions from a live canary deployment with a \
                     Mann-Whitney U-test, and halt the rollout on a statistically significant regression.",
        reference: "Spinnaker/Kayenta Automated Canary Analysis; Mann-Whitney U-test",
        fidelity: Fidelity::Aspirational,
        gap: "Deploys no canary and queries no telemetry. The review pipeline used to write the samples it \
              then judged. Mann-Whitney is not implemented: evaluate_canary_distributions compares two \
              arithmetic means against a fixed relative bound \
              (automated_canary/statistical_engine.rs:43). With no baseline_samples and no canary_samples \
              the gate now reports NotMeasured instead of a pass.",
        blocked_on: Some(
            "a canary deployment with a queryable Prometheus or OpenTelemetry endpoint",
        ),
    },
    GateFidelity {
        gate_id: "stacked_diffs_status",
        aspiration: "Read the pull request DAG from the forge, order the stack topologically, and verify \
                     every child is rebased on its parent before an atomic merge.",
        reference: "Phabricator/Graphite stacked diffs; Meta Sapling",
        fidelity: Fidelity::Aspirational,
        gap: "Reads no pull request DAG; the review pipeline passed an empty slice on every PR. Given a \
              real stack, compute_stack_plan still returns atomic_merge_ready unconditionally and orders \
              by input order rather than by parent links, so stack_depth is the only thing it derives \
              (stacked_diffs/dag_manager.rs:27-36). With no stack supplied the gate reports NotMeasured.",
        blocked_on: Some("a forge query for the pull requests stacked on this one"),
    },
    GateFidelity {
        gate_id: "microbench_status",
        aspiration: "Run criterion benchmarks on the base and head revisions and ratchet hotpath ns/op \
                     against a published trunk baseline.",
        reference: "criterion.rs; Google Fleetbench",
        fidelity: Fidelity::Aspirational,
        gap: "Executes no benchmark: this repository declares no criterion dependency and has no benches \
              directory, so there is no baseline to ratchet against and the review pipeline used to write \
              a base_ns_per_op equal to its own head_ns_per_op. evaluate_benchmark_diff is honest \
              arithmetic over a caller-supplied sample and is retained as the seam, but it reads neither \
              p99_cpu_cycles_base nor p99_cpu_cycles_head \
              (microbenchmark_ratchet/criterion_diff.rs:35-44).",
        blocked_on: Some("a criterion benchmark harness and a published trunk baseline"),
    },
    GateFidelity {
        gate_id: "shape_status",
        aspiration: "Measure a repository's distance to its declared monorepo shape — unit skeleton, \
                     satellite placement, root hygiene, naming, and the Dependency Rule over real \
                     build edges — and refuse any regression past a baseline frozen at the merge-base.",
        reference: "oyatie ADR-0562 placement rule and ci/facade/baseline-ratchet; Google Rosie/Tricorder \
                    ratchets; ArchUnit FreezingArchRule",
        fidelity: Fidelity::Partial,
        gap: "Placement, skeleton, root and naming rules are measured from the tree at the PR head with \
              seeded-defect fixtures. Dependency rules read Cargo path dependencies, Buck2 labels and \
              `use crate::` paths; TypeScript imports are declared unavailable, so a spec naming \
              ts-workspace gets NotMeasured for those rules. The adapter-naming rule is not implemented. \
              Contention metrics are not yet collected.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "replay_harness_status",
        aspiration: "Replay recorded production traces and assert byte-for-byte state parity.",
        reference: "Deterministic record-and-replay; VMware ReTrace",
        fidelity: Fidelity::Aspirational,
        gap: "No trace corpus is collected, so the replayer is never given one. Its whole check is \
              `traces.iter().all(|t| !t.input_payload.is_empty())`, which is vacuously true of an \
              empty slice -- and it used to answer one with a hardcoded count of five replayed \
              fixtures, which the scorecard published. Now reports NotMeasured \
              (replay_harness/trace_replayer.rs:17).",
        blocked_on: Some("a production trace recorder, which does not exist yet"),
    },
    GateFidelity {
        gate_id: "upgrade_train_status",
        aspiration: "Schedule autonomous semver and CVE upgrade PRs from the dependency graph.",
        reference: "Dependabot; Renovate",
        fidelity: Fidelity::Aspirational,
        gap: "The review pipeline supplied no candidates, and the verdict `let passed = breaking == 0;` \
              is trivially true of an empty list, so the train was certified without being read. \
              Now reports NotMeasured (upgrade_train/mod.rs:62).",
        blocked_on: Some("a dependency manifest reader and an advisory feed"),
    },
    GateFidelity {
        gate_id: "consistency_status",
        aspiration: "Verify multi-region write ordering via vector clocks and CRDT convergence.",
        reference: "Lamport clocks; Shapiro CRDTs",
        fidelity: Fidelity::Heuristic,
        gap: "Substring scan. A line naming a global table needs only to also contain \"version\" to \
              be treated as safely ordered, and \"version\" appears in most schema and dependency \
              diffs (consistency_guard/conflict_detector.rs:29).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "jittered_backoff_status",
        aspiration: "Prove full-jitter backoff and deadline propagation on every network retry.",
        reference: "AWS Architecture Blog, exponential backoff and jitter",
        fidelity: Fidelity::Heuristic,
        gap: "Substring scan. The jitter test is contains(\"rand\"), which any longer word containing \
              those four letters satisfies -- this repository's own brand-absence gate is one such \
              word -- and the deadline test is contains(\"context\"). Most diffs clear it without any \
              backoff at all (jittered_backoff/backoff_scanner.rs:31).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "hermetic_build_status",
        aspiration: "Build twice and compare the binaries byte for byte.",
        reference: "Reproducible Builds; Bazel hermeticity",
        fidelity: Fidelity::Heuristic,
        gap: "Builds nothing. Checks the diff for the literals SystemTime::now() and env!(\"HOME\") \
              (hermetic_build/reproducibility_checker.rs:29).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "auto_rollback_status",
        aspiration: "Watch canary error budget burn and roll back autonomously on degradation.",
        reference: "SRE Workbook, error budget policy",
        fidelity: Fidelity::Aspirational,
        gap: "The engine itself is correct: above its thresholds it rolls back and writes a \
              postmortem. Nothing measures error rate or latency, though -- the review pipeline \
              passed hardcoded healthy readings, so the degraded branch was unreachable on every \
              pull request. `is_degraded` is the whole decision, and nothing feeds it a reading \
              (auto_rollback/mod.rs:63).",
        blocked_on: Some("a canary telemetry source; there is none"),
    },
    GateFidelity {
        gate_id: "carbon_compute_status",
        aspiration: "Measure build energy cost and route heavy compute to low-carbon grid windows.",
        reference: "Green Software Foundation SCI",
        fidelity: Fidelity::Aspirational,
        gap: "Nothing meters CPU time or grid intensity; the review pipeline passed hardcoded \
              budget and actual figures, so the ratchet compared two constants and published a \
              joules figure derived from them. `evaluate_carbon_intensity` receives no measurement \
              (carbon_aware/mod.rs:62).",
        blocked_on: Some("a CPU-time meter and a grid carbon-intensity feed"),
    },
    GateFidelity {
        gate_id: "openvex_status",
        aspiration: "Attest CVE exploitability by pruning advisories against the real call graph.",
        reference: "OpenVEX; Google capslock",
        fidelity: Fidelity::Aspirational,
        gap: "No advisory feed or dependency inventory is read. The whole reachability decision is \
              `!source_code.contains(vuln_symbol)`, and the review pipeline supplied placeholder \
              CVE and symbol names, so every PR was attested NotAffected by an advisory that does \
              not exist. Now reports NotMeasured (vex_scanner/callgraph_pruner.rs:36).",
        blocked_on: Some("an advisory feed and a call graph; neither exists yet"),
    },
    GateFidelity {
        gate_id: "finops_status",
        aspiration: "Ratchet cost-per-outcome by budgeting heap allocations on hot paths.",
        reference: "Zero-copy parsing; allocation budgets",
        fidelity: Fidelity::Heuristic,
        gap: "Scope is a fixed list of path fragments -- `is_hotpath` matches network/, codec/, \
              engine/, hotpath, packet -- and no tracked file in this repository contains any of \
              them, so nothing is ever scanned here. The gate used to report a clean hotpath \
              budget from that empty scope; it now separates a clean scan from an empty one \
              (finops_ratchet/allocation_scanner.rs:30).",
        blocked_on: Some("a per-tenant hotpath declaration; the marker set is hardcoded"),
    },
];

/// Gate ids whose implementation has NOT been read.
///
/// Empty by construction: anything absent from `AUDITED_GATES` is unaudited.
/// This function exists so the count is derived rather than maintained by hand.
pub fn unaudited_count(total_gates: usize) -> usize {
    total_gates.saturating_sub(AUDITED_GATES.len())
}
