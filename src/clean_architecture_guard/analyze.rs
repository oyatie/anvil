//! The analysis both entrypoints funnel through.
//!
//! Takes a unified diff and returns a verdict; reads no filesystem and holds
//! no state, so the same rules apply to someone else's pull request and to
//! Anvil's own tree.

use regex::Regex;

use super::paths::{classify_layer, is_import_line, layer_name};
use super::report::{ArchLayer, ArchMeasurement, ArchViolation, CleanArchitectureReport};
use super::scan::{FaceScan, scan_faces};

pub(super) fn analyze_unified_diff(
    diff_content: &str,
    scope: String,
    local_crates: &[String],
) -> CleanArchitectureReport {
    let mut violations = Vec::new();
    let mut current_file = String::new();
    let mut current_layer: Option<ArchLayer> = None;
    let mut files_inspected = 0usize;
    let mut files_classified = 0usize;
    let mut rust_files_inspected = 0usize;
    let mut face_subjects = 0usize;
    // A `use` rustfmt broke across lines, held until its `;` arrives.
    let mut pending_use: Option<String> = None;

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
            current_layer = classify_layer(&current_file);
            files_inspected += 1;
            if current_layer.is_some() {
                files_classified += 1;
            }
            if current_file.ends_with(".rs") {
                rust_files_inspected += 1;
            }
            pending_use = None; // a statement never spans two files
            continue;
        }

        if line.starts_with('+') && !line.starts_with("+++") {
            let trimmed = line[1..].trim();
            // The rules below read Rust paths. Run them on anything else and
            // prose becomes code: this guard's own CHANGELOG entry, which
            // names `change_delivery::adapters::git_vcs` in order to DESCRIBE
            // the defect, was reported as committing it. Anvil's review of
            // this change is what caught that.
            if !current_file.ends_with(".rs") {
                continue;
            }
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
            let code = crate::source_scan::code_only(trimmed);
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
                Some(text) => scan_faces(text, &current_file, local_crates),
                // Still collecting the rest of this statement.
                None => FaceScan {
                    bypasses: Vec::new(),
                    subjects: 0,
                },
            };
            face_subjects += scan.subjects;
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

    // Two rules with two different subjects. The layer-direction rules need
    // a file that sits in a layer; the facade seal needs only Rust source.
    // Keying the verdict on the first alone reported a real, FOUND bypass as
    // "NOT MEASURED ... no inward-dependency claim can be made" whenever the
    // offending file happened to sit in no layer -- which is the common case,
    // and the exact case the seal exists for. An absence must never outrank a
    // finding.
    let measurement = if files_classified == 0 && face_subjects == 0 {
        ArchMeasurement::NotMeasured {
            reason: format!(
                "nothing to measure in {files_inspected} file(s) examined: 0 belong to a \
                 recognised layer, and no path names any unit's core/ports/adapters, so \
                 neither the layer-direction rules nor the facade seal had a subject"
            ),
            files_inspected,
        }
    } else {
        ArchMeasurement::Measured {
            files_inspected,
            files_classified,
        }
    };

    // An unmeasured run is not a clean run.
    let is_clean = violations.is_empty() && measurement.is_measured();

    let summary = match &measurement {
        ArchMeasurement::NotMeasured { reason, .. } => format!(
            "Clean Architecture NOT MEASURED for {scope}: {reason}. No inward-dependency \
             claim can be made about this tree."
        ),
        ArchMeasurement::Measured {
            files_inspected,
            files_classified,
        } => {
            if violations.is_empty() {
                // Say exactly what was measured. "Verified ... 100% intact"
                // over 8 layered files of 256 is a claim about the 248 the
                // classifier never saw (I2).
                format!(
                    "Clean Architecture: 0 layer-boundary violations across \
                     {files_classified} layered file(s) of {files_inspected} examined in \
                     {scope}; {} file(s) belong to no recognised layer and were not \
                     measured for layer direction. The facade seal examined \
                     {face_subjects} face reference(s) across {rust_files_inspected} Rust \
                     file(s).",
                    files_inspected.saturating_sub(*files_classified)
                )
            } else {
                // The denominator belongs on this branch too. A findings
                // list alone says nothing about the files the classifier
                // never saw, and "18 violations" reads as a complete
                // account of the tree when it is an account of 55 files.
                format!(
                    "Clean Architecture layer boundary violations ({} items) across \
                     {files_classified} layered file(s) of {files_inspected} examined in \
                     {scope}; {} file(s) belong to no recognised layer and were not \
                     measured. {}",
                    violations.len(),
                    files_inspected.saturating_sub(*files_classified),
                    violations
                        .iter()
                        .map(|v| format!("{}: {}", v.file_path, v.description))
                        .collect::<Vec<_>>()
                        .join("; ")
                )
            }
        }
    };

    CleanArchitectureReport {
        is_clean,
        violations,
        summary,
        measurement,
        scope,
    }
}
