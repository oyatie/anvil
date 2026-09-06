//! Inert diff decisions: no repository, provider, or executable fixture.

use super::analyze_with_inputs;
use crate::clean_architecture_guard::CleanArchitectureReport;
use crate::pre_merge_guard::GateStatus;
use crate::source_scan::paths::RootRelation;

fn report(path: &str, added: &str) -> CleanArchitectureReport {
    analyze_with_inputs(
        &format!("+++ b/{path}\n{added}\n"),
        "in-memory architecture subject".into(),
        |_| Ok(false),
        |_, root| match root {
            "crate" | "left" => RootRelation::SameCrate,
            "right" => RootRelation::OtherCrate,
            "unresolved" => RootRelation::Unknown,
            _ => RootRelation::Foreign,
        },
    )
    .expect("inert classification succeeds")
}

#[test]
fn layered_typescript_import_is_actually_checked() {
    let report = report(
        "src/alpha/core/service.ts",
        "+import { X } from '../adapters/x';",
    );
    assert!(
        !report.violations.is_empty(),
        "forbidden TS edge was skipped"
    );
    assert_eq!(report.measurement.files_classified(), 1);
    assert!(!report.is_clean);
    assert!(matches!(report.gate_status(), GateStatus::Failed(_)));
}

#[test]
fn equal_unit_names_do_not_exempt_another_crate() {
    let report = report(
        "crates/left/src/shared/mod.rs",
        "+use right::shared::core::X;",
    );
    assert_eq!(report.violations.len(), 1, "another crate owns this face");
    assert!(!report.is_clean);
    assert!(matches!(report.gate_status(), GateStatus::Failed(_)));
}

#[test]
fn a_layered_prose_path_is_not_a_measured_source_file() {
    let report = report(
        "src/alpha/core/notes.md",
        "+This describes crate::beta::adapters::X, not a dependency.",
    );
    assert!(report.violations.is_empty());
    assert_eq!(report.measurement.files_classified(), 0);
    assert!(!report.is_clean);
    assert!(matches!(
        report.gate_status(),
        GateStatus::NotMeasured { .. }
    ));
}

#[test]
fn unchanged_rust_and_absence_controls() {
    let own = report("src/shared/mod.rs", "+use crate::shared::core::X;");
    assert!(own.violations.is_empty());
    assert!(own.is_clean);
    assert!(matches!(own.gate_status(), GateStatus::Passed));

    let other = report("src/alpha/mod.rs", "+use crate::beta::core::X;");
    assert_eq!(other.violations.len(), 1);
    assert!(matches!(other.gate_status(), GateStatus::Failed(_)));

    let absent = report("README.md", "+ordinary prose");
    assert!(absent.violations.is_empty());
    assert!(!absent.is_clean);
    assert!(matches!(
        absent.gate_status(),
        GateStatus::NotMeasured { .. }
    ));
}

#[test]
fn classification_error_propagates_without_a_report() {
    let result = analyze_with_inputs(
        "+++ b/src/alpha/core/service.rs\n+use crate::beta::core::X;\n",
        "in-memory classification failure".into(),
        |_| Err(anyhow::anyhow!("synthetic classification unavailable")),
        |_, _| RootRelation::SameCrate,
    );
    assert!(result.is_err());
}

#[test]
fn supported_import_languages_retain_layer_direction() {
    for extension in ["ts", "tsx", "js", "jsx", "mjs", "cjs"] {
        let bad = report(
            &format!("src/alpha/ports/service.{extension}"),
            "+import { X } from '../adapters/x';",
        );
        assert!(matches!(bad.gate_status(), GateStatus::Failed(_)));
        let inward = report(
            &format!("src/alpha/adapters/service.{extension}"),
            "+import { X } from '../core/x';",
        );
        assert!(matches!(inward.gate_status(), GateStatus::Passed));
        assert_eq!(inward.measurement.files_classified(), 1);
    }
    let ordinary = report(
        "src/alpha/core/service.ts",
        "+const value = 1;\n+const other = 2;",
    );
    assert_eq!(ordinary.measurement.files_classified(), 1);
    assert!(matches!(ordinary.gate_status(), GateStatus::Passed));
}

#[test]
fn only_actual_added_production_subjects_count() {
    for (path, text) in [
        ("src/alpha/core/service.ts", ""),
        ("src/alpha/core/service.rs", "+// comment only"),
        ("src/alpha/core/service.rs", "-use crate::beta::core::X;"),
        (
            "src/alpha/service.ts",
            "+const prose = 'crate::beta::core::X';",
        ),
    ] {
        let value = report(path, text);
        assert!(value.violations.is_empty());
        assert!(matches!(
            value.gate_status(),
            GateStatus::NotMeasured { .. }
        ));
    }
    let excluded = analyze_with_inputs(
        "+++ b/src/alpha/core/service.ts\n+import { X } from '../adapters/x';\n",
        "excluded test".into(),
        |_| Ok(true),
        |_, _| RootRelation::OtherCrate,
    )
    .unwrap();
    assert_eq!(excluded.measurement.files_classified(), 0);
    assert!(matches!(
        excluded.gate_status(),
        GateStatus::NotMeasured { .. }
    ));
}

#[test]
fn named_self_foreign_and_unknown_roots_remain_distinct() {
    let own = report("src/shared/mod.rs", "+use left::shared::core::X;");
    assert!(own.violations.is_empty());
    assert!(matches!(own.gate_status(), GateStatus::Passed));
    assert!(own.summary.contains("1 face reference(s)"));
    let foreign = report("src/shared/mod.rs", "+use third_party::shared::core::X;");
    assert!(foreign.violations.is_empty());
    assert!(matches!(
        foreign.gate_status(),
        GateStatus::NotMeasured { .. }
    ));
    let unknown = report("src/shared/mod.rs", "+use unresolved::shared::core::X;");
    assert!(unknown.violations.is_empty());
    assert!(matches!(unknown.gate_status(), GateStatus::Errored(_)));
    let mixed = report(
        "src/shared/mod.rs",
        "+use unresolved::shared::core::X;\n+use right::shared::core::X;",
    );
    assert_eq!(mixed.violations.len(), 1);
    assert!(!mixed.is_clean);
    assert!(matches!(mixed.gate_status(), GateStatus::Failed(_)));
}

#[test]
fn rust_grouped_multiline_and_expression_edges_survive() {
    let grouped = report(
        "src/alpha/mod.rs",
        "+use crate::beta::{\n+core::X,\n+ports::Y\n+};",
    );
    assert_eq!(grouped.violations.len(), 2);
    let expression = report("src/alpha/mod.rs", "+let x = crate::beta::adapters::X;");
    assert_eq!(expression.violations.len(), 1);
    let text = report(
        "src/alpha/mod.rs",
        "+// crate::beta::core::X\n+let text = \"crate::beta::ports::X\";",
    );
    assert!(text.violations.is_empty());
    assert!(matches!(text.gate_status(), GateStatus::NotMeasured { .. }));
}

// Fixture outcomes exercise the actual seal/admission boundary, not PR identity
// or production acquisition. Every other gate is deliberately Passed.
fn certification(
    report: &CleanArchitectureReport,
) -> crate::pre_merge_guard::PreMergeCertificationReport {
    use crate::pre_merge_guard::PreMergeCertificationReport;
    let corpus = PreMergeCertificationReport::unmeasured("fixture corpus");
    let outcomes: Vec<_> = corpus
        .named_statuses()
        .into_iter()
        .map(|(name, _)| {
            (
                name,
                if name == "clean_arch_status" {
                    report.gate_status()
                } else {
                    GateStatus::Passed
                },
            )
        })
        .collect();
    PreMergeCertificationReport::from_gate_outcomes(&outcomes).expect("complete fixture outcomes")
}

#[test]
fn unknown_only_architecture_evidence_refuses_actual_admission() {
    let unknown = report("src/shared/mod.rs", "+use unresolved::shared::core::X;");
    assert!(unknown.violations.is_empty());
    let refusal = certification(&unknown)
        .admission_refusal()
        .expect_err("unresolved ownership must withhold admission");
    assert!(refusal.to_string().contains("clean_arch_status"));
    assert!(matches!(unknown.gate_status(), GateStatus::Errored(_)));
}

#[test]
fn architecture_admission_retains_violation_clean_and_empty_controls() {
    let mixed = report(
        "src/shared/mod.rs",
        "+use unresolved::shared::core::X;\n+use right::shared::core::X;",
    );
    assert_eq!(mixed.violations.len(), 1);
    assert!(matches!(mixed.gate_status(), GateStatus::Failed(_)));
    assert!(certification(&mixed).admission_refusal().is_err());

    let clean = report("src/shared/mod.rs", "+use left::shared::core::X;");
    assert!(matches!(clean.gate_status(), GateStatus::Passed));
    assert!(certification(&clean).admission_refusal().is_ok());

    let empty = report("README.md", "+ordinary prose");
    assert!(!empty.is_clean);
    assert!(matches!(
        empty.gate_status(),
        GateStatus::NotMeasured { .. }
    ));
    assert!(certification(&empty).admission_refusal().is_ok());
}

#[test]
fn missing_measurement_cannot_inherit_the_empty_subject_exemption() {
    let missing: CleanArchitectureReport = serde_json::from_str(
        r#"{"is_clean":false,"violations":[],"summary":"legacy missing record"}"#,
    )
    .unwrap();
    assert!(!missing.measurement.is_measured());
    assert!(certification(&missing).admission_refusal().is_err());
    assert!(matches!(missing.gate_status(), GateStatus::Errored(_)));
}

#[test]
fn typed_unavailable_round_trips_without_reinterpreting_legacy_absence() {
    use crate::clean_architecture_guard::ArchMeasurement;
    let unavailable = ArchMeasurement::Unavailable {
        reason: "synthetic gap".into(),
        files_inspected: 2,
    };
    let encoded = serde_json::to_string(&unavailable).unwrap();
    let decoded: ArchMeasurement = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, unavailable);
    assert!(!decoded.is_measured());
    assert_eq!(decoded.files_inspected(), 2);
    assert_eq!(decoded.files_classified(), 0);
    assert_eq!(decoded.not_measured_reason(), Some("synthetic gap"));
    assert!(matches!(
        ArchMeasurement::default(),
        ArchMeasurement::Unavailable { .. }
    ));
    let legacy: ArchMeasurement = serde_json::from_str(
        r#"{"NotMeasured":{"reason":"target ownership unresolved","files_inspected":1}}"#,
    )
    .unwrap();
    assert!(matches!(legacy, ArchMeasurement::NotMeasured { .. }));
    let mut value = report("README.md", "+ordinary prose");
    value.measurement = legacy;
    assert!(matches!(
        value.gate_status(),
        GateStatus::NotMeasured { .. }
    ));
}
