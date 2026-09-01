//! One bin of `AUDITED_GATES` entries.
//!
//! The split is by size, not by subject: `registry.rs` held every entry and
//! was 1166 lines against a 300-line budget. The order of the corpus lives in
//! `registry::AUDITED_GATES`, so nothing here is meant to be read as a grouping.

use crate::fidelity::{Fidelity, GateFidelity};

pub const COVERAGE_STATUS: GateFidelity = GateFidelity {
    gate_id: "coverage_status",
    aspiration: "Measure statement and branch coverage of the lines added by this PR, via compiler \
                 instrumentation, and block below a threshold.",
    reference: "cargo-llvm-cov; Google TAP coverage instrumentation",
    fidelity: Fidelity::Aspirational,
    gap: "Runs no compiler coverage instrumentation tool. When no executable lines are added it reports \
          `NothingToMeasure`, and with no llvm-cov tool it reports `NotMeasured` (coverage_guard.rs::CoverageMeasurement and coverage_guard.rs::gate_status-138).",
    blocked_on: None,
};

pub const KANI_STATUS: GateFidelity = GateFidelity {
    gate_id: "kani_status",
    aspiration: "Discharge memory-safety proof obligations for unsafe blocks using a bounded model \
                 checker.",
    reference: "Kani / CBMC; AWS Automated Reasoning Group",
    fidelity: Fidelity::Heuristic,
    gap: "No bounded model checker is invoked at any point: no Kani, no CBMC, no Miri. What runs \
          is a comment-presence lint over the diff. It matches an added line whose first non-space \
          token opens an `unsafe` item and asks whether a `// SAFETY:` comment sits on that line or \
          the five above it (kani_guard/mod.rs::lint_unsafe_safety_comments), and publishes the tally as \
          `unsafe_blocks_with_safety_comment` (kani_guard/mod.rs::KaniGuardReport). Presence is the whole \
          property. Nothing here reads the comment: whether it is true, whether it describes the \
          block beneath it, and whether the obligations it names are discharged are all outside \
          what this gate can see. It is a reimplementation of clippy's undocumented-unsafe-blocks \
          lint, which upstream files under the opt-in restriction group rather than correctness. \
          The scan is narrower than the sentence it publishes, too: an `unsafe` block opened \
          mid-line is matched by no pattern here, as is any `unsafe` code this pull request leaves \
          untouched, so a clean verdict from this gate is not a statement that the change added no \
          unsafe code.",
    blocked_on: None,
};

pub const SLO_STATUS: GateFidelity = GateFidelity {
    gate_id: "slo_status",
    aspiration: "Evaluate multi-window multi-burn-rate error budget consumption against live production \
                 telemetry (14.4x/1h, 6x/6h).",
    reference: "Google SRE Workbook, multiwindow multi-burn-rate alerting",
    fidelity: Fidelity::Aspirational,
    gap: "Queries no telemetry. With no Prometheus or `OpenTelemetry` endpoint configured, the gate \
          structurally validates touched OpenSLO specs and reports `NotMeasured` \
          (slo_canary_guard/mod.rs::MISSING_TELEMETRY_SOURCE and slo_canary_guard/mod.rs::evaluate_slo_canary_health-157).",
    blocked_on: Some("a reachable Prometheus or OpenTelemetry endpoint"),
};

pub const TRACE_STATUS: GateFidelity = GateFidelity {
    gate_id: "trace_status",
    aspiration: "Verify that W3C trace context is propagated across every async boundary a change \
                 introduces, so a request's spans stay one trace.",
    reference: "W3C Trace Context; OpenTelemetry context propagation; tracing-futures `Instrument`",
    fidelity: Fidelity::Heuristic,
    gap: "Runs no tracing runtime and resolves no types, and reads only the diff hunks -- never the \
          file -- so a boundary this pull request keeps outside a hunk is invisible to it and \
          no fix below removes that. It text-scans the added and retained lines of Rust chunks \
          for a call whose final path segment is `spawn`, `spawn_blocking` or `spawn_local` and \
          whose argument list is not empty (trace_context_guard/span_tracker.rs::BOUNDARY_RE), then \
          asks whether `instrument` or `in_current_span` appears anywhere inside the \
          parenthesised region that call opens, minus the regions the boundaries nested in it \
          own (trace_context_guard/span_tracker.rs::new); the published sentence carries that \
          qualifier rather than the stronger claim. Appearing in the region is not attachment: \
          the call is not matched against the spawned future, so a span attached to some other \
          value inside the body reads as clean, and the gate can tell neither that the span is \
          the right one nor that it is attached to this task at all. The published sentence \
          says what is measured rather than the word attaches. A task instrumented at its \
          definition by a tracing instrument attribute has no such call at the spawn site and is \
          reported detached (trace_context_guard/span_tracker.rs::INSTRUMENT_RE), and a boundary crossed \
          by a call it cannot name that way -- a spawn imported under another name, or a task \
          started by code this diff does not touch -- it cannot see at all. \
          It errs in the other direction too, and that is the direction that blocks a merge. \
          The matcher is nominal: no type is resolved, so any non-empty call whose final path \
          segment is one of those three names is treated as a task boundary and reported -- a \
          thread pool, a process supervisor, an actor handle, a domain type of one is own -- \
          and there is no allowlist, no annotation and no configuration by which an author can \
          say otherwise; the verdict is `Failed`, which blocks. Only a definition is excluded, \
          by the `fn` keyword before the name (trace_context_guard/span_tracker.rs::is_declaration), so \
          a wrapper named spawn is left alone where it is declared but reported at every call \
          site through it. And a span attached before the spawn call rather than inside it -- a \
          future built in steps and then handed over as a binding, which is the shape any \
          non-trivial spawn takes -- sits outside the parenthesised region, is not seen, and \
          the boundary is reported detached. \
          A region is bounded by the hunk the boundary opened in: the hunks of a file are \
          disjoint windows onto it, so the scan runs once per hunk and a boundary whose \
          parenthesis does not close inside its own hunk is `unresolved` -- seen and not \
          judged, not counted among the inspected, and not accused \
          (trace_context_guard/mod.rs::evaluate_trace_propagation). Every boundary on a line is inspected, not merely \
          the first via `BOUNDARY_RE` (trace_context_guard/span_tracker.rs::scan). Lexing is a line scanner \
          with state carried across lines, blanking string literals, raw strings, character \
          literals, line comments and block comments before anything counts a parenthesis \
          (trace_context_guard/span_tracker.rs::strip_noise); its state starts at code at the top of \
          every hunk, so a hunk whose first line is already inside a literal or a block comment \
          is read as code until that literal closes, and a `\'` that is neither a character \
          literal nor a lifetime is read as a lifetime. The diff is cut into one chunk per file \
          at a line beginning `diff --git ` (trace_context_guard/mod.rs::file_chunks), and a chunk \
          carrying no `+++ ` header names no path and is not read. An accusation names \
          post-image lines derived from the a hunk header header \
          (trace_context_guard/mod.rs::rust_path_of) -- a chunk with no such header declares no \
          position, a removed line is not numbered, and neither is the \
          a no-newline marker marker git writes mid-hunk -- and it covers retained \
          lines as well as added ones, because a region walk that skipped context lines could \
          bound nothing, so a named line may be one the change only carried past rather than \
          wrote; the failing sentence says so. \
          What the guard decides is what is published: it builds its own `GateStatus` and \
          `trace_status` clones it (pre_merge_guard/evaluator.rs::evaluate_pre_merge_gates), the shape \
          `slo_status` already used. A diff crossing no boundary it can see is \
          `NotApplicable` (trace_context_guard/mod.rs::evaluate_trace_propagation): the subject \
          set was searched and found empty, which neither blocks nor accuses, and is not \
          `Passed`, which carries no string and would discard the sentence. It was `Warning` \
          until the admission rule stopped treating every absence as a defect; `Warning` was \
          the only variant left that carried the sentence and blocked nothing, so it appeared \
          among the findings needing action on every pull request touching no async boundary. \
          A boundary seen and not judged is `NotMeasured` under this gate is own id \
          (trace_context_guard/mod.rs::evaluate_trace_propagation), which publishes the count and the reason and \
          blocks merge-queue admission through `is_admissible` \
          (pre_merge_guard/report.rs::is_admissible) -- undeclared in \
          `ABSENCE_POLICY` (pre_merge_guard/admission.rs::ABSENCE_POLICY), so it is a gate that could \
          have measured and did not. \
          What remains: the \
          scorecard renderer collapses an admissible report to a single verdict line and \
          enumerates no finding at all (publish/scorecard.rs::render), so on a pull request \
          carrying no other finding the warning row is still not printed and the row \
          \"End-to-end span instrumentation across async tasks\" \
          (pre_merge_guard/gate_labels.rs::GATE_LABELS) is counted in the total without a word. Making a \
          warning visible on a certified scorecard is a change to that renderer, which every \
          gate shares.",
    blocked_on: None,
};

pub const REMOTE_CACHE_STATUS: GateFidelity = GateFidelity {
    gate_id: "remote_cache_status",
    aspiration: "Report the real distributed build-cache hit rate and ratchet it upward.",
    reference: "Bazel/Buck2 remote execution CAS statistics",
    fidelity: Fidelity::Aspirational,
    gap: "With no sccache or Buck2 CAS statistics endpoint configured, reports `NotMeasured` \
          (remote_cache_optimizer/mod.rs::evaluate_cache_alignment). Cache keys use non-cryptographic FNV-1a hashing via \
          `compute_cache_key` (remote_cache_optimizer/cache_keys.rs::compute_cache_key).",
    blocked_on: Some("sccache or Buck2 CAS statistics"),
};

pub const MUTATION_STATUS: GateFidelity = GateFidelity {
    gate_id: "mutation_status",
    aspiration: "Build a mutant of every function the change touches, run the suite against each one, \
                 and block on any the suite fails to kill.",
    reference: "cargo-mutants; Google mutation testing at review time (Petrovic & Ivankovic, ICSE-SEIP \
                2018), which surfaces only surviving mutants on changed lines",
    fidelity: Fidelity::Partial,
    gap: "The filename match is gone: `run_bounded_for` spawns cargo-mutants with `--in-diff` over \
          the pull request's own diff (chaos_mutation_guard.rs::ChaosMutationGuard), so the suite really is run \
          against each mutant on the changed lines, and one it fails to kill is published as \
          `GateStatus::Failed` naming it. A seeded-defect fixture runs the real tool against a \
          deliberately inadequate suite and requires that failure. Measured nowhere it currently \
          runs, for two reasons and not one. (1) This repository's CI installs no cargo-mutants \
          (.github/workflows/ci.yml), so every run ends `Unavailable` and publishes `NotMeasured` \
          (chaos_mutation_guard.rs::gate_status). (2) Installing it would not change that: cargo-mutants \
          copies the tree without a .git directory, and the daemon-tree tests in \
          change_delivery_lane_test.rs run the lane at CARGO-MANIFEST-DIR, where the shape \
          adapter shells out to \"rev-parse\" (shape/adapters/git_tree_at_rev.rs::GitTreeAtRev) and fails in \
          the copy -- so the real tool exits 4 in about 30 s, which is `NotMeasured`, correctly, \
          with no false green. Unmeasured at this budget too: a diff this size generates 41 \
          mutants against a `MUTATION_BUDGET` that buys 8 to 17 of them \
          (chaos_mutation_guard.rs::MUTATION_GATE_ID). Still missing: no kill-rate threshold, no \
          equivalent-mutant suppression and no arid-node rules, so every survivor is reported \
          whether or not it is killable.",
    blocked_on: Some(
        "the daemon-tree tests running without .git (tests/change_delivery_lane_test.rs::the_daemon_tree_is_refused_unless_explicitly_allowed), \
         then the cargo-mutants binary on the runner and a budget that fits the mutant count",
    ),
};

pub const SUPPLY_CHAIN_STATUS: GateFidelity = GateFidelity {
    gate_id: "supply_chain_status",
    aspiration: "Resolve the dependency graph, match it against a live vulnerability database, and \
                 emit a signed SBOM.",
    reference: "osv-scanner, cargo-deny, CycloneDX",
    fidelity: Fidelity::Partial,
    gap: "No SBOM is produced -- neither syft nor cargo-cyclonedx is invoked -- no provenance is \
          signed, and no deny.toml license or ban policy is evaluated. The audit half is real: \
          `query_batch` sends every locked version to the OSV advisory database \
          (supply_chain_guard.rs::SupplyChainGuard), and a runner that cannot reach it publishes NotMeasured \
          rather than a pass. It reads one lockfile and only one: repo_dir.join with Cargo.lock \
          (supply_chain_guard.rs::SupplyChainGuard). Every repository in the fleet that is not a Cargo \
          workspace is therefore permanently NotMeasured on this gate -- a narrowing, since the \
          regex this replaced at least read a package.json filename -- and the reference tool \
          reads any recognised lockfile. Advisory lists are complete or absent, never short: a \
          next_page_token in any result aborts the audit (osv_stream.rs::OsvAdvisoryStream) rather than \
          publishing a truncated first page as the answer.",
    blocked_on: Some("an SBOM generator and a hosted signing platform; the advisory half is done"),
};

pub const FORMAL_VERIFICATION_STATUS: GateFidelity = GateFidelity {
    gate_id: "formal_verification_status",
    aspiration: "Encode authorization policy into SMT and prove non-escalation with a solver.",
    reference: "AWS Zelkova; Z3",
    fidelity: Fidelity::Heuristic,
    gap: "A chain of policy_content.contains(..) tests. No solver exists. The file and its types \
          were renamed from smt_solver.rs/SmtConstraintEngine to say so \
          (formal_verification/policy_scanner.rs::PolicyPatternScanner). The rename stopped one line short of the \
          verdict a reviewer reads: the gate discarded the findings and published in their place \
          a fixed sentence asserting that an SMT constraint solver had detected an unsafe state. \
          The findings carry the rule that matched and the text it matched on, and the gate now \
          reports those. Two further \
          defects went with it -- the scan read the whole diff including removals, so a pull \
          request DELETING a wildcard grant was refused for containing it, and a report with no \
          findings was `passed` whether a policy had been examined or the diff contained none, so \
          a change touching no policy file published a green formal-verification gate. Only added \
          lines in a policy file are scanned, and policy_files_seen \
          (formal_verification/mod.rs::policy_files_seen) makes an empty scan NotMeasured. Coverage is the stated \
          set of paths in is_policy_path (formal_verification/mod.rs::is_policy_path) and no wider.",
    blocked_on: None,
};

pub const DEADLOCK_STATUS: GateFidelity = GateFidelity {
    gate_id: "deadlock_status",
    aspiration: "Build an inter-procedural lock-acquisition graph and detect cycles.",
    reference: "Tarjan strongly connected components; Meta Infer Starvation; lockbud",
    fidelity: Fidelity::Heuristic,
    gap: "A graph is built and its cycles are reported, but the nodes are receiver expressions \
          spelled as the source spells them, not lock instances: no type is resolved and nothing is \
          aliased, so two spellings of one lock are two nodes and one spelling of two locks is one \
          node. An acquisition is any zero-argument `.lock()`, `.read()` or `.write()` matched by \
          name (lock_graph.rs::ACQUISITIONS), and a guard counts as held only where a plain `let` binds it, \
          with its scope approximated by brace depth (`guard_binding`, lock_graph.rs::guard_binding) and \
          ended early by an explicit `dropped_bindings` (lock_graph.rs::dropped_bindings) -- without that, the \
          idiomatic release-then-reacquire read as a self-deadlock. Braces are counted after \
          string and character literals are blanked (`without_literals`, lock_graph.rs::without_literals), \
          because an unbalanced brace inside a literal corrupted the depth and could leak a \
          guard into the next function. Edges come from the text of the diff alone \
          (`acquisition_edges`, lock_graph.rs::LockOrderGraph), so an inversion split across a call this \
          change does not touch is invisible -- that is the missing call graph, not a tuning \
          problem. Only cycles are reported (`find_lock_order_cycles`, lock_graph.rs::LockOrderGraph), never \
          nestings, because holding two locks at once is how correct code works. What is \
          reported is a strongly connected component and the field carrying it is named `locks`, \
          not a sequence: no witness path through the cycle is reconstructed.",
    blocked_on: Some(
        "MIR-level guard liveness and points-to analysis, for lock identity across \
                      aliases and call edges",
    ),
};
