//! One bin of `AUDITED_GATES` entries.
//!
//! The split is by size, not by subject: `registry.rs` held every entry and
//! was 1166 lines against a 300-line budget. The order of the corpus lives in
//! `registry::AUDITED_GATES`, so nothing here is meant to be read as a grouping.

use crate::fidelity::{Fidelity, GateFidelity};

pub const REPLAY_HARNESS_STATUS: GateFidelity = GateFidelity {
    gate_id: "replay_harness_status",
    aspiration: "Replay recorded production traces and assert byte-for-byte state parity.",
    reference: "Deterministic record-and-replay; VMware ReTrace",
    fidelity: Fidelity::Aspirational,
    gap: "No trace corpus is collected, so the replayer is never given one. Its whole check is \
          `traces.iter().all(|t| !t.input_payload.is_empty())`, which is vacuously true of an \
          empty slice -- and it used to answer one with a hardcoded count of five replayed \
          fixtures, which the scorecard published. Now reports NotMeasured \
          (replay_harness/trace_replayer.rs::TraceReplayer).",
    blocked_on: Some("a production trace recorder, which does not exist yet"),
};

pub const CONSISTENCY_STATUS: GateFidelity = GateFidelity {
    gate_id: "consistency_status",
    aspiration: "Verify multi-region write ordering via vector clocks and CRDT convergence.",
    reference: "Lamport clocks; Shapiro CRDTs",
    fidelity: Fidelity::Heuristic,
    gap: "Substring scan. A line naming a global table needs only to also contain \"version\" to \
          be treated as safely ordered, and \"version\" appears in most schema and dependency \
          diffs (consistency_guard/conflict_detector.rs::ConflictDetector).",
    blocked_on: None,
};

pub const JITTERED_BACKOFF_STATUS: GateFidelity = GateFidelity {
    gate_id: "jittered_backoff_status",
    aspiration: "Prove full-jitter backoff and deadline propagation on every network retry.",
    reference: "AWS Architecture Blog, exponential backoff and jitter",
    fidelity: Fidelity::Heuristic,
    gap: "Substring scan. The jitter test is contains(\"rand\"), which any longer word containing \
          those four letters satisfies -- this repository's own brand-absence gate is one such \
          word -- and the deadline test is contains(\"context\"). Most diffs clear it without any \
          backoff at all (jittered_backoff/backoff_scanner.rs::BackoffScanner).",
    blocked_on: None,
};

pub const HERMETIC_BUILD_STATUS: GateFidelity = GateFidelity {
    gate_id: "hermetic_build_status",
    aspiration: "Build twice and compare the binaries byte for byte.",
    reference: "Reproducible Builds; Bazel hermeticity",
    fidelity: Fidelity::Heuristic,
    gap: "Builds nothing, so the byte-for-byte comparison the name claims is unmeasured and the \
          gate withholds it. The impurity half is real and fires: it checks the diff for the \
          literals SystemTime::now() and env!(\"HOME\") \
          (hermetic_build/reproducibility_checker.rs::ReproducibilityChecker).",
    blocked_on: None,
};

pub const AUTO_ROLLBACK_STATUS: GateFidelity = GateFidelity {
    gate_id: "auto_rollback_status",
    aspiration: "Watch canary error budget burn and roll back autonomously on degradation.",
    reference: "SRE Workbook, error budget policy",
    fidelity: Fidelity::Aspirational,
    gap: "The engine itself is correct: above its thresholds it rolls back and writes a \
          postmortem. Nothing measures error rate or latency, though -- the review pipeline \
          passed hardcoded healthy readings, so the degraded branch was unreachable on every \
          pull request. `is_degraded` is the whole decision, and nothing feeds it a reading \
          (auto_rollback/mod.rs::AutoRollbackPostmortemEngine).",
    blocked_on: Some("a canary telemetry source; there is none"),
};

pub const CARBON_COMPUTE_STATUS: GateFidelity = GateFidelity {
    gate_id: "carbon_compute_status",
    aspiration: "Measure build energy cost and route heavy compute to low-carbon grid windows.",
    reference: "Green Software Foundation SCI",
    fidelity: Fidelity::Aspirational,
    gap: "Nothing meters CPU time or grid intensity; the review pipeline passed hardcoded \
          budget and actual figures, so the ratchet compared two constants and published a \
          joules figure derived from them. `evaluate_carbon_intensity` receives no measurement \
          (carbon_aware/mod.rs::evaluate_compute_carbon).",
    blocked_on: Some("a CPU-time meter and a grid carbon-intensity feed"),
};

pub const OPENVEX_STATUS: GateFidelity = GateFidelity {
    gate_id: "openvex_status",
    aspiration: "Attest CVE exploitability by pruning advisories against the real call graph.",
    reference: "OpenVEX; Google capslock",
    fidelity: Fidelity::Aspirational,
    gap: "No advisory feed or dependency inventory is read. The whole reachability decision is \
          `!source_code.contains(vuln_symbol)`, and the review pipeline supplied placeholder \
          CVE and symbol names, so every PR was attested NotAffected by an advisory that does \
          not exist. Now reports NotMeasured (vex_scanner/callgraph_pruner.rs::CallgraphPruner).",
    blocked_on: Some("an advisory feed and a call graph; neither exists yet"),
};

pub const FINOPS_STATUS: GateFidelity = GateFidelity {
    gate_id: "finops_status",
    aspiration: "Ratchet cost-per-outcome by budgeting heap allocations on hot paths.",
    reference: "Zero-copy parsing; allocation budgets",
    fidelity: Fidelity::Heuristic,
    gap: "Scope is a fixed list of path fragments -- `is_hotpath` matches network/, codec/, \
          engine/, hotpath, packet -- and no tracked file in this repository contains any of \
          them, so nothing is ever scanned here. The gate used to report a clean hotpath \
          budget from that empty scope; it now separates a clean scan from an empty one \
          (finops_ratchet/allocation_scanner.rs::is_hotpath).",
    blocked_on: Some("a per-tenant hotpath declaration; the marker set is hardcoded"),
};

pub const SANDBOX_STATUS: GateFidelity = GateFidelity {
    gate_id: "sandbox_status",
    aspiration: "Spin up a hermetic ephemeral sandbox per PR and prove zero host-state leakage.",
    reference: "Bazel sandboxfs; Firecracker microVMs",
    fidelity: Fidelity::Aspirational,
    gap: "No sandbox runtime exists, so nothing is started, bound or timed. The allocator it \
          used to call returned a struct literal, making the verdict a constant and publishing \
          a spin-up time nothing had measured; that pool is deleted and \
          `evaluate_without_sandbox_runtime` is now the only path \
          (ephemeral_sandbox/mod.rs::evaluate_without_sandbox_runtime).",
    blocked_on: Some("a container or microVM runtime the review pipeline can drive"),
};

pub const FLAKE_QUARANTINE_STATUS: GateFidelity = GateFidelity {
    gate_id: "flake_quarantine_status",
    aspiration: "Detect non-deterministic tests from run history and isolate them into a \
                 quarantine lane so they stop blocking merges.",
    reference: "Google flaky-test infrastructure; Meta's Probabilistic Flakiness Score",
    fidelity: Fidelity::Aspirational,
    gap: "Nothing retains test-run history, so no test can be shown to be non-deterministic and \
          there is no lane to isolate one into. The verdict used to be a literal, and the \
          counters come from a substring match for \"flaky\" against changed file paths -- \
          retained as data, not as evidence (flake_quarantine/quarantine_manager.rs::QuarantineManager).",
    blocked_on: Some("per-test run history across attempts, which Anvil does not record"),
};

pub const PREDICTIVE_TEST_STATUS: GateFidelity = GateFidelity {
    gate_id: "predictive_test_status",
    aspiration: "Select the tests a change can affect from the dependency graph and hold PR \
                 test wall-clock under a budget.",
    reference: "Bazel target determination; Google's affected-target selection",
    fidelity: Fidelity::Partial,
    gap: "The package DAG selection is real, but nothing times a test run, so the wall-clock \
          claim is unmeasured; the verdict is now whether the selection pruned anything at all \
          rather than the literal it was. Discovery used to invent a package when it found \
          none; the verdict is now `let is_optimized = skipped > 0;` \
          (predictive_test_selector/mod.rs::evaluate_test_selection).",
    blocked_on: Some("a timed test run to measure the wall-clock budget against"),
};

pub const COSIGN_STATUS: GateFidelity = GateFidelity {
    gate_id: "cosign_status",
    aspiration: "Sign the artefact through the Fulcio OIDC keyless flow, record it in the Rekor \
                 transparency log, and verify the certificate identity and the inclusion proof \
                 before publishing provenance.",
    reference: "Sigstore keyless signing (Fulcio, Rekor); SLSA provenance levels; in-toto attestations",
    fidelity: Fidelity::Aspirational,
    gap: "Signs nothing: no OIDC identity token is requested, no Fulcio certificate is issued, no \
          signature is computed and no transparency-log inclusion proof is fetched -- the crate \
          carries no signing key and no X.509 or ECDSA dependency to do any of it with. The \
          fabricated bundle it used to publish, an elided PEM certificate literal and a log id \
          derived from the artefact digest prefix, is deleted along with the attestor that built \
          it; `evaluate_without_signing_backend` publishing `MISSING_SIGNING_BACKEND` is now the \
          only path (cosign_signer/mod.rs::GATE_ID and cosign_signer/mod.rs::CosignProvenanceSigner-87).",
    blocked_on: Some(
        "a Sigstore signing backend -- an OIDC identity source, Fulcio and Rekor; none is reachable \
         from here",
    ),
};

pub const DEBT_SHRINK_STATUS: GateFidelity = GateFidelity {
    gate_id: "debt_shrink_status",
    aspiration: "Ratchet deprecation and reorganisation debt down: only shrinkage permitted on \
                 targets the organisation has decided to drain.",
    reference: "SonarQube Clean as You Code -- new-code quality gate",
    fidelity: Fidelity::Heuristic,
    gap: "Scope is three path fragments -- `is_deprecating_target` matches `deprecated`, `legacy`, \
          `/old/` -- plus any path named in a `REORG-DRAIN.md` drain ledger, and no tracked file in \
          this repository matches a fragment while the ledger does not exist, so nothing is ever \
          in scope here. The gate used to publish a drained ratchet from that empty scope; it now \
          separates a scanned target from an absent one \
          (debt_shrink_guard.rs::is_deprecating_target). The ledger is the only authoritative half: deprecation is \
          a decision somebody recorded, not a substring somebody typed into a path.",
    blocked_on: Some("a drain ledger; the marker set is hardcoded and unconfigurable"),
};

pub const GITOPS_DRIFT_STATUS: GateFidelity = GateFidelity {
    gate_id: "gitops_drift_status",
    aspiration: "Reconcile desired state against the cluster and refuse orphaning cascade \
                 deletions.",
    reference: "ArgoCD sync status and self-heal; `resources-finalizer.argocd.argoproj.io`",
    fidelity: Fidelity::Heuristic,
    gap: "Reads no cluster and computes no diff against live state, so it detects no drift at all; \
          what it checks is a deletion marker in the diff text. Scope is two path fragments -- \
          `is_gitops_manifest` matches `applicationset` and `application.yaml` \
          (gitops_drift_reconciler/orphan_sweeper.rs::is_gitops_manifest) -- and no tracked file in this \
          repository matches either, so nothing is ever in scope. ArgoCD is not the precedent \
          at its reporting layer -- it starts at Synced and downgrades only inside the loop over \
          target resources, so an Application with no targets displays green -- but it is at its \
          action layer, where auto-sync refuses to run when every managed resource would be \
          pruned unless allowEmpty is set. The gate used to publish declarative integrity from an \
          empty scope; it now reports that nothing was scanned, which is acceptable to the badge \
          and withheld by merge admission.",
    blocked_on: Some("a cluster the reconciler can read; nothing here talks to one"),
};

pub const MIGRATION_ORCH_STATUS: GateFidelity = GateFidelity {
    gate_id: "migration_orch_status",
    aspiration: "Enforce Expand-Contract phase ordering across releases so a contract step cannot \
                 ship before its expand has baked.",
    reference: "Parallel Change (expand/contract); Flyway and Liquibase validate; squawk",
    fidelity: Fidelity::Aspirational,
    gap: "Nothing spans releases: it reads one diff and asks whether the literal `-- PHASE: \
          CONTRACT` appears in the same file as a `DROP COLUMN`, so the phase order it is named \
          for is never checked and the annotation is self-attested by the author dropping the \
          column. Scope was `.sql` matched against the hunk *text*, which put a Rust file \
          mentioning a SQL filename into SQL scope and defaulted a header-less chunk to a \
          hardcoded migration filename that ended in that extension; it is now the path \
          (migration_orchestrator/phase_validator.rs::is_migration_sql), and no tracked file in this \
          repository ends in that extension, so nothing is ever in scope. The gate used to publish \
          lifecycle conformance from that empty scope and now reports that nothing was parsed.",
    blocked_on: Some("release-spanning migration history; one diff cannot show phase order"),
};

pub const GHOST_MIGRATION_STATUS: GateFidelity = GateFidelity {
    gate_id: "ghost_migration_status",
    aspiration: "Validate schema migrations against a shadow database for lock-free application and \
                 rollback parity.",
    reference: "gh-ost, which refuses an empty --alter outright; pt-online-schema-change",
    fidelity: Fidelity::Heuristic,
    gap: "Regex scan for CONCURRENTLY, DROP COLUMN and NOT NULL. Connects to no database and runs \
          no migration, so no lock is ever observed. Scope is the changed-file list filtered by \
          `is_migration_file`, which matched `migration` as a substring anywhere in the path -- \
          so the eight tracked Rust files spelling that word were schema scope, and \
          src/migration/registry.rs was certified lock-free. It is now `file_path.ends_with` \
          on `.sql`, or a path component equal to `migrations` or `migrate` \
          (ghost_migration_harness.rs::is_migration_file), so no tracked file in this repository is in \
          scope; a checked-in schema.rb or an Atlas hcl file carries DDL under neither and is \
          still missed. An empty result also cannot be told apart from a changed-file list that \
          never arrived. Both used to return early with a pass declaring the ghost migration \
          check clean; both now report that nothing was scanned.",
    blocked_on: Some("a shadow database"),
};
