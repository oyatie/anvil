# Changelog

All notable changes to the **Anvil** Autonomous Delivery Fabric will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **webhook**: Added `GET /healthz` liveness probe endpoint returning HTTP `200 OK` (`"ok"`) for container orchestrators and Kubernetes liveness checks ([#1](https://github.com/oyatie/anvil/pull/1)).
- **openapi**: Updated OpenAPI 3.0.3 specification in `openapi/openapi.yaml` declaring route definition and response schema for `GET /healthz`.
- **tests**: Added asynchronous unit test suite `test_healthz_handler` in `src/webhook/mod.rs` verifying deterministic `200 OK` response payload with 100% differential branch coverage.

## [0.1.0] - 2026-08-19

### Added

- Initial release of **Anvil: Hyperscale Autonomous Engineering Delivery Fabric**.
- Comprehensive 60-Gate pre-merge certification engine (`PreMergeGuard`, `DocGuard`, `ApiContractGuard`, `CedarGuard`, `ComplianceGuard`, `KaniGuard`, `SloCanaryGuard`, `GhostMigrationHarness`, `ChaosMutationGuard`, `AttestationGuard`, etc.).
- Webhook listener daemon and CLI operations (`serve`, `review`, `fix`, `certify`, `triage`, `enlist`, `heal-queue`, `reconcile`).
- Formal verification with Kani and SMT solver harnesses.
- Autonomous PR review, triaging, and merge queue self-healing pipelines.
