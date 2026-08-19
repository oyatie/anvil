use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeavyDependencyFinding {
    pub file_path: String,
    pub dependency_name: String,
    pub warning_message: String,
}

pub struct HeavyDependencyScanner;

impl HeavyDependencyScanner {
    pub fn new() -> Self {
        Self
    }

    /// 100% Deterministic scan of Cargo.toml and build.rs diffs for slow-to-compile macro dependencies
    pub fn scan_heavy_dependencies(&self, file_path: &str, content: &str) -> Vec<HeavyDependencyFinding> {
        let mut findings = Vec::new();

        if file_path.ends_with("Cargo.toml") {
            // Check for un-feature-gated heavy macro crates
            let full_syn_re = Regex::new(r#"syn\s*=\s*\{.*features\s*=\s*\[.*"full".*\]"#).unwrap();
            if full_syn_re.is_match(content) {
                findings.push(HeavyDependencyFinding {
                    file_path: file_path.to_string(),
                    dependency_name: "syn (full)".to_string(),
                    warning_message: "Heavy compile-time dependency `syn` with `features = [\"full\"]` added. Consider trimming features to avoid adding 15-30s compilation wallclock.".to_string(),
                });
            }
        } else if file_path.ends_with("build.rs") {
            // Check for build.rs without cargo:rerun-if-changed
            if !content.contains("cargo:rerun-if-changed") {
                findings.push(HeavyDependencyFinding {
                    file_path: file_path.to_string(),
                    dependency_name: "build.rs".to_string(),
                    warning_message: "New `build.rs` script added without `cargo:rerun-if-changed` triggers, causing unconditional re-execution on every compile.".to_string(),
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
    fn test_detects_uncached_build_rs() {
        let scanner = HeavyDependencyScanner::new();
        let code = "fn main() { println!(\"Compiling proto\"); }";
        let findings = scanner.scan_heavy_dependencies("build.rs", code);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn test_passes_cached_build_rs() {
        let scanner = HeavyDependencyScanner::new();
        let code = "fn main() { println!(\"cargo:rerun-if-changed=proto/\"); }";
        let findings = scanner.scan_heavy_dependencies("build.rs", code);
        assert_eq!(findings.len(), 0);
    }
}
