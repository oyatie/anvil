use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedTaskDefinition {
    pub task_id: String,
    pub source_doc_path: String,
    pub title: String,
    pub domain: String,
    pub priority: usize, // 0 = Critical/Foundational, 1 = High, 2 = Medium
    pub target_files: Vec<String>,
    pub dependencies: Vec<String>,
    pub required_invariants: Vec<String>,
    pub is_verified_ssot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationFinding {
    pub task_id: String,
    pub is_valid: bool,
    pub contradiction_reason: Option<String>,
    pub stale_references: Vec<String>,
}

#[derive(Clone, Default)]
pub struct SourceDocVerifier;

impl SourceDocVerifier {
    pub fn new() -> Self {
        Self
    }

    /// Verifies that a scoped task definition from an ADR or Issue reflects actual code truth with no contradictions or staleness
    pub fn verify_scoped_task(
        &self,
        task: &ScopedTaskDefinition,
        repo_root: &Path,
    ) -> Result<VerificationFinding> {
        info!(
            "🔍 [Source Doc Verifier] Auditing scoped task '{}' ({}) against repository truth...",
            task.task_id, task.source_doc_path
        );

        let mut stale_references = Vec::new();

        // 1. Verify target files exist or parent directories exist for new files
        for target_rel in &task.target_files {
            let full_path = repo_root.join(target_rel);
            let parent = full_path.parent();

            // If file doesn't exist, its parent directory must exist or be an expected module root
            if !full_path.exists() {
                if let Some(p) = parent {
                    if !p.exists() && !target_rel.starts_with("src/") && !target_rel.starts_with("tests/") {
                        stale_references.push(format!(
                            "Target path '{}' points to nonexistent parent directory {:?}",
                            target_rel, p
                        ));
                    }
                }
            }
        }

        // 2. Check for anti-patterns or contradictions in ADR/Issue description
        let mut contradiction_reason = None;

        // Check if the source doc is marked DEPRECATED or SUPERSEDED
        let source_full = repo_root.join(&task.source_doc_path);
        if source_full.exists() {
            if let Ok(content) = std::fs::read_to_string(&source_full) {
                let lower = content.to_lowercase();
                if lower.contains("status: superseded")
                    || lower.contains("status: deprecated")
                    || lower.contains("status: rejected")
                {
                    contradiction_reason = Some(format!(
                        "Source document '{}' is marked SUPERSEDED or DEPRECATED. Execution prohibited.",
                        task.source_doc_path
                    ));
                }
            }
        }

        let is_valid = contradiction_reason.is_none() && stale_references.is_empty();

        if is_valid {
            info!(
                "✅ [Source Doc Verifier] Task '{}' verified as Single Source of Truth (SSOT).",
                task.task_id
            );
        } else {
            warn!(
                "⚠️ [Source Doc Verifier] Task '{}' rejected: contradiction={:?}, stale_refs={:?}",
                task.task_id, contradiction_reason, stale_references
            );
        }

        Ok(VerificationFinding {
            task_id: task.task_id.clone(),
            is_valid,
            contradiction_reason,
            stale_references,
        })
    }

    /// Scans an ADR directory (e.g. `docs/adr/` or `docs/decisions/`) and extracts scoped task definitions
    pub fn scan_adrs_for_work(&self, repo_root: &Path) -> Result<Vec<ScopedTaskDefinition>> {
        let mut tasks = Vec::new();
        let adr_dir = repo_root.join("docs/adr");
        let decisions_dir = repo_root.join("docs/decisions");

        let search_dirs = [adr_dir, decisions_dir];

        for dir in &search_dirs {
            if !dir.exists() {
                continue;
            }

            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("md") {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Some(task) = self.parse_adr_to_task(&path, &content, repo_root) {
                                tasks.push(task);
                            }
                        }
                    }
                }
            }
        }

        Ok(tasks)
    }

    fn parse_adr_to_task(
        &self,
        path: &Path,
        content: &str,
        repo_root: &Path,
    ) -> Option<ScopedTaskDefinition> {
        let file_stem = path.file_stem()?.to_str()?;
        let rel_path = path.strip_prefix(repo_root).unwrap_or(path).to_string_lossy().to_string();

        let title_line = content
            .lines()
            .find(|l| l.starts_with("# "))
            .unwrap_or(file_stem);
        let title = title_line.trim_start_matches("# ").trim().to_string();

        // Extract priority and dependencies from frontmatter or content
        let priority = if content.contains("Priority: P0") || content.contains("Priority: Critical") {
            0
        } else if content.contains("Priority: P1") || content.contains("Priority: High") {
            1
        } else {
            2
        };

        let mut dependencies = Vec::new();
        for line in content.lines() {
            if line.starts_with("Depends-On:") || line.starts_with("depends_on:") {
                let deps_str = line.split(':').nth(1).unwrap_or("").trim();
                for dep in deps_str.split(',') {
                    let d = dep.trim().to_string();
                    if !d.is_empty() {
                        dependencies.push(d);
                    }
                }
            }
        }

        Some(ScopedTaskDefinition {
            task_id: file_stem.to_string(),
            source_doc_path: rel_path,
            title,
            domain: "architecture".to_string(),
            priority,
            target_files: vec!["src/lib.rs".to_string()],
            dependencies,
            required_invariants: Vec::new(),
            is_verified_ssot: true,
        })
    }
}
