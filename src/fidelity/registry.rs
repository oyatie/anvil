//! The declared fidelity of each audited gate.
//!
//! # Scope of this registry
//!
//! Entries exist ONLY for gates whose implementation was read directly during
//! the 2026-08-19/20 audit. Gates that were not read have no entry and are
//! counted as `unaudited` in the gap report.
//!
//! This is deliberate. Guessing a fidelity would reproduce exactly the failure
//! being corrected: a confident claim with nothing behind it. An honest
//! "not yet audited" is worth more than an invented "Heuristic".

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
              `trace_status` clones it (pre_merge_guard/evaluator.rs:296-302), the shape \
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
        fidelity: Fidelity::Partial,
        gap: "No SBOM is produced -- neither syft nor cargo-cyclonedx is invoked -- no provenance is \
              signed, and no deny.toml license or ban policy is evaluated. The audit half is real: \
              `query_batch` sends every locked version to the OSV advisory database \
              (supply_chain_guard.rs:192), and a runner that cannot reach it publishes NotMeasured \
              rather than a pass. It reads one lockfile and only one: repo_dir.join with Cargo.lock \
              (supply_chain_guard.rs:174). Every repository in the fleet that is not a Cargo \
              workspace is therefore permanently NotMeasured on this gate -- a narrowing, since the \
              regex this replaced at least read a package.json filename -- and the reference tool \
              reads any recognised lockfile. Advisory lists are complete or absent, never short: a \
              next_page_token in any result aborts the audit (osv_stream.rs:175) rather than \
              publishing a truncated first page as the answer.",
        blocked_on: Some(
            "an SBOM generator and a hosted signing platform; the advisory half is done",
        ),
    },
    GateFidelity {
        gate_id: "formal_verification_status",
        aspiration: "Encode authorization policy into SMT and prove non-escalation with a solver.",
        reference: "AWS Zelkova; Z3",
        fidelity: Fidelity::Heuristic,
        gap: "A chain of policy_content.contains(..) tests. No solver exists. The file and its types \
              were renamed from smt_solver.rs/SmtConstraintEngine to say so \
              (formal_verification/policy_scanner.rs:28). The rename stopped one line short of the \
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
        gate_id: "cleartext_transport_status",
        aspiration: "Verify workload identity via SPIFFE/SPIRE issued mTLS certificates.",
        reference: "SPIFFE/SPIRE; Istio PeerAuthentication STRICT. For the lint this actually is: \
                    CWE-319, gosec G107, semgrep insecure-transport",
        fidelity: Fidelity::Heuristic,
        gap: "Observes no SPIFFE ID, no SVID, no certificate and no mesh policy. All four are \
              runtime state -- a URI SAN inside an X.509 document handed to a process over the \
              Workload API after an agent attested its PID, a PeerAuthentication object in STRICT \
              mode inside a cluster -- and none of them can appear in a diff. Neither direction of \
              inference holds either: a tree with no plaintext URL can be running a mesh in \
              PERMISSIVE mode with no workload identity at all. What runs is a CWE-319 \
              cleartext-transport lint over added lines. It was the substring \"http://\", which \
              failed a merge for a licence URL in a doc comment; it now drops the comment tail in \
              code_before_comment (identity_auditor.rs:118), skips prose and fixture paths in \
              path_is_out_of_scope (identity_auditor.rs:98), and requires the authority after the \
              scheme to open with is_ascii_alphanumeric and not be one of the LOOPBACK_HOSTS \
              (identity_auditor.rs:159-160). Unlike the lints of that class it names, it is not \
              sink-anchored: there is no parser here, so it cannot tell a URL that reaches an HTTP \
              client from one that does not, and a URL assembled across lines or read from \
              configuration this diff does not touch is invisible to it. Comment stripping is \
              positional rather than lexical, so a `#` or a `//` inside a string that is not a URL \
              still truncates the line. The scorecard name, the published summary and the gate id \
              were all renamed to the lint this is, so nothing published still claims SPIFFE or \
              mTLS.",
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
        gate_id: "test_suite_status",
        aspiration: "Run the repository's own test suite against the pull request head and refuse a \
                     pull request whose tests fail.",
        reference: "GitHub required status checks; `cargo test`, `cargo nextest run`",
        fidelity: Fidelity::Partial,
        gap: "It runs the suite now, and only now: for a Cargo tree the gate ran a type-check, which \
              builds no test binary and executes no test, so a tree in which every test was red \
              passed the gate named Automated Test Suite. What runs is the repository's own suite -- \
              `cargo test --no-run` then `--no-fail-fast` (queue_healer.rs:698,733), or `npm test` \
              where a `package.json` names a test script (queue_healer.rs:666). Three ceilings \
              remain. It is Anvil's own run on one host against one toolchain, not the project's CI \
              matrix, so a platform-specific failure is invisible to it. It knows exactly two \
              ecosystems, and a Go, Python or Gradle repository offers it nothing. And a Cargo \
              repository with no tests at all exits zero and is reported as a pass, because cargo \
              has no distinct signal for an empty run. The build is a separate invocation because \
              cargo exits 101 for a compile error and libtest exits 101 for a failing test: a tree \
              that did not build ran no test, so it is `Errored` and not an accusation \
              (queue_healer.rs:714). The child environment is scrubbed of `CARGO_TARGET_DIR` and \
              `CARGO_BUILD_TARGET_DIR` (queue_healer.rs:700,735), because a target directory shared \
              between two ephemeral worktrees of one repository collapses the two steps back into \
              one and restores exactly the behaviour above; a cargo config file inside the \
              tenant tree can still redirect the target directory and is not defended against. Two further \
              ceilings. The `ExecClass::Build` bound of 1800s was sized for a type-check and now \
              has to cover a build and a run, and `heal_ejected_pr` calls `run_local_test_gate` twice \
              (queue_healer.rs:301,309), so one heal can spend an hour before reporting that it \
              measured nothing. And the run executes every `#[test]` in a contributor's branch inside \
              the daemon's own process environment, which holds `GITHUB_WEBHOOK_SECRET` \
              (config.rs:131) -- a type-check never ran that code. The cost is a cold build per \
              pull request, in an ephemeral worktree with no shared target directory.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "rust_skills_status",
        aspiration: "Enforce the project's Rust idiom and safety rules over changed code at the \
                     fidelity of a linter that parses the language.",
        reference: "`cargo clippy -- -D warnings`; clippy's restriction group; the upstream \
                    rust-skills corpus",
        fidelity: Fidelity::Heuristic,
        gap: "No clippy run, no rustc lint, no parser: seven regexes over the lines a diff adds \
              (rust_language_policy/engine.rs:88-125), four of which can block. `err-no-unwrap-prod` is a text match for \
              `.unwrap()` on any line whose path does not contain the word test, so it sees neither \
              the receiver's type nor whether the call is reachable; `unsafe-safety-comment` asks \
              only whether the preceding line carried a marker. The upstream corpus the gate is \
              named after is not fetched, parsed or consulted anywhere in this binary, and its size \
              was published on every pull request as a literal -- including on pull requests \
              changing no Rust at all, where the same literal was published beside the sentence \
              that the check had passed. `rules_evaluated_count` is now the length of the ruleset \
              that actually ran (rust_language_policy/mod.rs:172), and zero when nothing was scanned (rust_language_policy/mod.rs:121). Scope \
              is added lines, so Rust this pull request does not touch is never examined and a \
              clean verdict here is not a statement about the repository.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "attestation_status",
        aspiration: "Emit a signed provenance statement binding this artefact, by digest, to how it \
                     was produced, and record it where a third party who does not trust the \
                     producer can verify it.",
        reference: "in-toto attestation v1 (subject digest, predicateType); DSSE PAE envelopes; \
                    cosign attest with Fulcio and Rekor; SLSA v1.0 build levels; RFC 6962 \
                    transparency logs",
        fidelity: Fidelity::Aspirational,
        gap: "Attests nothing. No digest is computed over any artefact, no DSSE envelope is built, \
              no signature is produced -- the crate holds no signing key and no X.509 or ECDSA \
              dependency -- and no transparency log is written or read, so there is no verifier \
              here and nothing for one to check. What runs is `serde_json::to_string_pretty` and \
              `fs::write`, and the gate's pass used to be rebuilt in the wiring from a boolean \
              whose one production value was a literal, which made the failure arm unreachable. \
              The guard now owns the verdict and publishes `NO_PROVENANCE_BACKEND` \
              (attestation_guard.rs:116,209-211). A hash-chained receipt log was considered and \
              rejected rather than shipped: the chain would be unkeyed, so recomputing it after \
              an edit is the write path rather than an attack on it, and receipts are per-pull-\
              request files overwritten in place inside a per-run clone, so there is no \
              append-only log to chain in the first place. The receipt was also swept onto the \
              pull request by the certification pipeline's own staging sweep; all four staging \
              sites now share `stage_excluding_receipts` (git_manager/mod.rs:32).",
        blocked_on: Some(
            "a signing identity and a log to publish to -- a key or an OIDC issuer plus Fulcio, \
             and a transparency log; none is reachable from here",
        ),
    },
    GateFidelity {
        gate_id: "cedar_status",
        aspiration: "Decide, offline, whether the Cedar policy set authorises every action a change \
                     introduces: validate the policies against a schema, then answer authorization \
                     requests against an entity store.",
        reference: "cedar-policy crate (Authorizer::is_authorized, Validator::validate); the cedar CLI; \
                    AWS Verified Permissions; Zelkova / IAM Access Analyzer for SMT permissiveness \
                    comparison",
        fidelity: Fidelity::Partial,
        gap: "Parses; it does not validate. The reference checker is spawned over the .cedar files the \
              diff touched, one per file, and decides one property soundly: the policy set is \
              grammatical Cedar. Everything past the grammar needs a schema -- the validate and symcc \
              subcommands each take one as a required argument -- and this repository carries neither a \
              schema nor an entity store, so no entity type, action, attribute or operand type is \
              checked and no request is decided. The headline claim the gate is named for, that a \
              policy covers the route a pull request added, goes with them: it is reported as nothing \
              measured rather than put to a model. Scope used to be three path substrings, which \
              admitted any Rust file spelling one of them and no policy file spelling none; it is now \
              an `ends_with` test on the extension a parser can read (cedar_guard.rs:102). The verdict \
              used to be a literal on all three exits, including the one reached after the model this \
              gate paid had answered non-compliant; `verify` is now total over what the checker \
              returned, and the model is deleted (cedar_guard.rs:172). Where the checker is not \
              installed, and where it rejects Anvil own invocation rather than the policy, the gate \
              measures nothing and says so: `interpret_cedar_outcome` keeps those two exit codes apart \
              so a flag renamed here cannot read as a policy defect there (cedar_guard.rs:115).",
        blocked_on: Some(
            "a Cedar schema, and an entity store to decide a request against; this repository has \
             neither, and both validate and symcc require the schema",
        ),
    },
    GateFidelity {
        gate_id: "schema_evolution_status",
        aspiration: "Enforce strict backward and forward wire compatibility for Protobuf and OpenAPI \
                     schemas against the previously published version of each schema.",
        reference: "buf breaking against a stored image; Confluent Schema Registry \
                    BACKWARD/FORWARD/FULL; oasdiff",
        fidelity: Fidelity::Heuristic,
        gap: "Parses no schema and holds no baseline. buf compiles both revisions to descriptor sets \
              and compares them against an --against image, the registry compares a candidate against \
              a subject's registered versions, and oasdiff compares two resolved documents; this reads \
              the text of one pull request's diff. It also had no file-type scope of any kind, so any \
              removed line carrying a type word and an equals sign was published as a breaking wire \
              schema \
              change: over this repository's own last ten commits that failed four of them, every \
              finding a line of Rust, in a tree holding no protobuf file at all. Scope is now the \
              path -- `classify` returns a schema language for a name ending `.proto`, or a YAML one \
              beginning `openapi` or `swagger` (compatibility_checker.rs:50-59), and every other file \
              is skipped before a line of it is read (schema_evolution/mod.rs:66-67). Inside that \
              scope it covers two of buf's fifteen WIRE rules, \
              `FIELD_NO_DELETE_UNLESS_NUMBER_RESERVED` (compatibility_checker.rs:221-224) and \
              `MESSAGE_SAME_REQUIRED_FIELDS`, plus reuse of a deleted field number, and exactly one \
              of oasdiff's 219 checks -- `api-path-removed`, read off removed path keys rather than \
              off two documents (compatibility_checker.rs:238-259). A file the pull request creates -- one whose \
              diff section carries `new file mode` -- is skipped outright \
              (schema_evolution/mod.rs:74-76): it has no previous revision, and alleging a \
              break against no baseline is the same defect narrowed to one file type. A narrowed \
              response type, a newly required request property, a removed operation under a \
              surviving path, and any schema change outside a diff hunk are all invisible to it. A deleted enum value it does \
              report, which is the right verdict under buf's enum-deletion rule and the wrong noun: \
              it is published as a field. No tracked file in this repository is a protobuf \
              definition, so `NO_SCHEMA_IN_SCOPE` rather than a pass is its ordinary verdict on a \
              Rust change; the one OpenAPI description here does put it in scope, and reverting the \
              commit that published this repository's health endpoint is caught.",
        blocked_on: Some(
            "a descriptor set or registry baseline; one diff is not a published schema",
        ),
    },
    GateFidelity {
        gate_id: "zero_day_status",
        aspiration: "Detect upstream zero-day advisories against the workspace lockfiles and open the \
                     patch that closes them.",
        reference: "RustSec advisory-db; Dependabot security updates; Renovate",
        fidelity: Fidelity::Aspirational,
        gap: "Reads no advisory feed and writes no patch. The evaluation matched an empty advisory list \
              against the pull request diff, never against a lockfile, so every pull request was \
              certified clean; nothing in the module edits a manifest or opens a pull request. It now \
              publishes `NO_PATCH_SYNTHESIS` instead (zero_day_patcher/mod.rs:33). Advisory detection \
              against the locked dependency graph moved to gate 6, which is real.",
        blocked_on: Some(
            "a manifest writer and a bot identity with write access; detection alone is \
                          already covered by gate 6",
        ),
    },
    GateFidelity {
        gate_id: "feature_flag_status",
        aspiration: "Retire toggles the flag-management system records as stale, and delete the dead \
                     fallback branch each one guards.",
        reference: "LaunchDarkly flag health and ld-find-code-refs; Unleash flag lifecycle; Uber piranha",
        fidelity: Fidelity::Heuristic,
        gap: "Queries no flag-management system. Staleness is a fact LaunchDarkly, Unleash and \
              Statsig each compute on their own backend -- from flag age plus evaluation status, or \
              from an admin-set boolean -- and none of them expects anything in the source at all. \
              What ran here instead were three rules matching two invented annotations and a year \
              window that ended in 2025, none of which occurred anywhere outside this module's own \
              fixture, so the gate published a green no pull request could turn red. \
              What runs now is the half that has a real counterpart: a regex over the added lines \
              for a toggle read by a key written at the call site \
              (feature_flag_ratchet.rs:124), which is what ld-find-code-refs does. It is a proxy \
              in both directions. The call names are a fixed list -- `is_feature_enabled`, \
              `useFeatureFlag` and two more -- so a wrapper spelled any other way is invisible, and \
              a key passed as a variable, a constant or an enum is invisible to any text scan; \
              equally, a map lookup spelled the same way is counted as a toggle. \
              Whether a key it finds is stale is answered from a ledger the repository under \
              review may keep, `LEDGER_PATHS` (feature_flag_ratchet.rs:65), matched by `ledger_records_stale`, \
              which asks whether the key appears between backticks on a line \
              (feature_flag_ratchet.rs:245). That ledger is \
              Anvil's own convention rather than an industry one -- Chromium is the nearest real \
              precedent and keeps expiry in a JSON metadata file, not in source -- and it is \
              self-attested by whoever edits it. No tracked file in this repository is such a \
              ledger, so the gate reports that nothing was looked up. It also reports that when a \
              ledger exists and the change reads no toggle: an empty scope is not a retired flag. \
              Neither the dead fallback branch nor its deletion is detected; piranha does that by \
              tree-sitter AST rewriting, and nothing here parses anything.",
        blocked_on: Some(
            "a LaunchDarkly, Unleash or Statsig API to ask; the ledger is a self-attested stand-in",
        ),
    },
    GateFidelity {
        gate_id: "local_probe_status",
        aspiration: "Run the checks a developer's pre-commit and commit-msg hooks run -- commit \
                     message conformance and a credential scan -- against this pull request's own \
                     commits.",
        reference: "Conventional Commits 1.0.0; @commitlint/config-conventional; pre-commit commit-msg stage",
        fidelity: Fidelity::Heuristic,
        gap: "No AST is built and no parser crate is a dependency, so the AST linting the title \
              claimed never existed; a Rust file parser needs a whole valid file and the added lines \
              of a unified diff are not one. The title no longer claims it. \
              The commit half graded a string this file wrote: the caller passed a hardcoded \
              message to a check that was `starts_with` on a type prefix, which accepts a header \
              with no colon and no description and accepts `feature` as a type, none of which \
              Conventional Commits 1.0.0 admits. The subjects are now read from the clone the \
              pipeline already holds, by `commit_subjects` (git_manager/mod.rs:525), and judged \
              against `CONVENTIONAL_HEADER` (harness/judgement.rs:257) with commitlint's default type \
              list plus this repository's own promote type -- type-enum is configuration, not \
              specification, and hardcoding the default made the check red on the convention the \
              project follows. Two \
              gaps remain there: only the subject line is read, so a breaking-change footer and \
              a body are not checked, and none of commitlint's other default rules -- length, \
              case, trailing stop -- is enforced. Subjects git generates rather than the author \
              writes are skipped -- `GENERATED_SUBJECT_PREFIXES` (harness/judgement.rs:263), as commitlint's own defaultIgnores skip \
              them; a pull request made entirely of those is reported unmeasured rather than \
              clean. \
              The credential half delegates to `PreMergeScanner::scan_for_secrets` \
              (fast_validator.rs:61), which matches whole credentials on added lines only. It \
              used to be four bare vendor prefixes tested against the whole diff, so a change \
              that DELETED a leaked key was refused for containing one and any change touching \
              this repository's own AWS-key regex blocked itself. Six regexes is still not a \
              secret scanner: no entropy check, no bare token without a recognised shape, and \
              most vendors' formats pass it. \
              `latency_ms` is now this call's own elapsed time (local_inner_loop/mod.rs:143) \
              rather than a constant; it times the gate, and says nothing about the pull request \
              or about any developer's machine.",
        blocked_on: Some(
            "nothing external -- the remaining commitlint rules and a real secret scanner are \
             unwritten, not blocked",
        ),
    },
    GateFidelity {
        gate_id: "chaos_injection_status",
        aspiration: "Inject packet loss, DNS latency and a database leader failover into a running \
                     deployment of this change, and verify the steady state returns.",
        reference: "Netflix Chaos Monkey; AWS FIS; Gremlin; LitmusChaos; principlesofchaos.org",
        fidelity: Fidelity::Heuristic,
        gap: "Injects no fault into anything. Every tool this gate is named for acts on a running \
              system -- Chaos Monkey terminates live instances through Spinnaker, FIS and Gremlin \
              act on live resources, LitmusChaos on live workloads -- and a steady-state hypothesis \
              presupposes a system in a steady state to disturb. Nothing here starts one. \
              What ran before was worse than absent: three faults were declared and handed to a \
              simulator that never read the argument, so one two-substring scan produced three \
              identical verdicts, each carrying a fixed recovery time for an experiment that did \
              not run, and the blocking sentence named a preview sandbox that is not deployed, \
              spawned or configured anywhere in this repository. All of that is deleted. \
              What remains is a lint, published as one: added lines whose text contains \
              `.await.unwrap()` once whitespace is removed (chaos_injector/mod.rs:157). That is \
              the property `clippy::unwrap_used` checks, which upstream files under the opt-in \
              restriction group rather than correctness -- so a hit is a Warning, not a refused \
              merge. It blocked once, and was red on ten lines of its own diff with no true \
              positive among them. It is text, not syntax, but only over `code_only` \
              (chaos_injector/mod.rs:153), which drops a comment and empties a string literal, so \
              prose about the property is no longer counted as the property. That is one line at \
              a time with no memory of the last, so the continuation line of a multi-line string \
              literal -- this sentence, for one -- carries no opening quote and is still counted; \
              an unwrap split across lines is still invisible, and an expect on the same await is \
              not matched at all. Only added lines are read, as `code_line` \
              (chaos_injector/mod.rs:152), so an unwrap this change leaves untouched is invisible, \
              and a line in a test module is indistinguishable from one in production code -- which \
              is the other reason this warns rather than blocks. A diff \
              with no such line is reported unmeasured, not resilient: nothing was made to fail, so \
              nothing survived failing.",
        blocked_on: Some("a running deployment a fault injector can act on"),
    },
    GateFidelity {
        gate_id: "adr_status",
        aspiration: "Bind every architectural change to a decision record, and hold each record to the \
                     field schema its repository requires.",
        reference: "Nygard's ADR format; MADR 4.0; adr-tools; Structured MADR's JSON-Schema CI action",
        fidelity: Fidelity::Heuristic,
        gap: "Presence of a key, not conformance of a decision. The five field names were a Rust literal \
              matched word-by-word against the whole pull request diff, so achieves, origin, rule and \
              ensure were satisfied by ordinary English in any file the change touched, and only \
              overturn-when was rare enough to ever go red. The list is now read from the repository \
              under review, from one of `SCHEMA_PATHS` (adr_drift_ratchet.rs:154); a repository \
              declaring none reports `GateStatus::NotMeasured` (adr_drift_ratchet.rs:217); and a field is a key before a colon rather than a word, \
              which is what `declared_key` (adr_drift_ratchet.rs:108) decides -- it strips heading, \
              list and bold marks and compares alphanumerics only, so Overturn-When: is a field and \
              the sentence this rule achieves parity is not. What no part of that reads is the \
              decision. Whether the rule line states a rule, whether the change under review obeys it, \
              and whether the overturn-when condition has already occurred are all outside what a key \
              scan can see; this is the presence lint Structured MADR is, not the fitness function \
              Ford and Parsons describe, and no tool in the survey derives one from an ADR \
              mechanically. The record is read off disk when it is there and off the hunks via \
              `added_lines_for` when it is not (adr_drift_ratchet.rs:245), and only when the read \
              failed with `NotFound`, so an ADR that exists but is untouched by this pull request \
              is never checked at all and a record this diff deletes is skipped rather than charged \
              five missing fields. A change arriving with no \
              record fills `architectural_changes_without_adr` (adr_drift_ratchet.rs:188) and is not \
              charged: the predicate is a filename guess -- lib.rs, ports, adapters -- that this \
              repository's own history trips without any decision going unrecorded, and the branch it \
              replaces published an auto-scaffolded verdict naming a file nothing wrote.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "compliance_status",
        aspiration: "Evaluate a change against the statutes in force for it, across jurisdictions, on the \
                     date it is reviewed.",
        reference: "Google Cloud Sensitive Data Protection infoTypes; Microsoft Presidio; Semgrep registry rulesets",
        fidelity: Fidelity::Heuristic,
        gap: "A regex scan of added lines against five rules, presented as an engine spanning five \
              jurisdictions. The evaluation date is now `chrono::Utc::now()` \
              (compliance_guard/mod.rs:104) rather than a literal that had stopped moving; \
              `read_rule_pack` is called on the tree under review on every run \
              (compliance_guard/mod.rs:119) rather than by nothing; `statutes_evaluated` is built from \
              the rules that ran (compliance_guard/mod.rs:130) rather than from a list advertising \
              seventeen; and `r.pattern_regex.is_some()` now keeps a rule the engine cannot \
              evaluate out of the count published beside the verdict \
              (compliance_guard/upstream_sync.rs:58). The pack is returned by `load_rule_pack` \
              (compliance_guard/upstream_sync.rs:78) rather than written into shared state, so one \
              repository's rules do not judge the next, and a pack rule claiming a rule id already \
              enforced is rejected into `pack.rejected` (compliance_guard/upstream_sync.rs:119) \
              rather than \
              replacing the statute that would have judged the change adding it. A match can be \
              waived by a line naming the rule, `SUPPRESSION_MARKER` \
              (compliance_guard/engine.rs:37) -- the escape hatch every oracle here has and without \
              which a repository cannot carry a test PAN in a fixture -- and each waiver is counted \
              into the published sentence rather than being silent. What remains is still pattern \
              matching. \
              Sensitive Data Protection reaches a graded likelihood by combining a pattern with a \
              checksum and surrounding context, and Presidio routes every regex hit through a \
              validator that can zero the score; neither step exists here, so \
              `4[0-9]{12}(?:[0-9]{3})?` (compliance_guard/upstream_sync.rs:253) accepts any \
              sixteen-digit number opening with a four as a card number, with no Luhn check and no \
              context word. The ePHI rule is three literal column names, \
              `patient_icd10|medical_record_number|clinical_diagnosis` \
              (compliance_guard/upstream_sync.rs:229), which is a spelling list rather than a \
              detector: a schema abbreviating the medical record number is invisible to it. Scope is \
              added lines only, so a statute violated by code this pull request leaves alone is never \
              seen, and the pack is plain files with no version, no signature and none of the \
              staleness bound Grype imposes on its database.",
        blocked_on: None,
    },
    GateFidelity {
        gate_id: "cross_service_status",
        aspiration: "Prove a schema change breaks no downstream consumer, against the contracts those \
                     consumers registered.",
        reference: "buf breaking against a stored image; Pact Broker can-i-deploy; Confluent Schema Registry compatibility modes",
        fidelity: Fidelity::Heuristic,
        gap: "Names a removed field; proves nothing about a consumer. The predicate was a path holding \
              api or proto and the diff holding a minus sign, three spaces and required: -- three \
              exactly, which matches no line in this repository, whose every required: sits at eight \
              or fourteen -- and on a hit it published two invented service names as the impacted \
              services. Both are gone. What runs is a set difference over the names a required: key \
              carries on each side of a hunk, `let before = required_names` \
              (cross_service_impact/contract_scan.rs:131), read by `required_names` \
              (cross_service_impact/contract_scan.rs:84), which reads no column, so re-indenting a \
              block is not a break. That is a text scan of hunks, not the model comparison the oracles \
              perform: buf compiles both sides to a descriptor set and oasdiff parses both to an \
              OpenAPI model, so both see a reference resolved, a schema moved between files, and a \
              type narrowed, and none of those is visible here. Request and response direction are not \
              told apart, so a relaxed request schema is reported alongside a broken response one. \
              Scope is whatever `is_wire_contract` (cross_service_impact/contract_scan.rs:67) admits, \
              a filename and extension guess -- narrowed to YAML, because `.proto` and `.json` were \
              admitted by a parser that reads neither spelling, so a JSON Schema losing a required \
              field produced no finding under a sentence saying the file had been read. And the consumer set is not derived at all: \
              `NO_CONSUMER_REGISTRY` (cross_service_impact/contract_scan.rs:43) is the sentence the \
              finding carries instead, because Pact learns consumers from published pacts, Confluent \
              from a subject's registered versions and buf from a stored image, and none of those is \
              configured here.",
        blocked_on: Some("a Pact broker, schema registry or module graph naming the consumers"),
    },
    GateFidelity {
        gate_id: "security_scan_status",
        aspiration: "Detect credentials a change leaks, and block the merge on a credential that \
                     is live.",
        reference: "TruffleHog, whose distinguishing feature is calling the issuing provider to \
                    confirm the key is active; gitleaks per-rule entropy and allowlists; GitHub \
                    secret scanning push protection with partner validation",
        fidelity: Fidelity::Heuristic,
        gap: "Calls no provider and verifies nothing, which is the whole of what separates this \
              from the reference tool: a finding here is a shape that resembles a credential, \
              never a credential confirmed to be live, so it cannot tell a rotated key from a \
              working one. Seven regexes over added lines in SECRET_RULES (harness/judgement.rs:76). Four \
              carry a provider-issued prefix and are conclusive on their own, so they run with \
              min_entropy: 0.0 and no filtering (harness/judgement.rs:80). Two had no anchor and were the \
              two that produced this gate's false merge blocks; each now captures the candidate \
              rather than the line and filters it -- `sk-[A-Za-z0-9]{24,}` at min_entropy: 3.5 \
              (harness/judgement.rs:119,121) and a quoted value of eight or more characters after the word \
              password at min_entropy: 3.0 (harness/judgement.rs:140,142). shannon_entropy is a real \
              logarithm (harness/judgement.rs:155) and the file had none before, but it is the last filter \
              and not the decision: is_credential_shaped rejects a candidate made only of \
              is_ascii_alphabetic characters and identifier punctuation before entropy is ever \
              consulted (harness/judgement.rs:180,189), because entropy alone cannot reject a \
              kebab-case identifier. The reference config additionally anchors its own key rule on \
              the literal marker T3BlbkFJ that every issued key of that vendor embeds; this rule \
              does not, so a long enough base62 run behind the prefix is a finding whoever issued \
              it. Nothing here reads git history, so a credential added by an earlier commit and \
              merely retained by this one is outside the scan.",
        blocked_on: Some("network egress to the issuing providers, for verification"),
    },
    GateFidelity {
        gate_id: "canary_status",
        aspiration: "Evaluate a live canary deployment's error budget burn rate and tail latency \
                     against production telemetry, and trip a circuit breaker that halts the \
                     rollout before the budget is spent.",
        reference: "Argo Rollouts AnalysisTemplate over Prometheus; Flagger; Spinnaker/Kayenta; \
                    Google SRE Workbook multiwindow multi-burn-rate alerting",
        fidelity: Fidelity::Aspirational,
        gap: "Queries no telemetry: this crate carries no HTTP client, deploys no canary and reads \
              no metrics endpoint. The guard used to build the reading four lines above the ceiling \
              it was compared against, so the branch was decided at compile time and the published \
              sentence described a literal rather than the pull request. That reading is deleted \
              and evaluate_without_metrics_source is the only path the pipeline takes \
              (canary_rollout/mod.rs:106-110). The circuit breaker survives as the seam a real \
              query plugs into, and it is honest but narrower than the name: it compares \
              burn_rate_5m and p99_latency_ms against caller-supplied bounds \
              (canary_rollout/circuit_breaker.rs:41,51), which is a single-window rule. The SRE \
              Workbook walks that shape through as its Approach 4 and rejects it for recall, \
              recommending a long window paired with a short one and a threshold expressed as a \
              factor of the error budget rather than as a bare ratio; neither the pairing nor an \
              SLO target exists here, so what survives is not dimensionally a burn rate.",
        blocked_on: Some(
            "a canary deployment and a reachable Prometheus or OpenTelemetry endpoint; this crate \
             has no HTTP client to reach one with",
        ),
    },
    GateFidelity {
        gate_id: "shuffle_status",
        aspiration: "Verify that the tenant-to-cell assignment in force gives every tenant a \
                     distinct shuffle shard, and that no two tenants share enough cells for one \
                     cell's failure to take both of them down.",
        reference: "AWS Builders' Library, Workload isolation using shuffle-sharding; Route 53 \
                    infima; AWS cell-based architecture guidance",
        fidelity: Fidelity::Aspirational,
        gap: "Reads no tenant-to-cell mapping table, and a pull request diff carries none: the \
              assignment is control-plane state. The guard used to declare its own two-tenant \
              table, whose two shards shared exactly as many cells as the bound permitted, on \
              every pull request forever. That table is deleted and \
              evaluate_without_topology_source is the only path the pipeline takes \
              (shuffle_shard_simulator/mod.rs:122). The combinatorics survive as the seam a \
              real table plugs into -- calculate_combinations and evaluate_overlap are honest \
              (shuffle_shard_simulator/math.rs:41,57). What the gate published was also the wrong \
              quantity: cells per tenant over total cells is one tenant's infrastructure \
              footprint, and it rises as isolation improves. It is now \
              uniform_random_shard_collision_ratio, the reciprocal of the number of possible \
              shards that the infima javadoc defines as blast radius, and the name says \
              uniform_random because compute_metrics derives it from the two integers without \
              reading allocations at all \
              (shuffle_shard_simulator/math.rs:81). Checking a finished table is still weaker \
              than the oracle, which enforces the bound at assignment time with a sharder that \
              backtracks against every shard already handed out.",
        blocked_on: Some(
            "a tenant-to-cell mapping table, from a control plane or from a checked-in topology",
        ),
    },
    GateFidelity {
        gate_id: "progressive_ring_status",
        aspiration: "Advance a change through progressive-exposure rings only once the ring it \
                     occupies has baked for its declared minimum and no region pair is taking the \
                     rollout on both halves at once.",
        reference: "Azure Safe Deployment Practices; Azure Well-Architected OE:11 safe deployment; \
                    Azure region pairs",
        fidelity: Fidelity::Aspirational,
        gap: "Deploys nothing and reads no cloud control plane, so the elapsed bake time and the \
              live region set are both unknown and evaluate_without_rollout_state is the path the \
              pipeline takes (progressive_rollout/mod.rs:138). The health verdict used to be a \
              constant threaded through three calls and answered with the same literal in all four \
              arms of the scheduler; the field that carried it is gone, and the two validators \
              that check something real -- which had zero production callers -- are now reached \
              only through evaluate_ring_advance, which runs both \
              (progressive_rollout/mod.rs:103,114). validate_bake_window compares \
              elapsed_bake_minutes against the manifest's own min_bake_minutes, and an undeclared \
              ring is no longer treated as satisfied \
              (progressive_rollout/ring_scheduler.rs:127,137). compute_next_ring returns an \
              Option and holds the advance rather than reading an undeclared ring as \
              traffic_percentage zero, which was the same inversion one level up \
              (progressive_rollout/ring_scheduler.rs:103). AZURE_REGION_PAIRS held region codes \
              lifted from a different cloud, paired by a rule Azure does not use -- it \
              pairs East US with West US, not with East US 2 -- and now holds the published table \
              (progressive_rollout/ring_scheduler.rs:20-27). It stays partial: asymmetric pairs \
              and the growing set of nonpaired regions are not modelled, and a region it does not \
              name is treated as unpaired.",
        blocked_on: Some(
            "rollout state -- a bake clock over a deployed artefact, and the set of regions \
             currently taking the rollout",
        ),
    },
];

/// Gate ids whose implementation has NOT been read.
///
/// Empty by construction: anything absent from `AUDITED_GATES` is unaudited.
/// This function exists so the count is derived rather than maintained by hand.
pub fn unaudited_count(total_gates: usize) -> usize {
    total_gates.saturating_sub(AUDITED_GATES.len())
}
