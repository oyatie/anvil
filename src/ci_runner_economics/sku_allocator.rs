use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerSkuFinding {
    pub workflow_file: String,
    pub line_number: usize,
    pub runner_tag: String,
    pub reason: String,
}

pub struct RunnerSkuAllocator;

impl RunnerSkuAllocator {
    pub fn new() -> Self {
        Self
    }

    /// 100% Deterministic scan of GitHub Actions workflows for expensive runner allocation violations on PR triggers
    pub fn scan_workflow_runners(&self, file_path: &str, content: &str) -> Vec<RunnerSkuFinding> {
        let mut findings = Vec::new();

        if !file_path.contains(".github/workflows/") {
            return findings;
        }

        let runs_on_re = Regex::new(r#"(?i)runs-on:\s*\[?["']?([^\]"'\n]+)["']?\]?"#).unwrap();
        let is_pr_trigger = content.contains("pull_request:");

        for (idx, line) in content.lines().enumerate() {
            if let Some(caps) = runs_on_re.captures(line) {
                let runner_tag = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");

                // Expensive runners (macos-14, gpu) must not be attached to PR triggers
                if is_pr_trigger && (runner_tag.contains("macos") || runner_tag.contains("gpu")) {
                    findings.push(RunnerSkuFinding {
                        workflow_file: file_path.to_string(),
                        line_number: idx + 1,
                        runner_tag: runner_tag.to_string(),
                        reason: format!(
                            "Expensive runner SKU `{}` attached to PR trigger. Multi-arch/macOS runners must be tiered strictly to merge trains to protect billable compute economics.",
                            runner_tag
                        ),
                    });
                }
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_expensive_macos_runner_on_pr_trigger() {
        let alloc = RunnerSkuAllocator::new();
        let workflow = r#"
on:
  pull_request:
jobs:
  test:
    runs-on: macos-14
"#;
        let findings = alloc.scan_workflow_runners(".github/workflows/pr.yaml", workflow);
        assert_eq!(findings.len(), 1);
    }
}
