//! Rendering the gate matrix a pull request sees.
//!
//! The table it renders is `gate_labels::GATE_LABELS`, in its own file: a table
//! is a list and a renderer is logic, and only the renderer changes when the
//! presentation changes. Both grew past ADR-0719 D-35's budget together while
//! the corpus grew.

pub use super::gate_labels::{GATE_LABELS, label_for};

use super::PreMergeCertificationReport;

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
