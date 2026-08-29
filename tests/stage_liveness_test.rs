//! A stage can be complete, documented, tested — and never invoked.
//!
//! Three were found that way in a single day. None was a bug in the code that
//! failed; each was correct and unreached, which is the one thing a unit test
//! over that code can never catch.

use std::fs;
use std::path::Path;

use anvil::stage_liveness::{STAGES, STAGES_WITHOUT_A_CALLER, Stage, uninvoked};

fn production_sources() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![Path::new("src").to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let text = fs::read_to_string(&p).unwrap_or_default();
                out.push((p.to_string_lossy().replace('\\', "/"), text));
            }
        }
    }
    out
}

/// A ceiling for checkouts with no merge-base. The monotone bound is derived
/// below: an exact equality here is a number every lane must edit, and two
/// branches that both wire a stage write the same line and merge cleanly.
#[test]
fn the_uninvoked_count_does_not_exceed_the_ceiling() {
    let dead = uninvoked(&production_sources());
    assert!(
        dead.len() <= STAGES_WITHOUT_A_CALLER,
        "stages with no production caller moved. Each of these is written, \
         tested and run by nothing:\n{}",
        dead.iter()
            .map(|s| format!("  {} — {}", s.stage, s.loses))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The scan must find a caller that exists. Without this the count could be
/// "everything is dead" and still pass.
#[test]
fn a_stage_with_a_caller_is_not_reported() {
    let sources = vec![
        (
            "src/somewhere/else.rs".to_string(),
            "fn go() { crate::postmortem::classify(); }".to_string(),
        ),
        ("src/postmortem/mod.rs".to_string(), String::new()),
    ];
    let dead = uninvoked(&sources);
    assert!(
        !dead.iter().any(|s| s.stage == "postmortem"),
        "a stage with a real caller was reported dead"
    );
}

/// A stage must not prove its own liveness by mentioning itself.
#[test]
fn a_stage_referring_only_to_itself_is_still_dead() {
    let sources = vec![(
        "src/postmortem/mod.rs".to_string(),
        "fn inner() { crate::postmortem::classify(); }".to_string(),
    )];
    assert!(
        uninvoked(&sources).iter().any(|s| s.stage == "postmortem"),
        "a stage cited only inside its own files was counted as invoked"
    );
}

/// Documentation is not invocation. `next_phase` appeared exactly once in
/// `src/` — in a doc comment — and was reported reachable for it.
#[test]
fn a_mention_in_a_comment_or_string_is_not_a_caller() {
    for text in [
        "/// Bounded by `next_phase::MAX_AUTO_FIX_ATTEMPTS`.\nfn unrelated() {}",
        "// next_phase( is discussed here\nfn unrelated() {}",
        "fn log() { println!(\"next_phase( ran\"); }",
    ] {
        let sources = vec![
            ("src/state.rs".to_string(), text.to_string()),
            ("src/webhook/next_phase.rs".to_string(), String::new()),
        ];
        assert!(
            uninvoked(&sources)
                .iter()
                .any(|s| s.stage == "webhook::next_phase"),
            "a stage named only in prose was counted as invoked: {text}"
        );
    }
}

/// A caller that only exists under `#[cfg(test)]` does not make a stage live
/// in production — which is exactly the state `gate_proof` and `postmortem`
/// are in.
#[test]
fn a_caller_that_exists_only_in_tests_does_not_count() {
    let sources = vec![
        (
            "src/somewhere/else.rs".to_string(),
            "#[cfg(test)]\nmod tests {\n    fn t() { crate::gate_proof::check(); }\n}\n"
                .to_string(),
        ),
        ("src/gate_proof/mod.rs".to_string(), String::new()),
    ];
    assert!(
        uninvoked(&sources).iter().any(|s| s.stage == "gate_proof"),
        "a stage called only from a test module was counted as live in production"
    );
}

/// Every row must say what is lost, or it is a finding nobody can act on.
#[test]
fn every_stage_states_what_is_lost_while_it_is_dead() {
    for Stage { stage, loses, .. } in STAGES {
        assert!(
            loses.len() > 30,
            "`{stage}` does not say what is lost while nothing runs it"
        );
    }
}

/// The bound that holds: wiring a stage is progress and needs no bookkeeping.
///
/// `uninvoked` is already a pure function of a source list, so the merge-base
/// tree can be fed to the same code that judges the working tree — no second
/// implementation, and the two sides are the same measure by construction.
#[test]
fn uninvoked_stages_do_not_grow_against_the_merge_base() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let derived = rt.block_on(anvil::ratchet::facade::derived::at_merge_base(
        repo,
        "origin/dev",
        "HEAD",
        |p| p.starts_with("src/") && p.ends_with(".rs"),
        |tree| {
            let sources: Vec<(String, String)> = tree
                .paths()
                .iter()
                .filter(|p| p.starts_with("src/") && p.ends_with(".rs"))
                .filter_map(|p| {
                    let bytes = tree.read(p).ok().flatten()?;
                    let text = std::str::from_utf8(bytes).ok()?.to_string();
                    Some((p.clone(), text))
                })
                .collect();
            uninvoked(&sources).len()
        },
    ));
    let Ok(base) = derived else {
        eprintln!("skipped: no merge-base against origin/dev");
        return;
    };
    let dead = uninvoked(&production_sources());
    assert!(
        dead.len() <= base.at_merge_base,
        "stages with no production caller grew from {} at merge-base {} to {}:\n{}",
        base.at_merge_base,
        &base.merge_base[..12],
        dead.len(),
        dead.iter()
            .map(|s| format!("  {} — {}", s.stage, s.loses))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
