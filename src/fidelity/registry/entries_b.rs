//! One bin of `AUDITED_GATES` entries.
//!
//! The split is by size, not by subject: `registry.rs` held every entry and
//! was 1166 lines against a 300-line budget. The order of the corpus lives in
//! `registry::AUDITED_GATES`, so nothing here is meant to be read as a grouping.

use crate::fidelity::{Fidelity, GateFidelity};

pub const SEMANTIC_ABI_STATUS: GateFidelity = GateFidelity {
    gate_id: "semantic_abi_status",
    aspiration: "Compare the public API surface across revisions and require a semver major bump for \
                 breaking changes.",
    reference: "cargo-semver-checks over rustdoc JSON",
    fidelity: Fidelity::Heuristic,
    gap: "Compares the `pub fn` declarations a diff removes against the ones it adds, by name over the \
          whole diff rather than by two substring tests on it. A removal is reported only when the name \
          is added nowhere in the diff -- `added.get(name)` (signature_scanner.rs::scan_abi_diff) -- and a \
          signature only when the name occurs once on each side and both lines close their parameter \
          list, so a move and a rustfmt reflow both clear it while `unpaired_names` counts the pairs it \
          declined to compare (signature_scanner.rs::scan_abi_diff). It builds no rustdoc JSON, resolves no \
          module path and reads no baseline revision, so a function moved between modules reads as \
          clean and a major version bump is not read. Two shapes are declined outright rather than \
          guessed at: a name the diff removes but re-declares inside a string literal on the added \
          side -- an inline fixture rewritten from a raw string, which the anchor alone does not catch \
          because the indented body line reaches it bare -- and anything after a `#[cfg(test)]` marker \
          in the same file chunk, since `is_library_rust` tests directories and a test module inside \
          src publishes nothing (signature_scanner.rs::scan_abi_diff). Both cost recall: a genuine removal \
          re-mentioned as a declaration in a string, or declared below a test module, is not reported. \
          The memory-layout half of the old claim is withdrawn: nothing computes a layout, and a diff \
          that adds or removes a `#[repr(` line -- the symmetric difference, so a line present \
          identically on both sides was moved and does not count -- is reported `NotMeasured` rather \
          than passed. Adding a brand-new repr type also triggers it, which is why the published \
          sentence says add or remove rather than change -- `layout_files` \
          (semantic_abi_ratchet/mod.rs::evaluate_abi_stability).",
    blocked_on: None,
};

pub const CI_WALLCLOCK_STATUS: GateFidelity = GateFidelity {
    gate_id: "ci_wallclock_status",
    aspiration: "Measure this PR's actual CI wallclock and ratchet against a trunk baseline.",
    reference: "internal CI wallclock budgets; ADR-0718",
    fidelity: Fidelity::Aspirational,
    gap: "Without GitHub Actions timing API access, reports `NotMeasured` rather than measuring real \
          build duration or cost (ci_wallclock_ratchet/mod.rs::evaluate_ci_efficiency).",
    blocked_on: Some("GitHub Actions timing API"),
};

pub const CLUSTER_AUDIT_STATUS: GateFidelity = GateFidelity {
    gate_id: "cluster_audit_status",
    aspiration: "Compare live cluster state against the declared manifests and report drift.",
    reference: "ArgoCD drift detection; kube-rs",
    fidelity: Fidelity::Aspirational,
    gap: "With no Kubernetes API or ArgoCD access configured, reports `NotMeasured` and performs no \
          Git comparison (cluster_state_auditor/mod.rs::evaluate_cluster_parity).",
    blocked_on: Some("Kubernetes API or ArgoCD access"),
};

pub const SHADOW_TRAFFIC_STATUS: GateFidelity = GateFidelity {
    gate_id: "shadow_traffic_status",
    aspiration: "Mirror production traffic to a shadow deployment and diff the responses.",
    reference: "Envoy request mirroring; Diffy",
    fidelity: Fidelity::Aspirational,
    gap: "Without traffic mirror infrastructure or a replay target configured, reports `NotMeasured` \
          (shadow_traffic_harness/mod.rs::evaluate_shadow_verification).",
    blocked_on: Some("traffic mirroring infrastructure and a replay target"),
};

pub const WASM_SANDBOX_STATUS: GateFidelity = GateFidelity {
    gate_id: "wasm_sandbox_status",
    aspiration: "Execute untrusted policy in a WebAssembly sandbox with a constrained ABI.",
    reference: "Wasmtime",
    fidelity: Fidelity::Heuristic,
    gap: "No WebAssembly runtime. Checks lower.contains(\"process::abort\") \
          (wasm_sandbox/policy_runner.rs::WasmPolicyRunner).",
    blocked_on: None,
};

pub const CLEARTEXT_TRANSPORT_STATUS: GateFidelity = GateFidelity {
    gate_id: "cleartext_transport_status",
    aspiration: "Verify workload identity via SPIFFE/SPIRE issued mTLS certificates.",
    reference: "SPIFFE/SPIRE; Istio PeerAuthentication STRICT. For the lint this actually is: \
                CWE-319, gosec G107, semgrep insecure-transport",
    fidelity: Fidelity::Heuristic,
    gap: "Observes no SPIFFE ID, no SVID, no certificate and no mesh policy. All four are \
          runtime state -- a URI SAN inside an X.509 document handed to a process over the \
          Workload API after an agent attested its PID, a PeerAuthentication object in STRICT \
          mode inside a cluster -- and none of them can appear in a diff. Neither direction of \
          inference holds either: a tree with no plaintext URL can be running a mesh in \
          PERMISSIVE mode with no workload identity at all. What runs is a CWE-319 \
          cleartext-transport lint over added lines. It was the substring \"http://\", which \
          failed a merge for a licence URL in a doc comment; it now drops the comment tail in \
          code_before_comment (cleartext_scan.rs::code_before_comment), skips prose and fixture paths in \
          path_is_out_of_scope (cleartext_scan.rs::path_is_out_of_scope), and requires the authority after the \
          scheme to open with is_ascii_alphanumeric and not be one of the LOOPBACK_HOSTS \
          (cleartext_scan.rs::LOOPBACK_HOSTS). Unlike the lints of that class it names, it is not \
          sink-anchored: there is no parser here, so it cannot tell a URL that reaches an HTTP \
          client from one that does not, and a URL assembled across lines or read from \
          configuration this diff does not touch is invisible to it. Comment stripping is \
          positional rather than lexical, so a `#` or a `//` inside a string that is not a URL \
          still truncates the line. The scorecard name, the published summary and the gate id \
          were all renamed to the lint this is, so nothing published still claims SPIFFE or \
          mTLS.",
    blocked_on: Some("a SPIFFE/SPIRE control plane"),
};

pub const DOC_PARITY_STATUS: GateFidelity = GateFidelity {
    gate_id: "doc_parity_status",
    aspiration: "Verify documentation parity with the change, and amend affected documents.",
    reference: "docs-as-code; Google g3doc",
    fidelity: Fidelity::Partial,
    gap: "Fails closed, creates missing ADRs, and corpus_sync amends owned pages so published \
          gate counts match TOTAL_GATES -- but only in Anvil's own repository \
          (doc_guard::corpus_sync::is_anvils_own_repository); on any other repository the sync \
          does not apply and the gate says so. generate_and_write_docs still writes only when a \
          file does not exist (doc_guard::generate_and_write_docs); it does not rewrite existing \
          documents, and a named file it leaves unchanged is not reported as updated.",
    blocked_on: None,
};

/// Named for the gate's published label rather than for its id.
///
/// Every other entry here takes the id upper-cased. This one cannot: the id
/// carries a stamp the naming law forbids in a DECLARED name, and spelling it
/// as a const would put one there. The id itself is a string, which the same
/// law reaches under its display-string rule and the debt ledger records.
pub const VENDOR_NEUTRALITY_STATUS: GateFidelity = GateFidelity {
    gate_id: "cloud_native_status",
    aspiration: "Keep core layers free of proprietary cloud SDKs and hardcoded cloud endpoints.",
    reference: "hexagonal architecture; vendor-neutral core",
    fidelity: Fidelity::Heuristic,
    gap: "Substring matching over added lines for five SDK crate prefixes and a fixed endpoint \
          pattern list, with the layer decided by `/core/` or `-domain/` appearing in the path. It \
          sees neither the crate graph nor whether an import is reached, so a vendor dependency \
          introduced transitively, or through a re-export, or in a core module whose path does not \
          say `core`, is invisible. Scope is added lines, so a vendor SDK already in the tree is \
          never examined.",
    blocked_on: None,
};

pub const STACK_WHITELIST_STATUS: GateFidelity = GateFidelity {
    gate_id: "stack_whitelist_status",
    aspiration: "Refuse a technology the approved stack does not name.",
    reference: "ADR-0700..ADR-0718 approved stack manifest",
    fidelity: Fidelity::Heuristic,
    gap: "Six banned crate prefixes matched as substrings of added lines. A seventh unapproved \
          technology is not refused because it is not on the list, and the list is written here \
          rather than derived from the ADRs it cites, so an ADR that changes its mandate does not \
          change this gate. Two of the guard's three rules -- apex-ADR immutability and \
          unauthorized dependency expansion -- fire only when the author is known to be an agent, \
          which this pipeline does not establish; it passes `is_human_author: true` so those two \
          are INERT rather than asserting authorship nobody measured, and they stay inert until \
          the pipeline measures it.",
    blocked_on: None,
};

pub const UNRESOLVED_REVIEW_STATUS: GateFidelity = GateFidelity {
    gate_id: "unresolved_review_status",
    aspiration: "Block on unresolved review threads using authoritative thread state.",
    reference: "GitHub GraphQL pullRequest.reviewThreads.nodes { isResolved }",
    fidelity: Fidelity::Measured,
    // Was Heuristic, for inferring resolution from comment text. The
    // inference is gone -- `merge_enlister` no longer reads comment bodies
    // at all, and `unresolved_review_guard::parse_review_threads` answers
    // only from `isResolved`, refusing every way the answer can fail to
    // arrive rather than reporting an empty list. Every one of those ways
    // is a seeded defect in the proof named by `FAILURE_PROOFS`.
    gap: "",
    blocked_on: None,
};

pub const AUTOMATED_CANARY_STATUS: GateFidelity = GateFidelity {
    gate_id: "automated_canary_status",
    aspiration: "Compare canary and baseline metric distributions from a live canary deployment with a \
                 Mann-Whitney U-test, and halt the rollout on a statistically significant regression.",
    reference: "Spinnaker/Kayenta Automated Canary Analysis; Mann-Whitney U-test",
    fidelity: Fidelity::Aspirational,
    gap: "Deploys no canary and queries no telemetry. The review pipeline used to write the samples it \
          then judged. Mann-Whitney is not implemented: evaluate_canary_distributions compares two \
          arithmetic means against a fixed relative bound \
          (automated_canary/statistical_engine.rs::StatisticalCanaryEngine). With no baseline_samples and no canary_samples \
          the gate now reports NotMeasured instead of a pass.",
    blocked_on: Some("a canary deployment with a queryable Prometheus or OpenTelemetry endpoint"),
};

pub const STACKED_DIFFS_STATUS: GateFidelity = GateFidelity {
    gate_id: "stacked_diffs_status",
    aspiration: "Read the pull request DAG from the forge, order the stack topologically, and verify \
                 every child is rebased on its parent before an atomic merge.",
    reference: "Phabricator/Graphite stacked diffs; Meta Sapling",
    fidelity: Fidelity::Aspirational,
    gap: "Reads no pull request DAG; the review pipeline passed an empty slice on every PR. Given a \
          real stack, compute_stack_plan still returns atomic_merge_ready unconditionally and orders \
          by input order rather than by parent links, so stack_depth is the only thing it derives \
          (stacked_diffs/dag_manager.rs::StackedDagManager). With no stack supplied the gate reports NotMeasured.",
    blocked_on: Some("a forge query for the pull requests stacked on this one"),
};

pub const MICROBENCH_STATUS: GateFidelity = GateFidelity {
    gate_id: "microbench_status",
    aspiration: "Run criterion benchmarks on the base and head revisions and ratchet hotpath ns/op \
                 against a published trunk baseline.",
    reference: "criterion.rs; Google Fleetbench",
    fidelity: Fidelity::Aspirational,
    gap: "Executes no benchmark: this repository declares no criterion dependency and has no benches \
          directory, so there is no baseline to ratchet against and the review pipeline used to write \
          a base_ns_per_op equal to its own head_ns_per_op. Both that sample and the analyzer that \
          read it are deleted; the gate reports NotMeasured.",
    blocked_on: Some("a criterion benchmark harness and a published trunk baseline"),
};

pub const SHAPE_STATUS: GateFidelity = GateFidelity {
    gate_id: "shape_status",
    aspiration: "Measure a repository's distance to its declared monorepo shape — unit skeleton, \
                 satellite placement, root hygiene, naming, and the Dependency Rule over real \
                 build edges — and refuse any regression past a baseline frozen at the merge-base.",
    reference: "oyatie ADR-0562 placement rule and ci/facade/baseline-ratchet; Google Rosie/Tricorder \
                ratchets; ArchUnit FreezingArchRule",
    fidelity: Fidelity::Measured,
    gap: "Every rule the aspiration names is measured from the tree at the PR head, each with a \
          seeded-defect fixture and a conformant twin. Dependency edges are read for all four \
          declarable profiles: Cargo path dependencies, Buck2 labels, module paths, and \
          TypeScript specifiers, where a bare import resolves through the name a workspace \
          manifest declares and a relative one through `target_dir` \
          (shape/adapters/ts_import_deps.rs::target_dir). Adapter naming is the tenant's own \
          `naming.adapter_name` template checked against the ports its unit declares, with the \
          rename that closes it (shape/core/adapter_naming.rs::adapter_naming_findings); a unit \
          with no ports crate is reported unmeasured, never accused. A blocking rule the engine \
          could not evaluate withholds through `unmeasured_reason` \
          (pre_merge_guard/shape_gate.rs::shape_gate_status): it found nothing only because it \
          never ran. Two things it does not do, neither of them claimed above: contention \
          metrics are not collected, and a TypeScript path alias no manifest claims reads as an \
          external package rather than an edge.",
    blocked_on: None,
};
