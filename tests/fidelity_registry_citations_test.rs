#![allow(clippy::needless_range_loop, clippy::manual_is_multiple_of)]
//! The fidelity registry is *published output*, not a comment.
//!
//! `src/fidelity/registry.rs` renders onto the PR scorecard and the dashboard.
//! Every `gap` string in it makes a factual claim about this repository's own
//! source: it quotes a constant (`hit_rate_pct: 95.0`), a string literal
//! (`"replicas: 3"`), an identifier (`simulated_burn_rate_1h`) and pins it to a
//! `file.rs:line` citation.
//!
//! When the code being described is deleted or moved, nothing in the compiler
//! notices. `gap` is a `&'static str`; the constant it quotes was a `f64` in a
//! different crate module. Deleting the constant compiles cleanly and the
//! registry keeps publishing the quote. The result is the *same* dishonesty
//! class the registry was written to expose: a confident claim with nothing
//! behind it.
//!
//! # Why prompting cannot prevent this
//!
//! The failure is not a lapse of care at authoring time -- the quotes were
//! accurate when written. It is a *decoupling*: aspiration-text and the code it
//! describes live in different files with no reference between them, so drift
//! is invisible and requires no mistake to occur. That is precisely the root
//! cause `src/fidelity/mod.rs` names in its own module docs ("Aspiration and
//! reality lived in the same place ... so drift between them was invisible and
//! required no lie to occur"). An instruction to "keep the registry accurate"
//! binds only the person editing the registry; the person who *deletes the
//! constant* never opens registry.rs, and no prompt reaches them. Only a
//! mechanical check that re-derives the claim from the source can close it.
//!
//! # Why these tests are general, not a checklist
//!
//! They parse the registry at runtime and derive the claims. A hardcoded list
//! of the four known-stale quotes would go stale itself the moment a twentieth
//! gate is audited. Everything below scans `AUDITED_GATES` and re-verifies
//! every citation it finds, so a quote added tomorrow is covered on the day it
//! is added.

use anvil::fidelity::registry::AUDITED_GATES;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// How far from a cited line the quoted evidence may sit before the citation is
/// considered stale. Small on purpose: a citation that is off by more than a
/// couple of lines is not pointing a reader at anything.
const ANCHOR_TOLERANCE: usize = 2;

// ---------------------------------------------------------------------------
// Source tree access
// ---------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn all_source_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_rs_files(&repo_root().join("src"), &mut out);
    out.sort();
    out
}

/// Resolves a citation fragment (`kani_guard/proof_runner.rs`, `cache_keys.rs`)
/// to exactly one file under `src/`. Ambiguity is an error, not a guess.
fn resolve(fragment: &str, files: &[PathBuf]) -> Result<PathBuf, String> {
    let root = repo_root();
    let exact = root.join("src").join(fragment);
    if exact.is_file() {
        return Ok(exact);
    }
    let suffix = format!("/{}", fragment);
    let matches: Vec<&PathBuf> = files
        .iter()
        .filter(|p| p.to_string_lossy().ends_with(&suffix))
        .collect();
    match matches.len() {
        0 => Err(format!("no file under src/ matches `{}`", fragment)),
        1 => Ok(matches[0].clone()),
        n => Err(format!(
            "`{}` is ambiguous: {} files under src/ match it",
            fragment, n
        )),
    }
}

// ---------------------------------------------------------------------------
// Citation parsing:  path.rs:N   path.rs:N-M   path.rs:N,M
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Citation {
    /// The exact text as it appears in the gap, e.g. `coverage_guard.rs:135-141`.
    raw: String,
    /// The path fragment, e.g. `coverage_guard.rs`.
    fragment: String,
    /// Inclusive line spans. `N` becomes `(N, N)`; `N-M` becomes `(N, M)`;
    /// `N,M` becomes two spans.
    spans: Vec<(usize, usize)>,
    /// The symbol a `path.rs::name` citation points at.
    ///
    /// Line numbers rot. Every edit above a cited line invalidates it and
    /// moving code invalidates all of them at once -- that happened four times
    /// in a single day of gate work, each repair a hand-edit of numbers nobody
    /// can verify by reading. A symbol survives any edit that does not delete
    /// the thing it names.
    ///
    /// The parser used to walk over `path.rs::name` SILENTLY: it matched the
    /// `.rs:` prefix, found no digits after it, and dropped the citation. So
    /// writing one was not an error, it was invisible -- worse than the rot it
    /// was meant to replace.
    symbol: Option<String>,
    /// Byte range of `raw` inside the gap string, so it can be blanked out
    /// before identifier/number extraction (a line number is not a constant).
    at: (usize, usize),
}

fn is_path_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'/' || b == b'-'
}

/// Keywords that introduce a definition a citation can point at.
const DEF_KEYWORDS: &[&str] = &[
    "fn ", "struct ", "enum ", "const ", "static ", "trait ", "type ",
];

/// Everything a definition may carry before its keyword.
///
/// Order does not matter; the check is a prefix match against each. `async`
/// and the restricted visibilities are the ones whose absence made the
/// resolver blind to a large fraction of the tree.
const DEF_PREFIXES: &[&str] = &[
    "",
    "pub ",
    "pub(crate) ",
    "pub(super) ",
    "async ",
    "pub async ",
    "pub(crate) async ",
    "pub(super) async ",
    "unsafe ",
    "pub unsafe ",
];

/// The lines of `sym`'s definition, located rather than counted.
///
/// This is the whole difference from a line citation. The window is FOUND by
/// searching for the definition, so an edit above it moves the definition and
/// the window moves with it. A line span cannot do that: it is a number written
/// down once and invalidated by the next insertion above it.
///
/// Deliberately syntactic and deliberately strict about the start: it looks for
/// a definition keyword immediately before the name, so a symbol merely
/// MENTIONED in a comment does not satisfy a citation.
///
/// The end is the first line at or below the definition's indent that closes a
/// block, or the end of the file. Crude, and adequate -- the window only has to
/// contain the definition's own body for the quoted evidence to be found in it.
fn symbol_window(lines: &[&str], sym: &str) -> Option<(usize, usize)> {
    let a = lines.iter().position(|l| {
        let t = l.trim_start();
        // Every prefix a definition can carry before its keyword. Without
        // `async` and the restricted visibilities, `symbol_window` reported
        // "no definition found" for `async fn run_cargo_mutants` sitting in
        // plain sight -- and a resolver that cannot see half the functions in
        // the tree cannot replace a line number.
        DEF_KEYWORDS.iter().any(|kw| {
            DEF_PREFIXES
                .iter()
                .any(|pre| t.starts_with(&format!("{pre}{kw}{sym}")))
        })
            // A struct field is a definition a gap can legitimately cite, and
            // the visibility prefix is part of how it is spelled. Without these
            // two, `mod.rs::policy_files_seen` reported "no definition found"
            // for a field sitting in plain sight.
            || t.starts_with(&format!("{sym}:"))
            || t.starts_with(&format!("pub {sym}:"))
            || t.starts_with(&format!("pub(crate) {sym}:"))
    })?;
    // A doc comment and its attributes are part of the definition. Without
    // this, a gap that quotes the sentence explaining WHY a function is
    // written the way it is cites a symbol whose window starts below the
    // sentence, and the citation cannot resolve to the thing it is about.
    let mut top = a;
    while top > 0 {
        let prev = lines[top - 1].trim_start();
        if prev.starts_with("///") || prev.starts_with("#[") || prev.starts_with("//!") {
            top -= 1;
        } else {
            break;
        }
    }
    let indent = lines[a].len() - lines[a].trim_start().len();
    let z = lines[a + 1..]
        .iter()
        .position(|l| {
            !l.trim().is_empty()
                && l.len() - l.trim_start().len() <= indent
                && l.trim_start().starts_with('}')
        })
        .map(|k| a + 1 + k + 1)
        .unwrap_or(lines.len());
    Some((top, z.min(lines.len())))
}

/// Whether `text` defines `sym`.
fn defines(text: &str, sym: &str) -> bool {
    symbol_window(&text.lines().collect::<Vec<_>>(), sym).is_some()
}

fn parse_citations(gap: &str) -> Vec<Citation> {
    let b = gap.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 4 <= b.len() {
        if &b[i..i + 4] != b".rs:" {
            i += 1;
            continue;
        }
        // Walk backwards over the path.
        let mut start = i;
        while start > 0 && is_path_byte(b[start - 1]) {
            start -= 1;
        }
        let fragment = gap[start..i + 3].to_string();

        // Symbol form: `path.rs::name`.
        if b.get(i + 4) == Some(&b':') {
            let ss = i + 5;
            let mut se = ss;
            while se < b.len() && (b[se].is_ascii_alphanumeric() || b[se] == b'_') {
                se += 1;
            }
            if se > ss {
                out.push(Citation {
                    raw: gap[start..se].to_string(),
                    fragment,
                    spans: Vec::new(),
                    symbol: Some(gap[ss..se].to_string()),
                    at: (start, se),
                });
                i = se;
                continue;
            }
        }

        // Parse the line list that follows the colon.
        let mut j = i + 4;
        let mut spans: Vec<(usize, usize)> = Vec::new();
        loop {
            let ns = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j == ns {
                break;
            }
            let first: usize = gap[ns..j].parse().expect("digits parse");
            // Range?
            if j < b.len() && b[j] == b'-' && j + 1 < b.len() && b[j + 1].is_ascii_digit() {
                let rs = j + 1;
                j = rs;
                while j < b.len() && b[j].is_ascii_digit() {
                    j += 1;
                }
                let second: usize = gap[rs..j].parse().expect("digits parse");
                spans.push((first.min(second), first.max(second)));
            } else {
                spans.push((first, first));
            }
            // Another line in the same citation?
            if j < b.len() && b[j] == b',' && j + 1 < b.len() && b[j + 1].is_ascii_digit() {
                j += 1;
                continue;
            }
            break;
        }

        if spans.is_empty() || fragment.len() <= 3 {
            i += 4;
            continue;
        }
        out.push(Citation {
            raw: gap[start..j].to_string(),
            fragment,
            spans,
            symbol: None,
            at: (start, j),
        });
        i = j.max(i + 4);
    }
    out
}

/// Replaces every citation with spaces so the identifier/number scanners do not
/// mistake `mod.rs:63` for a quoted constant `63`.
fn blank_citations(gap: &str, citations: &[Citation]) -> String {
    let mut bytes = gap.as_bytes().to_vec();
    for c in citations {
        #[allow(clippy::needless_range_loop)]
        for k in c.at.0..c.at.1 {
            bytes[k] = b' ';
        }
    }
    String::from_utf8(bytes).expect("blanking kept ASCII boundaries")
}

/// Blanks bare filename mentions (`a file named smt_solver.rs whose ...`) that
/// carry no `:line`. A filename is a citation, not a quoted code symbol, so it
/// must not be scanned for identifiers -- otherwise `smt_solver.rs` would be
/// read as a claim that the identifier `smt_solver` exists in the source.
fn blank_rs_paths(text: &str) -> String {
    let b = text.as_bytes();
    let mut bytes = b.to_vec();
    let mut i = 0;
    while i + 3 <= b.len() {
        if &b[i..i + 3] != b".rs" {
            i += 1;
            continue;
        }
        let mut start = i;
        while start > 0 && is_path_byte(b[start - 1]) {
            start -= 1;
        }
        for k in start..i + 3 {
            bytes[k] = b' ';
        }
        i += 3;
    }
    String::from_utf8(bytes).expect("blanking kept ASCII boundaries")
}

// ---------------------------------------------------------------------------
// Evidence-token extraction
// ---------------------------------------------------------------------------

/// Pulls out the spans delimited by `delim`, returning (contents, text with
/// those spans blanked). Odd delimiter counts are reported by the caller.
fn take_delimited(text: &str, delim: char) -> (Vec<String>, String, bool) {
    let positions: Vec<usize> = text
        .char_indices()
        .filter(|(_, c)| *c == delim)
        .map(|(i, _)| i)
        .collect();
    let balanced = positions.len() % 2 == 0;
    let mut contents = Vec::new();
    let mut bytes = text.as_bytes().to_vec();
    for pair in positions.chunks(2) {
        if pair.len() < 2 {
            break;
        }
        let (a, z) = (pair[0], pair[1]);
        contents.push(text[a + 1..z].to_string());
        for k in a..=z {
            bytes[k] = b' ';
        }
    }
    (
        contents,
        String::from_utf8(bytes).expect("blanking kept ASCII boundaries"),
        balanced,
    )
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Identifiers the registry quotes as belonging to the code: anything with an
/// underscore, which covers `hit_rate_pct`, `simulated_burn_rate_1h` and
/// `VERIFIED_STATIC` while excluding ordinary English prose.
fn code_identifiers(text: &str) -> Vec<String> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if !is_ident_byte(b[i]) {
            i += 1;
            continue;
        }
        let s = i;
        while i < b.len() && is_ident_byte(b[i]) {
            i += 1;
        }
        let word = &text[s..i];
        if word.contains('_') && word.len() >= 6 {
            out.push(word.to_string());
        }
    }
    out
}

/// Numeric literals standing alone in the prose -- `95.0`, `142`, `0.4`.
/// Digits glued to letters (`FNV-1a`, `0a`) are not literals and are skipped.
fn numeric_literals(text: &str) -> Vec<String> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let s = i;
        while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
            i += 1;
        }
        let mut e = i;
        while e > s && b[e - 1] == b'.' {
            e -= 1; // trailing sentence period is not part of the number
        }
        let before_ok =
            s == 0 || !(is_ident_byte(b[s - 1]) || b[s - 1] == b'.' || b[s - 1] == b'-');
        let after_ok = e >= b.len() || !is_ident_byte(b[e]);
        if before_ok && after_ok && e > s {
            out.push(text[s..e].to_string());
        }
    }
    out
}

/// `ident: 95.0` / `ident = 1.02` -- a field-and-value quotation. Both halves
/// are treated as strong claims about the source.
fn field_value_pairs(text: &str) -> Vec<String> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if !is_ident_byte(b[i]) {
            i += 1;
            continue;
        }
        let s = i;
        while i < b.len() && is_ident_byte(b[i]) {
            i += 1;
        }
        let ident = &text[s..i];
        if !(ident.contains('_') && ident.len() >= 6) {
            continue;
        }
        let mut j = i;
        while j < b.len() && b[j] == b' ' {
            j += 1;
        }
        if j >= b.len() || (b[j] != b':' && b[j] != b'=') {
            continue;
        }
        j += 1;
        while j < b.len() && b[j] == b' ' {
            j += 1;
        }
        let ns = j;
        while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'.') {
            j += 1;
        }
        let mut e = j;
        while e > ns && b[e - 1] == b'.' {
            e -= 1;
        }
        if e > ns {
            out.push(ident.to_string());
            out.push(text[ns..e].to_string());
        }
    }
    out
}

#[derive(Debug)]
struct GapClaims {
    /// Verbatim quotations: `"..."` strings, `` `...` `` snippets, quoted
    /// identifiers, and both halves of every `field: value` pair. Each must
    /// exist somewhere in a file the gap cites.
    strong: Vec<String>,
    /// Everything usable to anchor a citation to specific lines: `strong`
    /// plus bare numeric literals.
    anchors: Vec<String>,
    citations: Vec<Citation>,
    unbalanced_quotes: bool,
}

fn claims_of(gap: &str) -> GapClaims {
    let citations = parse_citations(gap);
    let text = blank_rs_paths(&blank_citations(gap, &citations));
    let (quoted, text, q_balanced) = take_delimited(&text, '"');
    let (ticked, text, t_balanced) = take_delimited(&text, '`');

    let mut strong: Vec<String> = Vec::new();
    strong.extend(quoted);
    strong.extend(ticked);
    strong.extend(field_value_pairs(&text));
    strong.extend(code_identifiers(&text));
    strong.retain(|s| !s.trim().is_empty());

    let mut seen = BTreeSet::new();
    strong.retain(|s| seen.insert(s.clone()));

    let mut anchors = strong.clone();
    for n in numeric_literals(&text) {
        if seen.insert(n.clone()) {
            anchors.push(n);
        }
    }

    GapClaims {
        strong,
        anchors,
        citations,
        unbalanced_quotes: !(q_balanced && t_balanced),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Catches: a `file.rs:line` citation naming a file that no longer exists, or a
/// line past the end of the file it names.
///
/// This is the cheapest half of the defect -- a file that was renamed or a
/// range that outlived a shrinking file. Prompting does not prevent it because
/// the person renaming the file has no reason to open `registry.rs`, and
/// `cargo build` proves nothing: the citation lives inside a `&'static str`.
#[test]
fn every_cited_file_exists_and_every_cited_line_is_within_it() {
    let files = all_source_files();
    let mut failures: Vec<String> = Vec::new();

    for entry in AUDITED_GATES {
        for c in parse_citations(entry.gap) {
            let path = match resolve(&c.fragment, &files) {
                Ok(p) => p,
                Err(e) => {
                    failures.push(format!("{}: cites `{}` -- {}", entry.gate_id, c.raw, e));
                    continue;
                }
            };
            let text = std::fs::read_to_string(&path).expect("cited file is readable");

            // A symbol citation names a definition, and the check is that the
            // definition is there. Nothing about it can go stale under an edit
            // elsewhere in the file, which is the entire reason for the form.
            if let Some(sym) = &c.symbol {
                if !defines(&text, sym) {
                    failures.push(format!(
                        "{}: cites `{}` but {} defines no `{sym}`",
                        entry.gate_id,
                        c.raw,
                        path.display()
                    ));
                }
                continue;
            }

            let total = text.lines().count();
            for (a, z) in &c.spans {
                if *a == 0 || *z > total {
                    failures.push(format!(
                        "{}: cites `{}` but {} has only {} lines",
                        entry.gate_id,
                        c.raw,
                        path.display(),
                        total
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "registry citations point at files/lines that do not exist:\n  {}",
        failures.join("\n  ")
    );
}

/// Catches: the headline defect -- the registry quotes a constant that was
/// DELETED from the code, and publishes it to the PR scorecard as a statement
/// about the code.
///
/// Every verbatim quotation in a `gap` (a `"string"`, a `` `snippet` ``, an
/// identifier, or either half of a `field: value` pair) must still be findable
/// in at least one file that same gap cites. When the guard was rewritten and
/// the fabricated constant removed, this is the only thing that notices --
/// `gap` is a string literal, so the compiler is silent, and the scorecard
/// keeps publishing a claim that is now false.
///
/// Prompting cannot prevent it: the constant and the sentence quoting it are in
/// different files, edited by different changes, at different times. There is
/// no moment at which an instruction would fire.
#[test]
fn every_constant_quoted_in_a_gap_still_exists_in_a_file_that_gap_cites() {
    let files = all_source_files();
    let mut failures: Vec<String> = Vec::new();

    for entry in AUDITED_GATES {
        let claims = claims_of(entry.gap);
        if claims.unbalanced_quotes {
            failures.push(format!(
                "{}: gap has an odd number of `\"` or '`' delimiters, so its \
                 quotations cannot be checked",
                entry.gate_id
            ));
            continue;
        }
        if claims.citations.is_empty() {
            continue; // a gap that cites no file makes no file-specific claim
        }

        let mut bodies: Vec<(PathBuf, String)> = Vec::new();
        for c in &claims.citations {
            if let Ok(p) = resolve(&c.fragment, &files) {
                let body = std::fs::read_to_string(&p).expect("cited file is readable");
                bodies.push((p, code_only_body(&body)));
            }
        }
        if bodies.is_empty() {
            continue; // already reported by the file-existence test
        }

        for claim in &claims.strong {
            if bodies.iter().any(|(_, body)| body.contains(claim.as_str())) {
                continue;
            }
            let cited: Vec<String> = claims.citations.iter().map(|c| c.raw.clone()).collect();
            failures.push(format!(
                "{}: gap quotes `{}` but it does not appear in any file it cites ({})",
                entry.gate_id,
                claim,
                cited.join(", ")
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "the registry publishes {} quotation(s) that no longer exist in the code they name:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// Catches: a citation whose line numbers drifted -- the quoted code still
/// exists, but not where the registry says, so a reader following the pointer
/// lands on unrelated code and the evidence looks fabricated.
///
/// For each `file.rs:N-M` the gap gives, at least one thing the gap quotes must
/// actually appear inside those lines (plus a two-line tolerance). This is the
/// check that distinguishes "the constant is gone" from "the constant moved",
/// and it is the only one that fires when a file is edited far above the cited
/// range and every line number below shifts.
///
use anvil::source_scan::without_commentary as code_only_body;

/// The code on a line, with any trailing `//` comment removed.
///
/// A citation must be answerable by the code it names. Matching against the
/// raw line let a gap cite its own commentary: a draft in PR #63 quoted
/// "CVE-NONE" and "symbol_none" against `vex_scanner/mod.rs` and passed,
/// because the same PR had just written those strings into a doc comment two
/// lines above the cited line. The decision they described lived in another
/// file entirely.
///
/// One line as code, commentary gone and string literals kept.
///
/// Delegates to the shared scanner. It returns an owned `String` where this
/// returned a slice, which is why the wrapper stays: the call sites compare
/// trimmed text and a borrow would not outlive the temporary.
fn code_only(line: &str) -> String {
    anvil::source_scan::without_commentary(line)
}

#[test]
fn every_cited_line_range_actually_contains_the_evidence_it_is_cited_for() {
    let files = all_source_files();
    let mut failures: Vec<String> = Vec::new();

    for entry in AUDITED_GATES {
        let claims = claims_of(entry.gap);
        for c in &claims.citations {
            let Ok(path) = resolve(&c.fragment, &files) else {
                continue; // already reported by the file-existence test
            };
            let body = std::fs::read_to_string(&path).expect("cited file is readable");
            let lines: Vec<&str> = body.lines().collect();

            if claims.anchors.is_empty() {
                failures.push(format!(
                    "{}: cites `{}` but quotes nothing verifiable, so the \
                     citation cannot be checked against the code",
                    entry.gate_id, c.raw
                ));
                continue;
            }

            let mut window = String::new();
            if let Some(sym) = &c.symbol {
                match symbol_window(&lines, sym) {
                    Some((a, z)) => {
                        for line in &lines[a..z] {
                            window.push_str(&code_only(line));
                            window.push('\n');
                        }
                    }
                    None => {
                        failures.push(format!(
                            "{}: cites `{}` but no definition of `{sym}` was found in {}",
                            entry.gate_id,
                            c.raw,
                            path.display()
                        ));
                        continue;
                    }
                }
            }
            for (a, z) in &c.spans {
                let lo = a.saturating_sub(1 + ANCHOR_TOLERANCE);
                let hi = (z + ANCHOR_TOLERANCE).min(lines.len());
                for line in &lines[lo.min(lines.len())..hi] {
                    window.push_str(&code_only(line));
                    window.push('\n');
                }
            }

            let hit = claims
                .anchors
                .iter()
                .any(|a| window.contains(a.as_str()) && !a.trim().is_empty());
            if !hit {
                failures.push(format!(
                    "{}: `{}` -- none of the gap's quotations {:?} appear at those \
                     lines (+/-{}) of {}",
                    entry.gate_id,
                    c.raw,
                    claims.anchors,
                    ANCHOR_TOLERANCE,
                    path.display()
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} registry citation(s) point at lines that do not contain the quoted evidence:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

#[test]
fn code_only_strips_commentary_but_keeps_string_literals() {
    assert_eq!(code_only("    let x = 1; // CVE-NONE").trim(), "let x = 1;");
    assert_eq!(code_only("/// quotes \"symbol_none\" in prose").trim(), "");
    assert_eq!(code_only("//! module doc").trim(), "");

    // A comment opener inside a string literal is code, not commentary.
    let url = r#"    let u = "https://example.com/x";"#;
    assert_eq!(code_only(url), url, "a // inside a string is not a comment");

    // The evidence this check exists to protect: a real decision line survives.
    let decision = "        if !source_code.contains(vuln_symbol) {";
    assert_eq!(code_only(decision), decision);
}

// ---------------------------------------------------------------------------
// Symbol anchors
// ---------------------------------------------------------------------------

/// Line-anchored citations still in the registry.
///
/// Zero, and it may never rise. Every citation is now `path.rs::symbol`, whose
/// window is located by searching for the definition rather than counted from
/// the top of the file -- so an edit above it moves the definition and the
/// citation moves with it. A line number cannot do that: it is written down
/// once and invalidated by the next insertion above it, which happened four
/// times in a single day of gate work.
const LINE_CITATION_CEILING: usize = 0;

fn line_citation_count() -> usize {
    AUDITED_GATES
        .iter()
        .flat_map(|e| parse_citations(e.gap))
        .filter(|c| c.symbol.is_none())
        .count()
}

#[test]
fn a_symbol_citation_is_parsed_rather_than_silently_dropped() {
    // The defect that made this worth doing carefully: the old parser matched
    // the `.rs:` prefix, found no digits, and dropped the citation. Writing
    // `foo.rs::bar` was not an error -- it produced NO citation, so nothing was
    // checked and nothing said so.
    let cites = parse_citations("see harness/judgement.rs::scan_for_secrets for the rules");
    assert_eq!(cites.len(), 1, "{cites:?}");
    assert_eq!(cites[0].fragment, "harness/judgement.rs");
    assert_eq!(cites[0].symbol.as_deref(), Some("scan_for_secrets"));
    assert!(cites[0].spans.is_empty());
}

#[test]
fn the_two_forms_still_parse_side_by_side() {
    let cites = parse_citations("a.rs:12 and b.rs::thing and c.rs:3-9 and d.rs:1,2");
    let kinds: Vec<(&str, Option<&str>, usize)> = cites
        .iter()
        .map(|c| (c.fragment.as_str(), c.symbol.as_deref(), c.spans.len()))
        .collect();
    assert_eq!(
        kinds,
        vec![
            ("a.rs", None, 1),
            ("b.rs", Some("thing"), 0),
            ("c.rs", None, 1),
            ("d.rs", None, 2),
        ]
    );
}

#[test]
fn a_symbol_window_moves_with_the_code_and_a_line_span_does_not() {
    // The entire argument for the form, as an assertion rather than a claim.
    let before: Vec<&str> = vec![
        "fn first() {",
        "    let a = 1;",
        "}",
        "fn target() {",
        "    let evidence = 2;",
        "}",
    ];
    let (a, z) = symbol_window(&before, "target").expect("found");
    assert!(before[a..z].iter().any(|l| l.contains("evidence")));

    // Insert twelve lines ABOVE the definition. A line citation of `4` now
    // points at unrelated code; the symbol window still contains the evidence.
    let mut after: Vec<&str> = vec!["// added"; 12];
    after.extend(before.iter().copied());
    let (a2, z2) = symbol_window(&after, "target").expect("still found");
    assert_ne!(a, a2, "the definition moved");
    assert!(
        after[a2..z2].iter().any(|l| l.contains("evidence")),
        "the window followed the definition"
    );
    assert!(
        !after[3..4].iter().any(|l| l.contains("evidence")),
        "the old line number now points at something else"
    );
}

#[test]
fn a_symbol_that_is_only_mentioned_does_not_satisfy_a_citation() {
    // Otherwise a gap could cite a symbol that the file merely talks about,
    // which is the same unearned green as a line span pointing at a comment.
    let text = "// scan_for_secrets is described here\nfn other() {}\n";
    assert!(!defines(text, "scan_for_secrets"));
    assert!(defines(
        "fn scan_for_secrets(d: &str) {}\n",
        "scan_for_secrets"
    ));
    assert!(defines(
        "pub const SECRET_RULES: &[u8] = &[];\n",
        "SECRET_RULES"
    ));
}

#[test]
fn line_anchored_citations_may_fall_but_never_rise() {
    // The scan must still be able to find its subject. With the ceiling at
    // zero, "no line citations" and "the parser stopped working" produce the
    // same number, so the corpus is measured through the other form: if
    // nothing at all parses, this gate did not run.
    let symbols = AUDITED_GATES
        .iter()
        .flat_map(|e| parse_citations(e.gap))
        .filter(|c| c.symbol.is_some())
        .count();
    assert!(
        symbols > 0,
        "no citations were parsed at all, so this gate did not run"
    );

    // Equality rather than `<=`: at a ceiling of zero the two say the same
    // thing about a rise, and `count <= 0` on a `usize` is a comparison clippy
    // correctly calls always-true-or-false.
    let count = line_citation_count();
    assert_eq!(
        count, LINE_CITATION_CEILING,
        "{count} line-anchored citation(s), ceiling is {LINE_CITATION_CEILING}.\n\
         A line number is invalidated by the next insertion above it -- that \
         happened four times in one day. Cite `path.rs::symbol` instead; the \
         window is located rather than counted, so it moves with the code."
    );
}
