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
        gap: "Runs no coverage tool. Derives a figure from added test lines over added code lines times a \
              0.4 factor, then applies .max(85.0) against a >= 85.0 threshold -- so it is unfailable by \
              arithmetic (coverage_guard.rs:135-141).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "kani_status",
        aspiration: "Discharge memory-safety proof obligations for unsafe blocks using a bounded model \
                     checker.",
        reference: "Kani / CBMC; AWS Automated Reasoning Group",
        fidelity: Fidelity::Heuristic,
        gap: "Checks whether a `// SAFETY:` comment appears near `unsafe`. When the kani binary is absent it \
              returns status VERIFIED_STATIC -- a missing verifier reporting success \
              (kani_guard/proof_runner.rs:38-45).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "slo_status",
        aspiration: "Evaluate multi-window multi-burn-rate error budget consumption against live production \
                     telemetry (14.4x/1h, 6x/6h).",
        reference: "Google SRE Workbook, multiwindow multi-burn-rate alerting",
        fidelity: Fidelity::Aspirational,
        gap: "Queries no telemetry. Assigns simulated_burn_rate_1h = 1.02 and compares it against a 14.4 \
              threshold it can never reach (slo_canary_guard/mod.rs:83-86).",
        blocked_on: Some("a reachable Prometheus or OpenTelemetry endpoint"),
    },
    GateFidelity {
        gate_id: "remote_cache_status",
        aspiration: "Report the real distributed build-cache hit rate and ratchet it upward.",
        reference: "Bazel/Buck2 remote execution CAS statistics",
        fidelity: Fidelity::Aspirational,
        gap: "Hardcodes hit_rate_pct: 95.0 against an 85.0 threshold, so it cannot fail. Cache keys use \
              non-cryptographic FNV-1a, which is collision-attackable when PR content influences the key \
              (remote_cache_optimizer/mod.rs:63, cache_keys.rs:24).",
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
        gap: "A file named smt_solver.rs whose logic is policy_content.contains(\"permit(\"). No solver \
              exists (formal_verification/smt_solver.rs:24).",
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
        gap: "Hardcodes pr_wallclock_seconds: 142 with the source comment \"Under 5 min ceiling!\" \
              (ci_wallclock_ratchet/mod.rs:64).",
        blocked_on: Some("GitHub Actions timing API"),
    },
    GateFidelity {
        gate_id: "cluster_audit_status",
        aspiration: "Compare live cluster state against the declared manifests and report drift.",
        reference: "ArgoCD drift detection; kube-rs",
        fidelity: Fidelity::Aspirational,
        gap: "Compares two identical hardcoded literals (\"replicas: 3\" against \"replicas: 3\"), and the \
              detector requires \"replicas: 10\" on the live side, which those literals can never satisfy. \
              No cluster is contacted (cluster_state_auditor/mod.rs:45-46).",
        blocked_on: Some("Kubernetes API or ArgoCD access"),
    },
    GateFidelity {
        gate_id: "shadow_traffic_status",
        aspiration: "Mirror production traffic to a shadow deployment and diff the responses.",
        reference: "Envoy request mirroring; Diffy",
        fidelity: Fidelity::Aspirational,
        gap: "Returns a hardcoded struct: sampled_requests: 5000, payload_parity_pct: 99.98 \
              (shadow_traffic_harness/mod.rs:47).",
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
        gap: "Now fails closed after the Phase 0a fix, and creates missing ADRs. It cannot AMEND an existing \
              document: generate_and_write_docs writes only when !path.exists(), so README.md and \
              CHANGELOG.md are never updated (doc_guard/mod.rs:280).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "predictive_test_status",
        aspiration: "Select the minimal set of affected test targets from a real dependency graph, and \
                     report the pruning ratio actually achieved.",
        reference: "Google TAP affected-targets analysis; Meta Predictive Test Selection (arXiv:1810.05286)",
        fidelity: Fidelity::Heuristic,
        gap: "Computes a package DAG from `cargo metadata`, which is a real dependency closure, but then \
              hardcodes `let is_optimized = true` (predictive_test_selector/mod.rs:64) so the guard reports \
              PASSED regardless of the subprocess outcome. Closure-only selection is also ~99% waste at \
              scale per the TAP paper; a risk model is the actual target.",
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
];

/// Gate ids whose implementation has NOT been read.
///
/// Empty by construction: anything absent from `AUDITED_GATES` is unaudited.
/// This function exists so the count is derived rather than maintained by hand.
pub fn unaudited_count(total_gates: usize) -> usize {
    total_gates.saturating_sub(AUDITED_GATES.len())
}
