use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProgrammingLanguage {
    Rust,
    TypeScript,
    Python,
    Go,
    Protobuf,
    Sql,
    ManifestYaml,
    Documentation,
    Unknown,
}

impl ProgrammingLanguage {
    pub fn from_file_path(path: &str) -> Self {
        let p = Path::new(path);
        match p.extension().and_then(|ext| ext.to_str()) {
            Some("rs") => ProgrammingLanguage::Rust,
            Some("ts") | Some("tsx") | Some("js") | Some("jsx") => ProgrammingLanguage::TypeScript,
            Some("py") => ProgrammingLanguage::Python,
            Some("go") => ProgrammingLanguage::Go,
            Some("proto") => ProgrammingLanguage::Protobuf,
            Some("sql") => ProgrammingLanguage::Sql,
            Some("yaml") | Some("yml") | Some("toml") | Some("json") => {
                ProgrammingLanguage::ManifestYaml
            }
            Some("md") | Some("markdown") | Some("txt") => ProgrammingLanguage::Documentation,
            _ => ProgrammingLanguage::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskCategory {
    ArchitectureRefactor,
    BugfixInvestigation,
    NewFeatureSynthesis,
    SecurityAudit,
    ContractMigration,
    PerformanceOptimization,
    DocSweeping,
    TestGeneration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TaskComplexity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GranularTaskContext {
    pub primary_language: ProgrammingLanguage,
    pub secondary_languages: Vec<ProgrammingLanguage>,
    pub category: TaskCategory,
    pub complexity: TaskComplexity,
    pub files_count: usize,
    pub lines_changed: usize,
    pub touched_security_sensitive_paths: bool,
}

pub struct GranularTaskClassifier;

impl GranularTaskClassifier {
    /// Classifies a PR diff or task description into language, category, and complexity
    pub fn classify_task(
        changed_files: &[String],
        diff_content: &str,
        task_description: &str,
    ) -> GranularTaskContext {
        let mut languages = Vec::new();
        let mut security_sensitive = false;

        for file in changed_files {
            let lang = ProgrammingLanguage::from_file_path(file);
            if lang != ProgrammingLanguage::Unknown && !languages.contains(&lang) {
                languages.push(lang);
            }

            let lower = file.to_lowercase();
            if lower.contains("iam")
                || lower.contains("cedar")
                || lower.contains("auth")
                || lower.contains("secret")
                || lower.contains("security")
                || lower.contains("crypto")
                || lower.contains("migration")
            {
                security_sensitive = true;
            }
        }

        let primary_language = languages
            .first()
            .copied()
            .unwrap_or(ProgrammingLanguage::Rust);
        let secondary_languages = if languages.len() > 1 {
            languages[1..].to_vec()
        } else {
            Vec::new()
        };

        let lines_changed = diff_content.lines().count();
        let files_count = changed_files.len();

        let desc_lower = task_description.to_lowercase();
        let category = if desc_lower.contains("security")
            || desc_lower.contains("vulnerability")
            || desc_lower.contains("cve")
            || desc_lower.contains("audit")
            || security_sensitive
        {
            TaskCategory::SecurityAudit
        } else if desc_lower.contains("refactor")
            || desc_lower.contains("reorg")
            || desc_lower.contains("modular")
        {
            TaskCategory::ArchitectureRefactor
        } else if desc_lower.contains("proto")
            || desc_lower.contains("schema")
            || desc_lower.contains("migration")
        {
            TaskCategory::ContractMigration
        } else if desc_lower.contains("perf")
            || desc_lower.contains("latency")
            || desc_lower.contains("bench")
        {
            TaskCategory::PerformanceOptimization
        } else if desc_lower.contains("doc")
            || desc_lower.contains("adr")
            || desc_lower.contains("readme")
        {
            TaskCategory::DocSweeping
        } else if desc_lower.contains("test")
            || desc_lower.contains("fuzz")
            || desc_lower.contains("kani")
        {
            TaskCategory::TestGeneration
        } else if desc_lower.contains("fix")
            || desc_lower.contains("bug")
            || desc_lower.contains("flaky")
        {
            TaskCategory::BugfixInvestigation
        } else {
            TaskCategory::NewFeatureSynthesis
        };

        let complexity = if security_sensitive || files_count > 20 || lines_changed > 1000 {
            TaskComplexity::Critical
        } else if files_count > 8 || lines_changed > 400 {
            TaskComplexity::High
        } else if files_count > 2 || lines_changed > 100 {
            TaskComplexity::Medium
        } else {
            TaskComplexity::Low
        };

        GranularTaskContext {
            primary_language,
            secondary_languages,
            category,
            complexity,
            files_count,
            lines_changed,
            touched_security_sensitive_paths: security_sensitive,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_classifier_identifies_rust_security_audit() {
        let files = vec![
            "src/cedar_guard.rs".to_string(),
            "iam/core/pdp.rs".to_string(),
        ];
        let diff = "+ let policy = parse_cedar();\n+ evaluate();";
        let desc = "audit cedar authorization policy";

        let ctx = GranularTaskClassifier::classify_task(&files, diff, desc);
        assert_eq!(ctx.primary_language, ProgrammingLanguage::Rust);
        assert_eq!(ctx.category, TaskCategory::SecurityAudit);
        assert_eq!(ctx.complexity, TaskComplexity::Critical);
        assert!(ctx.touched_security_sensitive_paths);
    }

    #[test]
    fn test_task_classifier_identifies_docs_sweep() {
        let files = vec!["docs/architecture/overview.md".to_string()];
        let diff = "+ # System Overview";
        let desc = "update documentation and adrs";

        let ctx = GranularTaskClassifier::classify_task(&files, diff, desc);
        assert_eq!(ctx.primary_language, ProgrammingLanguage::Documentation);
        assert_eq!(ctx.category, TaskCategory::DocSweeping);
        assert_eq!(ctx.complexity, TaskComplexity::Low);
    }
}
