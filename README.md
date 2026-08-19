# 🔨 Anvil: Hyperscale Autonomous Engineering Delivery Fabric

> **Autonomous Hyperscale PR Reviewer, Triager, 23-Gate Domain Quality Fabric & Merge Queue Engine powered by Antigravity, Rust, and GitHub CLI.**

Anvil provides autonomous, end-to-end coverage across the entire PR and merge lifecycle on:
- [`https://github.com/oyatie/oyatie`](https://github.com/oyatie/oyatie)
- [`https://github.com/oyatie/console`](https://github.com/oyatie/console)

---

## 🏛️ Comprehensive 23-Gate Pre-Merge Certification Matrix

| Quality Gate | Description |
|---|---|
| **📚 Documentation & ADR Parity** | Verifies public APIs, platform doctrine & auto-syncs docs/ADRs (`DocGuard`) |
| **🛡️ Cedar Policy & IAM Boundaries** | Verifies AWS Cedar authorization policy coverage & tenant bounds (`CedarGuard`) |
| **🏛️ Systematic Regulatory & Statutory Compliance** | Dynamic temporal multi-jurisdiction regulatory engine (`ComplianceGuard`) |
| **📐 OpenAPI & Wire Contract Integrity** | Validates OpenAPI schemas & auto-syncs route definitions (`ApiContractGuard`) |
| **🌐 Cell Boundary & Tenant Isolation** | Enforces multi-tenant query scoping & zero cross-cell DB leaks (`CellIsolationGuard`) |
| **📦 Supply Chain & CVE Audit (SLSA L2+)** | Audits dependencies, Syft CycloneDX SBOM & SLSA L2+ provenance (`SupplyChainGuard`) |
| **🏛️ Clean Architecture** | Enforces Core -> Ports -> Adapters -> Facade layer boundaries (`CleanArchitectureGuard`) |
| **🏢 Monorepo Patterns & Hermeticity** | Hermetic package boundaries & zero path leaks (`MonorepoGuard`) |
| **📉 Deprecation & Reorg Drain Ratchet** | Only debt shrinks permitted on deprecating targets (`DebtShrinkGuard`) |
| **🧩 Code Modularization (100-300 lines)** | Componentized architecture with zero monoliths (`ModularizationGuard`) |
| **🎯 Differential Test Coverage (≥85%)** | Verified test coverage on added & modified lines (`CoverageGuard`) |
| **🦀 Rust 2024 Edition Quality** | 380 Rust rules: zero unwrap panics & zero-copy (`RustSkillsGuard`) |
| **🔬 Kani Formal Verification & Unsafe Proofs** | Mathematical memory safety & SAFETY: invariant proofs (`KaniGuard`) |
| **📊 OpenSLO & Error Budget Burn-Rate Gate** | Target reliability SLOs & <3x 5m burn rate verified (`SloCanaryGuard`) |
| **🐘 Ghost DB Migration & Zero-Lock Validator** | Zero exclusive table locks & rollback parity verified (`GhostMigrationHarness`) |
| **💥 AST Chaos Mutation Test Adequacy** | Critical branches verified against surviving mutants (`ChaosMutationGuard`) |
| **🚩 Feature Flag & Dead Branch Lifecycle** | Zero stale or dead toggle fallback branches (`FeatureFlagRatchet`) |
| **⚡ Micro-Benchmark & Latency Ratchet** | Hot paths within +3% latency & zero-leak budget (`CriterionBenchRatchet`) |
| **🔏 Cryptographic Provenance Attestation** | Stamped verification receipts in `.cursor/receipts` (`AttestationGuard`) |
| **🔐 Secret & Credential Scan** | Deep entropy scan for leaked credentials |
| **🔄 Schema & Migration Compatibility** | Zero destructive breakages across cell nodes |
| **⚡ Concurrency, Perf & Flake Guard** | Bounded execution and flake-resistant timings |
| **🧪 Automated Test Suite** | Local verification gate passed |

---

## 🛠️ CLI Operations

| Command | Description |
|---|---|
| `cargo run -- serve` | Starts the Anvil webhook listener daemon and automatic forwarders. |
| `cargo run -- review --repo <repo> --pr <number>` | Runs 16-lens adversarial review on any PR. |
| `cargo run -- fix --repo <repo> --pr <number>` | Evaluates, fixes, tests, and pushes code for review comments. |
| `cargo run -- certify --repo <repo> --pr <number>` | Runs the complete 23-gate certification scorecard & merge enlistment. |
| `cargo run -- triage --repo <repo> --run-id <id>` | Triages a failed CI workflow run on main/dev. |
| `cargo run -- enlist --repo <repo> --pr <number>` | Enlists an approved & certified PR into the Merge Queue. |
| `cargo run -- heal-queue --repo <repo> --pr <number>` | Auto-heals an ejected merge train PR with speculative bisection. |
| `cargo run -- reconcile --repo <repo> --pr <number>` | Reconciles lockfiles and truth ledgers. |

---

## ⚙️ Configuration (`.env`)

```env
HOST=127.0.0.1
PORT=3000
WATCHED_REPOS=oyatie/oyatie,oyatie/console
REPOS_DIR=./repos
DATA_DIR=./data
RULES_PATH=./rules.md
AGY_EFFORT=high
AUTO_FORWARD_WEBHOOKS=true
```
