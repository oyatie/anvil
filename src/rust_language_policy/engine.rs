//! The rule table, and what one finding of it looks like.
//!
//! The scan that applies these lives in `scan.rs`: the rules are a table and
//! the scan is a loop, and only the loop changes when a rule's subject changes.
//! `engine.rs` keeps the table because `fidelity::registry` cites these rule
//! declarations by line, and a citation is worth what it points at.

use serde::{Deserialize, Serialize};

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
    pub(super) fn from_rule(
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

pub(super) const ERR_NO_UNWRAP_PROD: RustRule = RustRule {
    id: "err-no-unwrap-prod",
    category: "Error Handling",
    severity: "HIGH",
};
pub(super) const OWN_SLICE_OVER_VEC: RustRule = RustRule {
    id: "own-slice-over-vec",
    category: "Ownership & Borrowing",
    severity: "MEDIUM",
};
/// Whether this text opens an async scope.
///
/// `async fn`, `async move {` and a bare `async {` all do. A `.await` does not
/// open one, but it can only appear inside one, so it is evidence too -- and it
/// is what a hunk header carries when the header names a line rather than the
/// signature.
pub(super) fn declares_async(text: &str) -> bool {
    text.contains("async fn")
        || text.contains("async move")
        || text.contains("async {")
        || text.contains(".await")
}

pub(super) const ASYNC_SPAWN_BLOCKING: RustRule = RustRule {
    id: "async-spawn-blocking",
    category: "Async/Await",
    severity: "HIGH",
};
pub(super) const ASYNC_NO_LOCK_AWAIT: RustRule = RustRule {
    id: "async-no-lock-await",
    category: "Async/Await",
    severity: "HIGH",
};
pub(super) const MEM_AVOID_FORMAT: RustRule = RustRule {
    id: "mem-avoid-format",
    category: "Memory Optimization",
    severity: "MEDIUM",
};
pub(super) const UNSAFE_SAFETY_COMMENT: RustRule = RustRule {
    id: "unsafe-safety-comment",
    category: "Unsafe Code",
    severity: "CRITICAL",
};
pub(super) const OWN_BORROW_OVER_CLONE: RustRule = RustRule {
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
