use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakingAbiFinding {
    pub file_path: String,
    pub symbol_name: String,
    pub change_kind: String,
    pub detail: String,
}

/// A public function declaration read off one line of a diff.
#[derive(Debug, Clone)]
struct Declaration<'a> {
    path: &'a str,
    /// The declaration with whitespace and the block opener removed, or `None`
    /// when the parameter list does not close on this line.
    ///
    /// `None` is not a failure to parse -- it is the honest answer for a
    /// signature rustfmt has spread over six lines. Comparing a `None` against
    /// anything would report every reflow as a change.
    signature: Option<String>,
}

/// What one pass over a diff found. Counts as well as findings, because the
/// gate publishes what it read, not only what it objected to.
#[derive(Debug, Default)]
pub struct AbiScan {
    pub findings: Vec<BreakingAbiFinding>,
    /// Public function declarations seen on either side of the diff.
    pub declarations_read: usize,
    /// Names declared on both sides more than once, so the removal and the
    /// addition could not be paired -- `new` on two unrelated impls is the
    /// ordinary case. Reported rather than guessed at.
    pub unpaired_names: usize,
    /// Files whose diff touches a `#[repr(...)]` line. The only case where the
    /// memory-layout half of this gate's claim is load-bearing, and the gate
    /// cannot compute a layout from diff text.
    pub layout_files: Vec<String>,
}

/// A public function declaration, without an anchor.
///
/// Restricted visibility is deliberately not matched: `pub(crate)`,
/// `pub(super)` and `pub(in path)` are not a published surface, so `pub` must
/// be followed by whitespace. That also makes a `pub fn` narrowed to
/// `pub(crate) fn` read as a removal, which is what it is.
const PUB_FN_PAT: &str =
    r#"pub\s+(?:(?:const|async|unsafe|extern\s+"[^"]*")\s+)*fn\s+([A-Za-z_][A-Za-z0-9_]*)"#;

/// `pub fn NAME`, anchored at the first non-space character of a diff body line.
///
/// Anchored on purpose. The predicate this replaced asked
/// `diff.contains("-pub fn ")`, which is satisfied by a signature quoted inside
/// a string literal -- the shape of this repository's own scanner fixtures --
/// and reported a removal that never happened.
static PUB_FN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("^{PUB_FN_PAT}")).expect("the pattern is a literal and compiles")
});

/// The same pattern unanchored, used only to ask whether a name is *mentioned*
/// as a declaration somewhere on the added side.
///
/// The anchor alone was not enough. `body.trim_start()` runs before it, so the
/// anchor protects a fixture line only when the opening quote sits on the same
/// source line as the `pub fn` -- the single-line `let diff = "-pub fn ..."`
/// shape. This repository writes its fixtures as multi-line raw strings
/// instead, where the indented body line reaches the anchor as bare `pub fn`.
/// So the anchored form matches the removed half, the added half (rewritten to
/// `"pub async fn handle_event() {",`, which starts with `"`) does not, and the
/// removal is never cleared. `c501d9a` reported `handle_event` removed twice on
/// exactly that shape; the function never existed.
static PUB_FN_MENTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(PUB_FN_PAT).expect("the pattern is a literal and compiles"));

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

    /// Compares the public function declarations a diff removes against the ones
    /// it adds, by name, over the whole diff.
    ///
    /// Whole-diff and not per-file: the public surface of a library is one set,
    /// and a per-file comparison reports every function moved between modules as
    /// a removal. Two rules keep the matcher off ordinary refactors:
    ///
    /// * a removal is reported only when the name is added nowhere in the diff,
    ///   so a move and a reflow both clear it;
    /// * a signature is compared only when the name occurs exactly once removed
    ///   and once added and both lines close their parameter list, so neither an
    ///   unpaired `new` nor a rustfmt reflow can be read as a change.
    ///
    /// Both rules trade recall for precision, and the direction is deliberate: a
    /// matcher that reports every refactor is switched off, which detects less
    /// than one that reports nothing.
    pub fn scan_abi_diff(&self, diff: &str) -> AbiScan {
        let mut removed: BTreeMap<String, Vec<Declaration<'_>>> = BTreeMap::new();
        let mut added: BTreeMap<String, Vec<Declaration<'_>>> = BTreeMap::new();
        let mut scan = AbiScan::default();

        // The path of the file whose body lines are currently being read, taken
        // off the `--- `/`+++ ` header pair rather than searched for in the
        // chunk: a Rust file whose own body names a `.rs` path -- four Markdown
        // files and every diff fixture in this repository do -- is otherwise
        // read as a header.
        let mut path: Option<&str> = None;
        let mut minus: Option<&str> = None;
        // Names written as `pub fn NAME` anywhere on an added line, anchored or
        // not. Consulted only for a name the diff removes and does not add.
        let mut mentioned_added: BTreeSet<String> = BTreeSet::new();
        // `#[cfg(test)]` seen in this file's chunk. Everything after it is a
        // test module: `pub` inside one publishes nothing, and `is_library_rust`
        // cannot see it because the file is under `src/`.
        let mut in_test_cfg = false;
        // Per file, the `#[repr(...)]` lines each side of the diff carries.
        let mut reprs: BTreeMap<&str, (BTreeSet<String>, BTreeSet<String>)> = BTreeMap::new();

        for line in diff.lines() {
            if let Some(rest) = line.strip_prefix("--- ") {
                minus = header_path(rest);
                continue;
            }
            if let Some(rest) = line.strip_prefix("+++ ") {
                // A deletion writes `+++ /dev/null`, and every public function
                // in the file it deletes leaves the surface with it. The
                // pre-image path is the only name that file still has.
                path = header_path(rest).or(minus).filter(|p| is_library_rust(p));
                minus = None;
                in_test_cfg = false;
                continue;
            }
            if line.starts_with("diff --git ") {
                path = None;
                minus = None;
                in_test_cfg = false;
                continue;
            }
            let Some(path) = path else { continue };
            let (sign, body) = line.split_at(line.chars().next().map_or(0, char::len_utf8));
            let body = body.trim_start();

            // Read off context lines as well as changed ones. `#[cfg(test)]` is
            // almost never the line a diff edits -- it is the context above the
            // fixture that is -- so testing only `+`/`-` would never see it.
            //
            // `#[cfg(test)] mod tests` is conventionally the last item in a Rust
            // file, so "the rest of this chunk" is the test module. Erring past
            // its closing brace costs a genuine removal declared below it and
            // buys silence on every inline fixture, which is the direction this
            // gate has already chosen everywhere else.
            //
            // The flag is chunk-wide and not per-side, which over-reaches when a
            // change *deletes* a test module and writes library code below it
            // (`553dd03`). That is safe only because the mention below is
            // recorded before the skip, so the added declaration still clears
            // its removal. Reordering those two lines invents an ABI break.
            if body.starts_with("#[cfg(test)]") {
                in_test_cfg = true;
            }

            let side = match sign {
                "+" => &mut added,
                "-" => &mut removed,
                // Hunk headers, `index` lines and git's `\ No newline at end of
                // file` marker are not declarations, and neither is a context
                // line: it is code the change carried past, not code it wrote.
                _ => continue,
            };
            // A quoted or test-module `pub fn` on the added side is still
            // evidence the name did not leave, and declining to accuse is the
            // safe direction, so the mention is recorded before the skip.
            if sign == "+"
                && let Some(m) = PUB_FN_MENTION.captures(body)
            {
                mentioned_added.insert(m[1].to_string());
            }
            if in_test_cfg {
                continue;
            }
            if body.starts_with("#[repr(") {
                let entry = reprs.entry(path).or_default();
                let repr_side = if sign == "+" {
                    &mut entry.1
                } else {
                    &mut entry.0
                };
                repr_side.insert(body.chars().filter(|c| !c.is_whitespace()).collect());
            }
            let Some(name) = PUB_FN.captures(body) else {
                continue;
            };
            scan.declarations_read += 1;
            side.entry(name[1].to_string())
                .or_default()
                .push(Declaration {
                    path,
                    signature: normalized(body),
                });
        }

        // A `#[repr(...)]` line present identically on both sides was moved, not
        // changed. Only the symmetric difference is a layout the diff touched.
        scan.layout_files = reprs
            .into_iter()
            .filter(|(_, (before, after))| before != after)
            .map(|(p, _)| p.to_string())
            .collect();

        for (name, gone) in &removed {
            if !added.contains_key(name) && mentioned_added.contains(name) {
                // A quoted declaration is not a declaration, but it is not
                // nothing either: a fixture rewritten from a raw string into a
                // `&[&str]` removes the anchored form and adds the quoted one.
                // Declining costs a genuine removal that happens to be
                // re-declared inside a string, and buys the whole phantom class.
                scan.unpaired_names += 1;
                continue;
            }
            let Some(arrived) = added.get(name) else {
                scan.findings
                    .extend(gone.iter().map(|d| BreakingAbiFinding {
                        file_path: d.path.to_string(),
                        symbol_name: name.clone(),
                        change_kind: "REMOVAL".to_string(),
                        detail: format!(
                            "`{name}` is removed from the public surface and no `pub fn {name}` is \
                         added anywhere in this diff."
                        ),
                    }));
                continue;
            };
            let ([one], [other]) = (&gone[..], &arrived[..]) else {
                scan.unpaired_names += 1;
                continue;
            };
            let (Some(before), Some(after)) = (&one.signature, &other.signature) else {
                scan.unpaired_names += 1;
                continue;
            };
            if before != after {
                scan.findings.push(BreakingAbiFinding {
                    file_path: other.path.to_string(),
                    symbol_name: name.clone(),
                    change_kind: "SIGNATURE_CHANGE".to_string(),
                    detail: format!("`{before}` became `{after}`."),
                });
            }
        }

        scan
    }
}

/// The path out of a `--- ` / `+++ ` diff header.
///
/// `None` for the `/dev/null` git writes on the absent side of a creation or a
/// deletion, which falls out of requiring the `a/`/`b/` prefix rather than being
/// tested for: a path without one is not a header this reads.
fn header_path(header: &str) -> Option<&str> {
    let path = header.split_whitespace().next()?;
    path.strip_prefix("a/").or_else(|| path.strip_prefix("b/"))
}

/// Whether a path is Rust that a library publishes.
///
/// `tests/`, `benches/` and `examples/` are Cargo targets, not library surface:
/// a `pub fn` helper deleted from an integration test breaks no downstream
/// caller, and reporting it is the false accusation that gets a gate disabled.
///
/// This is a directory test and cannot see a `#[cfg(test)] mod tests` inside
/// `src/`, which is where every inline fixture in this repository lives. That
/// half is handled in the scan loop, off the `#[cfg(test)]` marker.
fn is_library_rust(path: &str) -> bool {
    path.ends_with(".rs")
        && !path
            .split('/')
            .any(|c| matches!(c, "tests" | "benches" | "examples"))
}

/// A declaration line reduced to the part that is its signature: every space
/// dropped, and the block opener or trailing `;` with them, so that neither
/// rustfmt respacing a parameter list nor a method moved into a trait
/// declaration reads as a change. Whitespace separates tokens in Rust and
/// carries nothing else, so removing all of it compares the signature rather
/// than its formatting.
///
/// `None` when the parameter list does not open and close on this line, which is
/// every signature rustfmt has spread over several lines. The caller compares
/// nothing in that case rather than comparing a fragment.
fn normalized(body: &str) -> Option<String> {
    let mut depth = 0i32;
    let mut closed = false;
    for ch in body.chars() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                closed |= depth == 0;
            }
            _ => {}
        }
    }
    if !closed || depth != 0 {
        return None;
    }
    let sig = body.trim_end();
    let sig = sig.strip_suffix('{').unwrap_or(sig);
    let sig = sig.trim_end().strip_suffix(';').unwrap_or(sig);
    Some(sig.chars().filter(|c| !c.is_whitespace()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(diff: &str) -> AbiScan {
        SignatureScanner::new().scan_abi_diff(diff)
    }

    fn chunk(path: &str, body: &str) -> String {
        format!("diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1 +1 @@\n{body}\n")
    }

    #[test]
    fn test_detects_removed_public_function() {
        let findings = scan(&chunk(
            "src/api.rs",
            "-pub fn legacy_api() -> u32 {\n-    42\n-}",
        ))
        .findings;
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].symbol_name, "legacy_api");
        assert_eq!(findings[0].change_kind, "REMOVAL");
    }

    #[test]
    fn test_passes_added_public_function() {
        assert!(
            scan(&chunk(
                "src/api.rs",
                "+pub fn new_api() -> u32 {\n+    42\n+}"
            ))
            .findings
            .is_empty()
        );
    }

    #[test]
    fn a_deleted_file_still_names_the_functions_it_took_with_it() {
        let diff = "diff --git a/src/gone.rs b/src/gone.rs\n--- a/src/gone.rs\n+++ /dev/null\n@@ -1,3 +0,0 @@\n-pub fn vanished() -> u32 { 0 }\n";
        let findings = scan(diff).findings;
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].file_path, "src/gone.rs");
    }

    #[test]
    fn a_repr_attribute_is_recorded_as_a_layout_the_gate_cannot_compute() {
        let scan = scan(&chunk("src/wire.rs", "-#[repr(C)]\n+#[repr(C, packed)]"));
        assert_eq!(scan.layout_files, vec!["src/wire.rs".to_string()]);
        assert!(scan.findings.is_empty());
    }

    #[test]
    fn restricted_visibility_is_not_a_published_surface() {
        assert!(
            scan(&chunk("src/api.rs", "-pub(crate) fn internal() {}"))
                .findings
                .is_empty()
        );
        // ...and narrowing a published function to it is a removal.
        let findings = scan(&chunk(
            "src/api.rs",
            "-pub fn open() {}\n+pub(crate) fn open() {}",
        ))
        .findings;
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].change_kind, "REMOVAL");
    }

    #[test]
    fn normalized_ignores_spacing_and_the_block_opener() {
        assert_eq!(
            normalized("pub  fn  f(a: u32) -> u32 {"),
            normalized("pub fn f(a:u32)->u32;")
        );
        assert_eq!(
            normalized("pub fn f("),
            None,
            "an unclosed parameter list is not a signature"
        );
    }
}
