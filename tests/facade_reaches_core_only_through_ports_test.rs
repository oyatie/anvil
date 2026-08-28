//! A facade reaches its own core only through ports — measured on this tree.
//!
//! `.anvil/shape.json` declares `face_dependency_matrix.facade = ["ports",
//! "adapters"]`, and `face_edge_denied` is `baseline-block-on-new` with an
//! empty key set. So every facade -> core edge is a regression on a blocking
//! rule. Four of them stood for eight days.
//!
//! # Why nothing caught them
//!
//! `tests/shape_self_baseline_test.rs` is a document read, not a measurement:
//! it parses `.anvil/baselines/shape.baseline.json` and asserts the file holds
//! 258 keys and a 40-character sha. The baseline was frozen on 20 August; the
//! four edges landed on the 27th and 28th. A self-check that reads yesterday's
//! answer cannot see today's tree. `shape::facade::gate::judge_pr` does the
//! real measurement and its one production caller points at tenant pull
//! requests, never at this repository.
//!
//! # Why they were introduced deliberately
//!
//! Two guards enforce two halves of one invariant, and only one of them runs
//! here. `clean_architecture_guard` holds `FACADE_BYPASSES_IN_ANVIL = 0` by
//! measuring this tree, and its remedy for a bypass is to add a door to the
//! provider's facade. The shortest spelling of that door is
//! `pub use crate::<unit>::core::X` — which is precisely the edge the other
//! half forbids. Every one of the four was seal remediation: 580422e, 3e8c6b4,
//! 065bd24, 785cb66.
//!
//! # Why this test does not reuse the shipped parser
//!
//! `shape::adapters::rust_use_deps` matches a `use crate::` line prefix, so it
//! sees five of the nine real references and none of the inline paths
//! (`crate::shape::core::profile::LanguageProfile::…`) or grouped imports. A
//! self-check built on it would under-report the thing it exists to prevent,
//! so this one matches the reference wherever it appears.

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
