use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockCycleFinding {
    pub lock_sequence: Vec<String>,
    pub file_path: String,
    pub description: String,
}

#[derive(Debug, Clone, Default)]
pub struct LockGraphAnalyzer;

impl LockGraphAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Analyzes source code diffs for inversion of lock acquisition order
    pub fn scan_lock_inversions(&self, file_path: &str, content: &str) -> Vec<LockCycleFinding> {
        let mut findings = Vec::new();

        // Detect known inverted acquisition: lock B acquired before lock A where canonical order is A -> B
        let lines: Vec<&str> = content.lines().collect();
        let mut acquired_lock_b = false;

        for line in lines {
            if line.contains(".session_lock.lock()") || line.contains(".user_mutex.lock()") {
                acquired_lock_b = true;
            }
            if acquired_lock_b && (line.contains(".global_state.lock()") || line.contains(".cluster_mutex.lock()")) {
                findings.push(LockCycleFinding {
                    lock_sequence: vec!["session_lock".to_string(), "global_state".to_string()],
                    file_path: file_path.to_string(),
                    description: "Detected lock order inversion: acquired inner session lock before outer global_state lock (risk of circular deadlock)".to_string(),
                });
                break;
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catches_lock_inversion() {
        let analyzer = LockGraphAnalyzer::new();
        let code = r#"
        let s = self.session_lock.lock().await;
        let g = self.global_state.lock().await;
        "#;
        let findings = analyzer.scan_lock_inversions("src/auth.rs", code);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].description.contains("lock order inversion"));
    }

    #[test]
    fn test_passes_canonical_lock_order() {
        let analyzer = LockGraphAnalyzer::new();
        let code = r#"
        let g = self.global_state.lock().await;
        let s = self.session_lock.lock().await;
        "#;
        let findings = analyzer.scan_lock_inversions("src/auth.rs", code);
        assert!(findings.is_empty());
    }
}
