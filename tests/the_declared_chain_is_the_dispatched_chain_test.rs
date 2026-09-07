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

use anvil::ai_driver::chain::budget::{MIN_TIER_ALLOTMENT, StageBudget};
use anvil::ai_driver::{Stage, chain};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

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
[stage_meta.implementaton]
writes = []

[[stage.implementaton]]
model = "gemini-3.8-flash"
provider = "agy"
effort = "high"
timeout_secs = 600
"#;
    let e = anvil::ai_driver::chain::parse_table_for_test(bad)
        .expect_err("a misspelled stage must be refused");
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
    // Complete for `recon`, including its write scope, so the load reaches the
    // missing-chain check rather than stopping at a missing declaration.
    let only_one = r#"
[stage_meta.recon]
writes = []

[[stage.recon]]
model = "gemini-3.8-flash"
provider = "agy"
effort = "high"
timeout_secs = 600
"#;
    let e = anvil::ai_driver::chain::parse_table_for_test(only_one)
        .expect_err("a missing chain must be refused");
    assert!(
        e.to_string().contains("no chain"),
        "the error must say a stage has no chain: {e}"
    );
}

#[test]
fn an_unknown_provider_is_a_load_error() {
    let bad = r#"
[stage_meta.recon]
writes = []

[[stage.recon]]
model = "something"
provider = "notaprovider"
effort = "high"
timeout_secs = 600
"#;
    let e = anvil::ai_driver::chain::parse_table_for_test(bad)
        .expect_err("an unknown provider must be refused");
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
        // Omitted when muse was added, so a new site reaching the provider this
        // change introduces was admitted silently -- the ratchet was blind to
        // exactly the thing it shipped with.
        "muse_agent(",
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
fn a_supplied_budget_bounds_the_whole_stage_not_each_attempt() {
    // What this replaced asserted that the source contained the literal
    // `budget.map_or(tier.timeout, |b| b.min(tier.timeout))`. That is a
    // spelling test: rewriting the same arithmetic as a `match` fails it while
    // changing nothing, and -- worse -- it made a wrong SEMANTICS look
    // guarded. `b.min(tier.timeout)` per tier is exactly the defect. Each of
    // five tiers got the caller's whole bound.
    //
    // Measured against `config/model-routing.toml` and `queue_healer`'s
    // AGY_TURN_LIMIT (`ExecClass::Model.timeout()`, 600s):
    let bound = anvil::exec::ExecClass::Model.timeout();
    assert_eq!(bound, Duration::from_secs(600), "the healer's bound moved");

    let per_attempt: Duration = chain(Stage::Remediation)
        .iter()
        .map(|t| bound.min(t.timeout))
        .sum();
    assert!(
        per_attempt > bound,
        "if the old reading were already within bound there would be nothing \
         to fix; measured {per_attempt:?} against {bound:?}"
    );

    // Worst case under the new reading: every tier burns its full allotment.
    let mut left = StageBudget::of(Some(bound));
    let mut allotted = Duration::ZERO;
    let mut reached = 0usize;
    for tier in chain(Stage::Remediation) {
        let Some(t) = left.allot(tier.timeout) else {
            break;
        };
        allotted += t;
        left.spend(t);
        reached += 1;
    }
    assert!(
        allotted <= bound,
        "the stage must fit inside the bound its caller supplied: {allotted:?} \
         allotted across {reached} tiers against {bound:?} (the per-attempt \
         reading allotted {per_attempt:?})"
    );

    // The same number under `doc_guard`'s watchdog, which passes its own 120s
    // from INSIDE that watchdog. Under the per-attempt reading tier 1 could
    // consume the entire probe and tiers 2..5 were unreachable by construction.
    let probe = Duration::from_secs(120);
    let mut left = StageBudget::of(Some(probe));
    let mut allotted = Duration::ZERO;
    for tier in chain(Stage::SpecReview) {
        let Some(t) = left.allot(tier.timeout) else {
            break;
        };
        allotted += t;
        left.spend(t);
    }
    assert!(
        allotted <= probe,
        "{allotted:?} allotted under a {probe:?} supervisor"
    );

    // A fast refusal must not strand the tiers behind it: the chain is charged
    // what a tier TOOK, not what it was entitled to.
    let mut left = StageBudget::of(Some(bound));
    let first = &chain(Stage::Remediation)[0];
    assert!(left.allot(first.timeout).is_some());
    left.spend(Duration::from_secs(2));
    assert_eq!(
        left.remaining(),
        Some(bound - Duration::from_secs(2)),
        "a two-second refusal must cost two seconds, not the tier's declared \
         timeout, or one fast failure ends the chain"
    );

    // No budget is unbounded: every tier gets exactly what the table declares.
    let free = StageBudget::of(None);
    for tier in chain(Stage::Remediation) {
        assert_eq!(free.allot(tier.timeout), Some(tier.timeout));
    }
}

#[test]
fn a_remnant_too_small_to_produce_a_turn_is_not_tried() {
    // `agy_print_timeout_arg` subtracts a 30s margin and clamps to at least 1s
    // (`src/exec/tests.rs`: 5s and 0s both become "1s"). So a tier allotted
    // less than that margin tells the CLI it has one second -- a turn that
    // cannot happen, which under I1 must be reported as NOT TRIED rather than
    // as a model that answered and had nothing to say.
    assert!(
        MIN_TIER_ALLOTMENT >= anvil::exec::AGY_PRINT_TIMEOUT_MARGIN,
        "a tier may not be allotted less than the margin its own argv loses"
    );

    let mut left = StageBudget::of(Some(MIN_TIER_ALLOTMENT));
    assert_eq!(
        left.allot(Duration::from_secs(600)),
        Some(MIN_TIER_ALLOTMENT),
        "exactly the floor is still spendable"
    );
    left.spend(Duration::from_secs(1));
    assert_eq!(
        left.allot(Duration::from_secs(600)),
        None,
        "a remnant under the floor stops the chain rather than spawning a turn \
         that is cut off before it can answer"
    );
}

#[test]
fn every_stage_declares_what_it_may_write() {
    // A stage with no declared scope is a load error, not an unrestricted
    // stage. This asserts the declaration exists for all of them and that the
    // auditing stages declare nothing -- `audits = ..` and `writes = []` are
    // the same claim from two directions, and an auditor that commits is an
    // auditor that edited what it was judging.
    use anvil::ai_driver::chain::plan;
    for stage in Stage::ALL {
        let p = plan(*stage);
        for prefix in &p.writes {
            assert!(
                !prefix.starts_with('/') && !prefix.contains(".."),
                "`{}` declares an escaping write prefix {prefix:?}",
                stage.key()
            );
        }
    }
    // No hand-list. There are SEVEN `audits =` declarations, not the five this
    // once enumerated, and `falsification` legitimately writes `tests/` because
    // it judges by constructing a counterexample. The property is not "an
    // auditor writes nothing" -- it is that an auditor may not write what it
    // judges, which the loader now enforces for every declared pair.
    for stage in Stage::ALL {
        let p = plan(*stage);
        let Some(audited_key) = &p.audits else {
            continue;
        };
        let audited = Stage::ALL
            .iter()
            .copied()
            .find(|s| s.key() == audited_key)
            .unwrap_or_else(|| panic!("`{}` audits an unknown stage", stage.key()));
        for w in &p.writes {
            for a in &plan(audited).writes {
                let (w, a) = (w.trim_end_matches('/'), a.trim_end_matches('/'));
                assert!(
                    w != a && !w.starts_with(&format!("{a}/")) && !a.starts_with(&format!("{w}/")),
                    "`{}` audits `{audited_key}` and both may write {w:?}/{a:?}",
                    stage.key()
                );
            }
        }
    }

    // And the implementer may not write tests, which is what makes the
    // authoring stage mean anything.
    assert_eq!(
        plan(Stage::Implementation).writes,
        vec!["src/".to_string()],
        "an implementer that can write tests/ can relax a test it fails"
    );
}

#[test]
fn a_stage_with_no_declared_write_scope_is_a_load_error() {
    // The producer half of #215's defect: the hook read a file nothing wrote.
    // The other half would be a stage the producer cannot describe, which must
    // fail at load rather than declare an empty scope that reads as "audited".
    let no_meta = r#"
[[stage.recon]]
model = "gemini-3.8-flash"
provider = "agy"
effort = "high"
timeout_secs = 600
"#;
    let e = anvil::ai_driver::chain::parse_table_for_test(no_meta)
        .expect_err("a stage with no writes declaration must be refused");
    assert!(
        e.to_string().contains("writes") || e.to_string().contains("no chain"),
        "the error must name the missing declaration: {e}"
    );
}

#[test]
fn every_site_that_commits_is_scoped_or_declared_exempt() {
    // This replaces an assertion that checked the WRONG PLACE.
    //
    // It used to assert `RunScope::declare` appeared in `run_stage_within`,
    // which was both unverifiable (a source substring survives `let _ =`) and
    // aimed at a window where nothing commits: every prompt says "Do NOT
    // commit", and anvil stages and commits after the turn returns.
    //
    // The property that matters is about COMMIT sites, so this censuses them.
    // A site either holds a run scope or is named here with a reason. Neither
    // is optional: a seventh site added silently is the defect that put a
    // guardrail in `dev` reading a file nothing wrote.
    const EXEMPT: &[(&str, &str)] = &[
        (
            "src/pr_self_healer.rs",
            "commits with --no-verify, so the hook cannot run at all. That is a \
             defeat of the guardrail, recorded here rather than hidden: it is not \
             scoped because it CANNOT be, and closing it means removing \
             --no-verify, which is a separate decision",
        ),
        (
            "src/lockfile_reconciler.rs",
            "commits Cargo.lock, which no stage declares and which is a hub file; \
             scoping it needs a stage whose writes include it",
        ),
        (
            "src/webhook/pipelines/certify.rs",
            "commits certification evidence, not model output; not a milestone run",
        ),
        (
            "src/fixer/mod.rs",
            "dispatches Implementation and commits, so it SHOULD be scoped; \
             untouched here only because it is a separate change",
        ),
    ];

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sites: Vec<String> = Vec::new();
    let mut stack = vec![root.join("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("src listable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("readable");
            // Test code is excluded, never by filename alone.
            //
            // `without_test_modules` strips inline `#[cfg(test)] mod tests { .. }`
            // blocks, but a whole-file module carries its attribute on the
            // PARENT's `mod tests;` declaration, so the file itself has no
            // marker. That is verified against the parent, the way the spawn-seam
            // ratchet does it: a `tests.rs` the parent compiles unconditionally
            // is production code with a convenient name, and is scanned.
            if path.file_name().is_some_and(|n| n == "tests.rs") {
                let stem = path.parent().expect("has a parent");
                let declared_test_only = [stem.with_extension("rs"), stem.join("mod.rs")]
                    .iter()
                    .any(|parent| {
                        std::fs::read_to_string(parent).is_ok_and(|t| {
                            t.contains("#[cfg(test)]\nmod tests;")
                                || t.contains("#[cfg(test)]\npub mod tests;")
                        })
                    });
                if declared_test_only {
                    continue;
                }
            }
            let code = anvil::source_scan::without_commentary(
                &anvil::source_scan::without_test_modules(&text),
            );
            if !code.contains("\"commit\"") {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let scoped = code.contains("RunScope::declare");
            let exempt = EXEMPT.iter().any(|(f, _)| *f == rel);
            if !scoped && !exempt {
                sites.push(rel);
            }
        }
    }
    assert!(
        sites.is_empty(),
        "these sites run `git commit` without a declared run scope and are not \
         listed as exempt:\n  {}\nEither hold a `RunScope` across the commit, or \
         add it to EXEMPT with the reason it cannot be scoped.",
        sites.join("\n  ")
    );

    // The exempt list may not name a file that no longer commits: a stale
    // exemption is a hole nobody is looking at.
    for (file, _) in EXEMPT {
        let text = std::fs::read_to_string(root.join(file))
            .unwrap_or_else(|e| panic!("exempt file {file} must exist: {e}"));
        assert!(
            anvil::source_scan::without_commentary(&text).contains("\"commit\""),
            "{file} is listed exempt from the run-scope census but no longer commits"
        );
    }
}

#[test]
fn tests_are_authored_and_reviewed_before_the_code_that_satisfies_them() {
    // The reason this stage exists, stated as an assertion.
    //
    // A test written by whoever wrote the implementation inherits the blind
    // spot it was meant to cover. Measured in this repository, twice: #206
    // shipped a hook reading a file nothing wrote, green because its tests
    // wrote the file themselves; and the producer that fixed it shipped with an
    // end-to-end test that passed with the producer deleted, because the test
    // called it directly instead of asserting the dispatch path reached it.
    //
    // Authoring the tests BEFORE the implementation is the structural fix:
    // tests that exist first cannot be shaped to fit code that does not exist
    // yet, and the implementer cannot quietly relax one it fails.
    //
    // SCOPE, stated because this test used to imply more than it proved. What
    // follows is a property of the TABLE. No runner walks the order: measured
    // by dispatch function rather than by grepping for `Stage::`, five of
    // sixteen stages have a caller, and `test_authoring` is not one of them.
    // `every_declared_stage_is_dispatched_or_named_as_not_yet` is where that
    // gap is held; this test says only that the file, when a runner does walk
    // it, cannot order these three wrongly.
    use anvil::ai_driver::chain::{plan, runs_after_transitively};

    let authoring = plan(Stage::TestAuthoring);
    let review = plan(Stage::TestAuthoringReview);
    let implementation = plan(Stage::Implementation);

    assert_eq!(
        authoring.writes,
        vec!["tests/".to_string()],
        "the test author writes tests and nothing else: it must not be able to \
         change the code its tests are about"
    );
    assert!(
        review.writes.is_empty(),
        "the review of the tests commits nothing, like every other auditing stage"
    );

    // Three different providers across author, reviewer and implementer. The
    // point is not variety: a test written and then satisfied by the same model
    // is the same act twice.
    let author = &authoring.tiers[0].provider;
    let reviewer = &review.tiers[0].provider;
    let implementer = &implementation.tiers[0].provider;
    assert_ne!(
        author, implementer,
        "the model that writes the tests must not be the one that satisfies them"
    );
    assert_ne!(
        author, reviewer,
        "the model that writes the tests must not be the one that reviews them"
    );
    assert_ne!(
        reviewer, implementer,
        "the model that reviews the tests must not be the one they will judge"
    );

    // And the order itself, read from the file rather than asserted about it.
    // `implementation` names only `test_authoring_review`; the edge back to
    // `test_authoring` comes from that stage's `audits`, so a direct-edge check
    // would conclude the implementer may go first.
    assert!(
        runs_after_transitively(Stage::Implementation, Stage::TestAuthoring),
        "the implementation must be declared downstream of the stage that wrote \
         the tests it has to satisfy"
    );
    assert!(
        runs_after_transitively(Stage::Implementation, Stage::TestAuthoringReview),
        "and downstream of the review of those tests"
    );
    assert!(
        !runs_after_transitively(Stage::TestAuthoring, Stage::Implementation),
        "and not, in the other direction, downstream of the code it is supposed \
         to precede -- which the loader also refuses as a cycle"
    );
}

#[test]
fn every_declared_stage_is_dispatched_or_named_as_not_yet() {
    // The table declares sixteen stages. Measured by dispatch function --
    // `run_stage(` and `run_stage_within(` across `src/`, which finds a call
    // through a variable that grepping for `Stage::` would miss -- five have a
    // caller. The other eleven are rows in a file and nothing else.
    //
    // That is not a defect to fix in passing: the sequencer that walks the
    // declared order is a feature with its own review loop. What IS a defect is
    // that it was invisible, and that a twelfth inert stage could be added the
    // same way. This list is the visibility, and it may only shrink: wiring a
    // stage means deleting a line here, and adding a stage means either giving
    // it a caller or writing it down where a reviewer sees it.
    const NOT_YET_DISPATCHED: &[Stage] = &[
        Stage::Recon,
        Stage::Planning,
        Stage::PlanReview,
        Stage::ArchitectSpec,
        Stage::TestAuthoring,
        Stage::TestAuthoringReview,
        Stage::Falsification,
        Stage::SecurityAudit,
        Stage::TestHardening,
        Stage::TestAudit,
        Stage::Orchestration,
    ];

    let mut dispatched = BTreeSet::new();
    let mut sites = 0usize;
    let mut stack = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("src listable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("readable");
            let code = anvil::source_scan::without_commentary(
                &anvil::source_scan::without_test_modules(&text),
            );
            for call in ["run_stage(", "run_stage_within("] {
                let mut from = 0;
                while let Some(at) = code[from..].find(call) {
                    let start = from + at + call.len();
                    // The stage is the first argument. Take up to the comma.
                    let arg = code[start..]
                        .split(',')
                        .next()
                        .unwrap_or_default()
                        .trim()
                        .trim_start_matches("crate::ai_driver::")
                        .trim_start_matches("anvil::ai_driver::");
                    if let Some(name) = arg.strip_prefix("Stage::") {
                        if let Some(s) = Stage::ALL.iter().find(|s| format!("{s:?}") == name) {
                            dispatched.insert(*s);
                            sites += 1;
                        }
                    }
                    from = start;
                }
            }
        }
    }

    // The instrument first: a census that found nothing would report every
    // stage undispatched and pass this test by being blind.
    assert!(
        sites >= 5,
        "the dispatch census found {sites} call sites, which is fewer than the \
         five measured by hand -- the scan is broken, not the code"
    );

    let declared_inert: BTreeSet<Stage> = NOT_YET_DISPATCHED.iter().copied().collect();
    let actually_inert: BTreeSet<Stage> = Stage::ALL
        .iter()
        .copied()
        .filter(|s| !dispatched.contains(s))
        .collect();

    let newly_wired: Vec<&Stage> = declared_inert.difference(&actually_inert).collect();
    assert!(
        newly_wired.is_empty(),
        "these stages now have a dispatcher and must come off the list -- the \
         ratchet only counts if it tightens: {newly_wired:?}"
    );

    let newly_inert: Vec<&Stage> = actually_inert.difference(&declared_inert).collect();
    assert!(
        newly_inert.is_empty(),
        "these stages are declared in config/model-routing.toml and nothing \
         dispatches them: {newly_inert:?}. Give the stage a caller, or add it \
         here so the gap is reviewed rather than discovered."
    );
}

#[test]
fn a_stage_may_not_list_the_same_tier_twice() {
    // `recon` and `planning` shipped with ten tiers, five unique, because a
    // regeneration script ran twice. Worst-case fallback latency doubled and
    // nothing objected: one test asserted only non-emptiness, the other read
    // the first copy via `.nth(1)`.
    let dup = r#"
[stage_meta.recon]
writes = []

[[stage.recon]]
model = "gemini-3.8-flash"
provider = "agy"
effort = "high"
timeout_secs = 600

[[stage.recon]]
model = "gemini-3.8-flash"
provider = "agy"
effort = "high"
timeout_secs = 600
"#;
    let e = anvil::ai_driver::chain::parse_table_for_test(dup)
        .expect_err("a repeated tier must be refused");
    assert!(
        e.to_string().contains("more than once"),
        "the error must name the repetition: {e}"
    );

    // And the shipped table has none.
    use std::collections::BTreeSet;
    for stage in Stage::ALL {
        let mut seen = BTreeSet::new();
        for t in chain(*stage) {
            assert!(
                seen.insert((format!("{:?}", t.provider), t.model.clone())),
                "`{}` lists {} twice",
                stage.key(),
                t.model
            );
        }
    }
}

#[test]
fn a_stage_may_cap_what_a_turn_there_costs() {
    // `tests/issue_triage_routing_test.rs` enforced `low` effort and <=90s on
    // the hardcoded chain. That module was deleted and the test went with it,
    // unmentioned in any commit message, and the replacement table declared
    // `high`/600s. The constraint matters more now, not less: the old table was
    // dead, and `ci_triager` dispatches this one on every failed CI run.
    //
    // So the ceiling lives in the file it bounds. A test can be deleted quietly;
    // a rule the loader enforces cannot be, because deleting it means deleting
    // the stage.
    use anvil::ai_driver::chain::plan;
    let triage = plan(Stage::IssueTriage);
    for t in &triage.tiers {
        assert_eq!(
            t.effort, "low",
            "issue triage is classification, not repair: {} declares {:?}",
            t.model, t.effort
        );
        assert!(
            t.timeout.as_secs() <= 90,
            "a triage call allowed {}s has stopped being cheap: {}",
            t.timeout.as_secs(),
            t.model
        );
    }

    // And the ceiling refuses a table that exceeds it.
    let over = r#"
[stage_meta.issue_triage]
max_effort = "low"
max_timeout_secs = 90
writes = []

[[stage.issue_triage]]
model = "gemini-3.8-flash"
provider = "agy"
effort = "high"
timeout_secs = 90
"#;
    let e = anvil::ai_driver::chain::parse_table_for_test(over)
        .expect_err("effort above the declared ceiling must be refused");
    assert!(e.to_string().contains("caps effort"), "wrong refusal: {e}");

    let slow = over
        .replace("effort = \"high\"", "effort = \"low\"")
        .replace(
            "timeout_secs = 90\n\n[[stage",
            "timeout_secs = 90\n\n[[stage",
        );
    let slow = slow.replace(
        "effort = \"low\"\ntimeout_secs = 90\n",
        "effort = \"low\"\ntimeout_secs = 600\n",
    );
    let e = anvil::ai_driver::chain::parse_table_for_test(&slow)
        .expect_err("a timeout above the declared ceiling must be refused");
    assert!(
        e.to_string().contains("stopped being cheap"),
        "wrong refusal: {e}"
    );
}

#[test]
fn a_declaration_the_hook_would_refuse_is_a_load_error() {
    // These are the reviewer's measured forms. Each ACCEPTED by the loader's
    // old three-clause check and then REFUSED by the hook -- which does not
    // fail the run, it refuses every commit in it, because a declaration the
    // consumer cannot parse is one under which nothing may be staged.
    //
    // The newline case is worse than refusal: the hook read it as two prefixes
    // and silently granted more than the file showed.
    for bad in [
        "./src",        // "/./" component
        "src//lib",     // empty component
        "c:src",        // drive letter
        "sr\\c",        // backslash
        "src\"x",       // quote
        "src\r",        // control byte
        "src\t",        // control byte
        "src ",         // trailing space, taken literally, matches nothing
        "\u{feff}src/", // BOM
        "docs/\nsrc/",  // newline: silently two prefixes
        "..",
        "/etc",
        "",
    ] {
        let t = format!(
            "[stage_meta.recon]\nwrites = [{}]\n\n[[stage.recon]]\nmodel = \"gemini-3.8-flash\"\nprovider = \"agy\"\neffort = \"high\"\ntimeout_secs = 600\n",
            serde_json::to_string(bad).expect("quotable")
        );
        let r = anvil::ai_driver::chain::parse_table_for_test(&t);
        assert!(
            r.is_err(),
            "the hook would refuse {bad:?}, so the loader must too -- otherwise \
             every commit in the run fails, or the scope silently widens"
        );
    }

    // And the forms the hook accepts still load.
    for good in ["src/", "src", "docs/plan/", "tests/"] {
        let t = format!(
            "[stage_meta.recon]\nwrites = [\"{good}\"]\n\n[[stage.recon]]\nmodel = \"gemini-3.8-flash\"\nprovider = \"agy\"\neffort = \"high\"\ntimeout_secs = 600\n"
        );
        let e = anvil::ai_driver::chain::parse_table_for_test(&t)
            .expect_err("this fixture declares only recon, so it fails on the missing chains");
        assert!(
            !e.to_string().contains("write prefix"),
            "{good:?} is a valid declaration and must not be refused as a prefix: {e}"
        );
    }
}

/// One valid stage, so a fixture only has to add the thing under test.
fn one_stage(key: &str, meta_extra: &str) -> String {
    format!(
        "[stage_meta.{key}]\n{meta_extra}writes = []\n\n\
         [[stage.{key}]]\nmodel = \"gemini-3.8-flash\"\nprovider = \"agy\"\n\
         effort = \"high\"\ntimeout_secs = 600\n"
    )
}

#[test]
fn a_key_the_loader_does_not_read_is_a_load_error() {
    // The reason this exists, measured. `runs_after` was declared on four
    // stages and `authored_before` on one, and the pull request that added them
    // said "ordering is data in a validated file". serde dropped every one of
    // them: the struct had no such field, unknown keys were accepted, and
    // nothing anywhere read them.
    //
    // Turning that into a load error found FOUR MORE the same day, none of
    // which had a reader either: `mode = "quorum"` on `code_review_audit` and
    // on `security_audit` -- while `run_stage` takes the first tier that
    // answers, so the file claimed a quorum and the dispatcher ran a fallback
    // chain -- plus `mode = "deterministic"` and `residual_conditions` on
    // `orchestration`, a stage nothing dispatches at all.
    //
    // A file that silently accepts decoration is a file whose claims cannot be
    // trusted, and every one of those keys read as a promise to anyone opening
    // it.
    let decorated = one_stage("recon", "mode = \"quorum\"\n");
    let e = anvil::ai_driver::chain::parse_table_for_test(&decorated)
        .expect_err("a key with no reader must be refused, not ignored");
    // `{e:#}` and not `{e}`: the serde failure is the CAUSE, behind a context
    // line about the file. `to_string()` shows only "does not parse", which
    // names neither the key nor the stage -- and this assertion passed on it
    // the first time by failing for the right reason with the wrong evidence.
    assert!(
        format!("{e:#}").contains("unknown field `mode`"),
        "the error must name the key that will not be read: {e:#}"
    );

    // A tier is deserialized by a different struct and needs its own proof.
    let tier = "[stage_meta.recon]\nwrites = []\n\n[[stage.recon]]\n\
                model = \"gemini-3.8-flash\"\nprovider = \"agy\"\neffort = \"high\"\n\
                timeout_secs = 600\nweight = 3\n";
    let e = anvil::ai_driver::chain::parse_table_for_test(tier)
        .expect_err("a decorative TIER key must be refused too");
    assert!(
        format!("{e:#}").contains("unknown field `weight`"),
        "the error must name it: {e:#}"
    );
}

#[test]
fn an_order_over_a_stage_that_does_not_exist_is_a_load_error() {
    let ghost = one_stage("recon", "runs_after = [\"planing\"]\n");
    let e = anvil::ai_driver::chain::parse_table_for_test(&ghost)
        .expect_err("an order over a stage that does not exist must be refused");
    assert!(
        e.to_string().contains("planing") && e.to_string().contains("no `Stage` names"),
        "the error must name the stage that does not exist -- a typo in an \
         ordering key is exactly how an edge goes missing: {e}"
    );
}

#[test]
fn an_order_that_admits_no_sequence_is_a_load_error() {
    // A cycle is not a style complaint. It means no sequence satisfies the
    // file, so a runner walking it either loops or silently drops one edge --
    // and which edge it drops is not written down anywhere.
    let cycle = format!(
        "{}{}",
        one_stage("recon", "runs_after = [\"planning\"]\n"),
        one_stage("planning", "runs_after = [\"recon\"]\n")
    );
    let e = anvil::ai_driver::chain::parse_table_for_test(&cycle)
        .expect_err("a cyclic order must be refused");
    let msg = e.to_string();
    assert!(
        msg.contains("cycle") && msg.contains("recon") && msg.contains("planning"),
        "the error must NAME the stages in the cycle, not merely report that one \
         exists -- a diagnostic that leaves the reader to find it is half a \
         diagnostic: {e}"
    );

    // Self-reference is the one-node case and must not slip through.
    let loop_one = one_stage("recon", "runs_after = [\"recon\"]\n");
    assert!(
        anvil::ai_driver::chain::parse_table_for_test(&loop_one).is_err(),
        "a stage that runs after itself must be refused"
    );
}

#[test]
fn one_relation_may_not_be_spelled_two_ways() {
    // `audits = X` IS an ordering claim: nothing can judge what has not run.
    // Restating it as `runs_after` gives one relation two spellings, and two
    // spellings drift -- which is precisely what happened. `test_authoring`
    // carried `authored_before = "implementation"`; `implementation` carried
    // `runs_after = ["test_authoring_review"]`; and `test_authoring_review`
    // claimed no order at all. Read as written, the file did not say the test
    // author ran before the implementer.
    let both = format!(
        "{}{}",
        one_stage("recon", ""),
        one_stage("planning", "audits = \"recon\"\nruns_after = [\"recon\"]\n")
    );
    let e = anvil::ai_driver::chain::parse_table_for_test(&both)
        .expect_err("restating what `audits` implies must be refused");
    assert!(
        e.to_string().contains("both audits"),
        "the error must say which relation is doubled: {e}"
    );

    // The shipped table has exactly one spelling per pair.
    for stage in Stage::ALL {
        let p = anvil::ai_driver::chain::plan(*stage);
        if let Some(audited) = &p.audits {
            assert!(
                !p.runs_after.contains(audited),
                "{} restates its `audits` edge in `runs_after`",
                stage.key()
            );
        }
    }
}
