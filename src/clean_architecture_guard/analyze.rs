//! The analysis both entrypoints funnel through.
//!
//! Acquires test classification once, then runs the same pure diff analysis
//! for pull requests and Anvil's own tree.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use regex::Regex;

use super::paths::{classify_layer, is_import_line, layer_name};
use super::report::{ArchLayer, ArchViolation, CleanArchitectureReport, build_report};
use super::scan::{FaceScan, scan_faces};
use crate::source_scan::paths::{ArchitectureOwnership, RootRelation};

#[cfg(test)]
mod tests;

pub(super) fn analyze_unified_diff(
    diff_content: &str,
    scope: String,
    repo_root: &Path,
) -> Result<CleanArchitectureReport> {
    let test_sources = crate::source_scan::paths::TestSourceClassifier::new(repo_root)
        .map_err(anyhow::Error::msg)?;
    let ownership = ArchitectureOwnership::new(repo_root).map_err(anyhow::Error::msg)?;
    analyze_with_inputs(
        diff_content,
        scope,
        |path| test_sources.classify(path).map_err(anyhow::Error::msg),
        |file, root| ownership.relation(file, root),
    )
}

fn analyze_with_inputs(
    diff_content: &str,
    scope: String,
    classify_test: impl Fn(&Path) -> Result<bool>,
    resolve_root: impl Fn(&str, &str) -> RootRelation,
) -> Result<CleanArchitectureReport> {
    let mut violations = Vec::new();
    let mut current_file = String::new();
    let mut current_layer: Option<ArchLayer> = None;
    let mut inspected_files = BTreeSet::new();
    let mut classified_files = BTreeSet::new();
    let mut rust_files = BTreeSet::new();
    let mut face_subjects = 0usize;
    let mut unknown_faces = 0usize;
    // A `use` rustfmt broke across lines, held until its `;` arrives.
    let mut pending_use: Option<String> = None;
    let mut current_is_test = false;

    let core_forbidden_imports = [
        (
            r#"(?i)(?:use\s+|import\s+.*?from\s+['"]).*?\b(?:adapters?|adapter[-_]\w+|facade|rest)\b"#,
            "ADAPTERS/FACADE",
            "Core/Domain layer must never import from external Adapters or Facade layers",
        ),
        (
            r#"(?i)(?:use\s+|import\s+.*?from\s+['"]).*?\b(?:ports?|application)\b"#,
            "PORTS/APPLICATION",
            "Core/Domain layer must never import from Ports/Application layers",
        ),
    ];

    let ports_forbidden_imports = [(
        r#"(?i)(?:use\s+|import\s+.*?from\s+['"]).*?\b(?:adapters?|adapter[-_]\w+|facade|rest)\b"#,
        "ADAPTERS/FACADE",
        "Ports/Application layer must never import from concrete Adapters or Facade layers",
    )];

    for line in diff_content.lines() {
        if let Some(stripped) = line.strip_prefix("+++ b/") {
            current_file = stripped.trim().to_string();
            current_is_test = classify_test(Path::new(&current_file))?;
            // Test sources are out of scope, as they are for
            // `evaluate_source_tree`, which reads `src/` alone. The seal
            // governs the dependency structure that ships; a test reaching
            // into the unit under test is what a unit test IS, and holding the
            // two entry points to different scopes made the same guard report
            // four violations on a pull request and zero on the tree those
            // files live in.
            current_layer = if current_is_test {
                None
            } else {
                classify_layer(&current_file)
            };
            inspected_files.insert(current_file.clone());
            pending_use = None; // a statement never spans two files
            continue;
        }

        if line.starts_with('+') && !line.starts_with("+++") {
            let trimmed = line[1..].trim();
            let extension = Path::new(&current_file)
                .extension()
                .and_then(|ext| ext.to_str());
            let rust = extension == Some("rs");
            // Preserve TS/JS import checking, but never count prose or an
            // excluded test as a checked source just because its path has a layer.
            if current_is_test
                || !matches!(
                    extension,
                    Some("rs" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs")
                )
            {
                continue;
            }
            let code = crate::source_scan::code_only(trimmed);
            if code.trim().is_empty() {
                continue;
            }
            if current_layer.is_some() {
                classified_files.insert(current_file.clone());
            }
            if rust {
                rust_files.insert(current_file.clone());
            }
            // Only Rust has facade paths. Layer imports below intentionally
            // keep original text: masking TS string contents erases its target.
            if rust {
                // Only an import statement can create a dependency edge. A
                // comment containing "because ports -> core" matched the
                // unanchored `use\s+` through "beca|use" on Anvil's own tree.
                // A cross-unit binding is an edge however it is spelled. The
                // one in `git_manager` is an expression, not a `use`, so the
                // import-line filter below would hide it. Checked first, and
                // deliberately outside that filter.
                // `code_only` on the line, not just the tree: `evaluate_source_tree`
                // strips whole files, but a PR diff arrives as raw text and this
                // guard's own comment naming `crate::x::adapters::Y` was reported
                // as a violation of a unit called `x`. A line whose construct does
                // not terminate blanks to its end, which suppresses rather than
                // fabricates.
                let code = code.trim();

                // rustfmt breaks a long grouped `use` across lines by default,
                // so this is the ordinary spelling rather than an exotic one.
                // Read a line at a time, `use crate::beta::{` carries no face
                // and `core::X,` carries no unit, so neither line names a
                // bypass and the statement passes. Held and joined until its
                // `;`, it reads exactly as the single-line form.
                //
                // Only `use` statements are joined. Buffering every unbalanced
                // brace would swallow function bodies into one scan, where the
                // per-scan de-duplication would silently merge distinct
                // references and lower the count.
                let statement = match pending_use.take() {
                    Some(mut held) => {
                        held.push(' ');
                        held.push_str(code);
                        // Bounded: a `use` that never terminates must not grow
                        // without limit. Giving up scans what was collected.
                        if held.contains(';') || held.len() > 4096 {
                            Some(held)
                        } else {
                            pending_use = Some(held);
                            None
                        }
                    }
                    None => {
                        let is_use = code.starts_with("use ") || code.starts_with("pub use ");
                        if is_use && !code.contains(';') {
                            pending_use = Some(code.to_string());
                            None
                        } else {
                            Some(code.to_string())
                        }
                    }
                };

                let scan = match &statement {
                    // Test sources are out of scope for the seal as well as for
                    // layer classification. The seal flags an UNLAYERED importer
                    // too, so checking only `classify_layer` left every test file
                    // still reported.
                    Some(_) if current_is_test => FaceScan {
                        bypasses: Vec::new(),
                        subjects: 0,
                        unknown: 0,
                    },
                    Some(text) => scan_faces(text, &current_file, &resolve_root),
                    // Still collecting the rest of this statement.
                    None => FaceScan {
                        bypasses: Vec::new(),
                        subjects: 0,
                        unknown: 0,
                    },
                };
                face_subjects += scan.subjects;
                unknown_faces += scan.unknown;
                for (unit, face) in scan.bypasses {
                    violations.push(ArchViolation {
                        file_path: current_file.clone(),
                        source_layer: layer_name(current_layer).to_string(),
                        target_layer: format!("{unit}::{face}"),
                        description: format!(
                            "reaches past `{unit}`'s facade into its `{face}`; only a \
                         unit's facade is importable from outside it"
                        ),
                        snippet: trimmed.to_string(),
                    });
                }
            }

            // The layer-direction rules below are regex matches over an
            // unanchored `use\s+`, which matched the "use" inside
            // "because" on this very tree. They stay import-line-only.
            if !is_import_line(trimmed) {
                continue;
            }

            match current_layer {
                Some(ArchLayer::Core) => {
                    for (pattern, target_layer, desc) in &core_forbidden_imports {
                        if let Ok(re) = Regex::new(pattern)
                            && re.is_match(trimmed)
                        {
                            violations.push(ArchViolation {
                                file_path: current_file.clone(),
                                source_layer: "CORE/DOMAIN".to_string(),
                                target_layer: target_layer.to_string(),
                                description: desc.to_string(),
                                snippet: trimmed.to_string(),
                            });
                        }
                    }
                }
                Some(ArchLayer::Ports) => {
                    for (pattern, target_layer, desc) in &ports_forbidden_imports {
                        if let Ok(re) = Regex::new(pattern)
                            && re.is_match(trimmed)
                        {
                            violations.push(ArchViolation {
                                file_path: current_file.clone(),
                                source_layer: "PORTS/APPLICATION".to_string(),
                                target_layer: target_layer.to_string(),
                                description: desc.to_string(),
                                snippet: trimmed.to_string(),
                            });
                        }
                    }
                }
                // Adapters and Facade sit outermost within their own unit:
                // they may depend inward, so there is no forbidden
                // direction to check for them here.
                Some(ArchLayer::Adapters) | Some(ArchLayer::Facade) | None => {}
            }
        }
    }

    Ok(build_report(
        scope,
        violations,
        inspected_files.len(),
        classified_files.len(),
        rust_files.len(),
        face_subjects,
        unknown_faces,
    ))
}
