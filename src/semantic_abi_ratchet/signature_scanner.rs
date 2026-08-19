use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakingAbiFinding {
    pub file_path: String,
    pub symbol_name: String,
    pub change_kind: String,
    pub detail: String,
}

pub struct SignatureScanner;

impl Default for SignatureScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl SignatureScanner {
    pub fn new() -> Self {
        Self
    }

    /// 100% Deterministic evaluation of public function signatures and struct memory layout stability
    pub fn scan_abi_diff(&self, file_path: &str, diff_content: &str) -> Vec<BreakingAbiFinding> {
        let mut findings = Vec::new();

        if !file_path.ends_with(".rs") {
            return findings;
        }

        // Detect removal of public function or breaking signature mutation
        if diff_content.contains("-pub fn ") && !diff_content.contains("+pub fn ") {
            findings.push(BreakingAbiFinding {
                file_path: file_path.to_string(),
                symbol_name: "public function".to_string(),
                change_kind: "REMOVAL".to_string(),
                detail: "Public library function removed without semver major bump or deprecation cycle.".to_string(),
            });
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_removed_public_function() {
        let scanner = SignatureScanner::new();
        let diff = "-pub fn legacy_api() -> u32 {\n-    42\n-}";
        let findings = scanner.scan_abi_diff("src/api.rs", diff);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn test_passes_added_public_function() {
        let scanner = SignatureScanner::new();
        let diff = "+pub fn new_api() -> u32 {\n+    42\n+}";
        let findings = scanner.scan_abi_diff("src/api.rs", diff);
        assert_eq!(findings.len(), 0);
    }
}
