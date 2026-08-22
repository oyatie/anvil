use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeapAllocationFinding {
    pub file_path: String,
    pub line_number: usize,
    pub snippet: String,
    pub suggestion: String,
}

pub struct AllocationScanner;

impl Default for AllocationScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl AllocationScanner {
    pub fn new() -> Self {
        Self
    }

    /// Scans performance-critical files for avoidable heap allocations in hotpaths
    /// Whether a path is in the hotpath scope this scanner inspects.
    ///
    /// `pub` because the caller must distinguish "scanned and clean" from
    /// "nothing was in scope"; the predicate was inline and unreachable.
    pub fn is_hotpath(file_path: &str) -> bool {
        file_path.contains("network/")
            || file_path.contains("codec/")
            || file_path.contains("engine/")
            || file_path.contains("hotpath")
            || file_path.contains("packet")
    }

    pub fn scan_hotpath_allocations(
        &self,
        file_path: &str,
        content: &str,
    ) -> Vec<HeapAllocationFinding> {
        let mut findings = Vec::new();

        // Only enforce strict zero-copy on latency-critical modules
        if !Self::is_hotpath(file_path) {
            return findings;
        }

        let clone_in_loop_re = Regex::new(r"for\s+.*in\s+.*\{[\s\S]*?\.clone\(\)").unwrap();
        let box_raw_re = Regex::new(r"Box::new\s*\(\s*\[").unwrap();

        for (idx, line) in content.lines().enumerate() {
            if box_raw_re.is_match(line) {
                findings.push(HeapAllocationFinding {
                    file_path: file_path.to_string(),
                    line_number: idx + 1,
                    snippet: line.trim().to_string(),
                    suggestion: "Avoid dynamic `Box::new([..])` heap allocation in hotpaths; use zero-copy `bytes::Bytes` or statically-sized buffers.".to_string(),
                });
            }
        }

        if clone_in_loop_re.is_match(content) {
            findings.push(HeapAllocationFinding {
                file_path: file_path.to_string(),
                line_number: 1,
                snippet: "for .. in .. { ... .clone() ... }".to_string(),
                suggestion:
                    "Avoid deep `.clone()` heap allocations inside iterative loops in hotpaths."
                        .to_string(),
            });
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_hotpath_heap_allocation() {
        let scanner = AllocationScanner::new();
        let code = "+ let buf = Box::new([0u8; 4096]);";
        let findings = scanner.scan_hotpath_allocations("src/network/codec.rs", code);
        assert_eq!(findings.len(), 1);
    }
}
