//! Every `agy` turn must carry an explicit `--print-timeout`.
//!
//! agy's default is 5m0s and ends the turn with exit 1 and
//! `Error: timeout waiting for response`, no matter how long Anvil's own bound
//! is. Observed live on 2026-08-20: four queue-healer turns bounded at 600s by
//! Anvil were each cut off by agy at ~5m05s. Seventeen stage configs allow
//! 420s or 600s and had the same exposure.
//!
//! It used to be one argument per spawn site, and a new site written from the
//! shape of an old one would omit it. It is now one constructor:
//! `exec::turn::agy_turn` builds the argv for every turn and always passes the
//! flag, derived from the same budget that bounds the process. The property is
//! still checked over the source, because the constructor can be bypassed:
//! every `Command::new("agy")` whose argument list is a real turn -- anything
//! but `--help`/`--version` -- either passes `--print-timeout` itself or hands
//! the command to `agy_turn`.

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

/// The constructor every site now defers to must pass the flag itself.
///
/// Without this, the scan below is satisfiable by calling `agy_turn` while
/// `agy_turn` passes nothing -- one edit turning the whole guard off, which is
/// exactly what a single source of truth costs if nobody checks the source.
#[test]
fn the_constructor_every_site_defers_to_passes_the_flag() {
    let src = std::fs::read_to_string("src/exec/turn.rs").expect("the turn constructor exists");
    let at = src.find("pub fn agy_turn(").unwrap_or_else(|| {
        panic!(
            "`agy_turn` is gone. If the constructor moved, this test must \
             follow it -- a scan that stops finding its subject is not a fix."
        )
    });
    let body: String = src[at..].chars().take(700).collect();
    assert!(
        body.contains("\"--print-timeout\""),
        "`agy_turn` builds the argv for every turn and does not pass \
         `--print-timeout`, so every turn runs on agy's 5m default:\n{}",
        body.lines().take(20).collect::<Vec<_>>().join("\n")
    );
    assert!(
        body.contains("agy_print_timeout_arg("),
        "the value must be derived from this turn's own budget, not written as \
         a literal: two deadlines for one turn drift"
    );
}

#[test]
fn every_agy_turn_passes_an_explicit_print_timeout() {
    const SPAWN: &str = "Command::new(\"agy\")";
    let mut offenders = Vec::new();
    for path in rust_sources() {
        // Production code only. A fixture that builds a `Command` to read its
        // argv back is not a spawn site, and `without_test_modules` is the
        // stripper the rest of this codebase already relies on.
        let body = anvil::source_scan::without_test_modules(
            &std::fs::read_to_string(&path).unwrap_or_default(),
        );
        for (i, _) in body.match_indices(SPAWN) {
            // The argument-building calls follow the spawn; the next spawn (of
            // anything) or 1200 bytes bounds the window.
            let rest = &body[i + SPAWN.len()..];
            let end = rest.find("Command::new(").unwrap_or(rest.len()).min(1200);
            let window = &rest[..end];
            let is_probe = window.contains("\"--help\"") || window.contains("\"--version\"");
            let bounded = window.contains("\"--print-timeout\"") || window.contains("agy_turn(");
            if !is_probe && !bounded {
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
