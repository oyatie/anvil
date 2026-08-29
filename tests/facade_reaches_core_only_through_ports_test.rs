//! A facade reaches its own core only through ports.
//!
//! `.anvil/shape.json` allows `facade -> [ports, adapters]`; core is absent
//! from that list, so any such edge is a regression on a blocking rule.
//!
//! Matches the reference wherever it appears rather than reusing
//! `shape::adapters::rust_use_deps`, which recognises only a `use crate::`
//! line prefix and so cannot see an inline path or a grouped import.

use std::path::Path;

/// Every `<unit>/facade/**.rs` in this repository.
fn facade_sources() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let Ok(units) = std::fs::read_dir(&src) else {
        panic!("src/ must be readable");
    };
    for unit in units.flatten() {
        let facade = unit.path().join("facade");
        if !facade.is_dir() {
            continue;
        }
        let unit_name = unit.file_name().to_string_lossy().to_string();
        let Ok(files) = std::fs::read_dir(&facade) else {
            continue;
        };
        for f in files.flatten() {
            let p = f.path();
            if p.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            if let Ok(body) = std::fs::read_to_string(&p) {
                out.push((
                    format!("{unit_name}/facade/{}", f.file_name().to_string_lossy()),
                    body,
                ));
            }
        }
    }
    out
}

#[test]
fn no_facade_names_its_own_core() {
    let sources = facade_sources();
    assert!(
        !sources.is_empty(),
        "no facade sources found; this check would pass vacuously"
    );

    let mut offences = Vec::new();
    for (label, body) in &sources {
        let unit = label.split('/').next().unwrap_or_default();
        let needle = format!("crate::{unit}::core");
        // Comments and string literals are not dependency edges. This module's
        // own doc comment names the forbidden spelling several times.
        let code = anvil::source_scan::code_only(body);
        for (i, line) in code.lines().enumerate() {
            if line.contains(&needle) {
                offences.push(format!("{label}:{} {}", i + 1, line.trim()));
            }
        }
    }

    assert!(
        offences.is_empty(),
        "a facade may reach its own core only through ports \
         (.anvil/shape.json: facade -> [ports, adapters]).\n\
         Route the symbol through `crate::{{unit}}::ports` — the ports module \
         re-exports for exactly this reason — rather than naming core here.\n\
         {} offending reference(s):\n  {}",
        offences.len(),
        offences.join("\n  ")
    );
}
