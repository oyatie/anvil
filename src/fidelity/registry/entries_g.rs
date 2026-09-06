//! One bin of `AUDITED_GATES` entries.
//!
//! The split is by size, not by subject: `registry.rs` held every entry and
//! was 1166 lines against a 300-line budget. The order of the corpus lives in
//! `registry::AUDITED_GATES`, so nothing here is meant to be read as a grouping.

use crate::fidelity::{Fidelity, GateFidelity};

pub const API_CONTRACT_STATUS: GateFidelity = GateFidelity {
    gate_id: "api_contract_status",
    aspiration: "Validate every OpenAPI document the change touches against the specification, and \
                 reconcile the routes the service registers against the paths the document \
                 declares, in both directions.",
    reference: "OpenAPI 3.1 specification; Spectral and openapi-spec-validator; oasdiff for \
                contract diffing",
    fidelity: Fidelity::Heuristic,
    gap: "Parses no OpenAPI document and reconciles no routes; the two-way route sync in the \
          published label names nothing here. Scope is decided by substring over the changed \
          paths -- `openapi`, `route`, `handler`, `controller`, `.proto` -- and a change touching \
          none of them returns early with `is_intact` already true \
          (api_contract_guard.rs::ensure_contract_integrity). What the gate can run are two scripts \
          belonging to the repository under review, `scripts/check-openapi-refs.mjs` and \
          `scripts/union-openapi.py`. On a repository shipping neither -- this one included -- \
          nothing executes at all, and the report's own account of that outcome is `OpenAPI \
          schemas and API contracts are 100% in sync with zero drift.`, a claim no comparison \
          produced. No reader sees it: the wiring turns the boolean into a bare pass, which \
          carries no sentence, so the gate is silent exactly where it has least evidence. The \
          reconciliation half is a `git status --porcelain` read filtered to paths containing \
          `openapi`, `schema` or `contract`; `synced_files` being non-empty then CLEARS a failing \
          script, so a dirty file whose name matches is treated as the drift having been repaired. \
          `ApiContractReport` publishes `is_intact`, which is one boolean for a checked contract \
          and for a change nothing looked at (api_contract_guard.rs::ApiContractReport).",
    blocked_on: None,
};

pub const BENCH_STATUS: GateFidelity = GateFidelity {
    gate_id: "bench_status",
    aspiration: "Run the benchmark suite against this change and against the trunk it merges into, \
                 and refuse a hot path whose latency or allocation profile regresses past a stated \
                 budget.",
    reference: "criterion.rs; Google Fleetbench; regression budgets over a published baseline",
    fidelity: Fidelity::Heuristic,
    gap: "Runs no benchmark. Criterion is named in the type and invoked nowhere, this crate \
          declares no criterion dependency and has no benches directory, so no latency is sampled \
          on either side of the change and there is nothing for a three-percent budget to compare. \
          Two regexes over added lines stand in for it \
          (criterion_bench_ratchet.rs::evaluate_benchmarks). `clone_in_loop_re` fires only where the \
          line already carries a hand-written hotpath comment, so the author declares the finding \
          before the gate can make it; `unbounded_alloc_re` requires the collection to be bound to \
          a name spelled v inside a for loop, and any other name is invisible to it. \
          `hot_paths_evaluated` counts changed PATHS whose text contains bench, hotpath, proto, \
          serialize, hash or crypto -- nothing inside those files is read -- and that count is \
          spent on the sentence `Micro-benchmarks verified: {} hot path(s) evaluated within the \
          +3% latency & zero-leak budget.`, asserting a budget no measurement fed. The sentence is \
          then discarded: a clean verdict is `GateStatus::Passed`, which carries no string, and a \
          finding is `GateStatus::Warning`, so nothing here withholds a merge \
          (pre_merge_guard/evaluator.rs::evaluate_pre_merge_gates).",
    blocked_on: None,
};

pub const BRAND_ABSENCE_STATUS: GateFidelity = GateFidelity {
    gate_id: "brand_absence_status",
    aspiration: "Every declared name and every string a pull request sees describes what the code \
                 verifies rather than what its author was reaching for.",
    reference: "the naming aspect of Google's code review guidance; this repository's own \
                postmortem on guards that do not guard",
    fidelity: Fidelity::Heuristic,
    gap: "Membership in a fixed word list is the whole rule for two of its three checks: a \
          declared name or a string literal is a violation where it contains one of the recorded \
          stamps, and clean otherwise. So the failure this gate is named for is the one it cannot \
          see. A gate named for a model checker that runs a comment lint, or a ratchet named for a \
          benchmark that runs a regex, carries no listed word and passes here; the registry you \
          are reading exists because that class had to be found by hand. The third check does \
          measure: a hardcoded count claim in a string is compared against the corpus size \
          `real_gate_count` derives from the report's own fields \
          (brand_absence/mod.rs::real_gate_count). Nothing blocks either way -- `WARN_ONLY` is \
          fixed true, so the report's own severity is a warning on every path \
          (brand_absence/mod.rs::WARN_ONLY). And the subject is not the tree the published sentence \
          names: `scan_tree` walks the `repo_root` it is handed, which the evaluator fills with \
          `repo_working_dir`, the working directory of the repository under review \
          (brand_absence/mod.rs::scan_tree and pre_merge_guard/evaluator.rs::evaluate_pre_merge_gates), \
          while the debt ledger and the corpus count come from this crate's own manifest \
          directory. The verdict still reads `site(s) in Anvil's own tree` whichever tree was \
          walked (brand_absence/mod.rs::gate_status).",
    blocked_on: None,
};

pub const CELL_ISOLATION_STATUS: GateFidelity = GateFidelity {
    gate_id: "cell_isolation_status",
    aspiration: "Every query the change adds is scoped to the tenant that issued it, and no code \
                 path reaches across a cell boundary.",
    reference: "AWS cell-based architecture (Well-Architected reliability pillar); row-level \
                security and tenant scoping in multi-tenant stores",
    fidelity: Fidelity::Heuristic,
    gap: "Reads no schema, no tenant model and no cell topology: two regexes over the lines the \
          diff adds are the entire gate (cell_isolation_guard.rs::evaluate_cell_isolation). The \
          first accuses any single line matching a SELECT, DELETE or UPDATE with a WHERE in it \
          that does not also contain the text `tenant_id`, so a query scoped by a column spelled \
          any other way is reported as a leak, a comment or an unrelated identifier carrying that \
          text excuses one that is not scoped at all, and a statement built across lines or by a \
          query builder is matched by nothing. The second, `raw_socket_re`, matches only a literal \
          dotted-quad address and port inside quotes in a `TcpStream::connect` call; a hostname, a \
          variable, or any other client or transport crosses no boundary it can see. \
          `is_isolated` is the emptiness of that finding list \
          (cell_isolation_guard.rs::CellIsolationReport), so a change containing no query at all \
          reaches the same verdict as one that was checked, under the report's sentence `Cell \
          boundaries and tenant isolation invariants verified; zero cross-tenant query leaks.` -- \
          which the bare pass then drops. A finding blocks.",
    blocked_on: None,
};

pub const CLEAN_ARCH_STATUS: GateFidelity = GateFidelity {
    gate_id: "clean_arch_status",
    aspiration: "No dependency points outward-in: a unit's core and ports never name its adapters \
                 or facade, and no unit reaches past another unit's facade into its interior.",
    reference: "Robert C. Martin, Clean Architecture; Cockburn's ports and adapters; the four \
                faces the tenant's shape spec declares",
    fidelity: Fidelity::Heuristic,
    gap: "Resolves no type and builds no module graph. A file's layer is decided by substring on \
          its own path -- `/core/`, `/ports/`, `/adapter`, `/facade/` and their single-file \
          spellings -- so a unit whose layers are not spelled in its directory names is UNLAYERED \
          and its edges go unjudged (clean_architecture_guard/paths.rs::classify_layer), and the \
          `core_forbidden_imports` rules that follow are regexes over the text of lines that look \
          like imports (clean_architecture_guard/analyze.rs::analyze_unified_diff). The facade seal \
          is the sharper half and it is real: `expand_use_groups` flattens grouped and nested \
          paths first, the match is anchored on the identifier shape rather than on one path per \
          line, and a reference rooted in a crate this repository does not own is skipped rather \
          than accused (clean_architecture_guard/scan.rs::scan_faces). It is still text. An edge \
          spelled through a re-export, a type alias, a macro or a trait method is not an edge it \
          sees, and the diff entry point reads hunks, so a dependency this change leaves alone is \
          outside the subject entirely. `FACADE_BYPASSES_IN_ANVIL` records what the tree currently \
          holds (clean_architecture_guard/mod.rs::FACADE_BYPASSES_IN_ANVIL). What the gate gets \
          right, and most of this corpus does not, is the third state: a run that classified \
          nothing reports no measurement rather than a pass.",
    blocked_on: None,
};

pub const COMPILE_PROFILE_STATUS: GateFidelity = GateFidelity {
    gate_id: "compile_profile_status",
    aspiration: "Measure what this change does to compile wallclock -- macro expansion cost and \
                 build-script re-execution -- and hold the delta to a budget.",
    reference: "cargo build --timings; rustc self-profile; cargo-llvm-lines for expansion volume",
    fidelity: Fidelity::Heuristic,
    gap: "Compiles nothing and times nothing, so the profiler in the name measures no duration and \
          there is no budget for a delta to cross. Two substring rules are the whole of it \
          (compile_time_profiler/heavy_deps.rs::scan_heavy_dependencies). One matches a dependency \
          declaration for the syn crate carrying a full feature in a file whose path ends with the \
          manifest name, and names an estimate of 15-30s that nothing here observed. The other \
          reports a file whose path ends with the build-script name when the text handed to it \
          lacks `cargo:rerun-if-changed`; that text is what `after_change` returns -- the changed \
          hunk and the context around it, not the file -- so a script whose directive sits outside \
          the hunk is accused of not having one \
          (compile_time_profiler/mod.rs::evaluate_compile_profile). No other macro, derive or \
          generic instantiation is examined at all, and the verdict is a warning, so nothing \
          blocks.",
    blocked_on: None,
};
