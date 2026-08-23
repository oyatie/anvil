//! The certification table, one row per gate — rendered from the report's
//! own field list so a gate added to the struct appears here by construction.
//!
//! The previous renderer took 68 positional `&GateStatus` arguments against a
//! 71-field report: `review_verdict_status`, `brand_absence_status` and
//! `migration_boundary_status` were counted, sealed on, and absent from the
//! table. `GATE_LABELS` is pinned to `TOTAL_GATES` and to `named_statuses()`
//! by test, so the table and the verdict can no longer disagree about what
//! the gates are.
//!
//! Not what Anvil publishes on a pull request: the scorecard comment is
//! rendered by `crate::publish::scorecard::render`, which reports findings
//! only and counts the passes. This table populates
//! `PreMergeCertificationReport::summary_markdown` and is asserted against by
//! `tests/red_green_gates_test.rs`.

use super::PreMergeCertificationReport;

/// (field name in `named_statuses()`, row label, row detail), in matrix order.
pub const GATE_LABELS: &[(&str, &str, &str)] = &[
    (
        "doc_parity_status",
        "📚 Documentation & Doctrine Parity",
        "Verified against platform doctrine and docs",
    ),
    (
        "cedar_status",
        "🛡️ Cedar Policy & IAM Boundaries",
        "AWS Cedar policy coverage & offline PDP verification",
    ),
    (
        "compliance_status",
        "🏛️ Systematic Regulatory Compliance",
        "Dynamic temporal KR PIPA, FSS & HIPAA engine",
    ),
    (
        "api_contract_status",
        "📐 OpenAPI & Wire Contract Integrity",
        "Schema validation & two-way route sync",
    ),
    (
        "cell_isolation_status",
        "🌐 Cell Boundary & Tenant Isolation",
        "Multi-tenant query scoping & zero cross-cell leaks",
    ),
    (
        "supply_chain_status",
        "📦 Supply Chain & CVE Audit (SLSA L2+)",
        "Dependency audit, Syft CycloneDX SBOM & provenance",
    ),
    (
        "clean_arch_status",
        "🏛️ Clean Architecture",
        "Strict inward layer boundaries (Core -> Ports -> Adapters)",
    ),
    (
        "monorepo_status",
        "🏢 Monorepo Patterns & Hermeticity",
        "Hermetic package boundaries & zero path leakage",
    ),
    (
        "debt_shrink_status",
        "📉 Deprecation & Reorg Drain Ratchet",
        "Only debt shrinks permitted on deprecating targets",
    ),
    (
        "modularization_status",
        "🧩 Code Modularization (100-300 lines)",
        "Componentized architecture with zero monoliths",
    ),
    (
        "coverage_status",
        "🎯 Differential Test Coverage (≥85%)",
        "Verified test coverage on added & modified lines",
    ),
    (
        "rust_skills_status",
        "🦀 Rust 2024 Edition Quality (rust-skills)",
        "380 Rust rules: zero unwrap panics & zero-copy",
    ),
    (
        "kani_status",
        "🔬 Kani Formal Verification & Unsafe Proofs",
        "Mathematical memory safety & SAFETY: invariant proofs",
    ),
    (
        "slo_status",
        "📊 OpenSLO & Error Budget Burn-Rate Gate",
        "Target reliability SLOs & <3x 5m burn rate verified",
    ),
    (
        "adr_status",
        "📑 Living ADR 5-Field Schema Ratchet",
        "Mandatory achieves, origin, rule, ensure, overturn_when",
    ),
    (
        "shuffle_status",
        "🎲 Cell Shuffle-Sharding & Blast-Radius",
        "Combinatorial tenant isolation & bounded cell outage impact",
    ),
    (
        "trace_status",
        "📡 W3C TraceContext & Distributed Tracing",
        "End-to-end span instrumentation across async tasks",
    ),
    (
        "constant_work_status",
        "⏱️ Constant-Work Static Pool & Backpressure",
        "Zero unbounded channels & fixed capacity limits",
    ),
    (
        "idempotency_status",
        "💳 Stripe Idempotency Key & Outbox Gate",
        "Mutating route idempotency & transactional outbox safety",
    ),
    (
        "finops_status",
        "💰 FinOps Unit-Cost & Zero-Copy Ratchet",
        "Zero unbudgeted heap allocations in performance hotpaths",
    ),
    (
        "ghost_migration_status",
        "🐘 Ghost DB Migration & Zero-Lock Validator",
        "Zero exclusive table locks & rollback parity verified",
    ),
    (
        "gitops_promo_status",
        "📦 GitOps OCI Immutable Digest Pinning",
        "Zero :latest tags; verified sha256 container digests",
    ),
    (
        "gitops_drift_status",
        "🔄 GitOps ArgoCD/Flux Manifest Parity",
        "Zero unmanaged or unsafe cascade deletions",
    ),
    (
        "canary_status",
        "🐤 Progressive Canary Traffic & Burn Breaker",
        "5m error budget burn rate < 3.0x threshold",
    ),
    (
        "cluster_audit_status",
        "🔍 Live Cluster Readback & Drift Auditor",
        "100% parity between live cluster and Git trunk",
    ),
    (
        "migration_orch_status",
        "🗄️ Database Expand-Contract Lifecycle",
        "4-phase zero-lock schema state machine enforcement",
    ),
    (
        "ci_wallclock_status",
        "⚡ CI Wallclock & Compute Cost Ratchet",
        "Regression threshold & actionable optimization guidance",
    ),
    (
        "predictive_test_status",
        "🎯 DAG Predictive Test Selection",
        "Affected workspace member targeting; <90s PR test wallclock",
    ),
    (
        "compile_profile_status",
        "⏱️ Compile-Time & Macro Bloat Profiler",
        "Zero un-budgeted macro expansion or un-cached build.rs scripts",
    ),
    (
        "remote_cache_status",
        "📦 Remote Sccache & Object Key Parity",
        "Lockfile sha256 cache keys; >90% compilation cache hits",
    ),
    (
        "runner_economics_status",
        "💵 Runner SKU Tiering & PR Spot Allocation",
        "PRs run on low-cost spot; multi-arch macOS tiered to merge train",
    ),
    (
        "sandbox_status",
        "🏗️ Ephemeral Micro-Sandbox Isolation",
        "Sub-second sandbox spin-up; zero dirty host or port collisions",
    ),
    (
        "cross_service_status",
        "🌐 Monorepo Cross-Service Blast-Radius",
        "Wire contract compatibility proven across microservices",
    ),
    (
        "ephemeral_secret_status",
        "🔐 OIDC Zero-Trust Dynamic Credentials",
        "Ephemeral <=15m STS tokens; zero static secrets in workflows",
    ),
    (
        "psa_status",
        "🛡️ Native Kubernetes PSA Gate (ADR-0710)",
        "enforce: restricted or recorded expiring exception registry",
    ),
    (
        "shadow_traffic_status",
        "🌑 Dark-Traffic Shadow Replay Parity",
        "1% shadow mirror; >=99.5% payload parity before live traffic",
    ),
    (
        "unresolved_review_status",
        "💬 Zero-Unresolved-Comments Review Gate",
        "100% of review comments and threads must be resolved",
    ),
    (
        "local_probe_status",
        "⚡ Sub-100ms Inner-Loop Local Probe",
        "Instant pre-commit AST linting & conventional commit hygiene",
    ),
    (
        "semantic_abi_status",
        "🧬 Semantic ABI & Interface Stability",
        "Public library signatures & struct memory layout stability",
    ),
    (
        "zero_day_status",
        "🛡️ Zero-Day Vulnerability Auto-Patcher",
        "Autonomous upstream RustSec/CVE patch synthesis",
    ),
    (
        "formal_verification_status",
        "📐 Policy Pattern Scan",
        "Keyword scan for wildcard principals and open egress; not a proof",
    ),
    (
        "deadlock_status",
        "🔒 Lock Order Cycle Scan",
        "Cycles in a lock-order graph keyed on receiver text; no types, no call edges",
    ),
    (
        "review_verdict_status",
        "🧠 AI Code Review & 16-Lens Matrix",
        "Adversarial multi-lens review verdict; REQUEST_CHANGES or REJECT blocks",
    ),
    (
        "brand_absence_status",
        "🏷️ Name-Honesty (Brand Absence) Self-Gate",
        "Anvil's own names and PR-visible strings name what the code verifies, not an aspiration",
    ),
    (
        "migration_boundary_status",
        "🧭 Migration Boundary Direction Self-Gate",
        "Anvil's Migrating components never depend on code oyatie supersedes",
    ),
    (
        "shape_status",
        "📐 Monorepo Shape Conformance Ratchet",
        "Distance to the tenant's shape spec; blocking rules may not regress past the baseline frozen at merge-base",
    ),
    (
        "automated_canary_status",
        "📊 Automated Canary Analysis (ACA)",
        "Mann-Whitney U-test statistical distribution validation",
    ),
    (
        "progressive_ring_status",
        "💍 Progressive Regional Rollout Rings",
        "4-ring progressive promotion (Canary -> Dogfood -> Cell -> Global)",
    ),
    (
        "hermetic_build_status",
        "🧱 Hermetic & Bitwise Reproducible Build",
        "Byte-for-byte deterministic binary verification",
    ),
    (
        "openvex_status",
        "📋 OpenVEX Dead-Code Reachability",
        "Callgraph-pruned CVE exploitability attestations",
    ),
    (
        "cosign_status",
        "🔏 Cosign & Sigstore Keyless Provenance",
        "OIDC hardware-backed transparency log signatures",
    ),
    (
        "chaos_injection_status",
        "💥 Pre-Merge Simulated Chaos Injection",
        "Synthetic packet loss, DNS jitter & DB failover certification",
    ),
    (
        "stacked_diffs_status",
        "🌲 Stacked Diffs & PR DAG Orchestration",
        "Atomic parent-child branch cascade & merge train sync",
    ),
    (
        "microbench_status",
        "⚡ Nanosecond Hotpath Microbench Ratchet",
        "Criterion P99 CPU cycle & latency regression enforcement",
    ),
    (
        "jittered_backoff_status",
        "🎲 Jittered Exponential Backoff Gate",
        "Full Jitter & deadline propagation on all network retries",
    ),
    (
        "schema_evolution_status",
        "📐 Wire Schema Evolution Ratchet",
        "Strict backward/forward wire compatibility (Protobuf/OpenAPI)",
    ),
    (
        "auto_rollback_status",
        "🔄 Auto-Rollback & Postmortem Engine",
        "Autonomous canary rollback on degradation & blameless postmortem",
    ),
    (
        "wasm_sandbox_status",
        "📦 Dynamic WebAssembly Policy Sandbox",
        "Sub-millisecond sandboxed bytecode linting & policy evaluation",
    ),
    (
        "consistency_status",
        "🌐 Active-Active Consistency Guard",
        "Multi-region vector clock ordering & CRDT conflict resolution",
    ),
    (
        "flake_quarantine_status",
        "🧪 Flaky-Test Quarantine Lifecycle",
        "Isolated quarantine lane & autonomous 100x stress rehabilitation",
    ),
    (
        "zero_trust_workload_status",
        "🔐 Zero-Trust SPIFFE Workload Identity",
        "Cryptographic SPIFFE ID workload attestation & mTLS encryption",
    ),
    (
        "carbon_compute_status",
        "🌱 GreenOps Carbon-Aware Compute",
        "Energy-efficient CI compilation & green compute window routing",
    ),
    (
        "replay_harness_status",
        "📼 Deterministic Record-and-Replay",
        "Hermetic production trace replayer & offline bug reproduction",
    ),
    (
        "upgrade_train_status",
        "🚂 Proactive Dependency Upgrade Train",
        "Autonomous upstream semver & CVE patch PR scheduling",
    ),
    (
        "mutation_status",
        "💥 AST Chaos Mutation Test Adequacy",
        "Critical branches verified against surviving mutants",
    ),
    (
        "feature_flag_status",
        "🚩 Feature Flag & Dead Branch Lifecycle",
        "Zero stale or dead toggle fallback branches",
    ),
    (
        "bench_status",
        "⚡ Micro-Benchmark & Latency Ratchet",
        "Hot paths within +3% latency & zero-leak budget",
    ),
    (
        "attestation_status",
        "🔏 Cryptographic Provenance Attestation",
        "Stamped verification receipts in .anvil/receipts",
    ),
    (
        "security_scan_status",
        "🔐 Secret & Credential Scan",
        "Deep entropy scan for leaked credentials",
    ),
    (
        "schema_compat_status",
        "🔄 Schema & Migration Compatibility",
        "Zero destructive breakages across cell nodes",
    ),
    (
        "performance_concurrency_status",
        "⚡ Concurrency, Perf & Flake Guard",
        "Bounded execution and flake-resistant timings",
    ),
    (
        "test_suite_status",
        "🧪 Automated Test Suite",
        "Result of a suite run, when one was run; not measured in the PR pipeline",
    ),
];

pub fn label_for(name: &str) -> Option<(&'static str, &'static str)> {
    GATE_LABELS
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, l, d)| (*l, *d))
}

pub struct MatrixRenderer;

impl MatrixRenderer {
    /// One row per named gate, in the report's order. A gate without a label
    /// is rendered under its field name rather than dropped; the test that
    /// pins `GATE_LABELS` to `named_statuses()` keeps that path unreachable.
    pub fn render(report: &PreMergeCertificationReport) -> String {
        let readiness_badge = if report.is_certified_ready {
            "🟢 **READY FOR MERGE (Certified)**"
        } else {
            "🔴 **BLOCKERS DETECTED (Pre-Merge Incomplete)**"
        };
        let mut out = String::with_capacity(16 * 1024);
        out.push_str(
            "<!-- ANVIL_SCORECARD_RECEIPT -->\n### Full-lifecycle quality and GitOps matrix\n\n| Quality Gate | Status | Details |\n|---|---|---|\n",
        );
        for (name, status) in report.named_statuses() {
            let (label, detail) = label_for(name).unwrap_or((name, ""));
            out.push_str(&format!(
                "| **{label}** | {} | {detail} |\n",
                status.badge()
            ));
        }
        out.push_str(&format!(
            "\n---\n**Verdict**: {readiness_badge}\n\n*🤖 [Certified] by Oyatie Anvil*"
        ));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pre_merge_guard::report::TOTAL_GATES;

    #[test]
    fn every_named_gate_has_exactly_one_label_and_vice_versa() {
        let r = PreMergeCertificationReport::unmeasured("fixture");
        let names: Vec<&str> = r.named_statuses().into_iter().map(|(n, _)| n).collect();
        assert_eq!(GATE_LABELS.len(), TOTAL_GATES);
        assert_eq!(names.len(), TOTAL_GATES);
        for (i, (n, _, _)) in GATE_LABELS.iter().enumerate() {
            assert_eq!(
                *n, names[i],
                "GATE_LABELS order must follow named_statuses()"
            );
        }
    }

    #[test]
    fn the_three_previously_unrendered_gates_have_rows() {
        let r = PreMergeCertificationReport::unmeasured("fixture");
        let t = MatrixRenderer::render(&r);
        assert!(t.contains("AI Code Review & 16-Lens Matrix"));
        assert!(t.contains("Brand Absence"));
        assert!(t.contains("Migration Boundary"));
        assert_eq!(t.matches("\n| **").count(), TOTAL_GATES);
    }
}
