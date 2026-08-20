//! The boundary between `ai_driver`'s novel half and its superseded half.
//!
//! DEFECT UNDER TEST
//! -----------------
//! `src/ai_driver` is one module carrying two fates. The migration ledger
//! (`src/migration/registry.rs`, entry `"ai_driver"`) marks it `Rewired` with
//! `Verified` confidence, and the audit's evidence string splits it:
//!
//!   * the SUPERSEDED half -- `router.rs` (`SubscriptionExecutor`, 623 lines,
//!     spawns `claude` / `codex` / `cursor-agent` / `grok` / `agy` via
//!     `tokio::process::Command`) and the model routing around it -- has an
//!     oyatie counterpart: `intelligence/adapters/cli-session-driver` plus
//!     `model-routing-{kernel,usecase}`, `route-policy-kernel`,
//!     `provider-pool-app`;
//!   * the NOVEL half -- `task_classifier.rs`, the `AdaptiveRoutingBandit` in
//!     `telemetry_ledger.rs`, and `cross_model_validator.rs` -- has none.
//!
//! Measured on this tree, not quoted from the audit:
//!   task_classifier.rs 206, telemetry_ledger.rs 431, cross_model_validator.rs 190
//!   (`wc -l`), router.rs 623, stage_router.rs 466, provider.rs 86, mod.rs 15.
//!
//! The defect is that the two halves are welded together, so neither can move:
//!
//!   D1. `CrossModelDualValidator` *owns* a concrete `SubscriptionExecutor`
//!       (`cross_model_validator.rs:7` imports it, `:21` stores it, and `new()`
//!       constructs it at `:27`). Its consensus arithmetic -- the novel part -- therefore
//!       cannot be exercised without spawning vendor CLI subprocesses. The
//!       consequence is already visible in the tree: the file's only unit test
//!       hand-builds a `CrossModelConsensusReport` literal and asserts on the
//!       literal, so not one line of `verify_cross_model_consensus` is covered.
//!       (`grep -rn verify_cross_model_consensus src/ tests/` returns exactly one
//!       hit: the definition. Nothing calls it.)
//!
//!   D2. `telemetry_ledger.rs:6` imports `super::stage_router::AgenticStage`, and
//!       `stage_router.rs:7` imports `super::router::SubscriptionExecutor`. The
//!       bandit -- 431 lines of UCB1, Beta-prior shrinkage and Pareto reward that
//!       touch no process, no network and no vendor -- cannot be compiled out of
//!       this crate without dragging the subscription executor with it.
//!
//! WHAT THESE TESTS ASSERT
//! -----------------------
//! Not that the executor is deleted -- it is live and running, and the ledger is
//! explicit that `Superseded` is a destination, not an instruction. They assert
//! that the novel half depends on the executor only through an abstraction: no
//! novel file names a concrete subscription/CLI type, and no chain of imports
//! from a novel file reaches `router.rs` at all.
//!
//! The behavioural proof lives next door in `tests/ai_driver_executor_port_test.rs`,
//! which drives `verify_cross_model_consensus` against a scripted test double. That
//! file is a compile-level guarantee and cannot be satisfied by a cosmetic edit: a
//! validator that is generic over a port has no syntax in which to name
//! `SubscriptionExecutor`. It is gated by `required-features` only so that this
//! lane does not leave `cargo test` and `cargo clippy --all-targets` unable to
//! build; `the_compile_level_port_test_is_not_left_behind_a_feature_gate` below
//! fails until the gate is removed, so the gate cannot outlive the port.
//!
//! WHY PROMPTING WOULD NOT PREVENT THIS
//! ------------------------------------
//! "Keep the novel logic decoupled from the executor" is invisible to the
//! compiler. Rust is happy to let a pure statistics module import a process
//! spawner: the crate builds, every existing test passes, the dashboard renders
//! bandit views, and the entanglement shows up only at absorption time, months
//! later, as "we cannot delete `router.rs` because the bandit imports through it"
//! -- at which point the cheapest fix is to keep the superseded code. Worse, the
//! coupling is *self-concealing*: because `CrossModelDualValidator` cannot be
//! constructed without a real executor, nobody writes the test that would have
//! exposed it, and the absence of that test reads as "nothing to test here".
//! Only a check that keeps running after the prompt is gone notices.

use regex::Regex;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

/// The three files the audit identified as having no oyatie counterpart.
const NOVEL_UNITS: &[&str] = &[
    "src/ai_driver/task_classifier.rs",
    "src/ai_driver/telemetry_ledger.rs",
    "src/ai_driver/cross_model_validator.rs",
];

/// The module inside `ai_driver` that spawns vendor CLI subprocesses.
const EXECUTOR_MODULE: &str = "router";

/// Identifiers that only exist because a subscription CLI is being driven. A
/// novel unit that names one of these has a concrete dependency on the
/// superseded half, whatever the import line says.
const SUBSCRIPTION_EXECUTOR_TOKENS: &[&str] = &[
    "SubscriptionExecutor",
    "AccountPoolManager",
    "account_pool",
    "tokio::process",
    "run_with_prompt_on_stdin",
    "run_claude_subscription",
    "run_openai_subscription",
    "run_cursor_agent_subscription",
    "run_grok_subscription",
    "run_agy_subscription",
    "run_ensemble_subscription",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The production half of a source file: everything above `#[cfg(test)]`.
///
/// A test module is allowed to name the executor -- a test double for it, or an
/// ignored live-CLI test, is legitimate. The defect is a reference in the half
/// that ships.
fn production_half(rel: &str) -> String {
    let p = repo_root().join(rel);
    let s = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    match s.find("#[cfg(test)]") {
        Some(i) => s[..i].to_string(),
        None => s,
    }
}

/// Every `.rs` file directly under `src/ai_driver`, as a module name.
fn ai_driver_modules() -> BTreeSet<String> {
    let dir = repo_root().join("src/ai_driver");
    let mut out = BTreeSet::new();
    for entry in std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .flatten()
    {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str())
            && stem != "mod"
        {
            out.insert(stem.to_string());
        }
    }
    assert!(
        out.contains(EXECUTOR_MODULE),
        "src/ai_driver/{EXECUTOR_MODULE}.rs is gone. This test's whole premise is that the \
         executor is still here and still live -- if it really was deleted, delete this \
         assertion deliberately rather than letting the scan silently pass over nothing."
    );
    out
}

/// Sibling `ai_driver` modules that `rel`'s production half references, by any
/// syntax: `use super::x::`, `use crate::ai_driver::x::`, or a bare `super::x::T`
/// path written inline.
///
/// Import-line matching alone would be evaded by the first person who writes
/// `super::router::SubscriptionExecutor::new()` inline instead of importing it.
fn ai_driver_edges(rel: &str, modules: &BTreeSet<String>) -> BTreeSet<String> {
    let src = production_half(rel);
    let re = Regex::new(r"(?:super|crate::ai_driver)::([A-Za-z_][A-Za-z0-9_]*)")
        .expect("edge regex compiles");
    let self_name = Path::new(rel)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();

    let mut out = BTreeSet::new();
    for caps in re.captures_iter(&src) {
        let name = caps[1].to_string();
        if name != self_name && modules.contains(&name) {
            out.insert(name);
        }
    }
    out
}

/// The whole intra-`ai_driver` import graph, production halves only.
fn ai_driver_graph() -> BTreeMap<String, BTreeSet<String>> {
    let modules = ai_driver_modules();
    let mut graph = BTreeMap::new();
    for m in &modules {
        let rel = format!("src/ai_driver/{m}.rs");
        graph.insert(m.clone(), ai_driver_edges(&rel, &modules));
    }
    graph
}

/// Shortest import chain from `start` to `target`, or `None` if unreachable.
///
/// Reported as a path rather than a boolean because "telemetry_ledger reaches
/// router" is not actionable; "telemetry_ledger -> stage_router -> router" names
/// the edge to cut.
fn import_chain(
    graph: &BTreeMap<String, BTreeSet<String>>,
    start: &str,
    target: &str,
) -> Option<Vec<String>> {
    let mut prev: BTreeMap<String, String> = BTreeMap::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut queue = VecDeque::new();
    seen.insert(start.to_string());
    queue.push_back(start.to_string());

    while let Some(node) = queue.pop_front() {
        if node == target {
            let mut chain = vec![node.clone()];
            let mut cur = node;
            while let Some(p) = prev.get(&cur) {
                chain.push(p.clone());
                cur = p.clone();
            }
            chain.reverse();
            return Some(chain);
        }
        for next in graph.get(&node).into_iter().flatten() {
            if seen.insert(next.clone()) {
                prev.insert(next.clone(), node.clone());
                queue.push_back(next.clone());
            }
        }
    }
    None
}

/// D1: no novel unit names a concrete subscription/CLI type.
///
/// Catches the direct weld: `cross_model_validator.rs` importing and storing a
/// `SubscriptionExecutor`, and any future novel file that reaches for the
/// account pool or `tokio::process` because it is one import away.
///
/// Prompting cannot prevent it because reaching for the concrete type is the
/// path of least resistance -- it is in scope, it compiles, and the reviewer
/// sees a two-line diff. Nothing in the language or the build objects.
#[test]
fn novel_units_never_name_a_concrete_subscription_executor_type() {
    let mut offences: Vec<String> = Vec::new();

    for rel in NOVEL_UNITS {
        let src = production_half(rel);
        for (idx, line) in src.lines().enumerate() {
            for token in SUBSCRIPTION_EXECUTOR_TOKENS {
                if line.contains(token) {
                    offences.push(format!("{rel}:{}: {} <- {}", idx + 1, line.trim(), token));
                }
            }
        }
    }

    assert!(
        offences.is_empty(),
        "the novel half of ai_driver names the superseded executor directly, so neither half \
         can move: the bandit and the validator cannot migrate to oyatie while they depend on \
         a CLI spawner, and the spawner cannot be deleted while they do. Depend on an \
         abstraction instead. Offending references:\n  {}",
        offences.join("\n  ")
    );
}

/// D2: no chain of imports from a novel unit reaches `router.rs`.
///
/// The direct-token scan above is not enough. `telemetry_ledger.rs` names no
/// executor type at all -- it imports `AgenticStage` from `stage_router`, which
/// imports `SubscriptionExecutor` from `router`. The novel unit is still welded
/// to the process spawner; the weld just has one more link in it.
///
/// Prompting cannot prevent it because a transitive edge is invisible at the
/// point of writing: `use super::stage_router::AgenticStage` looks like a pure
/// enum import, and it is one -- the coupling lives in a file the author never
/// opened.
#[test]
fn no_import_chain_from_a_novel_unit_reaches_the_executor_module() {
    let graph = ai_driver_graph();
    let mut offences: Vec<String> = Vec::new();

    for rel in NOVEL_UNITS {
        let start = Path::new(rel)
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("novel unit path has a stem");
        if let Some(chain) = import_chain(&graph, start, EXECUTOR_MODULE) {
            offences.push(chain.join(" -> "));
        }
    }

    assert!(
        offences.is_empty(),
        "these import chains keep the novel half compiled against the subscription executor, \
         so the novel half cannot be built or tested without it:\n  {}\n\
         Cut the chain: the novel side should reach the executor only through a port, and \
         should share no module with it that carries a concrete CLI dependency.\n\
         Full intra-ai_driver import graph: {:?}",
        offences.join("\n  "),
        graph
    );
}

/// The `Rewired` verdict means the port survives and the adapter is swapped. A
/// port that nothing implements is not a port, and a `SubscriptionExecutor` that
/// implements nothing cannot be swapped for an oyatie-backed adapter without
/// touching every call site.
///
/// This asserts the other half of the boundary: that the abstraction the novel
/// code depends on is the same one the live executor satisfies today, so the
/// executor keeps running unchanged while the novel side stops naming it.
///
/// Prompting cannot prevent it because "introduce an abstraction" is routinely
/// satisfied by a trait that only the new code implements, leaving the real
/// adapter outside the boundary -- which reads as decoupled and is not.
#[test]
fn the_live_executor_implements_the_port_the_novel_half_depends_on() {
    let dir = repo_root().join("src/ai_driver");
    let re =
        Regex::new(r"impl\s+(?:<[^>]*>\s*)?([A-Za-z_][A-Za-z0-9_]*)\s+for\s+SubscriptionExecutor")
            .expect("impl regex compiles");

    let mut traits: BTreeSet<String> = BTreeSet::new();
    for entry in std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .flatten()
    {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let rel = format!(
            "src/ai_driver/{}",
            p.file_name().and_then(|s| s.to_str()).unwrap_or_default()
        );
        for caps in re.captures_iter(&production_half(&rel)) {
            traits.insert(caps[1].to_string());
        }
    }

    // Blanket derive-style impls are not ports.
    traits.remove("Default");
    traits.remove("Clone");
    traits.remove("Debug");

    assert!(
        !traits.is_empty(),
        "no trait is implemented for SubscriptionExecutor anywhere in src/ai_driver, so there \
         is no port -- only a concrete type that callers must name. The ledger's `Rewired` \
         verdict for ai_driver promises the port survives absorption and the adapter is \
         swapped; that promise is unfulfillable while the adapter is the only thing that \
         exists."
    );
}

/// The compile-level test must not stay switched off.
///
/// `tests/ai_driver_executor_port_test.rs` is the real guarantee -- it drives the
/// consensus logic through a test double, and it only compiles once the port
/// exists. Until then it cannot compile, and a test target that cannot compile
/// takes `cargo test` and `cargo clippy --all-targets` down with it for every
/// unrelated test in the tree. So it is declared in `Cargo.toml` with
/// `required-features`, and this test fails while that gate is present.
///
/// Prompting cannot prevent it because a feature-gated test is invisible: it
/// never runs, never fails, and never appears in a green run. Gates like this
/// outlive the reason they were added unless something fails while they exist.
#[test]
fn the_compile_level_port_test_is_not_left_behind_a_feature_gate() {
    let manifest =
        std::fs::read_to_string(repo_root().join("Cargo.toml")).expect("Cargo.toml is readable");

    assert!(
        manifest.contains("ai_driver_executor_port_test"),
        "tests/ai_driver_executor_port_test.rs is not declared in Cargo.toml at all. It is the \
         only check that proves the novel consensus logic runs without a subscription CLI; \
         losing it loses the guarantee."
    );

    let gated = manifest.split("[[test]]").any(|block| {
        block.contains("ai_driver_executor_port_test") && block.contains("required-features")
    });

    assert!(
        !gated,
        "the compile-level port test is still gated behind `required-features`, so it never \
         builds in a normal `cargo test` run and its guarantee is worth nothing. Once \
         src/ai_driver exposes the executor port, delete the `required-features` line (and \
         the now-unused feature) so the test runs by default."
    );
}
