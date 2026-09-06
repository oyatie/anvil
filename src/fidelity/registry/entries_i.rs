//! One bin of `AUDITED_GATES` entries.
//!
//! The split is by size, not by subject: `registry.rs` held every entry and
//! was 1166 lines against a 300-line budget. The order of the corpus lives in
//! `registry::AUDITED_GATES`, so nothing here is meant to be read as a grouping.

use crate::fidelity::{Fidelity, GateFidelity};

pub const MONOREPO_STATUS: GateFidelity = GateFidelity {
    gate_id: "monorepo_status",
    aspiration: "A package builds from its own declared inputs alone -- no path escaping its \
                 boundary, no undeclared dependency, no absolute host path -- so any target is \
                 reproducible from the dependency graph.",
    reference: "Bazel hermeticity and sandboxed actions; explicit dependency declaration in a \
                monorepo build graph",
    fidelity: Fidelity::Heuristic,
    gap: "Builds nothing and constructs no dependency graph, so hermeticity is inferred rather \
          than tested. Three regexes over added lines carry it: three or more parent-directory \
          hops inside an include-like call, a quoted absolute path under a short list of home \
          prefixes, and a prototype-assignment spelling that is not a monorepo property at all \
          (monorepo_guard/mod.rs::evaluate_monorepo_hygiene). The undeclared-import half is \
          delegated to `scripts/check-undeclared-imports.mjs`, a script the reviewed repository \
          must ship; where it exists the gate fails closed on it, and where it does not -- here, \
          and on most repositories -- nothing runs and its absence is not reported. Two sub-rules \
          do measure their subjects: changed paths are compared against \
          `BANNED_HARNESS_PREFIXES` (monorepo_guard/harness_quarantine.rs::BANNED_HARNESS_PREFIXES), \
          and the size rule reads the file from disk and charges only a change that grew it past \
          `MAX_WHOLE_FILE_LINES` (monorepo_guard/whole_file_expansion.rs::evaluate_whole_file). The \
          authority rule is substring again -- `canonical_authority: true`, or `source of truth` \
          alongside `canonical`, outside two directory prefixes \
          (monorepo_guard/harness_quarantine.rs::check_ssot_authority_location). A change none of \
          those rules can see leaves `is_compliant` true, under the report's sentence `Monorepo \
          boundary rules verified: hermetic boundaries, harness quarantine, and SSOT authority \
          rules 100% compliant.` -- which the bare pass drops. A finding blocks.",
    blocked_on: None,
};

pub const PERFORMANCE_CONCURRENCY_STATUS: GateFidelity = GateFidelity {
    gate_id: "performance_concurrency_status",
    aspiration: "Concurrent code the change adds is free of races and unbounded execution, and the \
                 tests it adds do not depend on real time.",
    reference: "ThreadSanitizer and loom for concurrency; hermetic, clock-free tests",
    fidelity: Fidelity::Heuristic,
    gap: "Two patterns, both for a sleep, are the whole gate: a Rust thread sleep with a \
          millisecond duration literal, and the equivalent spelling in Go \
          (pre_merge_guard/scanner.rs::scan_for_concurrency_and_flakes). No concurrency is \
          analysed, no execution bound is derived and no timing is measured, so of the three \
          words in the published name only the flake half has any implementation, and that half \
          is one shape of one construct: a sleep computed from a variable, a longer unit, or a \
          poll loop is not matched. Nothing distinguishes a sleep in a test from one in shipped \
          code. The scan returns on the first hit, so the count a reader is shown is one however \
          many there are, and the finding is `Concurrency/Timing Warning:` -- a warning, which \
          blocks nothing. Every other diff is `GateStatus::Passed`, which includes every change \
          that adds concurrency this scan cannot see \
          (pre_merge_guard/evaluator.rs::evaluate_pre_merge_gates).",
    blocked_on: None,
};

pub const PSA_STATUS: GateFidelity = GateFidelity {
    gate_id: "psa_status",
    aspiration: "Every namespace the change declares enforces the restricted Pod Security \
                 Standard, or carries an exception recorded with an owner and an expiry.",
    reference: "Kubernetes Pod Security Admission, restricted profile; time-boxed exception \
                registries",
    fidelity: Fidelity::Heuristic,
    gap: "Checks the presence of a label key, not its value. The rule is that the text contains \
          `kind: Namespace` and does not contain `pod-security.kubernetes.io/enforce:`, so a \
          manifest enforcing the privileged or baseline profile satisfies it exactly as the \
          restricted one does, and the word restricted in the label describes nothing the code \
          compares (psa_admission_guard/psa_rules.rs::evaluate_psa_manifest). No YAML is parsed: \
          the two tests run over the whole hunk, so a label on a different document in the same \
          file, or inside a comment, excuses the namespace. There is no exception registry. What \
          the label calls a recorded expiring registry is two hardcoded path substrings, \
          `local-path-storage` and `ci-workspace-storage`, carrying no owner, no expiry and no \
          record of who granted them. Every finding names the namespace `unlabelled`, because the \
          manifest's own namespace is never read, so the accusation cannot say which one it is \
          about. `is_compliant` is the emptiness of the findings \
          (psa_admission_guard/mod.rs::evaluate_psa_admission), and the verdict blocks.",
    blocked_on: None,
};

pub const REVIEW_VERDICT_STATUS: GateFidelity = GateFidelity {
    gate_id: "review_verdict_status",
    aspiration: "An adversarial review examines the change through a stated matrix of lenses, and \
                 a verdict asking for changes withholds the merge.",
    reference: "the aspects Google's code review guidance requires a review to cover; \
                model-as-judge against a published rubric",
    fidelity: Fidelity::Partial,
    gap: "A review does run and its verdict decides the gate, and the three-way split is right: a \
          response that could not be parsed is `Errored`, never `Failed`, so a review that did not \
          happen is not published as a refusal it never made. The gate itself is a three-arm match \
          on a string -- `APPROVE` and one other token pass, everything else fails \
          (pre_merge_guard/gates.rs::review_verdict_gate) -- and nothing about the review's content \
          is examined anywhere. In particular nothing verifies the sixteen lenses the published \
          name claims: `REVIEW_ASPECTS` names ten aspects taken from that guidance \
          (reviewer/rubric.rs::REVIEW_ASPECTS), and the number in the name survives only in the \
          response schema the prompt asks the model to fill in -- a table the model writes about \
          its own work. No check establishes that a lens was applied, that the diff was read, or \
          that a finding corresponds to anything in it, so the fidelity of this gate is the \
          reviewer's fidelity and that is not measured here.",
    blocked_on: None,
};

pub const RUNNER_ECONOMICS_STATUS: GateFidelity = GateFidelity {
    gate_id: "runner_economics_status",
    aspiration: "Pull-request jobs run on the cheapest capacity that can serve them, spot where it \
                 is available, and expensive runner classes are reserved for the merge train.",
    reference: "GitHub Actions runner labels and per-minute billing rates; preemptible and spot CI \
                capacity",
    fidelity: Fidelity::Heuristic,
    gap: "Knows no price. No billing rate, no runner inventory and no job duration is read, so \
          nothing here can order two runners by cost, and the spot allocation the published label \
          names has no implementation at all -- no rule mentions it. One rule exists: in a file \
          whose path contains a workflows directory, if the text contains `pull_request:` \
          anywhere, then any `runs-on:` value containing `macos` or `gpu` is a finding \
          (ci_runner_economics/sku_allocator.rs::scan_workflow_runners). `is_pr_trigger` is \
          computed over the whole hunk rather than per job, so a costly job reached only by a \
          different event is reported, and a matrix expression or a variable-valued runner is not \
          read at all. Two substrings are the entire definition of expensive. `is_cost_optimal` is \
          the emptiness of that list (ci_runner_economics/mod.rs::evaluate_runner_economics), and \
          the verdict is a warning, so nothing blocks.",
    blocked_on: None,
};

pub const SCHEMA_COMPAT_STATUS: GateFidelity = GateFidelity {
    gate_id: "schema_compat_status",
    aspiration: "A schema change stays compatible with the versions running beside it: nothing a \
                 live reader still needs is removed, and no constraint is tightened under a writer \
                 that does not yet satisfy it.",
    reference: "expand and contract (parallel change) migrations; breaking-change detection \
                against a recorded baseline",
    fidelity: Fidelity::Heuristic,
    gap: "Reads no schema and compares against no baseline, so compatibility is never computed. \
          The rule runs only where `has_migration` is true -- a changed path containing the word \
          migration, or ending in the SQL extension -- so a schema change in a file named any \
          other way is never scanned (pre_merge_guard/scanner.rs::scan_for_breaking_changes). \
          Within that, `destructive_patterns` holds three case-insensitive shapes: a dropped \
          column, a dropped table, and an altered column line mentioning a not-null constraint. A \
          rename, a narrowed type, a removed default, a new unique index and a new foreign key are \
          all destructive under load and match none of them. The scan returns on the first hit, so \
          the rest of the diff is unread. Nothing about a cell node is observed: none is \
          contacted, no deployed schema version is known, and the phrase in the published label \
          describes nothing the code does. Every other input is `GateStatus::Passed`, and a hit is \
          `GateStatus::Warning`, so nothing blocks \
          (pre_merge_guard/evaluator.rs::evaluate_pre_merge_gates).",
    blocked_on: None,
};
