//! Every `agy` turn must carry an explicit `--print-timeout`.
//!
//! agy's default is 5m0s and ends the turn with exit 1 and
//! `Error: timeout waiting for response`, no matter how long Anvil's own bound
//! is. Observed live on 2026-08-20: four queue-healer turns bounded at 600s by
//! Anvil were each cut off by agy at ~5m05s. Seventeen stage configs allow
//! 420s or 600s and had the same exposure.
//!
//! The fix is one argument per spawn site. A new site written from the shape
//! of an old one will omit it, so the property is checked over the source:
//! every `Command::new("agy")` whose argument list is a real turn (anything
//! but `--help`/`--version`) passes `--print-timeout`.

use std::path::PathBuf;

fn rust_sources() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::from("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "rs") {
                out.push(p);
            }
        }
    }
    out
}

#[test]
fn every_agy_turn_passes_an_explicit_print_timeout() {
    const SPAWN: &str = "Command::new(\"agy\")";
    let mut offenders = Vec::new();
    for path in rust_sources() {
        let body = std::fs::read_to_string(&path).unwrap_or_default();
        for (i, _) in body.match_indices(SPAWN) {
            // The argument-building calls follow the spawn; the next spawn (of
            // anything) or 1200 bytes bounds the window.
            let rest = &body[i + SPAWN.len()..];
            let end = rest.find("Command::new(").unwrap_or(rest.len()).min(1200);
            let window = &rest[..end];
            let is_probe = window.contains("\"--help\"") || window.contains("\"--version\"");
            if !is_probe && !window.contains("\"--print-timeout\"") {
                let line = body[..i].matches('\n').count() + 1;
                offenders.push(format!("{}:{}", path.display(), line));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "agy spawned without --print-timeout (agy's 5m default will end the turn \
         regardless of Anvil's bound): {offenders:#?}\n\
         Pass `\"--print-timeout\", &crate::exec::agy_print_timeout_arg(<this site's bound>)`."
    );
}
