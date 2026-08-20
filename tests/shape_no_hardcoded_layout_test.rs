//! I13: Anvil is tenant-neutral. No directory layout, ADR id, path convention,
//! crate prefix or satellite name may be hardcoded in the Shape Program's
//! source. Every such value is data the tenant repository carries.
//!
//! The rule is checked over string literals in production code (comments and
//! `#[cfg(test)]` items stripped) because that is where a layout constant
//! would appear. Exemptions are exact and listed: language-profile marker
//! files (facts about ecosystems, in `profile.rs`) and Anvil's own config path
//! `.anvil/shape.json` (in the facade).

use std::path::{Path, PathBuf};

const SCANNED_DIRS: &[&str] = &["src/shape", "src/ratchet", "src/change_delivery"];

const FORBIDDEN_LITERAL_FRAGMENTS: &[&str] = &[
    "core/",
    "ports/",
    "adapters/",
    "facade/",
    "app/",
    "governance/",
    "kernel/",
    "base/",
    "third-party",
    "oya-",
    "oya/",
    "ADR-",
    "openslo",
    "slos",
    "runbooks",
    "capability-registry",
    "manifest.json",
    "BUCK",
    "Cargo.toml",
    "package.json",
    "mod.rs",
];

/// (file suffix, literal fragments it may carry)
const EXEMPTIONS: &[(&str, &[&str])] = &[
    (
        "src/shape/core/profile.rs",
        &["BUCK", "Cargo.toml", "package.json", "mod.rs"],
    ),
    ("src/shape/facade/cli.rs", &[".anvil/shape.json"]),
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "rs") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Drops comment lines and `#[cfg(test)]` items (by indentation, the same
/// rule `tests/naming_law_survivors_test.rs` uses).
fn production_code(source: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut keep = vec![true; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim_start();
        if t.starts_with("//") {
            keep[i] = false;
            i += 1;
            continue;
        }
        if t.starts_with("#[cfg(test)]") {
            let indent = lines[i].len() - t.len();
            let mut j = i;
            while j < lines.len() && !lines[j].contains('{') {
                j += 1;
            }
            let mut k = j + 1;
            while k < lines.len() {
                let tk = lines[k].trim_start();
                if lines[k].len() - tk.len() == indent && tk.starts_with('}') {
                    break;
                }
                k += 1;
            }
            let end = k.min(lines.len() - 1);
            keep[i..=end].fill(false);
            i = k + 1;
            continue;
        }
        i += 1;
    }
    lines
        .iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|(l, _)| *l)
        .collect::<Vec<_>>()
        .join("\n")
}

fn string_literals(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = code.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != b'"' {
                if bytes[j] == b'\\' {
                    j += 1;
                }
                j += 1;
            }
            if j <= bytes.len() {
                out.push(code[start..j.min(bytes.len())].to_string());
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

#[test]
fn shape_program_source_carries_no_tenant_layout_literals() {
    let root = repo_root();
    let mut offenders = Vec::new();
    let mut scanned = 0usize;
    for dir in SCANNED_DIRS {
        for file in rust_files(&root.join(dir)) {
            scanned += 1;
            let rel = file
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .to_string();
            let allowed: &[&str] = EXEMPTIONS
                .iter()
                .find(|(suffix, _)| rel.ends_with(suffix))
                .map(|(_, a)| *a)
                .unwrap_or(&[]);
            let code = production_code(&std::fs::read_to_string(&file).unwrap());
            for lit in string_literals(&code) {
                for frag in FORBIDDEN_LITERAL_FRAGMENTS {
                    if lit.contains(frag) && !allowed.iter().any(|a| lit.contains(a)) {
                        offenders.push(format!("{rel}: {lit:?} contains {frag:?}"));
                    }
                }
            }
        }
    }
    assert!(
        scanned > 0,
        "nothing scanned; the Shape Program source must exist under {SCANNED_DIRS:?}"
    );
    assert!(
        offenders.is_empty(),
        "tenant layout hardcoded in Anvil source (I13) — move the value into the shape spec:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn the_scanner_catches_a_seeded_layout_literal() {
    let seeded = r#"
        fn dest() -> &'static str { "iam/core/" }
        #[cfg(test)]
        mod tests {
            const OK_IN_TESTS: &str = "adapters/";
        }
        // "facade/" in a comment is fine
    "#;
    let lits = string_literals(&production_code(seeded));
    assert!(
        lits.iter().any(|l| l.contains("core/")),
        "seeded literal must be seen: {lits:?}"
    );
    assert!(
        !lits.iter().any(|l| l.contains("adapters/")),
        "test-only literal must be stripped: {lits:?}"
    );
    assert!(
        !lits.iter().any(|l| l.contains("facade/")),
        "comment literal must be stripped: {lits:?}"
    );
}
