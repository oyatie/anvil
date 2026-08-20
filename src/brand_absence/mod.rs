//! Brand-absence gate: names and PR-visible strings must describe what the code
//! verifies, not the aspiration the author was reaching for.
//!
//! # What this gate actually checks
//!
//! Three mechanical checks, nothing more:
//!
//! 1. [`BrandViolationKind::Name`] — a module path or a declared item name
//!    (`struct`/`enum`/`trait`/`type`/`fn`/`mod`/`const`/`static`/`union`)
//!    whose words contain a product brand or an aspiration stamp from
//!    [`FORBIDDEN_STAMPS`].
//! 2. [`BrandViolationKind::DisplayString`] — the same vocabulary appearing
//!    inside a string literal. String literals are covered because they reach a
//!    pull request: Anvil posts them as review findings and log lines.
//! 3. [`BrandViolationKind::GateCountClaim`] — a hardcoded `<n> gate(s)` claim
//!    in a string literal that disagrees with the live corpus size returned by
//!    [`BrandAbsenceGate::real_gate_count`].
//!
//! The count is read from the one place that defines it — the `GateStatus`
//! fields of `PreMergeCertificationReport` — using the same mechanism
//! `pre_merge_guard::report`'s own `all_statuses_covers_every_gate_field` test
//! uses, so the claim is compared against the corpus rather than against another
//! constant.
//!
//! # Warn-only, with the existing violations recorded as debt
//!
//! The tree already contains violations of all three kinds. Renaming them is
//! explicitly out of scope here (plan §36.2 sequences renames after the
//! retain/discard determination — renaming code that is about to be deleted is
//! waste), so this gate ships warn-only: [`BrandAbsenceReport::is_blocking`] is
//! always `false`. The pre-existing violations are enumerated in
//! [`KNOWN_VIOLATIONS`] with an occurrence count each.
//!
//! The ledger is a **debt ledger, not an exemption**. Each entry records a
//! ceiling: the (N+1)th occurrence of the same stamp in an already-listed file
//! is reported as new. Every report states the total recorded debt so the number
//! can be watched shrinking.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The file that defines the vocabulary unavoidably contains every term in it,
/// so [`BrandAbsenceGate::scan_tree`] skips it. This is a mechanical necessity
/// rather than an exemption: [`BrandAbsenceGate::scan_source`] is path-agnostic
/// and still reports violations for this path when handed the content directly.
pub const VOCABULARY_DEFINITION_PATH: &str = "src/brand_absence/mod.rs";

/// This lane ships the gate warn-only. Flipping this to `false` makes new
/// violations fatal, and must not be done while [`KNOWN_VIOLATIONS`] is
/// non-empty.
pub const WARN_ONLY: bool = true;

/// Product brands and aspiration/category stamps that may not appear in a name
/// or a PR-visible string.
///
/// Matching is word-based, not substring-based: the text and the stamp are both
/// split into lowercase words on non-alphanumeric characters and `camelCase`
/// boundaries, and a stamp matches only where its whole word sequence appears.
/// `"aws"` therefore matches `AwsClient` and `"AWS, GCP"` but not `flaws`, and
/// `"cloud native"` matches `cloud_native_guard` without firing on every
/// sentence containing the word "cloud".
pub const FORBIDDEN_STAMPS: &[&str] = &[
    // Scale / tier aspirations. Named in the law (§33 D33.2).
    "hyperscale",
    "hyperscaler",
    "cloud native",
    "enterprise",
    "web scale",
    "planet scale",
    "google scale",
    "faang",
    "big tech",
    // Unearned quality claims.
    "world class",
    "industry leading",
    "industry standard",
    "best in class",
    "state of the art",
    "next generation",
    "next gen",
    "battle tested",
    "military grade",
    "bank grade",
    "production grade",
];

/// Vendor names, checked separately from [`FORBIDDEN_STAMPS`] and under a
/// different rule: a violation is only raised where **two or more distinct**
/// vendors appear in the same name or string.
///
/// A single vendor name is usually descriptive — `oidc_validator` says "AWS"
/// because it validates AWS OIDC tokens, and `cloud_native_guard` says "AWS"
/// because it looks for the AWS SDK. Flagging those would fill the ledger with
/// entries that are not defects, and a warn-only gate that mostly cries wolf is
/// switched off within a week. A vendor *roll-call* is the construction the law
/// is aimed at: `"5/5 Hyperscalers Approved: AWS, GCP, Meta, Azure, OCI"`
/// borrows five brands' credibility for a check that reads Rust source.
/// Ambiguous tokens are deliberately absent: `OCI` is the Open Container
/// Initiative far more often than it is Oracle Cloud Infrastructure (verified:
/// `src/pre_merge_guard/matrix.rs:123` is digest pinning), and `meta` is HTML
/// and metadata before it is a company. The roll-call in
/// `hyperscaler_consensus_guard` still trips on AWS + GCP + Azure without them.
pub const VENDOR_BRANDS: &[&str] = &[
    "aws",
    "gcp",
    "azure",
    "amazon web services",
    "google cloud",
    "oracle cloud",
    "alibaba cloud",
];

/// How many distinct vendors in one name or string constitute a roll-call.
const VENDOR_ROLL_CALL_THRESHOLD: usize = 2;

/// One recorded pre-existing violation: an exact path, an exact stamp, how many
/// times it currently occurs, and what the debt is owed for.
///
/// The ledger is an enumeration of verified facts about the tree. It carries no
/// patterns — no wildcard, prefix, or regex entry — so it cannot grow to cover
/// violations nobody has looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllowlistedDebt {
    /// Repository-relative path, exactly as scanned. No pattern characters.
    pub path: &'static str,
    /// The stamp from [`FORBIDDEN_STAMPS`], or the claimed count for a
    /// [`BrandViolationKind::GateCountClaim`].
    pub stamp: &'static str,
    /// How many occurrences are recorded. This is a **ceiling**: occurrence
    /// `occurrences + 1` is reported as new.
    pub occurrences: usize,
    /// Why the debt is still outstanding.
    pub debt_note: &'static str,
}

/// Which of the three checks a violation came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BrandViolationKind {
    /// A module path or declared item name.
    Name,
    /// A string literal, which reaches a pull request.
    DisplayString,
    /// A hardcoded gate count that disagrees with the live corpus.
    GateCountClaim,
}

/// A single violation, carrying enough context for the author to fix it without
/// re-running the scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrandViolation {
    /// Repository-relative path the violation was found in.
    pub path: String,
    /// 1-based line number, or 0 when the violation is the path itself.
    pub line: usize,
    /// Which check fired.
    pub kind: BrandViolationKind,
    /// The stamp that matched, or the claimed count for a gate-count claim.
    pub stamp: String,
    /// The offending name, string literal, or path.
    pub snippet: String,
}

/// The result of a scan.
#[derive(Debug, Clone)]
pub struct BrandAbsenceReport {
    /// Violations that are not covered by the ledger. Warn-only: these are
    /// printed, not fatal.
    pub new_violations: Vec<BrandViolation>,
    /// Hits absorbed by a ledger entry in this scan. Counted rather than
    /// dropped, so allowlisted debt stays visible.
    pub allowlisted_hits: usize,
    /// Total occurrences recorded across the whole ledger.
    pub allowlisted_debt_total: usize,
    /// Always `false` while [`WARN_ONLY`] is set.
    pub is_blocking: bool,
    /// One line stating the outcome, the recorded debt, and the real gate count.
    pub summary: String,
}

/// The gate itself.
pub struct BrandAbsenceGate {
    allowlist: &'static [AllowlistedDebt],
    real_gate_count: usize,
}

impl Default for BrandAbsenceGate {
    fn default() -> Self {
        Self::new()
    }
}

impl BrandAbsenceGate {
    /// Gate backed by the production debt ledger.
    pub fn new() -> Self {
        Self::with_allowlist(KNOWN_VIOLATIONS)
    }

    /// Gate backed by a caller-supplied ledger.
    pub fn with_allowlist(allowlist: &'static [AllowlistedDebt]) -> Self {
        Self {
            allowlist,
            real_gate_count: real_gate_count_from_report_source(),
        }
    }

    /// The live gate-corpus size: the number of `GateStatus` fields declared on
    /// `PreMergeCertificationReport`.
    ///
    /// Returns 0 if the declaration cannot be located, in which case gate-count
    /// claims are not checked — an unknown source of truth must not be reported
    /// as a passing comparison.
    pub fn real_gate_count(&self) -> usize {
        self.real_gate_count
    }

    /// Scans one file's contents. `path` is checked too: the two worst real
    /// instances are directory names, which a scanner that only read
    /// declarations would miss entirely.
    pub fn scan_source(&self, path: &str, source: &str) -> BrandAbsenceReport {
        let hits = self.collect_hits(path, &blank_cfg_test_modules(source));
        self.finish(hits)
    }

    /// Walks every `.rs` file under `root` and scans it. Paths are reported
    /// relative to `root`'s parent-of-`src` view, i.e. as they appear in the
    /// ledger.
    pub fn scan_tree(&self, repo_root: &Path) -> BrandAbsenceReport {
        let mut hits = Vec::new();
        let mut files = Vec::new();
        collect_rs_files(&repo_root.join("src"), &mut files);
        files.sort();
        for file in files {
            let rel = file
                .strip_prefix(repo_root)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            if rel == VOCABULARY_DEFINITION_PATH {
                continue;
            }
            let Ok(body) = std::fs::read_to_string(&file) else {
                continue;
            };
            hits.extend(self.collect_hits(&rel, &blank_cfg_test_modules(&body)));
        }
        self.finish(hits)
    }

    /// Every hit in the file, before the ledger is applied.
    fn collect_hits(&self, path: &str, source: &str) -> Vec<BrandViolation> {
        let mut hits = Vec::new();

        for (stamp, count) in hits_for(path) {
            for _ in 0..count {
                hits.push(BrandViolation {
                    path: path.to_string(),
                    line: 0,
                    kind: BrandViolationKind::Name,
                    stamp: stamp.to_string(),
                    snippet: path.to_string(),
                });
            }
        }

        let extracted = extract(source);

        for (line, name) in &extracted.declared_names {
            for (stamp, count) in hits_for(name) {
                for _ in 0..count {
                    hits.push(BrandViolation {
                        path: path.to_string(),
                        line: *line,
                        kind: BrandViolationKind::Name,
                        stamp: stamp.to_string(),
                        snippet: name.clone(),
                    });
                }
            }
        }

        for (line, literal) in &extracted.literals {
            for (stamp, count) in hits_for(literal) {
                for _ in 0..count {
                    hits.push(BrandViolation {
                        path: path.to_string(),
                        line: *line,
                        kind: BrandViolationKind::DisplayString,
                        stamp: stamp.to_string(),
                        snippet: literal.clone(),
                    });
                }
            }
            if self.real_gate_count > 0 {
                for claimed in gate_count_claims(literal) {
                    if claimed != self.real_gate_count {
                        hits.push(BrandViolation {
                            path: path.to_string(),
                            line: *line,
                            kind: BrandViolationKind::GateCountClaim,
                            stamp: claimed.to_string(),
                            snippet: literal.clone(),
                        });
                    }
                }
            }
        }

        hits
    }

    /// Applies the ledger as a per-`(path, stamp)` ceiling and builds the report.
    fn finish(&self, hits: Vec<BrandViolation>) -> BrandAbsenceReport {
        let mut ceilings: HashMap<(String, String), usize> = HashMap::new();
        for entry in self.allowlist {
            *ceilings
                .entry((entry.path.to_string(), normalize_stamp(entry.stamp)))
                .or_insert(0) += entry.occurrences;
        }

        let mut new_violations = Vec::new();
        let mut allowlisted_hits = 0usize;
        for hit in hits {
            let key = (hit.path.clone(), normalize_stamp(&hit.stamp));
            match ceilings.get_mut(&key) {
                Some(remaining) if *remaining > 0 => {
                    *remaining -= 1;
                    allowlisted_hits += 1;
                }
                _ => new_violations.push(hit),
            }
        }

        let allowlisted_debt_total: usize = self.allowlist.iter().map(|e| e.occurrences).sum();
        let summary = format!(
            "brand-absence gate [WARN-ONLY]: {} new violation(s), {} allowlisted hit(s); \
             recorded debt {} occurrence(s) across {} ledger entries; real gate count {}.",
            new_violations.len(),
            allowlisted_hits,
            allowlisted_debt_total,
            self.allowlist.len(),
            self.real_gate_count,
        );

        BrandAbsenceReport {
            new_violations,
            allowlisted_hits,
            allowlisted_debt_total,
            is_blocking: !WARN_ONLY,
            summary,
        }
    }
}

// ---------------------------------------------------------------------------
// Source of truth for the gate count
// ---------------------------------------------------------------------------

const REPORT_SOURCE: &str = include_str!("../pre_merge_guard/report.rs");

/// Counts the `GateStatus` fields declared on `PreMergeCertificationReport`.
///
/// Deliberately the same mechanism as that module's own
/// `all_statuses_covers_every_gate_field` test, so a claim is compared against
/// the corpus rather than against a second constant that can drift the same way
/// the first one did.
fn real_gate_count_from_report_source() -> usize {
    let Some(start) = REPORT_SOURCE.find("pub struct PreMergeCertificationReport") else {
        return 0;
    };
    let body = &REPORT_SOURCE[start..];
    let Some(end) = body.find("\n}\n") else {
        return 0;
    };
    body[..end].matches(": GateStatus,").count()
}

// ---------------------------------------------------------------------------
// Word-level matching
// ---------------------------------------------------------------------------

/// Splits text into lowercase words on non-alphanumeric characters and
/// `camelCase` boundaries. `EnterpriseThroughputOptimizer` and
/// `src/hyperscale_throughput_guard/mod.rs` both become word lists a stamp can
/// be matched against exactly.
fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev_was_lower_or_digit = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && prev_was_lower_or_digit && !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            cur.push(ch.to_ascii_lowercase());
            prev_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            prev_was_lower_or_digit = false;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Ledger key for a stamp: its words, lowercased, separator-independent.
///
/// A ledger entry must spell its stamp the way the file spells it — the test
/// suite verifies every entry against the file's text, so a stale entry cannot
/// hide — while the gate keys on the words, so `cloud_native` in `lib.rs` and
/// `Cloud-Native` in a display string are the same debt.
fn normalize_stamp(stamp: &str) -> String {
    words(stamp).join(" ")
}

/// A word matches a stamp word if it is equal, or is its plural — so
/// `Hyperscalers` in a vendor roll-call is caught by the stamp `hyperscaler`.
fn word_matches(word: &str, stamp_word: &str) -> bool {
    word == stamp_word
        || (word.len() == stamp_word.len() + 1 && word.strip_suffix('s') == Some(stamp_word))
}

/// Every forbidden term that occurs in `text`, with the number of occurrences:
/// aspiration stamps by word match, plus one entry for a vendor roll-call.
fn hits_for(text: &str) -> Vec<(&'static str, usize)> {
    let mut hits = stamp_hits(text);
    if let Some(first) = vendor_roll_call(text) {
        hits.push((first, 1));
    }
    hits
}

/// Every stamp that occurs in `text`, with the number of occurrences.
fn stamp_hits(text: &str) -> Vec<(&'static str, usize)> {
    let text_words = words(text);
    if text_words.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for stamp in FORBIDDEN_STAMPS {
        let stamp_words = stamp_words(stamp);
        if stamp_words.is_empty() || stamp_words.len() > text_words.len() {
            continue;
        }
        let count = text_words
            .windows(stamp_words.len())
            .filter(|window| {
                window
                    .iter()
                    .zip(stamp_words.iter())
                    .all(|(w, s)| word_matches(w, s))
            })
            .count();
        if count > 0 {
            out.push((*stamp, count));
        }
    }
    out
}

/// The first vendor of a roll-call in `text`, if `text` names at least
/// [`VENDOR_ROLL_CALL_THRESHOLD`] distinct vendors.
fn vendor_roll_call(text: &str) -> Option<&'static str> {
    let text_words = words(text);
    let present: Vec<&'static str> = VENDOR_BRANDS
        .iter()
        .filter(|brand| text_words.iter().any(|w| word_matches(w, brand)))
        .copied()
        .collect();
    if present.len() >= VENDOR_ROLL_CALL_THRESHOLD {
        present.first().copied()
    } else {
        None
    }
}

/// Word split of a stamp, computed once per stamp.
fn stamp_words(stamp: &'static str) -> &'static [String] {
    static CACHE: OnceLock<Vec<Vec<String>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| FORBIDDEN_STAMPS.iter().map(|s| words(s)).collect());
    let idx = FORBIDDEN_STAMPS
        .iter()
        .position(|s| std::ptr::eq(*s, stamp))
        .or_else(|| FORBIDDEN_STAMPS.iter().position(|s| *s == stamp))
        .unwrap_or(usize::MAX);
    cache.get(idx).map(Vec::as_slice).unwrap_or(&[])
}

/// Hardcoded counts claimed against the word "gate"/"gates": `70-Gate`,
/// `70 gates`, `70_gate`. A count in a PR-visible string is a claim, and a
/// claim is checked against the corpus.
///
/// The number must be directly adjacent to the word, at most one `-`, `_` or
/// space between them, and must itself start a word. Without that adjacency
/// rule the scanner reads `ADR-0710 D-1 Gate` as a claim of one gate and
/// `rgba(0,0,0,0.7); .gate-cell` as a claim of seven, and a check that fires on
/// identifiers and CSS is a check that gets switched off.
fn gate_count_claims(text: &str) -> Vec<usize> {
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = text.chars().map(|c| c.to_ascii_lowercase()).collect();
    let word = ['g', 'a', 't', 'e'];
    let mut out = Vec::new();

    for i in 0..lower.len() {
        if !lower[i..].starts_with(&word) {
            continue;
        }
        // "gate" must be a whole word: nothing wordy before it, and only an
        // optional plural "s" after it.
        if i > 0 && is_word_char(chars[i - 1]) && chars[i - 1] != '-' && chars[i - 1] != '_' {
            continue;
        }
        let mut after = i + word.len();
        if lower.get(after) == Some(&'s') {
            after += 1;
        }
        if lower
            .get(after)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
        {
            continue;
        }

        // Walk back over at most one separator, then over the digits.
        let mut j = i;
        if j > 0 && matches!(chars[j - 1], ' ' | '-' | '_') {
            j -= 1;
        }
        let digits_end = j;
        while j > 0 && chars[j - 1].is_ascii_digit() {
            j -= 1;
        }
        if j == digits_end {
            continue;
        }
        // The digits must start a word: `D-1 Gate` is a gate's designation, not
        // a count of gates.
        if j > 0 && (is_word_char(chars[j - 1]) || chars[j - 1] == '.') {
            continue;
        }
        let claimed: String = chars[j..digits_end].iter().collect();
        if let Ok(n) = claimed.parse::<usize>() {
            out.push(n);
        }
    }
    out
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

// ---------------------------------------------------------------------------
// Rust source extraction
// ---------------------------------------------------------------------------

/// Declared item names and string literals, with line numbers.
#[derive(Debug, Default)]
struct Extracted {
    declared_names: Vec<(usize, String)>,
    literals: Vec<(usize, String)>,
}

const DECL_KEYWORDS: &[&str] = &[
    "struct", "enum", "trait", "union", "type", "fn", "mod", "const", "static",
];

/// Splits a Rust source file into string-literal contents and code, so a name
/// check never fires on prose in a comment and a display-string check never
/// fires on an identifier.
fn extract(source: &str) -> Extracted {
    let mut out = Extracted::default();
    let chars: Vec<char> = source.chars().collect();
    let mut code = String::with_capacity(source.len());
    let mut line = 1usize;
    let mut i = 0usize;

    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();

        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && next == Some('*') {
            let mut depth = 1usize;
            i += 2;
            while i < chars.len() && depth > 0 {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                } else {
                    if chars[i] == '\n' {
                        line += 1;
                        code.push('\n');
                    }
                    i += 1;
                }
            }
            continue;
        }
        if c == '\'' {
            // A char literal, or a lifetime. Only a char literal is skipped;
            // mistaking a lifetime for one would swallow the code after it.
            let is_char_literal =
                next == Some('\\') || (next.is_some() && chars.get(i + 2) == Some(&'\''));
            if is_char_literal {
                i += 1;
                while i < chars.len() {
                    if chars[i] == '\\' {
                        i += 2;
                        continue;
                    }
                    if chars[i] == '\'' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                code.push(' ');
                continue;
            }
        }
        if c == 'r' || c == 'b' {
            // Raw string: r"..", r#".."#, br#".."#. Only when not part of an
            // identifier.
            let prev_is_ident =
                i > 0 && (chars[i - 1].is_ascii_alphanumeric() || chars[i - 1] == '_');
            if !prev_is_ident {
                let mut j = i;
                if chars[j] == 'b' {
                    j += 1;
                }
                if chars.get(j) == Some(&'r') {
                    j += 1;
                    let hash_start = j;
                    while chars.get(j) == Some(&'#') {
                        j += 1;
                    }
                    let hashes = j - hash_start;
                    if chars.get(j) == Some(&'"') {
                        j += 1;
                        let start_line = line;
                        let mut content = String::new();
                        let terminator = format!("\"{}", "#".repeat(hashes));
                        let term: Vec<char> = terminator.chars().collect();
                        while j < chars.len() {
                            if chars[j..].starts_with(term.as_slice()) {
                                j += term.len();
                                break;
                            }
                            if chars[j] == '\n' {
                                line += 1;
                            }
                            content.push(chars[j]);
                            j += 1;
                        }
                        out.literals.push((start_line, content));
                        code.push(' ');
                        i = j;
                        continue;
                    }
                }
            }
        }
        if c == '"' {
            let start_line = line;
            let mut content = String::new();
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' {
                    // Keep the escaped character's text out of the word split;
                    // an escape cannot form part of a stamp.
                    i += 2;
                    content.push(' ');
                    continue;
                }
                if chars[i] == '"' {
                    i += 1;
                    break;
                }
                if chars[i] == '\n' {
                    line += 1;
                }
                content.push(chars[i]);
                i += 1;
            }
            out.literals.push((start_line, content));
            code.push(' ');
            continue;
        }
        if c == '\n' {
            line += 1;
        }
        code.push(c);
        i += 1;
    }

    out.declared_names = declared_names(&code);
    out
}

/// `struct Foo` / `fn foo` / `mod foo` ... in comment- and literal-free code.
fn declared_names(code: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (idx, text) in code.lines().enumerate() {
        let tokens = text
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|t| !t.is_empty());
        let mut prev: Option<&str> = None;
        for token in tokens {
            if let Some(kw) = prev
                && DECL_KEYWORDS.contains(&kw)
                && !DECL_KEYWORDS.contains(&token)
            {
                out.push((idx + 1, token.to_string()));
            }
            prev = Some(token);
        }
    }
    out
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

// ---------------------------------------------------------------------------
// The debt ledger
// ---------------------------------------------------------------------------

/// Pre-existing violations, enumerated. Generated by `scan_tree` over `src/`
/// and verified by `tests/brand_absence_gate_test.rs`, which asserts every path
/// exists and every stamp still occurs in it.
///
/// This is debt, not an exemption. Each entry's `occurrences` is a ceiling.
pub static KNOWN_VIOLATIONS: &[AllowlistedDebt] = &[
    AllowlistedDebt {
        path: "src/cloud_native_guard/mod.rs",
        stamp: "cloud_native",
        occurrences: 7,
        debt_note: "pre-existing name + display string; renaming is sequenced after the retain/discard determination (plan 36.2)",
    },
    AllowlistedDebt {
        path: "src/doc_archival_sweeper/issue_doc_consolidator.rs",
        stamp: "hyperscaler",
        occurrences: 1,
        debt_note: "pre-existing display string; renaming is sequenced after the retain/discard determination (plan 36.2)",
    },
    AllowlistedDebt {
        path: "src/doc_archival_sweeper/mod.rs",
        stamp: "hyperscaler",
        occurrences: 1,
        debt_note: "pre-existing display string; renaming is sequenced after the retain/discard determination (plan 36.2)",
    },
    AllowlistedDebt {
        path: "src/doc_guard/frontmatter.rs",
        stamp: "hyperscaler",
        occurrences: 1,
        debt_note: "pre-existing display string; renaming is sequenced after the retain/discard determination (plan 36.2)",
    },
    AllowlistedDebt {
        path: "src/doc_guard/mod.rs",
        stamp: "hyperscaler",
        occurrences: 2,
        debt_note: "pre-existing display string; renaming is sequenced after the retain/discard determination (plan 36.2)",
    },
    AllowlistedDebt {
        path: "src/hyperscaler_consensus_guard/mod.rs",
        stamp: "aws",
        occurrences: 2,
        debt_note: "pre-existing display string; renaming is sequenced after the retain/discard determination (plan 36.2)",
    },
    AllowlistedDebt {
        path: "src/hyperscaler_consensus_guard/mod.rs",
        stamp: "hyperscaler",
        occurrences: 11,
        debt_note: "pre-existing name + display string; renaming is sequenced after the retain/discard determination (plan 36.2)",
    },
    AllowlistedDebt {
        path: "src/lib.rs",
        stamp: "cloud_native",
        occurrences: 1,
        debt_note: "pre-existing name; renaming is sequenced after the retain/discard determination (plan 36.2)",
    },
    AllowlistedDebt {
        path: "src/lib.rs",
        stamp: "hyperscaler",
        occurrences: 1,
        debt_note: "pre-existing name; renaming is sequenced after the retain/discard determination (plan 36.2)",
    },
    AllowlistedDebt {
        path: "src/monorepo_guard/mod.rs",
        stamp: "hyperscaler",
        occurrences: 2,
        debt_note: "pre-existing display string; renaming is sequenced after the retain/discard determination (plan 36.2)",
    },
    AllowlistedDebt {
        path: "src/pre_merge_guard/evaluator.rs",
        stamp: "70",
        occurrences: 1,
        debt_note: "pre-existing gate-count claim of 70 against a real corpus of 68; the correction is sequenced with the retain/discard determination (plan 36.2)",
    },
    AllowlistedDebt {
        path: "src/pre_merge_guard/evaluator.rs",
        stamp: "hyperscale",
        occurrences: 1,
        debt_note: "pre-existing display string; renaming is sequenced after the retain/discard determination (plan 36.2)",
    },
    AllowlistedDebt {
        path: "src/stack_whitelist_guard/mod.rs",
        stamp: "hyperscaler",
        occurrences: 1,
        debt_note: "pre-existing display string; renaming is sequenced after the retain/discard determination (plan 36.2)",
    },
    AllowlistedDebt {
        path: "src/webhook/pipelines/review.rs",
        stamp: "70",
        occurrences: 2,
        debt_note: "pre-existing gate-count claim of 70 against a real corpus of 68; the correction is sequenced with the retain/discard determination (plan 36.2)",
    },
    AllowlistedDebt {
        path: "src/webhook/pipelines/review.rs",
        stamp: "hyperscale",
        occurrences: 1,
        debt_note: "pre-existing display string; renaming is sequenced after the retain/discard determination (plan 36.2)",
    },
];

/// Blanks out `#[cfg(test)]` modules, preserving line numbering.
///
/// Test text never reaches a pull request. Counting a stamp that lives only in
/// a fixture does two kinds of damage: it inflates the debt ledger, and it lets
/// a real production violation hide beneath a ceiling that test data paid for.
///
/// Lines are replaced rather than removed so every reported line number still
/// points at the right line of the original file.
fn blank_cfg_test_modules(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut depth: i32 = 0;
    let mut in_test = false;
    let mut pending = false;

    for line in source.lines() {
        let trimmed = line.trim_start();

        if !in_test && trimmed.starts_with("#[cfg(test)]") {
            pending = true;
            out.push('\n');
            continue;
        }

        if pending && trimmed.starts_with("mod ") {
            in_test = true;
            pending = false;
            depth = line.matches('{').count() as i32 - line.matches('}').count() as i32;
            out.push('\n');
            continue;
        }
        // An attribute on something that is not a module: not a test module.
        if pending && !trimmed.is_empty() {
            pending = false;
        }

        if in_test {
            depth += line.matches('{').count() as i32;
            depth -= line.matches('}').count() as i32;
            if depth <= 0 {
                in_test = false;
            }
            out.push('\n');
            continue;
        }

        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regenerates the ledger body. Run with:
    /// `cargo test -p anvil brand_absence::tests::print_ledger -- --ignored --nocapture`
    #[test]
    #[ignore = "generator: prints the KNOWN_VIOLATIONS body for this file"]
    fn print_ledger() {
        let gate = BrandAbsenceGate::with_allowlist(&[]);
        let report = gate.scan_tree(Path::new(env!("CARGO_MANIFEST_DIR")));
        let mut counts: std::collections::BTreeMap<(String, String, BrandViolationKind), usize> =
            std::collections::BTreeMap::new();
        for v in &report.new_violations {
            *counts
                .entry((v.path.clone(), v.stamp.clone(), v.kind))
                .or_insert(0) += 1;
        }
        let mut merged: std::collections::BTreeMap<(String, String), (usize, Vec<String>)> =
            std::collections::BTreeMap::new();
        for ((path, stamp, kind), n) in counts {
            let e = merged.entry((path, stamp)).or_insert((0, Vec::new()));
            e.0 += n;
            let label = match kind {
                BrandViolationKind::Name => "name",
                BrandViolationKind::DisplayString => "display string",
                BrandViolationKind::GateCountClaim => "gate-count claim",
            };
            if !e.1.iter().any(|l| l == label) {
                e.1.push(label.to_string());
            }
        }
        for ((path, stamp), (n, kinds)) in &merged {
            let body = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
                .unwrap_or_default()
                .to_lowercase();
            let stamp = [
                stamp.clone(),
                stamp.replace(' ', "_"),
                stamp.replace(' ', "-"),
                stamp.replace(' ', ""),
            ]
            .into_iter()
            .find(|candidate| body.contains(candidate))
            .unwrap_or_else(|| stamp.clone());
            println!(
                "    AllowlistedDebt {{ path: {path:?}, stamp: {stamp:?}, occurrences: {n}, debt_note: \"pre-existing {}; rename deferred to the retain/discard determination (plan 36.2)\" }},",
                kinds.join(" + ")
            );
        }
        println!("// entries: {}", merged.len());
        println!("{}", report.summary);
    }

    /// Prints the gate's current verdict over `src/`. Not asserted: the tree is
    /// being edited by other lanes, and this gate is warn-only by design, so a
    /// new violation must show up in the report rather than break the build.
    #[test]
    #[ignore = "reporter: prints the warn-only verdict for src/"]
    fn print_tree_status() {
        let report = BrandAbsenceGate::new().scan_tree(Path::new(env!("CARGO_MANIFEST_DIR")));
        for v in &report.new_violations {
            println!(
                "{}:{} {:?} [{}] {}",
                v.path, v.line, v.kind, v.stamp, v.snippet
            );
        }
        println!("{}", report.summary);
    }

    #[test]
    fn real_gate_count_reads_the_corpus() {
        // Pinned to the corpus constant rather than a literal. This test
        // previously hardcoded 68, which is exactly how seven PR-visible
        // strings came to claim 70 against a corpus of 68.
        assert_eq!(
            BrandAbsenceGate::new().real_gate_count(),
            crate::pre_merge_guard::report::TOTAL_GATES
        );
    }

    #[test]
    fn ledger_has_no_duplicate_keys() {
        let mut keys: Vec<(&str, &str)> =
            KNOWN_VIOLATIONS.iter().map(|e| (e.path, e.stamp)).collect();
        let before = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(before, keys.len());
    }
}
