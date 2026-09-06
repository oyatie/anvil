//! The routing table decides what runs, and nothing else does.
//!
//! Before this, `config/model-routing.toml` was untracked, had no loader, and
//! had never affected a turn; `stage_router.rs` held a second hardcoded table
//! naming models removed from the fleet, with no production caller either. What
//! actually dispatched was one hardcoded hop to agy for a single call site, and
//! no fallback at all for the other five, which named `agy_agent` directly.
//!
//! These tests exist so that stays fixed: the file is the authority, a typo in
//! it is a loud error rather than a stage nobody dispatches, and no production
//! site reaches a provider constructor without going through the chain.

use anvil::ai_driver::{Stage, chain};
use std::collections::BTreeSet;
use std::path::Path;

fn repo(rel: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|e| panic!("{rel} must be readable: {e}"))
}

#[test]
fn every_stage_has_a_chain_and_every_chain_has_a_stage() {
    // The loader panics on a table it cannot validate, so reaching here at all
    // is the assertion; the counts make the coverage visible.
    let mut seen = BTreeSet::new();
    for stage in Stage::ALL {
        let tiers = chain(*stage);
        assert!(
            !tiers.is_empty(),
            "`{}` declares no tiers, so it has nothing to dispatch",
            stage.key()
        );
        seen.insert(stage.key());
    }
    assert_eq!(
        seen.len(),
        Stage::ALL.len(),
        "two stages share a key, so one of them can never be addressed"
    );
}

#[test]
fn a_stage_the_code_does_not_name_is_a_load_error() {
    // Absent evidence is never a pass: a typo'd stage key must fail the load,
    // not become a chain nothing dispatches.
    let bad = r#"
[[stage.implementaton]]
model = "gemini-3.8-flash"
provider = "agy"
effort = "high"
timeout_secs = 600
"#;
    let e = anvil::ai_driver::chain::parse(bad).expect_err("a misspelled stage must be refused");
    assert!(
        e.to_string().contains("implementaton"),
        "the error must name the key it could not place: {e}"
    );
}

#[test]
fn a_stage_with_no_chain_is_a_load_error() {
    // The other direction: a stage the code enumerates and the file omits would
    // have no provider to try, and would fail on its first turn instead of at
    // load.
    let only_one = r#"
[[stage.recon]]
model = "gemini-3.8-flash"
provider = "agy"
effort = "high"
timeout_secs = 600
"#;
    let e = anvil::ai_driver::chain::parse(only_one).expect_err("a missing chain must be refused");
    assert!(
        e.to_string().contains("no chain"),
        "the error must say a stage has no chain: {e}"
    );
}

#[test]
fn an_unknown_provider_is_a_load_error() {
    let bad = r#"
[[stage.recon]]
model = "something"
provider = "notaprovider"
effort = "high"
timeout_secs = 600
"#;
    let e = anvil::ai_driver::chain::parse(bad).expect_err("an unknown provider must be refused");
    assert!(e.to_string().contains("notaprovider"), "{e}");
}

#[test]
fn the_declared_primary_is_what_the_table_reports() {
    // Against the tracked file rather than a fixture: the numbers a reader sees
    // in `config/model-routing.toml` are the ones the code will dispatch.
    let text = repo("config/model-routing.toml");
    for stage in Stage::ALL {
        let declared_first = text
            .split(&format!("[[stage.{}]]", stage.key()))
            .nth(1)
            .and_then(|rest| {
                rest.lines()
                    .find(|l| l.trim_start().starts_with("model ="))
                    .map(|l| l.split('"').nth(1).unwrap_or("").to_string())
            })
            .unwrap_or_else(|| panic!("`{}` has no first tier in the file", stage.key()));
        assert_eq!(
            chain(*stage)[0].model,
            declared_first,
            "`{}` dispatches a different primary than the file declares",
            stage.key()
        );
    }
}

#[test]
fn no_production_site_reaches_a_provider_constructor_directly() {
    // The ratchet. Six sites used to spawn a model turn, and five of them named
    // `agy_agent` directly -- so they had one provider and no tier beneath it.
    // A seventh site added the same way would silently opt out of the table.
    const CONSTRUCTORS: &[&str] = &[
        "claude_agent(",
        "codex_agent(",
        "cursor_agent(",
        "grok_agent(",
        "agy_agent(",
    ];
    // The chain builds the tier's command, and the router still owns the
    // per-provider subscription paths it dispatches for `execute_prompt`.
    const MAY_CONSTRUCT: &[&str] = &[
        "src/ai_driver/chain.rs",
        "src/ai_driver/router.rs",
        "src/ai_driver/router/claude.rs",
        "src/exec/agent/provider.rs",
        "src/exec/mod.rs",
    ];

    let mut offenders = Vec::new();
    let mut walk = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(dir) = walk.pop() {
        for entry in std::fs::read_dir(&dir).expect("src must be listable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk.push(path);
                continue;
            }
            if path.extension().is_some_and(|e| e == "rs") {
                let rel = path
                    .strip_prefix(env!("CARGO_MANIFEST_DIR"))
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                if MAY_CONSTRUCT.contains(&rel.as_str()) {
                    continue;
                }
                let text = std::fs::read_to_string(&path).expect("readable");
                // A `tests.rs` module may name a constructor -- asserting one
                // refuses a bad model id is the point of `router/tests.rs`. The
                // exemption is VERIFIED rather than taken on the filename: the
                // PARENT must declare it `#[cfg(test)] mod tests;`, since that
                // attribute lives on the declaration and not in the file. A
                // file called `tests.rs` that the parent compiles into the
                // binary is production code with a convenient name, and is
                // scanned.
                if rel.ends_with("/tests.rs") {
                    let stem = path.parent().expect("has a parent");
                    let parent_src = [stem.with_extension("rs"), stem.join("mod.rs")];
                    let declared_test_only = parent_src.iter().any(|p| {
                        std::fs::read_to_string(p).is_ok_and(|t| {
                            t.contains("#[cfg(test)]\nmod tests;")
                                || t.contains("#[cfg(test)]\npub mod tests;")
                        })
                    });
                    if declared_test_only {
                        continue;
                    }
                }
                for (n, line) in text.lines().enumerate() {
                    if line.trim_start().starts_with("//") {
                        continue;
                    }
                    for c in CONSTRUCTORS {
                        if line.contains(c) {
                            offenders.push(format!("{rel}:{}: {}", n + 1, line.trim()));
                        }
                    }
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these sites pick a provider instead of naming a stage, so they have no \
         chain beneath them:\n  {}\nUse `ai_driver::run_stage(Stage::_, ..)`.",
        offenders.join("\n  ")
    );
}

#[test]
fn every_declared_model_id_can_be_built_into_a_command() {
    // A model id in the table that no constructor will accept is #212's defect
    // one layer up: three of six defaults named models their CLI rejected, and
    // nothing noticed until a turn failed. This runs every declared tier
    // through the real constructor -- argv validation and all -- without
    // spawning anything.
    let posture = anvil::exec::Posture::in_workspace(std::env::temp_dir());
    let mut refused = Vec::new();
    for stage in Stage::ALL {
        for tier in chain(*stage) {
            if let Err(e) = anvil::ai_driver::chain::command_for(
                tier,
                &posture,
                std::time::Duration::from_secs(60),
            ) {
                // A provider CLI absent from PATH is the machine, not the
                // table. CI installs none of them, and a test that passes only
                // where the binaries happen to be installed is measuring the
                // developer's laptop. Everything else -- a model id the
                // validator rejects, an effort it refuses, a provider with no
                // constructor -- is the table's problem and is still reported.
                let msg = e.to_string();
                if msg.contains("unavailable on the trusted service PATH") {
                    continue;
                }
                refused.push(format!("{}: {} -> {e}", stage.key(), tier.model));
            }
        }
    }
    assert!(
        refused.is_empty(),
        "declared tiers whose command cannot be built:\n  {}",
        refused.join("\n  ")
    );
}

#[test]
fn a_supplied_budget_caps_every_tier_and_reaches_the_provider() {
    // The doc parity probe runs under a watchdog and hands the chain its
    // budget. Two things must follow from that one value, and they used to be
    // spelled separately at the call site: the process bound, and the deadline
    // the provider is told in argv. Both now derive from the cap computed in
    // `run_stage_within`, so this is where that is asserted.
    // By MODULE, not by path. A path-keyed read goes blind the day the file
    // moves or becomes a directory -- issue #179, 332 of them -- and
    // `module_source` reads whichever form the module takes and refuses an
    // absent one. This test was written the wrong way first and the ratchet
    // caught it.
    let src = anvil::source_scan::paths::module_source(
        "src/ai_driver/chain",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    let code = anvil::source_scan::without_commentary(&src);

    assert!(
        code.contains("budget.map_or(tier.timeout, |b| b.min(tier.timeout))"),
        "a supplied budget must CAP the tier's declared timeout, not replace or \
         ignore it: a tier declaring 600s under a 300s watchdog must run 300s"
    );
    assert!(
        code.contains("command_for(tier, &posture, timeout)"),
        "the capped value must reach the provider constructor, or the CLI is \
         told a deadline the process bound does not share"
    );

    // And it is a real cap in both directions, not just a source string.
    let posture = anvil::exec::Posture::in_workspace(std::env::temp_dir());
    let tier = &chain(Stage::SpecReview)[0];
    // Same reason: absent on PATH is the machine. What must not happen is a
    // rejection of the capped budget itself.
    if let Err(e) =
        anvil::ai_driver::chain::command_for(tier, &posture, std::time::Duration::from_secs(1))
    {
        assert!(
            e.to_string()
                .contains("unavailable on the trusted service PATH"),
            "a capped budget must build a command wherever the provider exists: {e}"
        );
    }
}
