//! Reading a source tree off disk.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

/// Every `.rs` file under `root`, recursively, sorted for determinism.
/// Build output and hidden directories are skipped.
pub(super) fn collect_rust_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                if name == "target" || name.starts_with('.') {
                    continue;
                }
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Names of the crates this repository owns.
///
/// The facade seal must only judge paths rooted in code we own: a bare
/// `<ident>::<face>` also matches third-party paths, and `uuid::adapter::X`
/// is not a facade bypass. `crate::` covers the in-crate case; a member's own
/// name covers the cross-crate one, which is how `src/bin/occupancy.rs`
/// reaches `anvil::change_delivery::core`.
///
/// An unreadable or absent manifest yields an empty list, which narrows the
/// rule to `crate::` only. That under-reports; it never accuses.
pub(super) fn workspace_members(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    // The caller may hand us `<repo>/src`, whose manifest sits one level up.
    // Walking down from there found no `Cargo.toml`, so the member list came
    // back empty and the seal silently narrowed to `crate::` only -- which is
    // how `src/bin/occupancy.rs` reaching `anvil::change_delivery::core`
    // stopped being reported. Start from the nearest ancestor that has one.
    let mut base = root.to_path_buf();
    for _ in 0..8 {
        if base.join("Cargo.toml").is_file() {
            break;
        }
        if !base.pop() {
            break;
        }
    }
    let mut stack = vec![base];
    let mut budget = 4096usize;
    while let Some(dir) = stack.pop() {
        if budget == 0 {
            break;
        }
        budget -= 1;
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            if p.is_dir() {
                if !matches!(name, "target" | "buck-out" | ".git" | "third-party") {
                    stack.push(p);
                }
            } else if name == "Cargo.toml"
                && let Ok(text) = fs::read_to_string(&p)
            {
                for line in text.lines() {
                    let t = line.trim();
                    if let Some(rest) = t.strip_prefix("name")
                        && let Some(v) = rest.trim_start().strip_prefix('=')
                    {
                        let v = v.trim().trim_matches('"');
                        if !v.is_empty() {
                            // Cargo crate names reach code as snake_case.
                            out.push(v.replace('-', "_"));
                        }
                        break;
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}
