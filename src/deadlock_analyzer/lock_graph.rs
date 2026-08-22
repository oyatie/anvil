use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockOrderFinding {
    pub lock_sequence: Vec<String>,
    pub file_path: String,
    pub description: String,
}

#[derive(Debug, Clone, Default)]
pub struct LockOrderKeywordScanner;

impl LockOrderKeywordScanner {
    pub fn new() -> Self {
        Self
    }

    /// Scans a diff for one hardcoded lock-ordering pair.
    ///
    /// No lock graph is built and no cycle is detected. The scanner fires only
    /// on the four literal lock names below, none of which appear anywhere in
    /// this repository -- so on real code its finding rate is zero. Kept as a
    /// placeholder for a call-graph-based analysis; see issue #58.
    pub fn scan_known_lock_order_pairs(
        &self,
        file_path: &str,
        content: &str,
    ) -> Vec<LockOrderFinding> {
        let mut findings = Vec::new();

        // Detect known inverted acquisition: lock B acquired before lock A where canonical order is A -> B
        let lines: Vec<&str> = content.lines().collect();
        let mut acquired_lock_b = false;

        for line in lines {
            if line.contains(".session_lock.lock()") || line.contains(".user_mutex.lock()") {
                acquired_lock_b = true;
            }
            if acquired_lock_b
                && (line.contains(".global_state.lock()") || line.contains(".cluster_mutex.lock()"))
            {
                findings.push(LockOrderFinding {
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
        let analyzer = LockOrderKeywordScanner::new();
        let code = r#"
        let s = self.session_lock.lock().await;
        let g = self.global_state.lock().await;
        "#;
        let findings = analyzer.scan_known_lock_order_pairs("src/auth.rs", code);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].description.contains("lock order inversion"));
    }

    #[test]
    fn test_passes_canonical_lock_order() {
        let analyzer = LockOrderKeywordScanner::new();
        let code = r#"
        let g = self.global_state.lock().await;
        let s = self.session_lock.lock().await;
        "#;
        let findings = analyzer.scan_known_lock_order_pairs("src/auth.rs", code);
        assert!(findings.is_empty());
    }
}
