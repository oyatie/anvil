use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstMutation {
    pub file_path: String,
    pub original_line: String,
    pub mutated_line: String,
    pub mutation_type: String,
}

pub struct AstMutatorEngine;

impl AstMutatorEngine {
    pub fn new() -> Self {
        Self
    }

    /// Generates potential AST mutations for added/modified lines in Rust code
    pub fn generate_mutations(&self, file_path: &str, content: &str) -> Vec<AstMutation> {
        let mut mutations = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.is_empty() {
                continue;
            }

            // 1. Boolean Operator Mutations
            if trimmed.contains("==") {
                mutations.push(AstMutation {
                    file_path: file_path.to_string(),
                    original_line: trimmed.to_string(),
                    mutated_line: trimmed.replace("==", "!="),
                    mutation_type: "InvertEquality (== -> !=)".to_string(),
                });
            } else if trimmed.contains("!=") {
                mutations.push(AstMutation {
                    file_path: file_path.to_string(),
                    original_line: trimmed.to_string(),
                    mutated_line: trimmed.replace("!=", "=="),
                    mutation_type: "InvertInequality (!= -> ==)".to_string(),
                });
            }

            // 2. Boundary Comparison Mutations
            if trimmed.contains(" <= ") {
                mutations.push(AstMutation {
                    file_path: file_path.to_string(),
                    original_line: trimmed.to_string(),
                    mutated_line: trimmed.replace(" <= ", " < "),
                    mutation_type: "BoundaryShrink (<= -> <)".to_string(),
                });
            } else if trimmed.contains(" < ")
                && !trimmed.contains("<T>")
                && !trimmed.contains(" < ")
            {
                mutations.push(AstMutation {
                    file_path: file_path.to_string(),
                    original_line: trimmed.to_string(),
                    mutated_line: trimmed.replace(" < ", " <= "),
                    mutation_type: "BoundaryExpand (< -> <=)".to_string(),
                });
            }

            // 3. Boolean Literals
            if trimmed.contains("true") && !trimmed.contains("//") {
                mutations.push(AstMutation {
                    file_path: file_path.to_string(),
                    original_line: trimmed.to_string(),
                    mutated_line: trimmed.replace("true", "false"),
                    mutation_type: "InvertBoolLiteral (true -> false)".to_string(),
                });
            }
        }

        mutations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_mutator_generates_mutations() {
        let engine = AstMutatorEngine::new();
        let code = r#"
pub fn check_bound(val: usize) -> bool {
    if val <= 100 {
        true
    } else {
        false
    }
}
"#;
        let muts = engine.generate_mutations("src/bound.rs", code);
        assert!(!muts.is_empty());
        assert!(muts
            .iter()
            .any(|m| m.mutation_type.contains("BoundaryShrink")));
        assert!(muts
            .iter()
            .any(|m| m.mutation_type.contains("InvertBoolLiteral")));
    }
}
