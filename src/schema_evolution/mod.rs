pub mod compatibility_checker;

use compatibility_checker::{CompatibilityChecker, classify};

use crate::pre_merge_guard::report::GateStatus;

const GATE_ID: &str = "schema_evolution_status";

const NO_SCHEMA_IN_SCOPE: &str = "no changed file carries a wire schema (no `.proto`, no OpenAPI \
     description), so no schema was compared; an empty scope is not a compatible schema";

/// How many findings the published summary names before it stops listing.
const FINDINGS_LISTED: usize = 3;

#[derive(Clone, Debug)]
pub struct SchemaEvolutionReport {
    pub status: GateStatus,
    /// How many wire breaks were found. Read by `examples/schema_repro`, which
    /// is what makes the false-positive rate measurable against real history
    /// rather than asserted. Whether the gate passed is `status`, and only
    /// `status` -- a second boolean saying the same thing is the shape the
    /// evaluator guard forbids.
    pub breaking_field_changes: usize,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct SchemaEvolutionRatchet {
    checker: CompatibilityChecker,
}

impl Default for SchemaEvolutionRatchet {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaEvolutionRatchet {
    pub fn new() -> Self {
        Self {
            checker: CompatibilityChecker::new(),
        }
    }

    /// Compares the wire schemas this pull request touches, one file at a time.
    ///
    /// The per-file split is the point: the scope decision is made from the
    /// `diff --git` header of each section, so a pull request that changes a
    /// `.proto` and a `.rs` has the `.proto` scanned and the `.rs` left alone.
    /// Deciding once for the whole diff would put every line of every file into
    /// whichever scope won, which is how a removed `to_string()` came to be
    /// published as a breaking wire schema change.
    pub fn evaluate_schema_evolution(&self, diff_content: &str) -> SchemaEvolutionReport {
        let mut violations: Vec<String> = Vec::new();
        let mut scanned_a_schema = false;

        for file_diff in diff_content.split("diff --git") {
            let Some(path) = file_diff
                .lines()
                .next()
                .and_then(|header| header.split_whitespace().last())
                .map(|path| path.trim_start_matches("b/"))
            else {
                continue;
            };
            if classify(path).is_none() {
                continue;
            }
            // A file with no previous revision has no baseline to be
            // incompatible with. Reporting `required string tenant_id = 1;` in a
            // brand-new `.proto` as MESSAGE_SAME_REQUIRED_FIELDS is an
            // accusation against nothing -- the same defect this gate is here to
            // remove, narrowed to one file type.
            if file_diff.lines().any(|l| l.starts_with("new file mode ")) {
                continue;
            }
            scanned_a_schema = true;
            violations.extend(self.checker.check_file_diff(path, file_diff));
        }

        // Nothing in scope is not the same as nothing wrong. This repository
        // contains zero `.proto` files, so on almost every pull request the
        // honest report is that no schema was compared -- not that every schema
        // is compatible, which is what a pass here used to claim.
        if !scanned_a_schema {
            return SchemaEvolutionReport {
                status: GateStatus::NotMeasured {
                    gate_id: GATE_ID.to_string(),
                    reason: NO_SCHEMA_IN_SCOPE.to_string(),
                },
                breaking_field_changes: 0,
                summary: NO_SCHEMA_IN_SCOPE.to_string(),
            };
        }

        let passed = violations.is_empty();
        let summary = if passed {
            "Every touched wire schema keeps its field numbers reserved and its published \
             endpoints served."
                .to_string()
        } else {
            let mut listed: Vec<String> =
                violations.iter().take(FINDINGS_LISTED).cloned().collect();
            if violations.len() > FINDINGS_LISTED {
                listed.push(format!("and {} more", violations.len() - FINDINGS_LISTED));
            }
            format!(
                "{} breaking wire schema change(s): {}",
                violations.len(),
                listed.join("; ")
            )
        };

        SchemaEvolutionReport {
            status: if passed {
                GateStatus::Passed
            } else {
                GateStatus::Failed(summary.clone())
            },
            breaking_field_changes: violations.len(),
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_evolution_nominal() {
        let ratchet = SchemaEvolutionRatchet::new();

        // This asserted `report.passed` for `"+ optional string new_field = 4;"`
        // with no `diff --git` header at all, so it certified a pass over a
        // corpus with no file in it. Out of scope is now unmeasured.
        let report = ratchet.evaluate_schema_evolution("+ optional string new_field = 4;");
        assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));

        let in_scope = ratchet.evaluate_schema_evolution(
            "diff --git a/proto/order.proto b/proto/order.proto\n+ optional string new_field = 4;",
        );
        assert_eq!(in_scope.status, GateStatus::Passed, "{}", in_scope.summary);
    }
}
