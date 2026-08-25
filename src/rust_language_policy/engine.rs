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

impl RustQualityFinding {
    /// A finding for `rule`, taking its category and severity from the rule.
    ///
    /// Both were literals written a second time at each finding site. They are
    /// published -- `categories_evaluated` on the report, and the sentence
    /// "N rule(s), M of which can block" -- so a second copy is a second thing
    /// to drift, and the compiler relates the two not at all. Mutating
    /// `unsafe-safety-comment` to a fabricated category and a `MEDIUM` severity
    /// used to leave the whole suite green while the gate announced "3 of which
    /// can block" and went on blocking on 4. There is one table now.
    fn from_rule(
        rule: &RustRule,
        file_path: &str,
        line_snippet: &str,
        description: &str,
        recommendation: &str,
    ) -> Self {
        Self {
            rule_id: rule.id.to_string(),
            category: rule.category.to_string(),
            severity: rule.severity.to_string(),
            file_path: file_path.to_string(),
            line_snippet: line_snippet.to_string(),
            description: description.to_string(),
            recommendation: recommendation.to_string(),
        }
    }

    /// Whether this finding blocks the gate.
    ///
    /// Resolved through [`RULES`] by id rather than by re-testing the severity
    /// string, so the count the gate publishes ("M of which can block") and the
    /// set that actually blocks are read off one table. A finding carrying a
    /// `rule_id` absent from `RULES` does not block, which
    /// `every_rule_the_engine_reports_is_in_the_table_the_count_comes_from`
    /// makes unreachable.
    pub fn blocks(&self) -> bool {
        RULES
            .iter()
            .any(|rule| rule.id == self.rule_id && rule.blocks())
    }
}

/// One rule this engine implements.
///
/// The table below is the engine's inventory, and it is what
/// `rules_evaluated_count` and `categories_evaluated` are derived from. Those
/// two were literals -- `380` and a hand-written list of eight categories
/// carrying their own invented per-category totals -- describing an upstream
/// corpus (`jason931225/rust-skills`, `rules-434` today and 380 for a few days
/// in August 2026) that nothing in this process has ever loaded. A count is
/// only a measurement when it counts the ruleset that ran, which is what ESLint
/// reports from a resolved config and semgrep from `--config`.
///
/// `every_rule_the_engine_reports_is_in_the_table_the_count_comes_from` pins
/// this table to the `rule_id`s `scan_diff` actually emits, because nothing in
/// the compiler relates the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RustRule {
    pub id: &'static str,
    pub category: &'static str,
    /// `CRITICAL` and `HIGH` block the gate; `MEDIUM` is advisory.
    pub severity: &'static str,
}

impl RustRule {
    pub fn blocks(&self) -> bool {
        matches!(self.severity, "CRITICAL" | "HIGH")
    }
}

const ERR_NO_UNWRAP_PROD: RustRule = RustRule {
    id: "err-no-unwrap-prod",
    category: "Error Handling",
    severity: "HIGH",
};
const OWN_SLICE_OVER_VEC: RustRule = RustRule {
    id: "own-slice-over-vec",
    category: "Ownership & Borrowing",
    severity: "MEDIUM",
};
const ASYNC_SPAWN_BLOCKING: RustRule = RustRule {
    id: "async-spawn-blocking",
    category: "Async/Await",
    severity: "HIGH",
};
const ASYNC_NO_LOCK_AWAIT: RustRule = RustRule {
    id: "async-no-lock-await",
    category: "Async/Await",
    severity: "HIGH",
};
const MEM_AVOID_FORMAT: RustRule = RustRule {
    id: "mem-avoid-format",
    category: "Memory Optimization",
    severity: "MEDIUM",
};
const UNSAFE_SAFETY_COMMENT: RustRule = RustRule {
    id: "unsafe-safety-comment",
    category: "Unsafe Code",
    severity: "CRITICAL",
};
const OWN_BORROW_OVER_CLONE: RustRule = RustRule {
    id: "own-borrow-over-clone",
    category: "Ownership & Borrowing",
    severity: "MEDIUM",
};

/// Every rule `scan_diff` evaluates. Seven; four of them can block.
///
/// The rules are named constants rather than anonymous literals so that
/// `scan_diff` can reach the one it is evaluating at compile time. Each finding
/// site used to repeat that rule's category and severity as its own string
/// literals, and those two columns are published while nothing related them to
/// this table.
pub const RULES: &[RustRule] = &[
    ERR_NO_UNWRAP_PROD,
    OWN_SLICE_OVER_VEC,
    ASYNC_SPAWN_BLOCKING,
    ASYNC_NO_LOCK_AWAIT,
    MEM_AVOID_FORMAT,
    UNSAFE_SAFETY_COMMENT,
    OWN_BORROW_OVER_CLONE,
];

/// The distinct categories `RULES` covers, in table order.
pub fn categories() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for rule in RULES {
        if !out.iter().any(|c| c == rule.category) {
            out.push(rule.category.to_string());
        }
    }
    out
}

pub struct RustQualityEngine;

impl Default for RustQualityEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RustQualityEngine {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates added diff lines in `.rs` files against every rule in [`RULES`].
    pub fn scan_diff(&self, diff_ctx: &PrDiffContext) -> Result<Vec<RustQualityFinding>> {
        let mut findings = Vec::new();

        let unwrap_re = Regex::new(r#"\.unwrap\(\)"#).unwrap();
        let ref_string_re =
            Regex::new(r#"(?:pub\s+)?(?:async\s+)?fn\s+[a-zA-Z0-9_]+\s*\(.*?\s*:\s*&String"#)
                .unwrap();
        let ref_vec_re =
            Regex::new(r#"(?:pub\s+)?(?:async\s+)?fn\s+[a-zA-Z0-9_]+\s*\(.*?\s*:\s*&Vec<"#)
                .unwrap();
        let blocking_in_async_re = Regex::new(r#"(?i)std::fs::read|std::thread::sleep"#).unwrap();
        let format_literal_re = Regex::new(r#"format!\(\s*"[^"{}]*"\s*\)"#).unwrap();
        let unsafe_block_re = Regex::new(r#"\bunsafe\s*\{"#).unwrap();
        let sync_mutex_in_async_re = Regex::new(r#"(?i)std::sync::Mutex::lock"#).unwrap();
        let clone_on_copy_re = Regex::new(r#"(?i)\.(?:clone\(\))\s*;.*//\s*primitive"#).unwrap();

        // `None` until the diff names a file, rather than seeding it with the
        // first changed file.
        //
        // The seed made every line before the first `+++ b/` header a finding
        // against `changed_files[0]` -- a real file in the pull request, which
        // is what made it credible. Measured on the old code, a diff whose only
        // line was `+let v = maybe.unwrap();` produced:
        //
        //     rust_policy idiomatic=false findings=["src/innocent.rs"]
        //
        // A reviewer opens src/innocent.rs, finds no `.unwrap()`, and has
        // nothing to tell them the gate picked that name off a list.
        let mut current_file: Option<String> = None;
        let mut is_test_file = false;
        let mut prev_line = String::new();

        for line in diff_ctx.diff_content.lines() {
            if let Some(stripped) = line.strip_prefix("+++ b/") {
                let path = stripped.trim().to_string();
                is_test_file =
                    path.contains("test") || path.contains("bench") || path.contains("mock");
                current_file = Some(path);
                prev_line.clear();
                continue;
            }

            // No header yet: this hunk belongs to no file the diff has named,
            // and a finding that cannot name its file is not one to report.
            let Some(current_file) = current_file.as_deref() else {
                continue;
            };

            if !current_file.ends_with(".rs") {
                continue;
            }

            if line.starts_with('+') && !line.starts_with("+++") {
                let code_line = &line[1..].trim();

                // 1. [err-no-unwrap-prod] - Avoid .unwrap() or empty .expect("") in production code
                let has_unwrap = unwrap_re.is_match(code_line);
                let has_empty_expect =
                    code_line.contains(".expect(\"\")") || code_line.contains(".expect(\"TODO\")");
                if !is_test_file
                    && (has_unwrap || has_empty_expect)
                    && !code_line.contains("// test")
                    && !code_line.contains("#[cfg(test)]")
                {
                    findings.push(RustQualityFinding::from_rule(
                        &ERR_NO_UNWRAP_PROD,
                        current_file,
                        code_line,
                        "Use of `.unwrap()` or empty `.expect(\"\")` in production code can cause unrecoverable panics.",
                        "Use the `?` operator, `.context()`, or handle the error explicitly with `match`/`if let`.",
                    ));
                }

                // 2. [own-slice-over-vec] - Accept &str instead of &String, &[T] instead of &Vec<T>
                if ref_string_re.is_match(code_line) {
                    findings.push(RustQualityFinding::from_rule(
                        &OWN_SLICE_OVER_VEC,
                        current_file,
                        code_line,
                        "Accepting `&String` forces heap allocations for string slices.",
                        "Change parameter type from `&String` to `&str`.",
                    ));
                }
                if ref_vec_re.is_match(code_line) {
                    findings.push(RustQualityFinding::from_rule(
                        &OWN_SLICE_OVER_VEC,
                        current_file,
                        code_line,
                        "Accepting `&Vec<T>` prevents callers from passing array slices or small collections.",
                        "Change parameter type from `&Vec<T>` to `&[T]`.",
                    ));
                }

                // 3. [async-spawn-blocking] - Blocking calls inside async executor
                if blocking_in_async_re.is_match(code_line) && !is_test_file {
                    findings.push(RustQualityFinding::from_rule(
                        &ASYNC_SPAWN_BLOCKING,
                        current_file,
                        code_line,
                        "Synchronous blocking I/O or `std::thread::sleep` inside async code starves the Tokio worker threads.",
                        "Use `tokio::fs`, `tokio::time::sleep`, or offload blocking calls to `tokio::task::spawn_blocking`.",
                    ));
                }

                // 4. [async-no-lock-await] - Synchronous mutex lock inside async code
                if sync_mutex_in_async_re.is_match(code_line) && !is_test_file {
                    findings.push(RustQualityFinding::from_rule(
                        &ASYNC_NO_LOCK_AWAIT,
                        current_file,
                        code_line,
                        "Using `std::sync::Mutex` in async contexts can cause deadlocks if held across `.await` points.",
                        "Use `tokio::sync::Mutex` or narrow the lock scope to a synchronous block.",
                    ));
                }

                // 5. [mem-avoid-format] - Avoid format!() on constant strings
                if format_literal_re.is_match(code_line) {
                    findings.push(RustQualityFinding::from_rule(
                        &MEM_AVOID_FORMAT,
                        current_file,
                        code_line,
                        "Calling `format!(\"literal\")` without placeholders causes an unnecessary heap allocation.",
                        "Use `.to_string()`, `String::from(\"...\")`, or string literals directly.",
                    ));
                }

                // 6. [unsafe-safety-comment] - SAFETY: comment required for all unsafe blocks
                if unsafe_block_re.is_match(code_line) && !prev_line.contains("SAFETY:") {
                    findings.push(RustQualityFinding::from_rule(
                        &UNSAFE_SAFETY_COMMENT,
                        current_file,
                        code_line,
                        "Unsafe block missing mandatory `// SAFETY:` explanation justifying memory invariants.",
                        "Document memory layout, pointer validity, and alignment guarantees in a preceding `// SAFETY:` comment.",
                    ));
                }

                // 7. [own-borrow-over-clone] - Unnecessary clone on primitive
                if clone_on_copy_re.is_match(code_line) {
                    findings.push(RustQualityFinding::from_rule(
                        &OWN_BORROW_OVER_CLONE,
                        current_file,
                        code_line,
                        "Redundant `.clone()` on value that implements `Copy` or can be borrowed.",
                        "Remove `.clone()` call or pass by reference.",
                    ));
                }

                prev_line = code_line.to_string();
            }
        }

        Ok(findings)
    }
}
