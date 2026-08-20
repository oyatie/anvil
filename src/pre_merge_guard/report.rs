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
    #[serde(default)]
    pub unmeasured_gates: Vec<String>,
    pub summary_markdown: String,
}

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

    /// Whether this PR may be admitted to the merge queue.
    ///
    /// Deliberately stricter than `is_certified_ready`: a report may certify on
    /// every measured gate while some gate produced no measurement at all.
    /// Invariant I1 — absent evidence must not merge.
    pub fn is_admissible(&self) -> bool {
        self.is_certified_ready && self.unmeasured_gates.is_empty()
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
    use super::*;

    fn sample_report() -> PreMergeCertificationReport {
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
        // All 68 gates passing.
        let (p, f) = r.gate_counts();
        assert_eq!((p, f), (68, 0));

        // Three genuinely failing gates must report three, not the old constant 1.
        r.cedar_status = GateStatus::Failed("policy gap".into());
        r.coverage_status = GateStatus::Failed("below threshold".into());
        r.slo_status = GateStatus::Errored("probe timed out".into());
        let (p, f) = r.gate_counts();
        assert_eq!(f, 3, "must count every failing gate, not a hardcoded 1");
        assert_eq!(p, 65);
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
