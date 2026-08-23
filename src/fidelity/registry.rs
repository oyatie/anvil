//! The declared fidelity of each audited gate.
//!
//! # Scope of this registry
//!
//! Entries exist ONLY for gates whose implementation was read directly. Gates
//! that were not read have no entry and are counted as `unaudited` in the gap
//! report.
//!
//! This is deliberate. Guessing a fidelity would reproduce exactly the failure
//! being corrected: a confident claim with nothing behind it. An honest
//! "not yet audited" is worth more than an invented "Heuristic".
//!
//! Fifty-six of the seventy-two gates have an entry on this branch. The other
//! sixteen are each being rewritten in an open pull request that enters its
//! own, so auditing them here would collide and would describe code about to
//! change. They stay unaudited here, `unaudited_count` says sixteen, and the
//! ceiling in `withhold_aspirational_passes` exempts exactly those.
//!
//! # When two pull requests audit the same gate
//!
//! The surviving entry is the one written by the pull request that changed that
//! gate's code, and this branch drops its own.
//!
//! Not a tie-break by date or by author. A `gap` is not prose about a gate, it
//! is a set of `file:line` citations into the gate's implementation, and
//! `tests/fidelity_registry_citations_test.rs` requires each of them to name a
//! line that exists and to quote something the code at that line contains. An
//! entry written against the old implementation describes code that the other
//! pull request has just deleted; its citations do not merely drift, they point
//! at the wrong mechanism. The entry written alongside the rewrite is the only
//! one whose evidence is answerable.
//!
//! Thirteen entries were removed from this branch on that rule --
//! `security_scan`, `zero_day`, `canary`, `shuffle`, `progressive_ring`,
//! `feature_flag`, `local_probe`, `chaos_injection`, `test_suite`,
//! `rust_skills`, `adr`, `compliance` and `cross_service` -- together with
//! `cedar_status`, `attestation_status` and `schema_evolution_status`, which
//! were never entered here for the same reason. Merged naively the registry
//! held eighty-five entries and seventy-two distinct ids, which is
//! `gate_ids_are_unique` red and a gap report that has lost thirteen gates.

use super::{Fidelity, GateFidelity};

/// Gates audited by direct code reading, with file:line evidence in the gap text.
pub const AUDITED_GATES: &[GateFidelity] = &[
    GateFidelity {
        gate_id: "coverage_status",
        aspiration: "Measure statement and branch coverage of the lines added by this PR, via compiler \
                     instrumentation, and block below a threshold.",
        reference: "cargo-llvm-cov; Google TAP coverage instrumentation",
        fidelity: Fidelity::Aspirational,
        gap: "Runs no compiler coverage instrumentation tool. When no executable lines are added it reports \
              `NothingToMeasure`, and with no llvm-cov tool it reports `NotMeasured` (coverage_guard.rs:85-88,135-138).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "kani_status",
        aspiration: "Discharge memory-safety proof obligations for unsafe blocks using a bounded model \
                     checker.",
        reference: "Kani / CBMC; AWS Automated Reasoning Group",
        fidelity: Fidelity::Heuristic,
        gap: "No bounded model checker is invoked at any point: no Kani, no CBMC, no Miri. What runs \
              is a comment-presence lint over the diff. It matches an added line whose first non-space \
              token opens an `unsafe` item and asks whether a `// SAFETY:` comment sits on that line or \
              the five above it (kani_guard/mod.rs:80-81,102), and publishes the tally as \
              `unsafe_blocks_with_safety_comment` (kani_guard/mod.rs:46). Presence is the whole \
              property. Nothing here reads the comment: whether it is true, whether it describes the \
              block beneath it, and whether the obligations it names are discharged are all outside \
              what this gate can see. It is a reimplementation of clippy's undocumented-unsafe-blocks \
              lint, which upstream files under the opt-in restriction group rather than correctness. \
              The scan is narrower than the sentence it publishes, too: an `unsafe` block opened \
              mid-line is matched by no pattern here, as is any `unsafe` code this pull request leaves \
              untouched, so a clean verdict from this gate is not a statement that the change added no \
              unsafe code.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "slo_status",
        aspiration: "Evaluate multi-window multi-burn-rate error budget consumption against live production \
                     telemetry (14.4x/1h, 6x/6h).",
        reference: "Google SRE Workbook, multiwindow multi-burn-rate alerting",
        fidelity: Fidelity::Aspirational,
        gap: "Queries no telemetry. With no Prometheus or `OpenTelemetry` endpoint configured, the gate \
              structurally validates touched OpenSLO specs and reports `NotMeasured` \
              (slo_canary_guard/mod.rs:45-47,154-157).",
        blocked_on: Some("a reachable Prometheus or OpenTelemetry endpoint"),
    },
    GateFidelity {
        gate_id: "trace_status",
        aspiration: "Verify that W3C trace context is propagated across every async boundary a change \
                     introduces, so a request's spans stay one trace.",
        reference: "W3C Trace Context; OpenTelemetry context propagation; tracing-futures `Instrument`",
        fidelity: Fidelity::Heuristic,
        gap: "Runs no tracing runtime and resolves no types, and reads only the diff hunks -- never the \
              file -- so a boundary this pull request keeps outside a hunk is invisible to it and \
              no fix below removes that. It text-scans the added and retained lines of Rust chunks \
              for a call whose final path segment is `spawn`, `spawn_blocking` or `spawn_local` and \
              whose argument list is not empty (trace_context_guard/span_tracker.rs:28-30), then \
              asks whether `instrument` or `in_current_span` appears anywhere inside the \
              parenthesised region that call opens, minus the regions the boundaries nested in it \
              own (trace_context_guard/span_tracker.rs:83-102); the published sentence carries that \
              qualifier rather than the stronger claim. Appearing in the region is not attachment: \
              the call is not matched against the spawned future, so a span attached to some other \
              value inside the body reads as clean, and the gate can tell neither that the span is \
              the right one nor that it is attached to this task at all. The published sentence \
              says what is measured rather than the word attaches. A task instrumented at its \
              definition by a tracing instrument attribute has no such call at the spawn site and is \
              reported detached (trace_context_guard/span_tracker.rs:43-45), and a boundary crossed \
              by a call it cannot name that way -- a spawn imported under another name, or a task \
              started by code this diff does not touch -- it cannot see at all. \
              It errs in the other direction too, and that is the direction that blocks a merge. \
              The matcher is nominal: no type is resolved, so any non-empty call whose final path \
              segment is one of those three names is treated as a task boundary and reported -- a \
              thread pool, a process supervisor, an actor handle, a domain type of one is own -- \
              and there is no allowlist, no annotation and no configuration by which an author can \
              say otherwise; the verdict is `Failed`, which blocks. Only a definition is excluded, \
              by the `fn` keyword before the name (trace_context_guard/span_tracker.rs:215-221), so \
              a wrapper named spawn is left alone where it is declared but reported at every call \
              site through it. And a span attached before the spawn call rather than inside it -- a \
              future built in steps and then handed over as a binding, which is the shape any \
              non-trivial spawn takes -- sits outside the parenthesised region, is not seen, and \
              the boundary is reported detached. \
              A region is bounded by the hunk the boundary opened in: the hunks of a file are \
              disjoint windows onto it, so the scan runs once per hunk and a boundary whose \
              parenthesis does not close inside its own hunk is `unresolved` -- seen and not \
              judged, not counted among the inspected, and not accused \
              (trace_context_guard/mod.rs:67-76). Every boundary on a line is inspected, not merely \
              the first via `BOUNDARY_RE` (trace_context_guard/span_tracker.rs:128-132). Lexing is a line scanner \
              with state carried across lines, blanking string literals, raw strings, character \
              literals, line comments and block comments before anything counts a parenthesis \
              (trace_context_guard/span_tracker.rs:314-320); its state starts at code at the top of \
              every hunk, so a hunk whose first line is already inside a literal or a block comment \
              is read as code until that literal closes, and a `\'` that is neither a character \
              literal nor a lifetime is read as a lifetime. The diff is cut into one chunk per file \
              at a line beginning `diff --git ` (trace_context_guard/mod.rs:263-276), and a chunk \
              carrying no `+++ ` header names no path and is not read. An accusation names \
              post-image lines derived from the a hunk header header \
              (trace_context_guard/mod.rs:298-333) -- a chunk with no such header declares no \
              position, a removed line is not numbered, and neither is the \
              a no-newline marker marker git writes mid-hunk -- and it covers retained \
              lines as well as added ones, because a region walk that skipped context lines could \
              bound nothing, so a named line may be one the change only carried past rather than \
              wrote; the failing sentence says so. \
              What the guard decides is what is published: it builds its own `GateStatus` and \
              `trace_status` clones it (pre_merge_guard/evaluator.rs:291-297), the shape \
              `slo_status` already used. A diff crossing no boundary it can see is `Warning` \
              carrying `NOTHING TO MEASURE` (trace_context_guard/mod.rs:176-179): acceptable, so it \
              neither blocks nor accuses, and not `Passed`, which carries no string and would \
              discard the sentence. A boundary seen and not judged is `NotMeasured` under this \
              gate is own id (trace_context_guard/mod.rs:148-158), which publishes the count and \
              the reason and blocks merge-queue admission through `is_admissible` \
              (pre_merge_guard/report.rs:317-321) -- the same split between `NothingToMeasure` and \
              `NotMeasured` that `gate_status` makes (coverage_guard.rs:135,139). What remains: the \
              scorecard renderer collapses an admissible report to a single verdict line and \
              enumerates no finding at all (publish/scorecard.rs:133-137), so on a pull request \
              carrying no other finding the warning row is still not printed and the row \
              \"End-to-end span instrumentation across async tasks\" \
              (pre_merge_guard/matrix.rs:102-104) is counted in the total without a word. Making a \
              warning visible on a certified scorecard is a change to that renderer, which every \
              gate shares.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "remote_cache_status",
        aspiration: "Report the real distributed build-cache hit rate and ratchet it upward.",
        reference: "Bazel/Buck2 remote execution CAS statistics",
        fidelity: Fidelity::Aspirational,
        gap: "With no sccache or Buck2 CAS statistics endpoint configured, reports `NotMeasured` \
              (remote_cache_optimizer/mod.rs:79-82). Cache keys use non-cryptographic FNV-1a hashing via \
              `compute_cache_key` (remote_cache_optimizer/cache_keys.rs:16).",
        blocked_on: Some("sccache or Buck2 CAS statistics"),
    },
    GateFidelity {
        gate_id: "mutation_status",
        aspiration: "Build a mutant of every function the change touches, run the suite against each one, \
                     and block on any the suite fails to kill.",
        reference: "cargo-mutants; Google mutation testing at review time (Petrovic & Ivankovic, ICSE-SEIP \
                    2018), which surfaces only surviving mutants on changed lines",
        fidelity: Fidelity::Partial,
        gap: "The filename match is gone: `run_bounded_for` spawns cargo-mutants with `--in-diff` over \
              the pull request's own diff (chaos_mutation_guard.rs:306,316), so the suite really is run \
              against each mutant on the changed lines, and one it fails to kill is published as \
              `GateStatus::Failed` naming it. A seeded-defect fixture runs the real tool against a \
              deliberately inadequate suite and requires that failure. Measured nowhere it currently \
              runs, for two reasons and not one. (1) This repository's CI installs no cargo-mutants \
              (.github/workflows/ci.yml), so every run ends `Unavailable` and publishes `NotMeasured` \
              (chaos_mutation_guard.rs:176). (2) Installing it would not change that: cargo-mutants \
              copies the tree without a .git directory, and the daemon-tree tests in \
              change_delivery_lane_test.rs run the lane at CARGO-MANIFEST-DIR, where the shape \
              adapter shells out to \"rev-parse\" (shape/adapters/git_tree_at_rev.rs:56) and fails in \
              the copy -- so the real tool exits 4 in about 30 s, which is `NotMeasured`, correctly, \
              with no false green. Unmeasured at this budget too: a diff this size generates 41 \
              mutants against a `MUTATION_BUDGET` that buys 8 to 17 of them \
              (chaos_mutation_guard.rs:105). Still missing: no kill-rate threshold, no \
              equivalent-mutant suppression and no arid-node rules, so every survivor is reported \
              whether or not it is killable.",
        blocked_on: Some(
            "the daemon-tree tests running without .git (tests/change_delivery_lane_test.rs:104), \
             then the cargo-mutants binary on the runner and a budget that fits the mutant count",
        ),
    },
    GateFidelity {
        gate_id: "supply_chain_status",
        aspiration: "Resolve the dependency graph, match it against a live vulnerability database, and \
                     emit a signed SBOM.",
        reference: "osv-scanner, cargo-deny, CycloneDX",
        fidelity: Fidelity::Heuristic,
        gap: "Matches a short list of banned package names by regex. No dependency resolution, no CVE \
              database, no SBOM.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "formal_verification_status",
        aspiration: "Encode authorization policy into SMT and prove non-escalation with a solver.",
        reference: "AWS Zelkova; Z3",
        fidelity: Fidelity::Heuristic,
        gap: "A chain of policy_content.contains(..) tests. No solver exists. The file and its types \
              were renamed from smt_solver.rs/SmtConstraintEngine to say so \
              (formal_verification/policy_scanner.rs:28).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "deadlock_status",
        aspiration: "Build an inter-procedural lock-acquisition graph and detect cycles.",
        reference: "Tarjan strongly connected components; Meta Infer Starvation; lockbud",
        fidelity: Fidelity::Heuristic,
        gap: "A graph is built and its cycles are reported, but the nodes are receiver expressions \
              spelled as the source spells them, not lock instances: no type is resolved and nothing is \
              aliased, so two spellings of one lock are two nodes and one spelling of two locks is one \
              node. An acquisition is any zero-argument `.lock()`, `.read()` or `.write()` matched by \
              name (lock_graph.rs:75), and a guard counts as held only where a plain `let` binds it, \
              with its scope approximated by brace depth (`guard_binding`, lock_graph.rs:334) and \
              ended early by an explicit `dropped_bindings` (lock_graph.rs:263) -- without that, the \
              idiomatic release-then-reacquire read as a self-deadlock. Braces are counted after \
              string and character literals are blanked (`without_literals`, lock_graph.rs:211), \
              because an unbalanced brace inside a literal corrupted the depth and could leak a \
              guard into the next function. Edges come from the text of the diff alone \
              (`acquisition_edges`, lock_graph.rs:113), so an inversion split across a call this \
              change does not touch is invisible -- that is the missing call graph, not a tuning \
              problem. Only cycles are reported (`find_lock_order_cycles`, lock_graph.rs:100), never \
              nestings, because holding two locks at once is how correct code works. What is \
              reported is a strongly connected component and the field carrying it is named `locks`, \
              not a sequence: no witness path through the cycle is reconstructed.",
        blocked_on: Some(
            "MIR-level guard liveness and points-to analysis, for lock identity across \
                          aliases and call edges",
        ),
    },
    GateFidelity {
        gate_id: "semantic_abi_status",
        aspiration: "Compare the public API surface across revisions and require a semver major bump for \
                     breaking changes.",
        reference: "cargo-semver-checks over rustdoc JSON",
        fidelity: Fidelity::Heuristic,
        gap: "Compares the `pub fn` declarations a diff removes against the ones it adds, by name over the \
              whole diff rather than by two substring tests on it. A removal is reported only when the name \
              is added nowhere in the diff -- `added.get(name)` (signature_scanner.rs:230) -- and a \
              signature only when the name occurs once on each side and both lines close their parameter \
              list, so a move and a rustfmt reflow both clear it while `unpaired_names` counts the pairs it \
              declined to compare (signature_scanner.rs:244,248). It builds no rustdoc JSON, resolves no \
              module path and reads no baseline revision, so a function moved between modules reads as \
              clean and a major version bump is not read. Two shapes are declined outright rather than \
              guessed at: a name the diff removes but re-declares inside a string literal on the added \
              side -- an inline fixture rewritten from a raw string, which the anchor alone does not catch \
              because the indented body line reaches it bare -- and anything after a `#[cfg(test)]` marker \
              in the same file chunk, since `is_library_rust` tests directories and a test module inside \
              src publishes nothing (signature_scanner.rs:221,169). Both cost recall: a genuine removal \
              re-mentioned as a declaration in a string, or declared below a test module, is not reported. \
              The memory-layout half of the old claim is withdrawn: nothing computes a layout, and a diff \
              that adds or removes a `#[repr(` line -- the symmetric difference, so a line present \
              identically on both sides was moved and does not count -- is reported `NotMeasured` rather \
              than passed. Adding a brand-new repr type also triggers it, which is why the published \
              sentence says add or remove rather than change -- `layout_files` \
              (semantic_abi_ratchet/mod.rs:104,116).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "ci_wallclock_status",
        aspiration: "Measure this PR's actual CI wallclock and ratchet against a trunk baseline.",
        reference: "internal CI wallclock budgets; ADR-0718",
        fidelity: Fidelity::Aspirational,
        gap: "Without GitHub Actions timing API access, reports `NotMeasured` rather than measuring real \
              build duration or cost (ci_wallclock_ratchet/mod.rs:71-74).",
        blocked_on: Some("GitHub Actions timing API"),
    },
    GateFidelity {
        gate_id: "cluster_audit_status",
        aspiration: "Compare live cluster state against the declared manifests and report drift.",
        reference: "ArgoCD drift detection; kube-rs",
        fidelity: Fidelity::Aspirational,
        gap: "With no Kubernetes API or ArgoCD access configured, reports `NotMeasured` and performs no \
              Git comparison (cluster_state_auditor/mod.rs:79-82).",
        blocked_on: Some("Kubernetes API or ArgoCD access"),
    },
    GateFidelity {
        gate_id: "shadow_traffic_status",
        aspiration: "Mirror production traffic to a shadow deployment and diff the responses.",
        reference: "Envoy request mirroring; Diffy",
        fidelity: Fidelity::Aspirational,
        gap: "Without traffic mirror infrastructure or a replay target configured, reports `NotMeasured` \
              (shadow_traffic_harness/mod.rs:76-79).",
        blocked_on: Some("traffic mirroring infrastructure and a replay target"),
    },
    GateFidelity {
        gate_id: "wasm_sandbox_status",
        aspiration: "Execute untrusted policy in a WebAssembly sandbox with a constrained ABI.",
        reference: "Wasmtime",
        fidelity: Fidelity::Heuristic,
        gap: "No WebAssembly runtime. Checks lower.contains(\"process::abort\") \
              (wasm_sandbox/policy_runner.rs:20).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "zero_trust_workload_status",
        aspiration: "Verify workload identity via SPIFFE/SPIRE issued mTLS certificates.",
        reference: "SPIFFE/SPIRE",
        fidelity: Fidelity::Heuristic,
        gap: "Checks for the substring \"http://\" in the diff \
              (zero_trust_workload/identity_auditor.rs:15).",
        blocked_on: Some("a SPIFFE/SPIRE control plane"),
    },
    GateFidelity {
        gate_id: "doc_parity_status",
        aspiration: "Verify documentation parity with the change, and amend affected documents.",
        reference: "docs-as-code; Google g3doc",
        fidelity: Fidelity::Partial,
        gap: "Fails closed, creates missing ADRs, and corpus_sync amends owned pages so published \
              gate counts match TOTAL_GATES -- but only in Anvil's own repository \
              (doc_guard::corpus_sync::is_anvils_own_repository); on any other repository the sync \
              does not apply and the gate says so. generate_and_write_docs still writes only when a \
              file does not exist (doc_guard::generate_and_write_docs); it does not rewrite existing \
              documents, and a named file it leaves unchanged is not reported as updated.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "unresolved_review_status",
        aspiration: "Block on unresolved review threads using authoritative thread state.",
        reference: "GitHub GraphQL pullRequest.reviewThreads.nodes { isResolved }",
        fidelity: Fidelity::Heuristic,
        gap: "Infers resolution from comment text rather than querying isResolved, so informational comments \
              can hold a PR at 'unresolved' indefinitely.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "automated_canary_status",
        aspiration: "Compare canary and baseline metric distributions from a live canary deployment with a \
                     Mann-Whitney U-test, and halt the rollout on a statistically significant regression.",
        reference: "Spinnaker/Kayenta Automated Canary Analysis; Mann-Whitney U-test",
        fidelity: Fidelity::Aspirational,
        gap: "Deploys no canary and queries no telemetry. The review pipeline used to write the samples it \
              then judged. Mann-Whitney is not implemented: evaluate_canary_distributions compares two \
              arithmetic means against a fixed relative bound \
              (automated_canary/statistical_engine.rs:43). With no baseline_samples and no canary_samples \
              the gate now reports NotMeasured instead of a pass.",
        blocked_on: Some(
            "a canary deployment with a queryable Prometheus or OpenTelemetry endpoint",
        ),
    },
    GateFidelity {
        gate_id: "stacked_diffs_status",
        aspiration: "Read the pull request DAG from the forge, order the stack topologically, and verify \
                     every child is rebased on its parent before an atomic merge.",
        reference: "Phabricator/Graphite stacked diffs; Meta Sapling",
        fidelity: Fidelity::Aspirational,
        gap: "Reads no pull request DAG; the review pipeline passed an empty slice on every PR. Given a \
              real stack, compute_stack_plan still returns atomic_merge_ready unconditionally and orders \
              by input order rather than by parent links, so stack_depth is the only thing it derives \
              (stacked_diffs/dag_manager.rs:27-36). With no stack supplied the gate reports NotMeasured.",
        blocked_on: Some("a forge query for the pull requests stacked on this one"),
    },
    GateFidelity {
        gate_id: "microbench_status",
        aspiration: "Run criterion benchmarks on the base and head revisions and ratchet hotpath ns/op \
                     against a published trunk baseline.",
        reference: "criterion.rs; Google Fleetbench",
        fidelity: Fidelity::Aspirational,
        gap: "Executes no benchmark: this repository declares no criterion dependency and has no benches \
              directory, so there is no baseline to ratchet against and the review pipeline used to write \
              a base_ns_per_op equal to its own head_ns_per_op. Both that sample and the analyzer that \
              read it are deleted; the gate reports NotMeasured.",
        blocked_on: Some("a criterion benchmark harness and a published trunk baseline"),
    },
    GateFidelity {
        gate_id: "shape_status",
        aspiration: "Measure a repository's distance to its declared monorepo shape — unit skeleton, \
                     satellite placement, root hygiene, naming, and the Dependency Rule over real \
                     build edges — and refuse any regression past a baseline frozen at the merge-base.",
        reference: "oyatie ADR-0562 placement rule and ci/facade/baseline-ratchet; Google Rosie/Tricorder \
                    ratchets; ArchUnit FreezingArchRule",
        fidelity: Fidelity::Partial,
        gap: "Placement, skeleton, root and naming rules are measured from the tree at the PR head with \
              seeded-defect fixtures. Dependency rules read Cargo path dependencies, Buck2 labels and \
              `use crate::` paths; TypeScript imports are declared unavailable, so a spec naming \
              ts-workspace gets NotMeasured for those rules. The adapter-naming rule is not implemented. \
              Contention metrics are not yet collected.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "replay_harness_status",
        aspiration: "Replay recorded production traces and assert byte-for-byte state parity.",
        reference: "Deterministic record-and-replay; VMware ReTrace",
        fidelity: Fidelity::Aspirational,
        gap: "No trace corpus is collected, so the replayer is never given one. Its whole check is \
              `traces.iter().all(|t| !t.input_payload.is_empty())`, which is vacuously true of an \
              empty slice -- and it used to answer one with a hardcoded count of five replayed \
              fixtures, which the scorecard published. Now reports NotMeasured \
              (replay_harness/trace_replayer.rs:17).",
        blocked_on: Some("a production trace recorder, which does not exist yet"),
    },
    GateFidelity {
        gate_id: "upgrade_train_status",
        aspiration: "Schedule autonomous semver and CVE upgrade PRs from the dependency graph.",
        reference: "Dependabot; Renovate",
        fidelity: Fidelity::Aspirational,
        gap: "The review pipeline supplied no candidates, and the verdict `let passed = breaking == 0;` \
              is trivially true of an empty list, so the train was certified without being read. \
              Now reports NotMeasured (upgrade_train/mod.rs:62).",
        blocked_on: Some("a dependency manifest reader and an advisory feed"),
    },
    GateFidelity {
        gate_id: "consistency_status",
        aspiration: "Verify multi-region write ordering via vector clocks and CRDT convergence.",
        reference: "Lamport clocks; Shapiro CRDTs",
        fidelity: Fidelity::Heuristic,
        gap: "Substring scan. A line naming a global table needs only to also contain \"version\" to \
              be treated as safely ordered, and \"version\" appears in most schema and dependency \
              diffs (consistency_guard/conflict_detector.rs:29).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "jittered_backoff_status",
        aspiration: "Prove full-jitter backoff and deadline propagation on every network retry.",
        reference: "AWS Architecture Blog, exponential backoff and jitter",
        fidelity: Fidelity::Heuristic,
        gap: "Substring scan. The jitter test is contains(\"rand\"), which any longer word containing \
              those four letters satisfies -- this repository's own brand-absence gate is one such \
              word -- and the deadline test is contains(\"context\"). Most diffs clear it without any \
              backoff at all (jittered_backoff/backoff_scanner.rs:31).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "hermetic_build_status",
        aspiration: "Build twice and compare the binaries byte for byte.",
        reference: "Reproducible Builds; Bazel hermeticity",
        fidelity: Fidelity::Heuristic,
        gap: "Builds nothing, so the byte-for-byte comparison the name claims is unmeasured and the \
              gate withholds it. The impurity half is real and fires: it checks the diff for the \
              literals SystemTime::now() and env!(\"HOME\") \
              (hermetic_build/reproducibility_checker.rs:29).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "auto_rollback_status",
        aspiration: "Watch canary error budget burn and roll back autonomously on degradation.",
        reference: "SRE Workbook, error budget policy",
        fidelity: Fidelity::Aspirational,
        gap: "The engine itself is correct: above its thresholds it rolls back and writes a \
              postmortem. Nothing measures error rate or latency, though -- the review pipeline \
              passed hardcoded healthy readings, so the degraded branch was unreachable on every \
              pull request. `is_degraded` is the whole decision, and nothing feeds it a reading \
              (auto_rollback/mod.rs:47).",
        blocked_on: Some("a canary telemetry source; there is none"),
    },
    GateFidelity {
        gate_id: "carbon_compute_status",
        aspiration: "Measure build energy cost and route heavy compute to low-carbon grid windows.",
        reference: "Green Software Foundation SCI",
        fidelity: Fidelity::Aspirational,
        gap: "Nothing meters CPU time or grid intensity; the review pipeline passed hardcoded \
              budget and actual figures, so the ratchet compared two constants and published a \
              joules figure derived from them. `evaluate_carbon_intensity` receives no measurement \
              (carbon_aware/mod.rs:62).",
        blocked_on: Some("a CPU-time meter and a grid carbon-intensity feed"),
    },
    GateFidelity {
        gate_id: "openvex_status",
        aspiration: "Attest CVE exploitability by pruning advisories against the real call graph.",
        reference: "OpenVEX; Google capslock",
        fidelity: Fidelity::Aspirational,
        gap: "No advisory feed or dependency inventory is read. The whole reachability decision is \
              `!source_code.contains(vuln_symbol)`, and the review pipeline supplied placeholder \
              CVE and symbol names, so every PR was attested NotAffected by an advisory that does \
              not exist. Now reports NotMeasured (vex_scanner/callgraph_pruner.rs:36).",
        blocked_on: Some("an advisory feed and a call graph; neither exists yet"),
    },
    GateFidelity {
        gate_id: "finops_status",
        aspiration: "Ratchet cost-per-outcome by budgeting heap allocations on hot paths.",
        reference: "Zero-copy parsing; allocation budgets",
        fidelity: Fidelity::Heuristic,
        gap: "Scope is a fixed list of path fragments -- `is_hotpath` matches network/, codec/, \
              engine/, hotpath, packet -- and no tracked file in this repository contains any of \
              them, so nothing is ever scanned here. The gate used to report a clean hotpath \
              budget from that empty scope; it now separates a clean scan from an empty one \
              (finops_ratchet/allocation_scanner.rs:30).",
        blocked_on: Some("a per-tenant hotpath declaration; the marker set is hardcoded"),
    },
    GateFidelity {
        gate_id: "sandbox_status",
        aspiration: "Spin up a hermetic ephemeral sandbox per PR and prove zero host-state leakage.",
        reference: "Bazel sandboxfs; Firecracker microVMs",
        fidelity: Fidelity::Aspirational,
        gap: "No sandbox runtime exists, so nothing is started, bound or timed. The allocator it \
              used to call returned a struct literal, making the verdict a constant and publishing \
              a spin-up time nothing had measured; that pool is deleted and \
              `evaluate_without_sandbox_runtime` is now the only path \
              (ephemeral_sandbox/mod.rs:37).",
        blocked_on: Some("a container or microVM runtime the review pipeline can drive"),
    },
    GateFidelity {
        gate_id: "flake_quarantine_status",
        aspiration: "Detect non-deterministic tests from run history and isolate them into a \
                     quarantine lane so they stop blocking merges.",
        reference: "Google flaky-test infrastructure; Meta's Probabilistic Flakiness Score",
        fidelity: Fidelity::Aspirational,
        gap: "Nothing retains test-run history, so no test can be shown to be non-deterministic and \
              there is no lane to isolate one into. The verdict used to be a literal, and the \
              counters come from a substring match for \"flaky\" against changed file paths -- \
              retained as data, not as evidence (flake_quarantine/quarantine_manager.rs:14).",
        blocked_on: Some("per-test run history across attempts, which Anvil does not record"),
    },
    GateFidelity {
        gate_id: "predictive_test_status",
        aspiration: "Select the tests a change can affect from the dependency graph and hold PR \
                     test wall-clock under a budget.",
        reference: "Bazel target determination; Google's affected-target selection",
        fidelity: Fidelity::Partial,
        gap: "The package DAG selection is real, but nothing times a test run, so the wall-clock \
              claim is unmeasured; the verdict is now whether the selection pruned anything at all \
              rather than the literal it was. Discovery used to invent a package when it found \
              none; the verdict is now `let is_optimized = skipped > 0;` \
              (predictive_test_selector/mod.rs:84).",
        blocked_on: Some("a timed test run to measure the wall-clock budget against"),
    },
    GateFidelity {
        gate_id: "cosign_status",
        aspiration: "Sign the artefact through the Fulcio OIDC keyless flow, record it in the Rekor \
                     transparency log, and verify the certificate identity and the inclusion proof \
                     before publishing provenance.",
        reference: "Sigstore keyless signing (Fulcio, Rekor); SLSA provenance levels; in-toto attestations",
        fidelity: Fidelity::Aspirational,
        gap: "Signs nothing: no OIDC identity token is requested, no Fulcio certificate is issued, no \
              signature is computed and no transparency-log inclusion proof is fetched -- the crate \
              carries no signing key and no X.509 or ECDSA dependency to do any of it with. The \
              fabricated bundle it used to publish, an elided PEM certificate literal and a log id \
              derived from the artefact digest prefix, is deleted along with the attestor that built \
              it; `evaluate_without_signing_backend` publishing `MISSING_SIGNING_BACKEND` is now the \
              only path (cosign_signer/mod.rs:63,82-87).",
        blocked_on: Some(
            "a Sigstore signing backend -- an OIDC identity source, Fulcio and Rekor; none is reachable \
             from here",
        ),
    },
    GateFidelity {
        gate_id: "debt_shrink_status",
        aspiration: "Ratchet deprecation and reorganisation debt down: only shrinkage permitted on \
                     targets the organisation has decided to drain.",
        reference: "SonarQube Clean as You Code -- new-code quality gate",
        fidelity: Fidelity::Heuristic,
        gap: "Scope is three path fragments -- `is_deprecating_target` matches `deprecated`, `legacy`, \
              `/old/` -- plus any path named in a `REORG-DRAIN.md` drain ledger, and no tracked file in \
              this repository matches a fragment while the ledger does not exist, so nothing is ever \
              in scope here. The gate used to publish a drained ratchet from that empty scope; it now \
              separates a scanned target from an absent one \
              (debt_shrink_guard.rs:57-60). The ledger is the only authoritative half: deprecation is \
              a decision somebody recorded, not a substring somebody typed into a path.",
        blocked_on: Some("a drain ledger; the marker set is hardcoded and unconfigurable"),
    },
    GateFidelity {
        gate_id: "gitops_drift_status",
        aspiration: "Reconcile desired state against the cluster and refuse orphaning cascade \
                     deletions.",
        reference: "ArgoCD sync status and self-heal; `resources-finalizer.argocd.argoproj.io`",
        fidelity: Fidelity::Heuristic,
        gap: "Reads no cluster and computes no diff against live state, so it detects no drift at all; \
              what it checks is a deletion marker in the diff text. Scope is two path fragments -- \
              `is_gitops_manifest` matches `applicationset` and `application.yaml` \
              (gitops_drift_reconciler/orphan_sweeper.rs:34-35) -- and no tracked file in this \
              repository matches either, so nothing is ever in scope. ArgoCD is not the precedent \
              at its reporting layer -- it starts at Synced and downgrades only inside the loop over \
              target resources, so an Application with no targets displays green -- but it is at its \
              action layer, where auto-sync refuses to run when every managed resource would be \
              pruned unless allowEmpty is set. The gate used to publish declarative integrity from an \
              empty scope; it now reports that nothing was scanned, which is acceptable to the badge \
              and withheld by merge admission.",
        blocked_on: Some("a cluster the reconciler can read; nothing here talks to one"),
    },
    GateFidelity {
        gate_id: "migration_orch_status",
        aspiration: "Enforce Expand-Contract phase ordering across releases so a contract step cannot \
                     ship before its expand has baked.",
        reference: "Parallel Change (expand/contract); Flyway and Liquibase validate; squawk",
        fidelity: Fidelity::Aspirational,
        gap: "Nothing spans releases: it reads one diff and asks whether the literal `-- PHASE: \
              CONTRACT` appears in the same file as a `DROP COLUMN`, so the phase order it is named \
              for is never checked and the annotation is self-attested by the author dropping the \
              column. Scope was `.sql` matched against the hunk *text*, which put a Rust file \
              mentioning a SQL filename into SQL scope and defaulted a header-less chunk to a \
              hardcoded migration filename that ended in that extension; it is now the path \
              (migration_orchestrator/phase_validator.rs:43-44), and no tracked file in this \
              repository ends in that extension, so nothing is ever in scope. The gate used to publish \
              lifecycle conformance from that empty scope and now reports that nothing was parsed.",
        blocked_on: Some("release-spanning migration history; one diff cannot show phase order"),
    },
    GateFidelity {
        gate_id: "ghost_migration_status",
        aspiration: "Validate schema migrations against a shadow database for lock-free application and \
                     rollback parity.",
        reference: "gh-ost, which refuses an empty --alter outright; pt-online-schema-change",
        fidelity: Fidelity::Heuristic,
        gap: "Regex scan for CONCURRENTLY, DROP COLUMN and NOT NULL. Connects to no database and runs \
              no migration, so no lock is ever observed. Scope is the changed-file list filtered by \
              `is_migration_file`, which matched `migration` as a substring anywhere in the path -- \
              so the eight tracked Rust files spelling that word were schema scope, and \
              src/migration/registry.rs was certified lock-free. It is now `file_path.ends_with` \
              on `.sql`, or a path component equal to `migrations` or `migrate` \
              (ghost_migration_harness.rs:63-66), so no tracked file in this repository is in \
              scope; a checked-in schema.rb or an Atlas hcl file carries DDL under neither and is \
              still missed. An empty result also cannot be told apart from a changed-file list that \
              never arrived. Both used to return early with a pass declaring the ghost migration \
              check clean; both now report that nothing was scanned.",
        blocked_on: Some("a shadow database"),
    },
    GateFidelity {
        gate_id: "api_contract_status",
        aspiration: "Parse the OpenAPI document, validate it, and diff it against the routes the service \
                     actually registers so a contract and its implementation cannot drift apart.",
        reference: "OpenAPI Specification 3.1; Spectral and Redocly lint; openapi-diff \
                    backward-compatibility checks",
        fidelity: Fidelity::Aspirational,
        gap: "Parses no schema and enumerates no route. The only two checks shell out to marker scripts -- \
              \"scripts/check-openapi-refs.mjs\" (api_contract_guard.rs:58) and \
              \"scripts/union-openapi.py\" (api_contract_guard.rs:96) -- and neither file exists in this \
              repository, so neither branch is entered and `let is_intact = unverifiable.is_none() && \
              (!script_failed || !synced_files.is_empty());` (api_contract_guard.rs:152) reduces to a \
              constant true for every change in scope. What is left is a `--porcelain` status read \
              filtered for paths that name a schema, which is empty on a clean tree. A pass here reports \
              that two absent scripts did not complain.",
        blocked_on: Some(
            "an OpenAPI validator; the two scripts this shells out to are not in this \
            repository",
        ),
    },
    GateFidelity {
        gate_id: "cell_isolation_status",
        aspiration: "Prove that every data access a change introduces is scoped to one tenant, and that no \
                     call crosses a cell boundary outside the cell gateway.",
        reference: "AWS cell-based architecture (Well-Architected reliability pillar); PostgreSQL \
                    row-level security",
        fidelity: Fidelity::Heuristic,
        gap: "Two regexes over added lines. No query is parsed, no schema is read and no call graph is \
              built. A line matched by `sql_re` (cell_isolation_guard.rs:43) -- the word SELECT, DELETE or \
              UPDATE followed anywhere on the line by WHERE -- is a violation unless that same line also \
              contains the text `tenant_id` (cell_isolation_guard.rs:52). So prose, a log message and a \
              comment about a query are accused; an ORM call scoped one line above is accused; and a query \
              naming the column without filtering on it is cleared. The cross-cell half matches only a \
              `TcpStream::connect` call whose argument is a literal dotted quad and port \
              (cell_isolation_guard.rs:44,61), and this repository's own connect call passes a variable, \
              so it is invisible to the rule. `is_isolated` (cell_isolation_guard.rs:71) therefore says \
              nothing about cell boundaries, blast radius, or the tenant scoping of anything the change \
              did not spell out on a single line.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "clean_arch_status",
        aspiration: "Enforce the Dependency Rule over the real build graph: no inner layer may depend on \
                     an outer one, whatever the file happens to be called.",
        reference: "Robert C. Martin, Clean Architecture (the Dependency Rule); ArchUnit \
                    layered-architecture rules",
        fidelity: Fidelity::Heuristic,
        gap: "A text scan that is candid about its own blind spot. A file's layer is its path -- \
              `classify_layer` (clean_architecture_guard.rs:164) matches path fragments -- and an edge is \
              an added line that `is_import_line` (clean_architecture_guard.rs:148) accepts and whose text \
              carries a layer word from `core_forbidden_imports` (clean_architecture_guard.rs:308). No \
              module path is resolved, so an import of a crate whose name merely contains one of those \
              words is an edge, while an inward dependency reached through a re-export, a trait object or \
              a plain call rather than an import is not one at all. Only added lines are read, so a \
              violation the change carries past untouched is invisible, and a file outside the four path \
              conventions is unclassified however it depends. Two things it does right, and this registry \
              records them rather than hiding them: a run for which `files_classified == 0` \
              (clean_architecture_guard.rs:385) reports NotMeasured instead of clean, and `let is_clean = \
              violations.is_empty() && measurement.is_measured();` (clean_architecture_guard.rs:401) \
              refuses to call an unmeasured run a pass.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "monorepo_status",
        aspiration: "Enforce package boundaries and hermeticity across the repository: no undeclared \
                     dependency, no path escape, no non-hermetic build input.",
        reference: "Bazel strict deps and sandboxing; Google monorepo BUILD visibility; Nx and Turborepo \
                    boundary rules",
        fidelity: Fidelity::Heuristic,
        gap: "No package graph is read, so hermeticity is approximated by text rules plus one absent \
              script. The rules match real idioms and do fire: a deep relative include and an absolute \
              path under a user home (`HARDCODED_ABSOLUTE_PATH`, monorepo_guard/mod.rs:80-96), an agent \
              scratch directory in the changed-file list (`BANNED_HARNESS_PREFIXES`, \
              monorepo_guard/harness_quarantine.rs:22-23), and a whole-file pass counting lines against \
              `MAX_WHOLE_FILE_LINES` and flagging any `.unwrap()` outside a path spelling test \
              (monorepo_guard/whole_file_expansion.rs:35,75). That last rule dominates the verdict on any \
              Rust repository, and is most of what `is_compliant` (monorepo_guard/mod.rs:166) actually \
              reports; it reads the file on disk rather than the change, so a pre-existing occurrence in a \
              file the change merely touches is charged to the change. The undeclared-import half is a \
              marker script, \"scripts/check-undeclared-imports.mjs\" (monorepo_guard/mod.rs:118), which \
              does not exist here -- so the one check that would need a package graph never runs.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "modularization_status",
        aspiration: "Keep every module inside a size envelope, and refuse a change that grows a file past \
                     the ceiling.",
        reference: "Google style guides on file size; SonarQube maintainability rules; Parnas, On the \
                    Criteria To Be Used in Decomposing Systems",
        fidelity: Fidelity::Heuristic,
        gap: "It does not measure file length. `line_count` is incremented once per ADDED line in a chunk \
              (modularization_guard.rs:66-68) and compared against `MAX_RECOMMENDED_LINES` \
              (modularization_guard.rs:54), so a file already far past the ceiling passes as long as this \
              change adds few lines, a new file just over the ceiling fails, and the finding then \
              publishes that added-line count as the file's total length -- a number the gate never read. \
              The lower bound in the row's title is not checked at all: nothing looks for a file that is \
              too small. The second rule is real and does measure what it claims, comparing directory \
              depth per category against `max_depth` (modularization_guard.rs:82,97), and `is_modular` \
              (modularization_guard.rs:109) is the conjunction of the two.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "constant_work_status",
        aspiration: "Prove a change adds no unbounded queue, pool or retry: every buffer has a fixed \
                     capacity and every producer meets backpressure.",
        reference: "Amazon Builders' Library, reliability and constant work; tokio bounded mpsc and \
                    backpressure",
        fidelity: Fidelity::Heuristic,
        gap: "One regex, `unbounded_chan_re` (constant_work_guard/buffer_limits.rs:32,36), matching a \
              single constructor spelling. The idiom is real and does occur in this repository, so the \
              check can fire -- but it is the whole check. A `Vec` that grows per request, a thread pool \
              with no ceiling, work proportional to fleet size on an interval, and every channel from \
              another library are all outside it, and backpressure is never examined in any form. \
              `scan_unbounded_structures` (constant_work_guard/mod.rs:60-62) is handed the entire file \
              chunk -- added, removed and context lines alike -- so a change that DELETES an unbounded \
              channel is reported as adding one, and `is_bounded` (constant_work_guard/mod.rs:66) is a \
              statement about that one spelling.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "idempotency_status",
        aspiration: "Require every state-mutating endpoint to accept and honour an idempotency key, and \
                     every side effect to leave through a transactional outbox.",
        reference: "Stripe idempotent requests; the transactional outbox pattern (Richardson, \
                    Microservices Patterns)",
        fidelity: Fidelity::Heuristic,
        gap: "The outbox half has no implementation: nothing looks for a transaction, a relay, or an event \
              table. What runs is `post_route_re` (idempotency_guard/outbox_rules.rs:32,37), one \
              router-registration spelling taking a string path and a mutating verb, which does match this \
              repository's own registrations. It is cleared whenever `idempotency_header_re` \
              (idempotency_guard/outbox_rules.rs:34,41) matches anywhere in the same file chunk, so a \
              single mention of the header exempts every route in that file, and nothing checks that a key \
              is read, stored, or compared against a previous request. Routes registered in any other \
              spelling, and mutating handlers not registered on a route at all, are invisible. \
              `is_idempotent` (idempotency_guard/mod.rs:66) reports on that one registration shape.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "gitops_promo_status",
        aspiration: "Require every container reference a manifest promotes to be an immutable digest, \
                     resolved and verified against the registry that serves it.",
        reference: "OCI image specification content-addressable digests; Kubernetes imagePullPolicy \
                    guidance; Flux image automation",
        fidelity: Fidelity::Heuristic,
        gap: "Resolves nothing against a registry. The check is `image_line_re` \
              (gitops_promotion/digest_pinner.rs:35) over the chunk text, failing a reference that carries \
              a mutable tag or lacks `@sha256:` (gitops_promotion/digest_pinner.rs:36-37,43-44). Scope is \
              not the path but the chunk's own text -- `is_gitops_manifest` (gitops_promotion/mod.rs:49) \
              is satisfied by a substring naming a manifest extension appearing anywhere in it -- so a \
              source file that merely mentions a manifest filename is in GitOps scope, while no tracked \
              manifest in this repository carries an image reference for it to read. Every line of the \
              chunk is scanned, removed and context alike, so deleting an unpinned image is reported as \
              adding one. `is_pinned` (gitops_promotion/mod.rs:72) says nothing about whether a digest \
              that IS present names the artefact this change built.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "compile_profile_status",
        aspiration: "Measure a change's effect on compile wallclock -- macro expansion, codegen work, \
                     dependency cost -- and refuse a regression past a budget.",
        reference: "cargo build --timings; cargo-bloat and cargo-llvm-lines; the Rust compiler performance \
                    dashboard",
        fidelity: Fidelity::Heuristic,
        gap: "Compiles nothing and times nothing, so there is no budget for anything to regress past. Two \
              literal checks stand in for one. In a manifest, `full_syn_re` \
              (compile_time_profiler/heavy_deps.rs:34-35) matches one dependency spelling on one line, and \
              no other crate is treated as heavy however long it takes to build. In a build script, the \
              absence of `cargo:rerun-if-changed` (compile_time_profiler/heavy_deps.rs:44) is the entire \
              caching check. Both read the whole file chunk rather than its added lines, so removing \
              either construct reads the same as adding it. `is_lean` (compile_time_profiler/mod.rs:62) is \
              a statement about those two spellings and about nothing else in the row's title.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "runner_economics_status",
        aspiration: "Route CI work to the cheapest runner that can do it, and keep expensive SKUs off \
                     pull-request triggers.",
        reference: "GitHub Actions larger-runner billing; AWS Spot Instances; Bazel remote execution pools",
        fidelity: Fidelity::Heuristic,
        gap: "Prices nothing and allocates nothing: there is no rate card, no runner inventory, and no \
              measured minute. One rule, needing two coincidences in the same chunk -- `is_pr_trigger` \
              (ci_runner_economics/sku_allocator.rs:34) is the substring `pull_request:` appearing \
              anywhere in it, and a `runs-on` value containing macos or gpu \
              (ci_runner_economics/sku_allocator.rs:41). The spot allocation the row's detail promises is \
              not checked at all: no label is compared against a spot pool and nothing distinguishes an \
              on-demand runner from a preemptible one. Both workflows in this repository run on the \
              default hosted runner, so the rule does not fire here, though the idiom is generic enough to \
              fire elsewhere. `is_cost_optimal` (ci_runner_economics/mod.rs:62) is that one coincidence \
              and nothing more.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "ephemeral_secret_status",
        aspiration: "Prove every credential a workflow uses is a short-lived token federated through OIDC, \
                     and that no long-lived secret is reachable from a pull-request trigger.",
        reference: "GitHub Actions OIDC hardening guidance; AWS STS AssumeRoleWithWebIdentity; HashiCorp \
                    Vault dynamic secrets",
        fidelity: Fidelity::Heuristic,
        gap: "One regex, over one vendor's one variable name. `static_aws_secret_re` \
              (ephemeral_secrets/oidc_validator.rs:37,41) matches an AWS secret-key assignment drawing its \
              value from the secrets context, and scope is any path containing \".github/workflows/\" \
              (ephemeral_secrets/oidc_validator.rs:33). Everything else the row claims is unexamined: no \
              token lifetime is read anywhere, so the fifteen-minute ceiling is never checked; no OIDC \
              configuration is required, so a workflow that federates nothing passes as long as it names \
              no AWS key; and cloud, registry and package credentials from every other vendor are not \
              matched at all. A chunk carrying no path header keeps a default workflow filename \
              (ephemeral_secrets/mod.rs:49), which puts text belonging to no file into workflow scope. \
              `is_zero_trust` (ephemeral_secrets/mod.rs:62) is a statement about that single assignment \
              spelling.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "psa_status",
        aspiration: "Admit no workload below the restricted Pod Security Standard, and hold every \
                     exception in a registry that carries an owner and an expiry.",
        reference: "Kubernetes Pod Security Admission and the Pod Security Standards; OPA Gatekeeper and \
                    Kyverno admission policies",
        fidelity: Fidelity::Heuristic,
        gap: "Admits nothing and reaches no cluster: this is a substring test over chunks whose path ends \
              in a YAML extension. A chunk containing `kind: Namespace` and not containing \
              `pod-security.kubernetes.io/enforce:` is the only violation it can raise \
              (psa_admission_guard/psa_rules.rs:33-34), so every Pod, Deployment, StatefulSet and CronJob \
              goes unexamined, the label's VALUE is never read -- baseline and privileged clear the check \
              as readily as restricted -- and a namespace labelled elsewhere or inheriting its policy is \
              judged on the text of one file. The registry the row's detail names is two hardcoded path \
              substrings carrying no owner and no expiry (psa_admission_guard/psa_rules.rs:35-36). No \
              tracked YAML here declares a namespace, so `is_compliant` (psa_admission_guard/mod.rs:60) \
              has never been anything but true in this repository, though the idiom is generic and would \
              fire on one that carries manifests.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "review_verdict_status",
        aspiration: "Review the change adversarially and block on a finding a human reviewer would block \
                     on.",
        reference: "Google Critique and Tricorder; automated model-driven code review (CodeRabbit, \
                    Greptile)",
        fidelity: Fidelity::Heuristic,
        gap: "A language model's opinion, taken as the verdict. The plumbing around it is mechanical and \
              correct: a response that does not parse, or that omits the field, reports `VERDICT_ERRORED` \
              (reviewer.rs:28,334) rather than an implicit pass, and `review_verdict_status` admits only \
              an explicit approval or comment (pre_merge_guard/evaluator.rs:604-605), so an absent \
              review blocks. The \
              judgement is not mechanical. The sixteen lenses this row is named for exist as prompt text \
              -- `16-lens` (reviewer.rs:304) -- and nothing checks that any lens was applied, that the \
              same diff twice yields the same verdict, or that a finding it reports is present in the \
              change at all. What the gate measures is what one model said on one occasion.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "bench_status",
        aspiration: "Run the benchmark suite on base and head and refuse a hot-path latency or allocation \
                     regression beyond a published budget.",
        reference: "criterion.rs; Google Fleetbench; continuous benchmarking gates (bencher, CodSpeed)",
        fidelity: Fidelity::Heuristic,
        gap: "Times nothing and counts no allocation: no benchmark harness is declared by this crate and \
              none is invoked, so there is no budget for anything to be within. Two regexes stand in for \
              one. `clone_in_loop_re` (criterion_bench_ratchet.rs:68) fires only where an added line \
              already carries a trailing hot-path marker the author wrote. That marker is rare: the only \
              tracked occurrence is this guard's own test fixture. But a diff CAN add it, and one has -- \
              replaying commit 8014e94 (merged PR #1, which added that fixture) through this guard \
              yields one EXCESSIVE_HOTPATH_CLONE and `is_within_budget` \
              (criterion_bench_ratchet.rs:105) false. The verdict is therefore not a constant, which is \
              why this entry is Heuristic and not Aspirational -- but one author-written marker is \
              the entire check. \
              `unbounded_alloc_re` (criterion_bench_ratchet.rs:66,92) needs a loop header and a \
              binding of one specific name on the line below it. `hot_paths_evaluated` \
              (criterion_bench_ratchet.rs:64) counts changed paths by fragment and then scopes nothing, \
              since the scan runs over the whole diff either way -- yet the passing summary reports those \
              paths as evaluated within a latency and leak budget nothing measured.",
        blocked_on: Some(
            "a benchmark harness and a published trunk baseline; this crate declares neither",
        ),
    },
    GateFidelity {
        gate_id: "schema_compat_status",
        aspiration: "Detect a destructive schema change and prove it is safe for every node still running \
                     the previous release.",
        reference: "Expand-contract (Parallel Change); squawk and strong_migrations linters; Liquibase \
                    preconditions",
        fidelity: Fidelity::Heuristic,
        gap: "Three regexes for destructive DDL over added lines, entered only where `has_migration` \
              (pre_merge_guard/scanner.rs:37-39) finds a changed path spelling that word or ending in a \
              SQL extension. Neither half of the row's claim is measured: no node version is known, so \
              compatibility across cell nodes is asserted by the summary rather than checked, and nothing \
              looks at the release the schema ships with. The path filter is a plain substring, so the six \
              tracked source paths here that spell that word put a change into schema scope while no \
              tracked file is a schema at all. And the verdict is capped: the worst this scan can return \
              is `GateStatus::Warning` (pre_merge_guard/scanner.rs:52), which is acceptable and blocks \
              nothing, so a destructive migration it does detect is published as advice.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "performance_concurrency_status",
        aspiration: "Bound the concurrency a change introduces and catch a timing-dependent test before it \
                     flakes in CI.",
        reference: "Google flaky-test infrastructure; loom and shuttle concurrency model checking; \
                    ThreadSanitizer",
        fidelity: Fidelity::Heuristic,
        gap: "Two regexes for one shape: a real-clock sleep with a literal duration, in Rust or in Go \
              (`flake_patterns`, pre_merge_guard/scanner.rs:65-74), matched against added lines. It is a \
              real idiom and it fires, but it is everything this gate does: no interleaving is explored, \
              nothing is run twice, no test is repeated, no timing is recorded, and the concurrency half \
              of the row's title has no implementation at all. The verdict is capped as well -- the worst \
              result this scan can return is `GateStatus::Warning` (pre_merge_guard/scanner.rs:80), which \
              is acceptable, so a match neither blocks a merge nor withholds admission -- and every other \
              input returns `GateStatus::Passed` (pre_merge_guard/scanner.rs:89).",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "brand_absence_status",
        aspiration: "Refuse a name or a pull-request-visible string that stamps an aspiration, or borrows \
                     a vendor's credibility, instead of naming what the code verifies.",
        reference: "Google Rosie and Tricorder ratchets; ArchUnit FreezingArchRule; the debt-ledger \
                    ratchet pattern",
        fidelity: Fidelity::Partial,
        gap: "This one runs against the real tree and really fails. `scan_tree` (brand_absence/mod.rs:260) \
              reads Anvil's own source, extracts declared item names and string literals, matches them \
              word-wise against a fixed vocabulary (`FORBIDDEN_STAMPS`, brand_absence/mod.rs:63) plus a \
              vendor roll-call rule needing `VENDOR_ROLL_CALL_THRESHOLD` distinct vendors \
              (brand_absence/mod.rs:115), and reports as `new_violations` (brand_absence/mod.rs:362) \
              anything a ledger entry does not already account for. Three limits. The vocabulary is \
              hand-written, so a stamp nobody thought of is not a violation, and what the gate actually \
              measures is conformance to that list rather than the property in its own title. The scan is \
              of Anvil's own tree on every run, so on a foreign pull request the verdict describes THIS \
              repository and not the change under review. And the module ships itself advisory -- \
              `is_blocking: !WARN_ONLY` (brand_absence/mod.rs:381) is always false -- while the \
              certification run ignores that field and fails the gate on any new violation, so the \
              published severity is stricter than the module computing it declares.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "migration_boundary_status",
        aspiration: "Refuse a dependency pointing the wrong way across the migration boundary: code that \
                     is moving must not be anchored to code that is being deleted.",
        reference: "Strangler Fig migration (Fowler); the Dependency Rule; ArchUnit layered-architecture \
                    rules",
        fidelity: Fidelity::Partial,
        gap: "This one runs against the real tree and really fails. `live_tree_violations` \
              (migration/mod.rs:51) walks Anvil's own source, collects each top-level module's `crate::` \
              references, and applies `edge_is_allowed` (migration/boundary.rs:97,103) to the verdict each \
              side carries, reporting NotMeasured where the tree cannot be read rather than passing. Three \
              limits. Edges are extracted from text after dropping whole-line comments only, so a \
              reference inside a string literal, a trailing comment or a block comment counts as an edge, \
              while one reached through a re-export or a type alias does not. Granularity is two path \
              segments, so an edge into a module the ledger splits more finely is attributed to its \
              parent. And the verdicts themselves are hand-written data in `MIGRATION_LEDGER` \
              (migration/boundary.rs:38): the rule is enforced mechanically, but what it is enforced \
              against is somebody's classification, which no code here can check.",
        blocked_on: None,
    },
];

/// Gate ids whose implementation has NOT been read.
///
/// Empty by construction: anything absent from `AUDITED_GATES` is unaudited.
/// This function exists so the count is derived rather than maintained by hand.
pub fn unaudited_count(total_gates: usize) -> usize {
    total_gates.saturating_sub(AUDITED_GATES.len())
}
