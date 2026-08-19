use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentDisposition {
    Move,     // Pure 1-to-1 path bijection
    Refactor, // Decompose into core / ports / adapters
    Rewrite,  // Replace obsolete paradigm with live code/proto AST
    Retire,   // Safe dead-code deletion (0 inbound callers)
    Evaluate, // Marginal utility / needs human product decision
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentEvaluationReport {
    pub target: String,
    pub inbound_dependents: usize,
    pub is_clean_architecture: bool,
    pub max_file_lines: usize,
    pub disposition: ComponentDisposition,
    pub rationale: String,
    pub recommended_action: String,
}

pub struct ComponentDispositionClassifier;

impl ComponentDispositionClassifier {
    /// Evaluates a component/crate/module to determine its optimal reorganization disposition
    pub fn evaluate_component(
        repo_dir: &Path,
        target_rel_path: &str,
        inbound_dependents: usize,
    ) -> ComponentEvaluationReport {
        let full_path = repo_dir.join(target_rel_path);

        // Check if path exists
        if !full_path.exists() {
            return ComponentEvaluationReport {
                target: target_rel_path.to_string(),
                inbound_dependents: 0,
                is_clean_architecture: false,
                max_file_lines: 0,
                disposition: ComponentDisposition::Retire,
                rationale: "Target path does not exist in repository.".to_string(),
                recommended_action: "No action required.".to_string(),
            };
        }

        // Rule 1: Zero inbound dependents -> Candidate for RETIRE
        if inbound_dependents == 0
            && !target_rel_path.starts_with("app/")
            && !target_rel_path.starts_with("apps/")
        {
            return ComponentEvaluationReport {
                target: target_rel_path.to_string(),
                inbound_dependents: 0,
                is_clean_architecture: false,
                max_file_lines: 0,
                disposition: ComponentDisposition::Retire,
                rationale:
                    "Component has 0 inbound workspace callers and is not a top-level application."
                        .to_string(),
                recommended_action: format!(
                    "Safe retirement via `git rm -r {}` with tombstone entry.",
                    target_rel_path
                ),
            };
        }

        // Rule 2: Check for obsolete YAML catalog sprawl -> Candidate for REWRITE
        if target_rel_path.contains("catalog") && target_rel_path.ends_with(".yaml") {
            return ComponentEvaluationReport {
                target: target_rel_path.to_string(),
                inbound_dependents,
                is_clean_architecture: false,
                max_file_lines: 0,
                disposition: ComponentDisposition::Rewrite,
                rationale: "Hand-edited YAML catalog detected. Hyperscaler pattern mandates live Rust AST / Protobuf reflection.".to_string(),
                recommended_action: "Deprecate YAML catalog; generate runtime metadata directly from crate structs.".to_string(),
            };
        }

        // Rule 3: Check Clean Architecture & Line Counts
        let mut max_lines = 0;
        let mut has_mixed_io = false;

        if full_path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&full_path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() && (p.extension().and_then(|s| s.to_str()) == Some("rs")) {
                        if let Ok(content) = std::fs::read_to_string(&p) {
                            let lines = content.lines().count();
                            max_lines = max_lines.max(lines);
                            if content.contains("sqlx::")
                                || content.contains("reqwest::")
                                || content.contains("tokio::net")
                            {
                                has_mixed_io = true;
                            }
                        }
                    }
                }
            }
        }

        let is_clean_architecture = !has_mixed_io && max_lines <= 300;

        if is_clean_architecture {
            ComponentEvaluationReport {
                target: target_rel_path.to_string(),
                inbound_dependents,
                is_clean_architecture: true,
                max_file_lines: max_lines,
                disposition: ComponentDisposition::Move,
                rationale: "Component conforms to Clean Architecture and line length limits (<= 300 lines).".to_string(),
                recommended_action: "Execute pure 1-to-1 path bijection move to canonical capability folder.".to_string(),
            }
        } else if max_lines > 600 || (has_mixed_io && max_lines > 300) {
            ComponentEvaluationReport {
                target: target_rel_path.to_string(),
                inbound_dependents,
                is_clean_architecture: false,
                max_file_lines: max_lines,
                disposition: ComponentDisposition::Refactor,
                rationale: format!("Component mixes I/O with domain logic or exceeds line ceilings (max {} lines).", max_lines),
                recommended_action: "Decompose into `<capability>/core/` (pure logic), `<capability>/ports/` (traits), and `<capability>/adapters/` (I/O).".to_string(),
            }
        } else {
            ComponentEvaluationReport {
                target: target_rel_path.to_string(),
                inbound_dependents,
                is_clean_architecture: false,
                max_file_lines: max_lines,
                disposition: ComponentDisposition::Evaluate,
                rationale: "Component has moderate complexity. Requires architectural review before migration.".to_string(),
                recommended_action: "Review component ROI and decide between Refactor vs. Retire.".to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_zero_inbound_classified_as_retire() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("crates/orphan-tool");
        std::fs::create_dir_all(&target).unwrap();

        let report =
            ComponentDispositionClassifier::evaluate_component(dir.path(), "crates/orphan-tool", 0);
        assert_eq!(report.disposition, ComponentDisposition::Retire);
    }

    #[test]
    fn test_clean_arch_classified_as_move() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("oya/billing/crates/oya-billing-domain/src");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(
            target.join("lib.rs"),
            "pub struct Invoice { pub id: u64 }\n",
        )
        .unwrap();

        let report = ComponentDispositionClassifier::evaluate_component(
            dir.path(),
            "oya/billing/crates/oya-billing-domain/src",
            5,
        );
        assert_eq!(report.disposition, ComponentDisposition::Move);
    }
}
