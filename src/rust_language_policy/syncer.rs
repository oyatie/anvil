use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tokio::process::Command;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct UpstreamRuleMeta {
    pub rule_id: String,
    pub category: String,
    pub prefix: String,
    pub title: String,
    pub summary: String,
    pub file_path: PathBuf,
}

pub struct UpstreamRustSkillsSyncer {
    cache_dir: PathBuf,
    rules: RwLock<HashMap<String, UpstreamRuleMeta>>,
    repo_url: String,
}

impl UpstreamRustSkillsSyncer {
    pub fn new(data_dir: &Path) -> Self {
        let cache_dir = data_dir.join("rust-skills");
        Self {
            cache_dir,
            rules: RwLock::new(HashMap::new()),
            repo_url: "https://github.com/jason931225/rust-skills.git".to_string(),
        }
    }

    /// Clones or pulls the latest upstream rust-skills repository and re-indexes all rules
    pub async fn ensure_synced(&self) -> Result<usize> {
        if !self.cache_dir.exists() {
            info!(
                "Cloning upstream rust-skills from {} into {:?}...",
                self.repo_url, self.cache_dir
            );
            if let Some(parent) = self.cache_dir.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            let mut clone_cmd = Command::new("git");
            clone_cmd.args([
                "clone",
                "--depth",
                "1",
                &self.repo_url,
                self.cache_dir.to_str().unwrap(),
            ]);
            let output = crate::exec::run_bounded(
                clone_cmd,
                crate::exec::ExecClass::Vcs,
                "git clone rust-skills",
            )
            .await
            .context("Failed to clone upstream rust-skills repo")?;

            if !output.status.success() {
                warn!(
                    "git clone rust-skills failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        } else {
            info!(
                "Pulling latest upstream rust-skills updates in {:?}...",
                self.cache_dir
            );
            let mut pull_cmd = Command::new("git");
            pull_cmd
                .current_dir(&self.cache_dir)
                .args(["pull", "--ff-only"]);
            let _ = crate::exec::run_bounded(
                pull_cmd,
                crate::exec::ExecClass::Vcs,
                "git pull --ff-only (rust-skills)",
            )
            .await;
        }

        self.index_rules_from_cache().await
    }

    /// Indexes all rule files and categories from SKILL.md and rules/
    pub async fn index_rules_from_cache(&self) -> Result<usize> {
        let skill_md_path = self.cache_dir.join("SKILL.md");
        let rules_dir = self.cache_dir.join("rules");

        let mut indexed = HashMap::new();

        if skill_md_path.exists()
            && let Ok(content) = tokio::fs::read_to_string(&skill_md_path).await
        {
            let mut current_category = "General".to_string();

            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(stripped) = trimmed.strip_prefix("### ") {
                    current_category = stripped.trim().to_string();
                    continue;
                }

                if trimmed.starts_with("- [`") && trimmed.contains("`](") {
                    // - [`own-borrow-over-clone`](rules/own-borrow-over-clone.md) - Prefer `&T` borrowing over `.clone()`
                    if let Some(end_id) = trimmed[4..].find("`]") {
                        let rule_id = &trimmed[4..4 + end_id];
                        let rest = &trimmed[4 + end_id + 2..];

                        let file_path = rules_dir.join(format!("{}.md", rule_id));
                        let summary = if let Some(dash_idx) = rest.find(" - ") {
                            rest[dash_idx + 3..].to_string()
                        } else {
                            rest.to_string()
                        };

                        let prefix = rule_id.split('-').next().unwrap_or("").to_string();

                        indexed.insert(
                            rule_id.to_string(),
                            UpstreamRuleMeta {
                                rule_id: rule_id.to_string(),
                                category: current_category.clone(),
                                prefix,
                                title: rule_id.replace('-', " "),
                                summary,
                                file_path,
                            },
                        );
                    }
                }
            }
        }

        let count = indexed.len();
        if count > 0 {
            info!(
                "UpstreamRustSkillsSyncer: Indexed {} live rules from upstream rust-skills repository.",
                count
            );
            let mut w = self.rules.write().unwrap();
            *w = indexed;
        }

        Ok(count)
    }

    /// Returns all indexed upstream rules
    pub fn get_all_rules(&self) -> Vec<UpstreamRuleMeta> {
        let r = self.rules.read().unwrap();
        r.values().cloned().collect()
    }

    /// Returns rule contents for a specific category prefix (e.g. "async", "own", "err")
    pub async fn get_rules_for_prefixes(&self, prefixes: &[&str]) -> Vec<(String, String)> {
        let rules = {
            let r = self.rules.read().unwrap();
            r.values()
                .filter(|rule| prefixes.contains(&rule.prefix.as_str()))
                .cloned()
                .collect::<Vec<_>>()
        };

        let mut loaded = Vec::new();
        for rule in rules {
            if let Ok(content) = tokio::fs::read_to_string(&rule.file_path).await {
                loaded.push((rule.rule_id, content));
            }
        }
        loaded
    }
}
