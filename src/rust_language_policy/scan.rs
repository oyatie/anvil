//! The diff scan: added lines in `.rs` files, against the rule table.
//!
//! # What the scan withholds, and why
//!
//! **The file a finding names.** `current_file` is `None` until the diff names
//! one, rather than being seeded with `changed_files[0]`. The seed made every
//! line before the first `+++ b/` header a finding against a real file in the
//! pull request, which is what made it credible: a diff whose only line was
//! `+let v = maybe.unwrap();` reported `findings=["src/innocent.rs"]`, and a
//! reviewer opening that file found no `.unwrap()` and nothing to tell them the
//! name came off a list.
//!
//! **Async scope.** Two rules here are named for what they do to an async
//! executor -- starving Tokio workers, deadlocking across an `.await`. Neither
//! is true of the same call in a synchronous function, where blocking is simply
//! how the code works, so firing on every line was a HIGH-severity accusation
//! against correct code: I1's symmetric violation. The answer is diff-scoped
//! because the evidence is: `@@` headers carry the enclosing item and added
//! lines can declare their own, and when neither says async the rules withhold
//! rather than guess. Any `fn`, nested or not, closes the scope -- a
//! synchronous function declared inside an async one is still synchronous.
//!
//! **The sync-mutex pattern.** `.lock().unwrap()` is the std spelling and only
//! the std spelling: `std::sync::Mutex::lock` returns a `LockResult` that must
//! be unwrapped, while `tokio::sync::Mutex::lock` returns a future that must be
//! awaited. Keying on the literal path `std::sync::Mutex::lock`, which is not
//! how anyone calls it, meant the rule could hardly fire.

use anyhow::Result;
use regex::Regex;

use super::engine::{
    ASYNC_NO_LOCK_AWAIT, ASYNC_SPAWN_BLOCKING, ERR_NO_UNWRAP_PROD, MEM_AVOID_FORMAT,
    OWN_BORROW_OVER_CLONE, OWN_SLICE_OVER_VEC, RustQualityFinding, UNSAFE_SAFETY_COMMENT,
    declares_async,
};
use crate::git_manager::PrDiffContext;

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
        let sync_mutex_in_async_re = Regex::new(r#"\.lock\(\)\s*\.(?:unwrap|expect)\("#).unwrap();
        let clone_on_copy_re = Regex::new(r#"(?i)\.(?:clone\(\))\s*;.*//\s*primitive"#).unwrap();

        // `None` until the diff names a file. See the module docs.
        let mut current_file: Option<String> = None;
        let mut is_test_file = false;
        let mut prev_line = String::new();
        // Whether this line sits inside an async item. See the module docs.
        let mut in_async = false;

        for line in diff_ctx.diff_content.lines() {
            if let Some(rest) = line.strip_prefix("@@") {
                // `@@ -a,b +c,d @@ <enclosing item>` -- git's function context.
                in_async = rest
                    .split_once("@@")
                    .map(|(_, ctx)| declares_async(ctx))
                    .unwrap_or(false);
                prev_line.clear();
                continue;
            }

            if let Some(stripped) = line.strip_prefix("+++ b/") {
                let path = stripped.trim().to_string();
                // `path.contains("test")` spared `src/latest_state.rs` and
                // `src/attestation_guard.rs` while charging every production
                // file that merely says the word. Cargo's layout is the rule.
                is_test_file = crate::source_scan::paths::is_test_source(&path)
                    || path.contains("/benches/")
                    || path.ends_with("_bench.rs")
                    || path.contains("/mocks/")
                    || path.ends_with("_mock.rs");
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

            // Context and added lines both move the enclosing item.
            let body = line.strip_prefix(['+', ' ']).unwrap_or(line);
            if declares_async(body) {
                in_async = true;
            } else if body.trim_start().starts_with("fn ")
                || body.trim_start().starts_with("pub fn ")
            {
                in_async = false;
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
                if blocking_in_async_re.is_match(code_line) && !is_test_file && in_async {
                    findings.push(RustQualityFinding::from_rule(
                        &ASYNC_SPAWN_BLOCKING,
                        current_file,
                        code_line,
                        "Synchronous blocking I/O or `std::thread::sleep` inside async code starves the Tokio worker threads.",
                        "Use `tokio::fs`, `tokio::time::sleep`, or offload blocking calls to `tokio::task::spawn_blocking`.",
                    ));
                }

                // 4. [async-no-lock-await] - Synchronous mutex lock inside async code
                if sync_mutex_in_async_re.is_match(code_line) && !is_test_file && in_async {
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
