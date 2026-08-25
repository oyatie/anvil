//! Why a gate has no measurement, and which of those reasons may block a merge.
//!
//! # The over-correction this repairs
//!
//! `admission_refusal` refuses a report in which any gate produced no
//! measurement. That was the right fix for the original defect -- absence of a
//! finding read as absence of a problem -- and it went one step too far.
//!
//! Run against this repository's own pull requests, 34 of 72 gates report
//! `NotMeasured`, so no pull request has ever been admissible. A gate that can
//! never pass does not make merges safer; it makes the queue unreachable, and
//! an unreachable queue is drained by hand, which is how the certification
//! corpus stopped being consulted at all.
//!
//! # Three reasons, one of which is a defect
//!
//! Absence of evidence has three causes and they are not the same fact:
//!
//! * **Not provisioned.** The capability does not exist in this deployment. No
//!   Prometheus endpoint, no Sigstore backend, no cluster. The gate cannot
//!   measure for ANY pull request, and no author can act on it. A property of
//!   the deployment.
//! * **Not applicable.** The gate ran, searched a named subject set, and found
//!   it empty -- no `.sql` file in the change, no Cedar policy, no wire schema.
//!   The correct outcome for that change. A property of the change.
//! * **Not measured.** The gate could have measured and did not: a tool was
//!   missing, a call timed out, a parse failed. A defect, and the only one of
//!   the three that blocks.
//!
//! # Why this is declared and not inferred
//!
//! The three are distinguishable today only by reading the prose in a gate's
//! `reason`, and classifying by string match on prose is exactly the fragility
//! this codebase keeps paying for. The table below is checked in, reviewable,
//! and reconciled against `fidelity::registry` by test.
//!
//! # Why it cannot become a rubber stamp
//!
//! Four properties, each enforced:
//!
//! 1. **Unlisted means blocking.** A gate absent from this table is
//!    `Provisioned`, so a NEW gate that fails to measure blocks by default.
//!    Invariant I1 is intact for everything nobody has argued about.
//! 2. **Every entry names what is missing**, so an operator reads a capability
//!    or a subject set rather than a shrug.
//! 3. **A `NotProvisioned` gate that ever passes is a stale declaration** and
//!    fails a test: it measured something after all.
//! 4. **The count ratchets down.** `NOT_PROVISIONED_COUNT` is exact, so
//!    standing up a capability forces the row out in the same change.

/// Why a gate may be without a measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Absence {
    /// The deployment lacks the capability. Names it.
    NotProvisioned { capability: &'static str },
    /// The gate searched a subject set and found it empty. Names the set.
    NotApplicable { subject: &'static str },
    /// The gate could have measured. Absence is a defect and blocks.
    Provisioned,
}

impl Absence {
    /// Whether an absent measurement from this gate withholds the merge.
    ///
    /// Only `Provisioned` does. The other two are absent for reasons no author
    /// can act on and no commit can change.
    pub fn blocks_admission(self) -> bool {
        matches!(self, Absence::Provisioned)
    }
}

/// Gates whose absence is not a defect, and why.
///
/// Ordered as the matrix reports them. Every row is a claim someone can
/// disagree with in review, which is the point of writing it down.
pub const ABSENCE_POLICY: &[(&str, Absence)] = &[
    // ---- not provisioned: no such capability in this deployment -----------
    (
        "slo_status",
        Absence::NotProvisioned {
            capability: "a Prometheus or OpenTelemetry endpoint to query error-budget burn",
        },
    ),
    (
        "canary_status",
        Absence::NotProvisioned {
            capability: "a canary deployment and a metrics endpoint",
        },
    ),
    (
        "automated_canary_status",
        Absence::NotProvisioned {
            capability: "a canary deployment and a metrics endpoint",
        },
    ),
    (
        "auto_rollback_status",
        Absence::NotProvisioned {
            capability: "canary error-rate and latency telemetry",
        },
    ),
    (
        "progressive_ring_status",
        Absence::NotProvisioned {
            capability: "a ring deployer and a reachable cloud control plane",
        },
    ),
    (
        "cluster_audit_status",
        Absence::NotProvisioned {
            capability: "Kubernetes API or ArgoCD cluster access",
        },
    ),
    (
        "shuffle_status",
        Absence::NotProvisioned {
            capability: "a tenant-to-cell mapping from a control plane or checked-in topology",
        },
    ),
    (
        "ci_wallclock_status",
        Absence::NotProvisioned {
            capability: "GitHub Actions workflow-run timing API access",
        },
    ),
    (
        "remote_cache_status",
        Absence::NotProvisioned {
            capability: "an sccache or Buck2 CAS statistics endpoint",
        },
    ),
    (
        "sandbox_status",
        Absence::NotProvisioned {
            capability: "an ephemeral sandbox runtime",
        },
    ),
    (
        "shadow_traffic_status",
        Absence::NotProvisioned {
            capability: "a traffic mirror and a replay target",
        },
    ),
    (
        "replay_harness_status",
        Absence::NotProvisioned {
            capability: "a production trace corpus to replay",
        },
    ),
    (
        "zero_day_status",
        Absence::NotProvisioned {
            capability: "an advisory feed and a patch writer",
        },
    ),
    (
        "openvex_status",
        Absence::NotProvisioned {
            capability: "an advisory feed and a dependency inventory",
        },
    ),
    (
        "upgrade_train_status",
        Absence::NotProvisioned {
            capability: "a dependency manifest and advisory feed to audit upgrades against",
        },
    ),
    (
        "cosign_status",
        Absence::NotProvisioned {
            capability: "a Sigstore signing backend (Fulcio, Rekor)",
        },
    ),
    (
        "attestation_status",
        Absence::NotProvisioned {
            capability: "a provenance backend: a signing key and a transparency log",
        },
    ),
    (
        "carbon_compute_status",
        Absence::NotProvisioned {
            capability: "a CPU-time or grid-intensity reading",
        },
    ),
    (
        "flake_quarantine_status",
        Absence::NotProvisioned {
            capability: "retained test-run history and a quarantine lane",
        },
    ),
    (
        "microbench_status",
        Absence::NotProvisioned {
            capability: "a criterion harness and a published trunk baseline",
        },
    ),
    (
        "stacked_diffs_status",
        Absence::NotProvisioned {
            capability: "a forge query for the pull-request DAG",
        },
    ),
    (
        "coverage_status",
        Absence::NotProvisioned {
            capability: "a coverage instrumentation run; the registry records this gate as aspirational",
        },
    ),
    (
        "hermetic_build_status",
        Absence::NotProvisioned {
            capability: "a second build to compare bit-for-bit",
        },
    ),
    (
        "chaos_injection_status",
        Absence::NotProvisioned {
            capability: "a running deployment and a fault injector",
        },
    ),
    (
        "feature_flag_status",
        Absence::NotProvisioned {
            capability: "a flag lifecycle source, or a STALE-FLAGS ledger in the repository under review",
        },
    ),
    (
        "adr_status",
        Absence::NotProvisioned {
            capability: "an ADR field schema declared by the repository under review",
        },
    ),
    // ---- not applicable: the change carries no subject for this gate ------
    (
        "cedar_status",
        Absence::NotApplicable {
            subject: "a Cedar policy file (*.cedar) in the change",
        },
    ),
    (
        "clean_arch_status",
        Absence::NotApplicable {
            subject: "a changed file inside a core/ports/adapters/facade layer",
        },
    ),
    (
        "debt_shrink_status",
        Absence::NotApplicable {
            subject: "a changed file marked deprecating, or named by a drain ledger",
        },
    ),
    (
        "finops_status",
        Absence::NotApplicable {
            subject: "a changed file matching the hotpath marker set",
        },
    ),
    (
        "ghost_migration_status",
        Absence::NotApplicable {
            subject: "a changed .sql file or migrations/ path",
        },
    ),
    (
        "migration_orch_status",
        Absence::NotApplicable {
            subject: "a changed .sql migration",
        },
    ),
    (
        "gitops_drift_status",
        Absence::NotApplicable {
            subject: "a changed GitOps manifest (applicationset, application.yaml)",
        },
    ),
    (
        "schema_evolution_status",
        Absence::NotApplicable {
            subject: "a changed wire schema (.proto, or an OpenAPI description)",
        },
    ),
];

/// How many rows currently say the deployment cannot measure.
///
/// EXACT, and it must fall. Standing up a capability removes its row in the
/// same change, and a table that only ever grows is a way of switching the
/// corpus off one gate at a time.
pub const NOT_PROVISIONED_COUNT: usize = 26;

/// Why this gate's absence is what it is. `Provisioned` unless argued otherwise.
pub fn absence_of(gate_id: &str) -> Absence {
    ABSENCE_POLICY
        .iter()
        .find(|(id, _)| *id == gate_id)
        .map(|(_, a)| *a)
        .unwrap_or(Absence::Provisioned)
}

/// Whether an absent measurement from this gate withholds the merge.
pub fn absence_blocks(gate_id: &str) -> bool {
    absence_of(gate_id).blocks_admission()
}
