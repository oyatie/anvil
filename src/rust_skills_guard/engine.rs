use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RustQualityFinding {
    pub rule_id: String,
    pub category: String,
    pub severity: String,
    pub file_path: String,
    pub line_snippet: String,
    pub description: String,
    pub recommendation: String,
}

pub struct RustQualityEngine;

impl RustQualityEngine {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates PR diffs against high-priority deterministic rules from the full 380-rule corpus
    pub fn scan_diff(&self, diff_ctx: &PrDiffContext) -> Result<Vec<RustQualityFinding>> {
        let mut findings = Vec::new();

        let unwrap_re = Regex::new(r#"\.unwrap\(\)"#).unwrap();
        let ref_string_re = Regex::new(r#"(?:pub\s+)?(?:async\s+)?fn\s+[a-zA-Z0-9_]+\s*\(.*?\s*:\s*&String"#).unwrap();
        let ref_vec_re = Regex::new(r#"(?:pub\s+)?(?:async\s+)?fn\s+[a-zA-Z0-9_]+\s*\(.*?\s*:\s*&Vec<"#).unwrap();
        let blocking_in_async_re = Regex::new(r#"(?i)std::fs::read|std::thread::sleep"#).unwrap();
        let format_literal_re = Regex::new(r#"format!\(\s*"[^"{}]*"\s*\)"#).unwrap();
        let unsafe_block_re = Regex::new(r#"\bunsafe\s*\{"#).unwrap();
        let sync_mutex_in_async_re = Regex::new(r#"(?i)std::sync::Mutex::lock"#).unwrap();
        let clone_on_copy_re = Regex::new(r#"(?i)\.(?:clone\(\))\s*;.*//\s*primitive"#).unwrap();

        let mut current_file = diff_ctx.changed_files.first().cloned().unwrap_or_default();
        let mut is_test_file = false;
        let mut prev_line = String::new();

        for line in diff_ctx.diff_content.lines() {
            if line.starts_with("+++ b/") {
                current_file = line[6..].trim().to_string();
                is_test_file = current_file.contains("test") || current_file.contains("bench") || current_file.contains("mock");
                prev_line.clear();
                continue;
            }

            if !current_file.ends_with(".rs") {
                continue;
            }

            if line.starts_with('+') && !line.starts_with("+++") {
                let code_line = &line[1..].trim();

                // 1. [err-no-unwrap-prod] - Avoid .unwrap() in production code
                if !is_test_file && unwrap_re.is_match(code_line) && !code_line.contains("// test") && !code_line.contains("#[cfg(test)]") {
                    findings.push(RustQualityFinding {
                        rule_id: "err-no-unwrap-prod".to_string(),
                        category: "Error Handling (Priority 2)".to_string(),
                        severity: "HIGH".to_string(),
                        file_path: current_file.clone(),
                        line_snippet: code_line.to_string(),
                        description: "Use of `.unwrap()` in production code can cause unrecoverable panics.".to_string(),
                        recommendation: "Use the `?` operator, `.context()`, or handle the error explicitly with `match`/`if let`.".to_string(),
                    });
                }

                // 2. [own-slice-over-vec] - Accept &str instead of &String, &[T] instead of &Vec<T>
                if ref_string_re.is_match(code_line) {
                    findings.push(RustQualityFinding {
                        rule_id: "own-slice-over-vec".to_string(),
                        category: "Ownership & Borrowing (Priority 1)".to_string(),
                        severity: "MEDIUM".to_string(),
                        file_path: current_file.clone(),
                        line_snippet: code_line.to_string(),
                        description: "Accepting `&String` forces heap allocations for string slices.".to_string(),
                        recommendation: "Change parameter type from `&String` to `&str`.".to_string(),
                    });
                }
                if ref_vec_re.is_match(code_line) {
                    findings.push(RustQualityFinding {
                        rule_id: "own-slice-over-vec".to_string(),
                        category: "Ownership & Borrowing (Priority 1)".to_string(),
                        severity: "MEDIUM".to_string(),
                        file_path: current_file.clone(),
                        line_snippet: code_line.to_string(),
                        description: "Accepting `&Vec<T>` prevents callers from passing array slices or small collections.".to_string(),
                        recommendation: "Change parameter type from `&Vec<T>` to `&[T]`.".to_string(),
                    });
                }

                // 3. [async-spawn-blocking] - Blocking calls inside async executor
                if blocking_in_async_re.is_match(code_line) && !is_test_file {
                    findings.push(RustQualityFinding {
                        rule_id: "async-spawn-blocking".to_string(),
                        category: "Async/Await (Priority 6)".to_string(),
                        severity: "HIGH".to_string(),
                        file_path: current_file.clone(),
                        line_snippet: code_line.to_string(),
                        description: "Synchronous blocking I/O or `std::thread::sleep` inside async code starves the Tokio worker threads.".to_string(),
                        recommendation: "Use `tokio::fs`, `tokio::time::sleep`, or offload blocking calls to `tokio::task::spawn_blocking`.".to_string(),
                    });
                }

                // 4. [async-no-lock-await] - Synchronous mutex lock inside async code
                if sync_mutex_in_async_re.is_match(code_line) && !is_test_file {
                    findings.push(RustQualityFinding {
                        rule_id: "async-no-lock-await".to_string(),
                        category: "Async/Await (Priority 6)".to_string(),
                        severity: "HIGH".to_string(),
                        file_path: current_file.clone(),
                        line_snippet: code_line.to_string(),
                        description: "Using `std::sync::Mutex` in async contexts can cause deadlocks if held across `.await` points.".to_string(),
                        recommendation: "Use `tokio::sync::Mutex` or narrow the lock scope to a synchronous block.".to_string(),
                    });
                }

                // 5. [mem-avoid-format] - Avoid format!() on constant strings
                if format_literal_re.is_match(code_line) {
                    findings.push(RustQualityFinding {
                        rule_id: "mem-avoid-format".to_string(),
                        category: "Memory Optimization (Priority 3)".to_string(),
                        severity: "MEDIUM".to_string(),
                        file_path: current_file.clone(),
                        line_snippet: code_line.to_string(),
                        description: "Calling `format!(\"literal\")` without placeholders causes an unnecessary heap allocation.".to_string(),
                        recommendation: "Use `.to_string()`, `String::from(\"...\")`, or string literals directly.".to_string(),
                    });
                }

                // 6. [unsafe-safety-comment] - SAFETY: comment required for all unsafe blocks
                if unsafe_block_re.is_match(code_line) && !prev_line.contains("SAFETY:") {
                    findings.push(RustQualityFinding {
                        rule_id: "unsafe-safety-comment".to_string(),
                        category: "Unsafe Code (Priority 4)".to_string(),
                        severity: "CRITICAL".to_string(),
                        file_path: current_file.clone(),
                        line_snippet: code_line.to_string(),
                        description: "Unsafe block missing mandatory `// SAFETY:` explanation justifying memory invariants.".to_string(),
                        recommendation: "Document memory layout, pointer validity, and alignment guarantees in a preceding `// SAFETY:` comment.".to_string(),
                    });
                }

                // 7. [own-borrow-over-clone] - Unnecessary clone on primitive
                if clone_on_copy_re.is_match(code_line) {
                    findings.push(RustQualityFinding {
                        rule_id: "own-borrow-over-clone".to_string(),
                        category: "Ownership & Borrowing (Priority 1)".to_string(),
                        severity: "MEDIUM".to_string(),
                        file_path: current_file.clone(),
                        line_snippet: code_line.to_string(),
                        description: "Redundant `.clone()` on value that implements `Copy` or can be borrowed.".to_string(),
                        recommendation: "Remove `.clone()` call or pass by reference.".to_string(),
                    });
                }

                prev_line = code_line.to_string();
            }
        }

        Ok(findings)
    }
}
