use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreMergeCertificationReport {
    pub is_certified_ready: bool,
    pub doc_parity_status: GateStatus,
    pub cedar_status: GateStatus,
    pub compliance_status: GateStatus,
    pub api_contract_status: GateStatus,
    pub cell_isolation_status: GateStatus,
    pub supply_chain_status: GateStatus,
    pub clean_arch_status: GateStatus,
    pub monorepo_status: GateStatus,
    pub debt_shrink_status: GateStatus,
    pub modularization_status: GateStatus,
    pub coverage_status: GateStatus,
    pub rust_skills_status: GateStatus,
    pub kani_status: GateStatus,
    pub slo_status: GateStatus,
    pub adr_status: GateStatus,
    pub shuffle_status: GateStatus,
    pub trace_status: GateStatus,
    pub constant_work_status: GateStatus,
    pub idempotency_status: GateStatus,
    pub finops_status: GateStatus,
    pub ghost_migration_status: GateStatus,
    pub gitops_promo_status: GateStatus,
    pub gitops_drift_status: GateStatus,
    pub canary_status: GateStatus,
    pub cluster_audit_status: GateStatus,
    pub migration_orch_status: GateStatus,
    pub ci_wallclock_status: GateStatus,
    pub predictive_test_status: GateStatus,
    pub compile_profile_status: GateStatus,
    pub remote_cache_status: GateStatus,
    pub runner_economics_status: GateStatus,
    pub sandbox_status: GateStatus,
    pub cross_service_status: GateStatus,
    pub ephemeral_secret_status: GateStatus,
    pub psa_status: GateStatus,
    pub shadow_traffic_status: GateStatus,
    pub unresolved_review_status: GateStatus,
    pub local_probe_status: GateStatus,
    pub semantic_abi_status: GateStatus,
    pub zero_day_status: GateStatus,
    pub formal_verification_status: GateStatus,
    pub deadlock_status: GateStatus,
    /// Verdict of the AI code review and 16-lens matrix.
    ///
    /// This was computed in the evaluator and then thrown away: it never became
    /// a field, so `all_statuses()` could not see it and `seal()` could not gate
    /// on it. A pull request whose review returned REQUEST_CHANGES or REJECT was
    /// still certified. The original chain in 117a1f6 ended
    /// `&& review_verdict_status.is_acceptable()`; when certification moved to
    /// `seal()`, the value was left behind rather than carried across.
    ///
    /// Nothing failed when that happened -- the gate simply stopped mattering,
    /// silently, which is why an unused-variable lint found it and no review did.
    pub review_verdict_status: GateStatus,
    /// Names and PR-visible strings must describe what the code verifies, not
    /// stamp an aspiration onto it. Anvil enforced naming discipline on other
    /// repositories while carrying `hyperscaler_consensus_guard` and
    /// `EnterpriseAgenticPipelineRouter` itself.
    pub brand_absence_status: GateStatus,
    /// Code that is migrating to oyatie must not depend on code oyatie
    /// supersedes -- it cannot migrate while anchored to something being
    /// deleted.
    pub migration_boundary_status: GateStatus,
    /// Distance to the tenant's shape spec, judged against the baseline frozen
    /// at the merge-base (Shape Program). Blocking rules may not regress.
    pub shape_status: GateStatus,
    pub automated_canary_status: GateStatus,
    pub progressive_ring_status: GateStatus,
    pub hermetic_build_status: GateStatus,
    pub openvex_status: GateStatus,
    pub cosign_status: GateStatus,
    pub chaos_injection_status: GateStatus,
    pub stacked_diffs_status: GateStatus,
    pub microbench_status: GateStatus,
    pub jittered_backoff_status: GateStatus,
    pub schema_evolution_status: GateStatus,
    pub auto_rollback_status: GateStatus,
    pub wasm_sandbox_status: GateStatus,
    pub consistency_status: GateStatus,
    pub flake_quarantine_status: GateStatus,
    pub zero_trust_workload_status: GateStatus,
    pub carbon_compute_status: GateStatus,
    pub replay_harness_status: GateStatus,
    pub upgrade_train_status: GateStatus,
    pub mutation_status: GateStatus,
    pub feature_flag_status: GateStatus,
    pub bench_status: GateStatus,
    pub attestation_status: GateStatus,
    pub security_scan_status: GateStatus,
    pub schema_compat_status: GateStatus,
    pub performance_concurrency_status: GateStatus,
    pub test_suite_status: GateStatus,
    /// Gate ids that reported `NotMeasured`. Non-empty means certification is
    /// incomplete even when `is_certified_ready` is true: the badge and the
    /// merge-admission decision are deliberately decoupled (invariant I1 —
    /// absent evidence is never a pass, but nor is it a false accusation).
    /// No `serde(default)`: a payload without this field would deserialise to
    /// an empty vector, and an empty vector means admissible. Absent evidence
    /// must fail to parse, not arrive looking measured.
    pub unmeasured_gates: Vec<String>,
    pub summary_markdown: String,
    /// Where the seventy-two statuses above came from.
    ///
    /// A report that a certification run produced and a report a caller wrote
    /// are the same fields: `is_admissible()` says yes to both and
    /// `gate_counts()` scores both at the whole corpus. Only this says which
    /// one is in hand, and it is deliberately not serialisable -- a report that
    /// arrived over a wire or out of a cache is a copy of a measurement, not
    /// one.
    ///
    /// Not `pub`: a `Copy` field readable and writable from anywhere is a mark
    /// that can be lifted off a genuine report and dropped onto a struct
    /// literal. It has no public reader either — `admission_refusal` below is
    /// the only thing that asks, and a report has exactly one door, so an
    /// accessor would be public surface with nothing on the other side of it.
    #[serde(skip)]
    pub(super) provenance: GateProvenance,
    /// The pull request and the commit this run measured, or `None` for a
    /// report that was not produced against one.
    ///
    /// Without it a report proves "some certification run produced an
    /// all-passing report" and never "...for this pull request at the commit
    /// about to be queued", and the two are not the same claim: the healer
    /// pushes a healed commit and then certifies whatever GitHub reports as
    /// head, and a contributor can push while the corpus is running.
    /// `enlist_into_merge_queue` re-reads the head and refuses when it has
    /// moved.
    #[serde(default)]
    /// `pub(crate)` rather than `pub(super)`: the door that acts on this lives
    /// in `merge_enlister`, and while only the evaluator may set it, the check
    /// that reads it was untestable from outside `pre_merge_guard` — which is
    /// why it went unpinned.
    pub(crate) subject: Option<CertifiedSubject>,
}

/// The pull request and commit a certification run was performed against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertifiedSubject {
    pub repo: String,
    pub pr_number: u64,
    pub head_sha: String,
}

/// Whether a report's gate statuses were handed to it by a certification run.
///
/// What this actually establishes, and what it does not:
///
/// - `Default` -- what a **deserialised** report gets, because the field is
///   `#[serde(skip)]` -- is "no run produced this". So a report reloaded from
///   state, read out of a cache, or round-tripped through serde with its
///   statuses overwritten carries no mark, and `admission_refusal` refuses it.
///   That is the forgery this design stops.
/// - It does **not** stop an outcome list. `from_gate_outcomes` is a `pub`
///   constructor -- the spec suite calls it from outside the crate -- and it
///   confers this mark on whatever seventy-two statuses it is handed. A caller
///   who can write `(name, GateStatus::Passed)` seventy-two times gets a
///   genuinely marked, fully admissible report. The mark means "these statuses
///   arrived as gate outcomes", not "a gate produced them".
///
/// The inner flag is private to this module and `certification_run()` is
/// `pub(super)`, so the mark cannot be minted outside `pre_merge_guard` --
/// but `from_gate_outcomes` is the public door through it, and the forgery
/// scan in `tests/enlist_authority_test.rs` is what keeps production code
/// from walking through that door instead of running the corpus.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GateProvenance(bool);

impl GateProvenance {
    /// Minted only where gate outcomes become a report: `from_gate_outcomes`
    /// and the evaluator that runs the corpus.
    pub(super) fn certification_run() -> Self {
        Self(true)
    }

    /// Whether the statuses on this report were produced by a certification run.
    pub fn is_from_a_certification_run(self) -> bool {
        self.0
    }
}

/// Size of the gate corpus.
///
/// Anvil published "70 gates" in seven PR-visible strings while the matrix held
/// 68. A count posted onto a pull request is a claim like any other, and this
/// one was wrong in every place it appeared -- each string had been written by
/// hand and none was rechecked when the corpus changed.
///
/// `all_statuses_matches_the_declared_total` pins this against the real field
/// count, so the next corpus change fails a test instead of silently making
/// seven strings lie.
pub const TOTAL_GATES: usize = 72;

impl PreMergeCertificationReport {
    /// Every gate status on this report, in declaration order.
    pub fn all_statuses(&self) -> Vec<&GateStatus> {
        vec![
            &self.doc_parity_status,
            &self.cedar_status,
            &self.compliance_status,
            &self.api_contract_status,
            &self.cell_isolation_status,
            &self.supply_chain_status,
            &self.clean_arch_status,
            &self.monorepo_status,
            &self.debt_shrink_status,
            &self.modularization_status,
            &self.coverage_status,
            &self.rust_skills_status,
            &self.kani_status,
            &self.slo_status,
            &self.adr_status,
            &self.shuffle_status,
            &self.trace_status,
            &self.constant_work_status,
            &self.idempotency_status,
            &self.finops_status,
            &self.ghost_migration_status,
            &self.gitops_promo_status,
            &self.gitops_drift_status,
            &self.canary_status,
            &self.cluster_audit_status,
            &self.migration_orch_status,
            &self.ci_wallclock_status,
            &self.predictive_test_status,
            &self.compile_profile_status,
            &self.remote_cache_status,
            &self.runner_economics_status,
            &self.sandbox_status,
            &self.cross_service_status,
            &self.ephemeral_secret_status,
            &self.psa_status,
            &self.shadow_traffic_status,
            &self.unresolved_review_status,
            &self.local_probe_status,
            &self.semantic_abi_status,
            &self.zero_day_status,
            &self.formal_verification_status,
            &self.deadlock_status,
            &self.review_verdict_status,
            &self.brand_absence_status,
            &self.migration_boundary_status,
            &self.shape_status,
            &self.automated_canary_status,
            &self.progressive_ring_status,
            &self.hermetic_build_status,
            &self.openvex_status,
            &self.cosign_status,
            &self.chaos_injection_status,
            &self.stacked_diffs_status,
            &self.microbench_status,
            &self.jittered_backoff_status,
            &self.schema_evolution_status,
            &self.auto_rollback_status,
            &self.wasm_sandbox_status,
            &self.consistency_status,
            &self.flake_quarantine_status,
            &self.zero_trust_workload_status,
            &self.carbon_compute_status,
            &self.replay_harness_status,
            &self.upgrade_train_status,
            &self.mutation_status,
            &self.feature_flag_status,
            &self.bench_status,
            &self.attestation_status,
            &self.security_scan_status,
            &self.schema_compat_status,
            &self.performance_concurrency_status,
            &self.test_suite_status,
        ]
    }

    /// Every gate status paired with its field name, in declaration order.
    ///
    /// Used to record which specific gates failed, rather than only how many.
    pub fn named_statuses(&self) -> Vec<(&'static str, &GateStatus)> {
        vec![
            ("doc_parity_status", &self.doc_parity_status),
            ("cedar_status", &self.cedar_status),
            ("compliance_status", &self.compliance_status),
            ("api_contract_status", &self.api_contract_status),
            ("cell_isolation_status", &self.cell_isolation_status),
            ("supply_chain_status", &self.supply_chain_status),
            ("clean_arch_status", &self.clean_arch_status),
            ("monorepo_status", &self.monorepo_status),
            ("debt_shrink_status", &self.debt_shrink_status),
            ("modularization_status", &self.modularization_status),
            ("coverage_status", &self.coverage_status),
            ("rust_skills_status", &self.rust_skills_status),
            ("kani_status", &self.kani_status),
            ("slo_status", &self.slo_status),
            ("adr_status", &self.adr_status),
            ("shuffle_status", &self.shuffle_status),
            ("trace_status", &self.trace_status),
            ("constant_work_status", &self.constant_work_status),
            ("idempotency_status", &self.idempotency_status),
            ("finops_status", &self.finops_status),
            ("ghost_migration_status", &self.ghost_migration_status),
            ("gitops_promo_status", &self.gitops_promo_status),
            ("gitops_drift_status", &self.gitops_drift_status),
            ("canary_status", &self.canary_status),
            ("cluster_audit_status", &self.cluster_audit_status),
            ("migration_orch_status", &self.migration_orch_status),
            ("ci_wallclock_status", &self.ci_wallclock_status),
            ("predictive_test_status", &self.predictive_test_status),
            ("compile_profile_status", &self.compile_profile_status),
            ("remote_cache_status", &self.remote_cache_status),
            ("runner_economics_status", &self.runner_economics_status),
            ("sandbox_status", &self.sandbox_status),
            ("cross_service_status", &self.cross_service_status),
            ("ephemeral_secret_status", &self.ephemeral_secret_status),
            ("psa_status", &self.psa_status),
            ("shadow_traffic_status", &self.shadow_traffic_status),
            ("unresolved_review_status", &self.unresolved_review_status),
            ("local_probe_status", &self.local_probe_status),
            ("semantic_abi_status", &self.semantic_abi_status),
            ("zero_day_status", &self.zero_day_status),
            (
                "formal_verification_status",
                &self.formal_verification_status,
            ),
            ("deadlock_status", &self.deadlock_status),
            ("review_verdict_status", &self.review_verdict_status),
            ("brand_absence_status", &self.brand_absence_status),
            ("migration_boundary_status", &self.migration_boundary_status),
            ("shape_status", &self.shape_status),
            ("automated_canary_status", &self.automated_canary_status),
            ("progressive_ring_status", &self.progressive_ring_status),
            ("hermetic_build_status", &self.hermetic_build_status),
            ("openvex_status", &self.openvex_status),
            ("cosign_status", &self.cosign_status),
            ("chaos_injection_status", &self.chaos_injection_status),
            ("stacked_diffs_status", &self.stacked_diffs_status),
            ("microbench_status", &self.microbench_status),
            ("jittered_backoff_status", &self.jittered_backoff_status),
            ("schema_evolution_status", &self.schema_evolution_status),
            ("auto_rollback_status", &self.auto_rollback_status),
            ("wasm_sandbox_status", &self.wasm_sandbox_status),
            ("consistency_status", &self.consistency_status),
            ("flake_quarantine_status", &self.flake_quarantine_status),
            (
                "zero_trust_workload_status",
                &self.zero_trust_workload_status,
            ),
            ("carbon_compute_status", &self.carbon_compute_status),
            ("replay_harness_status", &self.replay_harness_status),
            ("upgrade_train_status", &self.upgrade_train_status),
            ("mutation_status", &self.mutation_status),
            ("feature_flag_status", &self.feature_flag_status),
            ("bench_status", &self.bench_status),
            ("attestation_status", &self.attestation_status),
            ("security_scan_status", &self.security_scan_status),
            ("schema_compat_status", &self.schema_compat_status),
            (
                "performance_concurrency_status",
                &self.performance_concurrency_status,
            ),
            ("test_suite_status", &self.test_suite_status),
        ]
    }

    /// Real pass/fail counts, computed from the statuses.
    ///
    /// These were previously hardcoded at the call site as `(70, 0)` when
    /// certified and `(69, 1)` otherwise -- so every failing PR was recorded as
    /// exactly one failed gate regardless of how many actually failed, and the
    /// resulting "95% of PRs stuck at 69/70" in telemetry was an artefact of
    /// that constant, not a measurement (invariant I2).
    pub fn gate_counts(&self) -> (usize, usize) {
        let all = self.all_statuses();
        let passed = all.iter().filter(|s| s.is_acceptable()).count();
        (passed, all.len() - passed)
    }

    /// Recomputes `unmeasured_gates` from the current gate statuses.
    ///
    /// Called at construction so the field can never drift from the statuses it
    /// summarises. A gate reporting `NotMeasured` is acceptable individually but
    /// must still block merge-queue admission — see `is_admissible`.
    pub fn recompute_unmeasured(&mut self) {
        self.unmeasured_gates = self
            .all_statuses()
            .iter()
            .filter_map(|s| s.unmeasured_gate_id().map(str::to_string))
            .collect();
    }

    /// The pull request and commit this report was measured against, if it was
    /// measured against one.
    pub fn subject(&self) -> Option<&CertifiedSubject> {
        self.subject.as_ref()
    }

    /// Whether the evidence in this report admits a pull request to the merge
    /// queue. `Ok(())` admits; `Err` refuses and says why.
    ///
    /// This is the single definition of admissibility.
    /// `MergeEnlister::admission_refusal` is a one-line delegation to it, so
    /// the door and the two publishers ask the same question of the same value.
    ///
    /// Three ways evidence can be absent, and all three withhold the merge
    /// (invariant I1):
    ///
    /// 1. the report did not come from a certification run, so its statuses are
    ///    somebody's opinion in the shape of a measurement;
    /// 2. a gate produced no measurement — `NotMeasured`, which is individually
    ///    acceptable and still absent evidence, or `Errored`, which
    ///    `unmeasured_gates` does not record at all;
    /// 3. the report does not certify.
    ///
    /// The refusal names the gates, because an operator watching a pull request
    /// sit in the queue has nothing else to act on.
    ///
    /// Deliberately not a check on the *subject*: whether this report is about
    /// the commit being queued is a question about the pull request as it is
    /// now, not about the report, and it is asked at the entry point where the
    /// head can be re-read. See `MergeEnlister::enlist_into_merge_queue`.
    pub fn admission_refusal(&self) -> anyhow::Result<()> {
        if !self.provenance.is_from_a_certification_run() {
            anyhow::bail!(
                "merge queue admission withheld: this certification report was not produced \
                 by a certification run, so nothing in it was measured."
            );
        }

        let without_a_measurement: Vec<&str> = self
            .named_statuses()
            .into_iter()
            .filter(|(_, status)| {
                matches!(
                    status,
                    GateStatus::Errored(_) | GateStatus::NotMeasured { .. }
                )
            })
            .map(|(gate, _)| gate)
            .collect();
        if !without_a_measurement.is_empty() {
            anyhow::bail!(
                "merge queue admission withheld: {} gate(s) produced no measurement: {}",
                without_a_measurement.len(),
                without_a_measurement.join(", ")
            );
        }

        if !self.is_certified_ready {
            let blocking: Vec<&str> = self
                .named_statuses()
                .into_iter()
                .filter(|(_, status)| !status.is_acceptable())
                .map(|(gate, _)| gate)
                .collect();
            anyhow::bail!(
                "merge queue admission withheld: the pull request is not certified; {} gate(s) \
                 did not pass: {}",
                blocking.len(),
                blocking.join(", ")
            );
        }

        Ok(())
    }

    /// A weaker, diagnostic reading of the same fields: certified, and no gate
    /// reported `NotMeasured`.
    ///
    /// **This is not the admission decision** — `admission_refusal()` is, and
    /// nothing in production gates a merge on this predicate. It is
    /// deliberately weaker in two ways the spec suite pins and depends on
    /// (`nothing_is_endorsed_on_evidence_that_cannot_admit_the_pull_request`
    /// requires the two to disagree): it does not see `Errored`, which
    /// `recompute_unmeasured` never records, and it does not see provenance, so
    /// it says yes to a report no run produced. Use it to describe a report — a
    /// receipt verdict, a scorecard line — never to let one through a door.
    pub fn is_admissible(&self) -> bool {
        self.is_certified_ready && self.unmeasured_gates.is_empty()
    }

    /// A report built from what the gates actually measured: every gate in the
    /// corpus named with the status it produced. `Err` when the outcomes do
    /// not cover the corpus, because a report missing a gate is a report with
    /// a hole in it.
    ///
    /// This is the only way gate outcomes enter a report, and a report that
    /// did not come through here did not come from a measurement — it was
    /// deserialised, cloned from `unmeasured`, or assembled by a caller who
    /// decided what the gates would have said. `admission_refusal` has to be
    /// able to tell those apart, so the difference is carried by the value and
    /// not by the spelling of whatever produced it: a report knows whether its
    /// statuses were handed to it as gate outcomes.
    ///
    /// What this constructor does and does not establish. It confers the
    /// provenance mark on whatever statuses it is handed, and it is `pub`
    /// because the spec suite builds its fixtures through it from outside the
    /// crate. So it is a public door onto the mark: a caller who writes out
    /// seventy-two `(name, GateStatus::Passed)` pairs gets a fully admissible
    /// report, and `admission_refusal` cannot tell it from one the corpus
    /// produced. What the mark does rule out is the *deserialised* report — the
    /// field is `#[serde(skip)]`, so anything that arrived over a wire, out of
    /// a cache, or through a serde round-trip carries no mark at all. Keeping
    /// production code off this door is the forgery scan's job, not the type's:
    /// see `every_door_hands_the_merge_queue_evidence_a_certification_run_produced`.
    ///
    /// It confers no subject: a report built from an outcome list was not
    /// measured against a pull request, so it names none, and the entry point
    /// refuses to queue a commit no report was measured against.
    pub fn from_gate_outcomes(outcomes: &[(&str, GateStatus)]) -> anyhow::Result<Self> {
        let mut by_gate: std::collections::HashMap<&str, GateStatus> =
            std::collections::HashMap::with_capacity(outcomes.len());
        let mut named_twice: Vec<&str> = Vec::new();
        for (gate, status) in outcomes {
            if by_gate.insert(gate, status.clone()).is_some() {
                named_twice.push(gate);
            }
        }

        // Every field asks for its own outcome, so a gate nobody reported is
        // discovered by the construction rather than by a length check: the
        // right *number* of outcomes naming one gate twice is still a corpus
        // with a hole in it.
        let mut unreported: Vec<&'static str> = Vec::new();
        let mut report = Self::build(&mut |gate| match by_gate.remove(gate) {
            Some(status) => status,
            None => {
                unreported.push(gate);
                GateStatus::Errored(format!("no gate outcome was reported for `{gate}`"))
            }
        });
        let mut not_in_the_corpus: Vec<&str> = by_gate.keys().copied().collect();
        not_in_the_corpus.sort_unstable();

        if !unreported.is_empty() || !not_in_the_corpus.is_empty() || !named_twice.is_empty() {
            let mut why: Vec<String> = Vec::new();
            if !unreported.is_empty() {
                why.push(format!(
                    "{} gate(s) reported no outcome: {}",
                    unreported.len(),
                    unreported.join(", ")
                ));
            }
            if !named_twice.is_empty() {
                why.push(format!("named more than once: {}", named_twice.join(", ")));
            }
            if !not_in_the_corpus.is_empty() {
                why.push(format!(
                    "not gates in this corpus: {}",
                    not_in_the_corpus.join(", ")
                ));
            }
            anyhow::bail!(
                "a certification report covers every one of the {} gates in the corpus, and \
                 these outcomes do not: {}",
                TOTAL_GATES,
                why.join("; ")
            );
        }

        report.provenance = GateProvenance::certification_run();
        report.seal();
        report.summary_markdown = super::matrix::MatrixRenderer::render(&report);
        Ok(report)
    }

    /// The one place the seventy-two gate fields are written down as a
    /// construction, so a gate added to the struct has to be given a value here
    /// or the build fails. `status_for` is asked for each gate by its field
    /// name, in declaration order.
    ///
    /// The report it returns carries no provenance and no verdict: sealing and
    /// the provenance mark belong to the constructors that know where the
    /// statuses came from.
    fn build(status_for: &mut dyn FnMut(&'static str) -> GateStatus) -> Self {
        PreMergeCertificationReport {
            is_certified_ready: false,
            doc_parity_status: status_for("doc_parity_status"),
            cedar_status: status_for("cedar_status"),
            compliance_status: status_for("compliance_status"),
            api_contract_status: status_for("api_contract_status"),
            cell_isolation_status: status_for("cell_isolation_status"),
            supply_chain_status: status_for("supply_chain_status"),
            clean_arch_status: status_for("clean_arch_status"),
            monorepo_status: status_for("monorepo_status"),
            debt_shrink_status: status_for("debt_shrink_status"),
            modularization_status: status_for("modularization_status"),
            coverage_status: status_for("coverage_status"),
            rust_skills_status: status_for("rust_skills_status"),
            kani_status: status_for("kani_status"),
            slo_status: status_for("slo_status"),
            adr_status: status_for("adr_status"),
            shuffle_status: status_for("shuffle_status"),
            trace_status: status_for("trace_status"),
            constant_work_status: status_for("constant_work_status"),
            idempotency_status: status_for("idempotency_status"),
            finops_status: status_for("finops_status"),
            ghost_migration_status: status_for("ghost_migration_status"),
            gitops_promo_status: status_for("gitops_promo_status"),
            gitops_drift_status: status_for("gitops_drift_status"),
            canary_status: status_for("canary_status"),
            cluster_audit_status: status_for("cluster_audit_status"),
            migration_orch_status: status_for("migration_orch_status"),
            ci_wallclock_status: status_for("ci_wallclock_status"),
            predictive_test_status: status_for("predictive_test_status"),
            compile_profile_status: status_for("compile_profile_status"),
            remote_cache_status: status_for("remote_cache_status"),
            runner_economics_status: status_for("runner_economics_status"),
            sandbox_status: status_for("sandbox_status"),
            cross_service_status: status_for("cross_service_status"),
            ephemeral_secret_status: status_for("ephemeral_secret_status"),
            psa_status: status_for("psa_status"),
            shadow_traffic_status: status_for("shadow_traffic_status"),
            unresolved_review_status: status_for("unresolved_review_status"),
            local_probe_status: status_for("local_probe_status"),
            semantic_abi_status: status_for("semantic_abi_status"),
            zero_day_status: status_for("zero_day_status"),
            formal_verification_status: status_for("formal_verification_status"),
            deadlock_status: status_for("deadlock_status"),
            review_verdict_status: status_for("review_verdict_status"),
            brand_absence_status: status_for("brand_absence_status"),
            migration_boundary_status: status_for("migration_boundary_status"),
            shape_status: status_for("shape_status"),
            automated_canary_status: status_for("automated_canary_status"),
            progressive_ring_status: status_for("progressive_ring_status"),
            hermetic_build_status: status_for("hermetic_build_status"),
            openvex_status: status_for("openvex_status"),
            cosign_status: status_for("cosign_status"),
            chaos_injection_status: status_for("chaos_injection_status"),
            stacked_diffs_status: status_for("stacked_diffs_status"),
            microbench_status: status_for("microbench_status"),
            jittered_backoff_status: status_for("jittered_backoff_status"),
            schema_evolution_status: status_for("schema_evolution_status"),
            auto_rollback_status: status_for("auto_rollback_status"),
            wasm_sandbox_status: status_for("wasm_sandbox_status"),
            consistency_status: status_for("consistency_status"),
            flake_quarantine_status: status_for("flake_quarantine_status"),
            zero_trust_workload_status: status_for("zero_trust_workload_status"),
            carbon_compute_status: status_for("carbon_compute_status"),
            replay_harness_status: status_for("replay_harness_status"),
            upgrade_train_status: status_for("upgrade_train_status"),
            mutation_status: status_for("mutation_status"),
            feature_flag_status: status_for("feature_flag_status"),
            bench_status: status_for("bench_status"),
            attestation_status: status_for("attestation_status"),
            security_scan_status: status_for("security_scan_status"),
            schema_compat_status: status_for("schema_compat_status"),
            performance_concurrency_status: status_for("performance_concurrency_status"),
            test_suite_status: status_for("test_suite_status"),
            unmeasured_gates: Vec::new(),
            summary_markdown: String::new(),
            provenance: GateProvenance::default(),
            subject: None,
        }
    }

    /// A report in which nothing has been measured: every gate is
    /// `NotMeasured` with `reason`, nothing is certified, nothing is
    /// admissible. The honest starting point for a fixture or a preview —
    /// there is deliberately no "all passed" constructor (I2), and this one
    /// confers no provenance: nothing ran.
    pub fn unmeasured(reason: &str) -> Self {
        let mut r = Self::build(&mut |gate_id| GateStatus::NotMeasured {
            gate_id: gate_id.to_string(),
            reason: reason.to_string(),
        });
        r.seal();
        r
    }

    /// Derives every summary field from the gate statuses.
    ///
    /// `is_certified_ready` is the conjunction of `all_statuses()`, so a gate
    /// added to the struct is in the verdict by construction. The evaluator
    /// previously held a hand-written 68-term conjunction that was computed
    /// before the two self-directed gates existed, so `brand_absence_status`
    /// and `migration_boundary_status` could fail while the report certified.
    /// The field list is pinned to `TOTAL_GATES` by test; the verdict now
    /// reads that list rather than a second copy of it.
    pub fn seal(&mut self) {
        self.recompute_unmeasured();
        self.is_certified_ready = self.all_statuses().iter().all(|s| s.is_acceptable());
    }

    /// Withholds the pass of every gate the fidelity registry records as
    /// `Aspirational`, replacing it with `NotMeasured` naming the registry.
    ///
    /// `Fidelity::Aspirational` means "named only; no implementation of the
    /// claimed capability exists", and `Fidelity::may_report_pass()` has said
    /// since it was written that such a gate must report
    /// `GateStatus::NotMeasured`. Nothing called it. The rule was stated in the
    /// enum's doc comment, encoded in a method and enforced nowhere, so seven
    /// aspirational gates published `Passed` on every certified pull request.
    /// This is that method's production consumer.
    ///
    /// Three boundaries, each pinned by
    /// `tests/aspirational_gates_cannot_pass_test.rs`:
    ///
    /// - **Only a pass is withheld.** `Failed`, `Warning`, `Errored` and a
    ///   gate's own `NotMeasured` come through untouched. Erasing a finding
    ///   behind a fidelity rule would hide the very defect the rule exists for,
    ///   and overwriting a guard's own account of why it could not measure
    ///   would replace something specific with something generic.
    /// - **Only an aspirational gate.** A `Heuristic`, `Partial` or `Measured`
    ///   gate measured something; taking its pass away deletes real evidence.
    /// - **Only an audited gate.** A gate with no registry entry is left exactly
    ///   as its guard reported it. The registry has not read it, so it has no
    ///   opinion, and manufacturing a `NotMeasured` for a gate nobody examined
    ///   is the symmetric violation of I1 -- a fabricated absence in place of a
    ///   fabricated pass. That exemption covers three of the seventy-two gates
    ///   -- the three being rewritten in open pull requests, which the registry
    ///   deliberately does not describe ahead of them -- and it is not silent:
    ///   `fidelity::gap_report().unaudited` publishes its size.
    ///
    /// Applied by `evaluate_pre_merge_gates` before it seals, so the withheld
    /// gates land in `unmeasured_gates`, in the verdict and in the matrix. Why
    /// the rule lives here and not inside `seal()` is argued in the pull
    /// request that added it.
    ///
    /// Ends by sealing, so the method is total: `is_certified_ready` and
    /// `unmeasured_gates` are derived from the statuses this just rewrote, and
    /// leaving them carried across from before the rewrite would hand a caller
    /// a report whose verdict disagrees with its own matrix.
    ///
    /// `from_gate_outcomes` does not apply this ceiling, so a report built
    /// through that door can be all-green, carry the certification mark and
    /// still contain aspirational passes; what keeps production off that door
    /// is the source scan
    /// `every_door_hands_the_merge_queue_evidence_a_certification_run_produced`,
    /// which is a lint rather than an invariant.
    pub fn withhold_aspirational_passes(&mut self) {
        // Rewritten through `build()` rather than by mutating each field:
        // `build` is the one place the seventy-two fields are written down, and
        // a `named_statuses_mut()` would be a third copy of that list. The cost
        // is that the fields `build` does not set have to be carried across by
        // hand below, which
        // `withholding_carries_across_every_field_that_is_not_a_gate_status`
        // pins against the struct's own source.
        let mut held: std::collections::HashMap<&'static str, GateStatus> = self
            .named_statuses()
            .into_iter()
            .map(|(gate, status)| (gate, status.clone()))
            .collect();

        let mut rebuilt = Self::build(&mut |gate| {
            // `named_statuses` and `build` are two hand-written spellings of one
            // field list. They agree, and several tests pin that they do -- but
            // a disagreement must not panic a live certification run, so it
            // fails closed the way `from_gate_outcomes` does: `Errored` is not
            // acceptable, so the report does not certify and nothing is admitted
            // on a status this could not read back.
            let Some(status) = held.remove(gate) else {
                return GateStatus::Errored(format!(
                    "`{gate}` is a field on this report and `named_statuses()` does not name \
                     it, so its status could not be read back"
                ));
            };
            match crate::fidelity::declared_fidelity(gate) {
                Some(declared)
                    if !declared.may_report_pass()
                        && matches!(status, GateStatus::Passed | GateStatus::AutoUpdated) =>
                {
                    GateStatus::NotMeasured {
                        gate_id: gate.to_string(),
                        reason: format!(
                            "src/fidelity/registry.rs records this gate as {}: no implementation \
                             of the capability it is named for exists, so it has nothing to pass \
                             on and its pass is withheld",
                            declared.label()
                        ),
                    }
                }
                _ => status,
            }
        });

        rebuilt.is_certified_ready = self.is_certified_ready;
        rebuilt.unmeasured_gates = std::mem::take(&mut self.unmeasured_gates);
        rebuilt.summary_markdown = std::mem::take(&mut self.summary_markdown);
        rebuilt.provenance = self.provenance;
        rebuilt.subject = self.subject.take();
        *self = rebuilt;
        // The carries above make the rebuild lossless for every field `build()`
        // does not set. Two of them are derived from the statuses that were just
        // rewritten, so re-derive them rather than publishing the pre-rewrite
        // values: `seal` is idempotent and the evaluator seals again right after.
        self.seal();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateStatus {
    Passed,
    AutoUpdated,
    Warning(String),
    Failed(String),
    /// The gate was configured to run and had a data source, but could not
    /// produce a measurement: the tool was missing, the subprocess failed to
    /// spawn, the call timed out, or the response could not be parsed.
    ///
    /// This is NOT acceptable. Invariant I1: absent evidence is never a pass.
    Errored(String),
    /// The gate has no data source configured, so it makes no claim in either
    /// direction. Acceptable on its own — reporting a failure here would be a
    /// fabricated accusation, the symmetric violation of I1 — but it is
    /// recorded in `PreMergeCertificationReport::unmeasured_gates` and blocks
    /// merge-queue admission separately from `is_certified_ready`.
    NotMeasured {
        gate_id: String,
        reason: String,
    },
}

impl GateStatus {
    pub fn badge(&self) -> &'static str {
        match self {
            GateStatus::Passed => "✅ PASSED",
            GateStatus::AutoUpdated => "✨ AUTO-SYNCED",
            GateStatus::Warning(_) => "⚠️ WARNING",
            GateStatus::Failed(_) => "❌ FAILED",
            GateStatus::Errored(_) => "🛑 ERRORED",
            GateStatus::NotMeasured { .. } => "➖ NOT MEASURED",
        }
    }

    /// Whether this status permits certification.
    ///
    /// `Errored` is deliberately false: a gate that could not measure must not
    /// pass. `NotMeasured` is deliberately true, because an unconfigured gate
    /// has not found a defect — it is gated instead via `unmeasured_gates`.
    pub fn is_acceptable(&self) -> bool {
        match self {
            GateStatus::Passed | GateStatus::AutoUpdated => true,
            GateStatus::Warning(_) => true,
            GateStatus::Failed(_) => false,
            GateStatus::Errored(_) => false,
            GateStatus::NotMeasured { .. } => true,
        }
    }

    /// Whether this gate actually produced a measurement.
    pub fn is_measured(&self) -> bool {
        !matches!(self, GateStatus::NotMeasured { .. })
    }

    /// The gate id, when this status is `NotMeasured`.
    pub fn unmeasured_gate_id(&self) -> Option<&str> {
        match self {
            GateStatus::NotMeasured { gate_id, .. } => Some(gate_id.as_str()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {

    /// A report whose `unmeasured_gates` field is absent used to deserialise to
    /// an empty vector, and an empty vector is what `is_admissible` reads as
    /// "every gate was measured". Absent evidence has to fail here, because
    /// nothing downstream can tell it apart from measured-and-clean.
    #[test]
    fn a_report_missing_unmeasured_gates_does_not_parse() {
        let mut json = serde_json::to_value(sample_report()).expect("serialise");
        json.as_object_mut()
            .expect("object")
            .remove("unmeasured_gates");

        assert!(
            serde_json::from_value::<PreMergeCertificationReport>(json).is_err(),
            "a payload with no unmeasured_gates parsed anyway, so absent evidence \
             arrives looking measured and admissible"
        );
    }
    use super::*;

    pub(super) fn sample_report() -> PreMergeCertificationReport {
        PreMergeCertificationReport {
            is_certified_ready: false,
            doc_parity_status: GateStatus::Passed,
            cedar_status: GateStatus::Passed,
            compliance_status: GateStatus::Passed,
            api_contract_status: GateStatus::Passed,
            cell_isolation_status: GateStatus::Passed,
            supply_chain_status: GateStatus::Passed,
            clean_arch_status: GateStatus::Passed,
            monorepo_status: GateStatus::Passed,
            debt_shrink_status: GateStatus::Passed,
            modularization_status: GateStatus::Passed,
            coverage_status: GateStatus::Passed,
            rust_skills_status: GateStatus::Passed,
            kani_status: GateStatus::Passed,
            slo_status: GateStatus::Passed,
            adr_status: GateStatus::Passed,
            shuffle_status: GateStatus::Passed,
            trace_status: GateStatus::Passed,
            constant_work_status: GateStatus::Passed,
            idempotency_status: GateStatus::Passed,
            finops_status: GateStatus::Passed,
            ghost_migration_status: GateStatus::Passed,
            gitops_promo_status: GateStatus::Passed,
            gitops_drift_status: GateStatus::Passed,
            canary_status: GateStatus::Passed,
            cluster_audit_status: GateStatus::Passed,
            migration_orch_status: GateStatus::Passed,
            ci_wallclock_status: GateStatus::Passed,
            predictive_test_status: GateStatus::Passed,
            compile_profile_status: GateStatus::Passed,
            remote_cache_status: GateStatus::Passed,
            runner_economics_status: GateStatus::Passed,
            sandbox_status: GateStatus::Passed,
            cross_service_status: GateStatus::Passed,
            ephemeral_secret_status: GateStatus::Passed,
            psa_status: GateStatus::Passed,
            shadow_traffic_status: GateStatus::Passed,
            unresolved_review_status: GateStatus::Passed,
            local_probe_status: GateStatus::Passed,
            semantic_abi_status: GateStatus::Passed,
            zero_day_status: GateStatus::Passed,
            formal_verification_status: GateStatus::Passed,
            deadlock_status: GateStatus::Passed,
            review_verdict_status: GateStatus::Passed,
            brand_absence_status: GateStatus::Passed,
            migration_boundary_status: GateStatus::Passed,
            shape_status: GateStatus::Passed,
            automated_canary_status: GateStatus::Passed,
            progressive_ring_status: GateStatus::Passed,
            hermetic_build_status: GateStatus::Passed,
            openvex_status: GateStatus::Passed,
            cosign_status: GateStatus::Passed,
            chaos_injection_status: GateStatus::Passed,
            stacked_diffs_status: GateStatus::Passed,
            microbench_status: GateStatus::Passed,
            jittered_backoff_status: GateStatus::Passed,
            schema_evolution_status: GateStatus::Passed,
            auto_rollback_status: GateStatus::Passed,
            wasm_sandbox_status: GateStatus::Passed,
            consistency_status: GateStatus::Passed,
            flake_quarantine_status: GateStatus::Passed,
            zero_trust_workload_status: GateStatus::Passed,
            carbon_compute_status: GateStatus::Passed,
            replay_harness_status: GateStatus::Passed,
            upgrade_train_status: GateStatus::Passed,
            mutation_status: GateStatus::Passed,
            feature_flag_status: GateStatus::Passed,
            bench_status: GateStatus::Passed,
            attestation_status: GateStatus::Passed,
            security_scan_status: GateStatus::Passed,
            schema_compat_status: GateStatus::Passed,
            performance_concurrency_status: GateStatus::Passed,
            test_suite_status: GateStatus::Passed,
            unmeasured_gates: Vec::new(),
            summary_markdown: String::new(),
            provenance: GateProvenance::default(),
            subject: None,
        }
    }

    #[test]
    fn errored_is_not_acceptable() {
        // I1: a gate that could not measure must never certify.
        assert!(!GateStatus::Errored("agy spawn failed".into()).is_acceptable());
    }

    #[test]
    fn not_measured_is_acceptable_but_not_measured() {
        let s = GateStatus::NotMeasured {
            gate_id: "slo_canary".into(),
            reason: "no Prometheus endpoint configured".into(),
        };
        // Acceptable: an unconfigured gate has not found a defect.
        assert!(s.is_acceptable());
        // But it is tracked, so admission can be blocked separately.
        assert!(!s.is_measured());
        assert_eq!(s.unmeasured_gate_id(), Some("slo_canary"));
    }

    #[test]
    fn measured_statuses_report_as_measured() {
        for s in [
            GateStatus::Passed,
            GateStatus::AutoUpdated,
            GateStatus::Warning("advisory".into()),
            GateStatus::Failed("boom".into()),
            GateStatus::Errored("timeout".into()),
        ] {
            assert!(s.is_measured(), "{:?} should count as measured", s);
        }
    }

    #[test]
    fn unmeasured_gate_blocks_admission_even_when_certified() {
        // The exact I1 hole this guards: every measured gate passes, one gate
        // never measured, is_certified_ready is true -> must NOT be admissible.
        let mut r = sample_report();
        r.is_certified_ready = true;
        r.slo_status = GateStatus::NotMeasured {
            gate_id: "slo_canary".into(),
            reason: "no metrics source configured".into(),
        };
        r.recompute_unmeasured();
        assert_eq!(r.unmeasured_gates, vec!["slo_canary".to_string()]);
        assert!(r.is_certified_ready);
        assert!(!r.is_admissible(), "unmeasured gate must block admission");
    }

    #[test]
    fn fully_measured_and_certified_is_admissible() {
        let mut r = sample_report();
        r.is_certified_ready = true;
        r.recompute_unmeasured();
        assert!(r.unmeasured_gates.is_empty());
        assert!(r.is_admissible());
    }

    #[test]
    fn all_statuses_covers_every_gate_field() {
        // Guards against a new GateStatus field being added to the struct without
        // being added to all_statuses(), which would silently hide it from the
        // unmeasured sweep.
        let src = include_str!("report.rs");
        // Count only within the struct body, so this test's own string literals
        // below cannot inflate the total.
        let start = src
            .find("pub struct PreMergeCertificationReport")
            .expect("struct declaration");
        let body = &src[start..];
        let end = body.find("\n}\n").expect("struct terminator");
        let decl = body[..end].matches(": GateStatus,").count();
        assert_eq!(
            sample_report().all_statuses().len(),
            decl,
            "all_statuses() must list every GateStatus field"
        );
    }

    #[test]
    fn gate_counts_reflect_reality_not_a_constant() {
        let mut r = sample_report();
        // Every gate passing; pinned to the corpus constant, not a literal.
        let (p, f) = r.gate_counts();
        assert_eq!((p, f), (TOTAL_GATES, 0));

        // Three genuinely failing gates must report three, not the old constant 1.
        r.cedar_status = GateStatus::Failed("policy gap".into());
        r.coverage_status = GateStatus::Failed("below threshold".into());
        r.slo_status = GateStatus::Errored("probe timed out".into());
        let (p, f) = r.gate_counts();
        assert_eq!(f, 3, "must count every failing gate, not a hardcoded 1");
        assert_eq!(p, TOTAL_GATES - 3, "the rest still pass");
        assert_eq!(p + f, r.all_statuses().len());
    }

    #[test]
    fn named_statuses_identifies_which_gates_failed() {
        let mut r = sample_report();
        r.cedar_status = GateStatus::Failed("policy gap".into());
        r.slo_status = GateStatus::Errored("probe timed out".into());

        let failing: Vec<&str> = r
            .named_statuses()
            .into_iter()
            .filter(|(_, s)| !matches!(s, GateStatus::Passed | GateStatus::AutoUpdated))
            .map(|(n, _)| n)
            .collect();
        assert_eq!(failing, vec!["cedar_status", "slo_status"]);
    }

    #[test]
    fn named_statuses_and_all_statuses_stay_aligned() {
        let r = sample_report();
        assert_eq!(r.named_statuses().len(), r.all_statuses().len());
    }

    #[test]
    fn existing_semantics_are_unchanged() {
        assert!(GateStatus::Passed.is_acceptable());
        assert!(GateStatus::AutoUpdated.is_acceptable());
        assert!(GateStatus::Warning("w".into()).is_acceptable());
        assert!(!GateStatus::Failed("f".into()).is_acceptable());
    }
}

#[cfg(test)]
mod total_gates_pin {
    use super::*;

    /// `TOTAL_GATES` is published onto pull requests. It must equal what the
    /// report actually carries, or the next corpus change turns every one of
    /// those strings into a false claim -- which is exactly how "70 gates"
    /// survived in seven places against a corpus of 68.
    #[test]
    fn all_statuses_matches_the_declared_total() {
        assert_eq!(
            super::tests::sample_report().all_statuses().len(),
            TOTAL_GATES,
            "the gate corpus changed but TOTAL_GATES did not; every PR-visible count \
             claim is now wrong"
        );
    }
}

#[cfg(test)]
mod seal_tests {
    use super::GateStatus;
    use super::tests::sample_report;

    #[test]
    fn seal_derives_the_verdict_from_every_gate_including_the_self_directed_ones() {
        // The defect: brand_absence_status and migration_boundary_status were
        // computed after the certification conjunction, so they never blocked.
        let mut r = sample_report();
        r.brand_absence_status = GateStatus::Failed("stamp".into());
        r.is_certified_ready = true;
        r.seal();
        assert!(
            !r.is_certified_ready,
            "a failing brand_absence_status must uncertify"
        );

        let mut r = sample_report();
        r.migration_boundary_status = GateStatus::Failed("edge".into());
        r.seal();
        assert!(
            !r.is_certified_ready,
            "a failing migration_boundary_status must uncertify"
        );
    }

    #[test]
    fn seal_certifies_an_all_passing_report_and_withholds_an_unmeasured_one() {
        let mut r = sample_report();
        r.is_certified_ready = false;
        r.seal();
        assert!(r.is_certified_ready);
        assert!(r.is_admissible());

        let mut r = sample_report();
        r.slo_status = GateStatus::NotMeasured {
            gate_id: "slo_status".into(),
            reason: "no endpoint".into(),
        };
        r.seal();
        assert!(
            r.is_certified_ready,
            "NotMeasured is individually acceptable"
        );
        assert!(!r.is_admissible(), "but it withholds admission (I1)");
        assert_eq!(r.unmeasured_gates, vec!["slo_status".to_string()]);
    }

    #[test]
    fn seal_overrides_a_stale_precomputed_verdict() {
        let mut r = sample_report();
        r.test_suite_status = GateStatus::Errored("did not run".into());
        r.is_certified_ready = true; // a caller's stale opinion
        r.seal();
        assert!(!r.is_certified_ready);
    }
}

#[cfg(test)]
mod review_verdict_is_binding {
    use super::*;

    /// A blocking review verdict must prevent certification.
    ///
    /// It did not. `review_verdict_status` was computed in the evaluator and
    /// never became a field, so `all_statuses()` could not see it and `seal()`
    /// could not gate on it. A pull request whose 16-lens review returned
    /// REQUEST_CHANGES or REJECT was certified anyway.
    ///
    /// The original chain ended `&& review_verdict_status.is_acceptable()`.
    /// When certification moved into `seal()`, the value was left behind. No
    /// test failed, because no test asserted the gate was reachable -- it simply
    /// stopped mattering. An unused-variable lint found it; no review did.
    #[test]
    fn a_blocking_review_verdict_prevents_certification() {
        let mut r = tests::sample_report();
        r.seal();
        assert!(r.is_certified_ready, "the clean fixture must certify");

        r.review_verdict_status =
            GateStatus::Failed("16-Lens Matrix issued blocking verdict: REJECT".to_string());
        r.seal();
        assert!(
            !r.is_certified_ready,
            "a REJECT review certified the pull request anyway; the review gate is not wired"
        );
    }

    /// An unobtained review is Errored, not Failed -- the model did not judge
    /// the code adversely, the review did not happen -- and both must block.
    #[test]
    fn a_review_that_never_completed_also_blocks() {
        let mut r = tests::sample_report();
        r.review_verdict_status = GateStatus::Errored("no parseable verdict".to_string());
        r.seal();
        assert!(!r.is_certified_ready, "absent evidence must not certify");
    }
}
