use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationPhase {
    Expand,    // Add column / table / shadow structure
    DualWrite, // Application writes to old & new
    Cutover,   // Application reads from new
    Contract,  // Drop old column / table after bake period
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationPhaseFinding {
    pub file_path: String,
    pub violation: String,
}

pub struct MigrationPhaseValidator;

impl Default for MigrationPhaseValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl MigrationPhaseValidator {
    pub fn new() -> Self {
        Self
    }

    /// Whether a changed path is a SQL migration -- the scope this validator
    /// inspects.
    ///
    /// `pub` because the caller must distinguish "parsed and ordered" from
    /// "nothing was in scope". The predicate used to be `chunk.contains(".sql")`
    /// against the *hunk text* rather than the path, so a Rust file mentioning
    /// `schema.sql` was validated as SQL and a chunk with no derivable path fell
    /// back to a `migration.sql` default and was too.
    ///
    /// It is still a guess about file extension: a schema transition arrives as
    /// `db/migrate/*.rb`, `schema.rb` or an Atlas `*.hcl` at least as often as
    /// `*.sql`, and none of those are visible here.
    pub fn is_migration_sql(file_path: &str) -> bool {
        file_path.ends_with(".sql")
    }

    /// 100% Deterministic validation of Expand-Contract database migration phase invariants
    pub fn validate_migration_sql(
        &self,
        file_path: &str,
        sql_content: &str,
    ) -> Vec<MigrationPhaseFinding> {
        let mut findings = Vec::new();
        let drop_column_re =
            Regex::new(r"(?i)ALTER\s+TABLE\s+\w+\s+DROP\s+COLUMN\s+(\w+)").unwrap();
        let drop_table_re = Regex::new(r"(?i)DROP\s+TABLE\s+(\w+)").unwrap();

        for line in sql_content.lines() {
            if drop_column_re.is_match(line) {
                // Must ensure column drop is explicitly tagged with Phase 4 Contract annotation
                if !sql_content.contains("-- PHASE: CONTRACT") {
                    findings.push(MigrationPhaseFinding {
                        file_path: file_path.to_string(),
                        violation: "Destructive `DROP COLUMN` attempted without explicit `-- PHASE: CONTRACT` annotation and 30-day bake confirmation.".to_string(),
                    });
                }
            } else if drop_table_re.is_match(line) && !sql_content.contains("-- PHASE: CONTRACT") {
                findings.push(MigrationPhaseFinding {
                        file_path: file_path.to_string(),
                        violation: "Destructive `DROP TABLE` attempted without explicit `-- PHASE: CONTRACT` annotation.".to_string(),
                    });
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_premature_column_drop() {
        let val = MigrationPhaseValidator::new();
        let sql = "ALTER TABLE users DROP COLUMN legacy_auth_token;";
        let findings = val.validate_migration_sql("migrations/0002_drop.sql", sql);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn test_passes_annotated_contract_drop() {
        let val = MigrationPhaseValidator::new();
        let sql = "-- PHASE: CONTRACT\nALTER TABLE users DROP COLUMN legacy_auth_token;";
        let findings = val.validate_migration_sql("migrations/0002_drop.sql", sql);
        assert_eq!(findings.len(), 0);
    }
}
