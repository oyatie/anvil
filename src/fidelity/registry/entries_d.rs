//! One bin of `AUDITED_GATES` entries.
//!
//! The split is by size, not by subject: `registry.rs` held every entry and
//! was 1166 lines against a 300-line budget. The order of the corpus lives in
//! `registry::AUDITED_GATES`, so nothing here is meant to be read as a grouping.

use crate::fidelity::{Fidelity, GateFidelity};

pub const TEST_SUITE_STATUS: GateFidelity = GateFidelity {
    gate_id: "test_suite_status",
    aspiration: "Run the repository's own test suite against the pull request head and refuse a \
                 pull request whose tests fail.",
    reference: "GitHub required status checks; `cargo test`, `cargo nextest run`",
    fidelity: Fidelity::Partial,
    gap: "It runs the suite now, and only now: for a Cargo tree the gate ran a type-check, which \
          builds no test binary and executes no test, so a tree in which every test was red \
          passed the gate named Automated Test Suite. What runs is the repository's own suite -- \
          `cargo test --no-run` then `--no-fail-fast` (queue_healer.rs::run_cargo_test_gate), or `npm test` \
          where a `package.json` names a test script (queue_healer.rs::run_local_test_gate). Three ceilings \
          remain. It is Anvil's own run on one host against one toolchain, not the project's CI \
          matrix, so a platform-specific failure is invisible to it. It knows exactly two \
          ecosystems, and a Go, Python or Gradle repository offers it nothing. And a Cargo \
          repository with no tests at all exits zero and is reported as a pass, because cargo \
          has no distinct signal for an empty run. The build is a separate invocation because \
          cargo exits 101 for a compile error and libtest exits 101 for a failing test: a tree \
          that did not build ran no test, so it is `Errored` and not an accusation \
          (queue_healer.rs::run_cargo_test_gate). The child environment is CLEARED and rebuilt from `BUILD_INHERITED` \
          (exec/build_env.rs::BUILD_INHERITED), so neither a shared cargo target directory nor \
          the daemon's webhook secret can reach a contributor's tests; a cargo config file inside the \
          tenant tree can still redirect the target directory and is not defended against. Two further \
          ceilings. The `ExecClass::Build` bound of 1800s was sized for a type-check and now \
          has to cover a build and a run, and `heal_ejected_pr` calls `run_local_test_gate` twice \
          (queue_healer.rs::heal_in_worktree), so one heal can spend an hour before reporting that it \
          measured nothing. And the run executes every `#[test]` in a contributor's branch, which a \
          type-check never did; what it may read is bounded by the allowlist above and by \
          nothing else, because this is not a sandbox. The cost is a cold build per \
          pull request, in an ephemeral worktree with no shared target directory.",
    blocked_on: None,
};

pub const RUST_SKILLS_STATUS: GateFidelity = GateFidelity {
    gate_id: "rust_skills_status",
    aspiration: "Enforce the project's Rust idiom and safety rules over changed code at the \
                 fidelity of a linter that parses the language.",
    reference: "`cargo clippy -- -D warnings`; clippy's restriction group; the upstream \
                rust-skills corpus",
    fidelity: Fidelity::Heuristic,
    gap: "No clippy run, no rustc lint, no parser: seven regexes over the lines a diff adds \
          (rust_language_policy/engine.rs::ERR_NO_UNWRAP_PROD and rust_language_policy/engine.rs::UNSAFE_SAFETY_COMMENT), four of which can block. `err-no-unwrap-prod` is a text match for \
          `.unwrap()` on any line whose path does not contain the word test, so it sees neither \
          the receiver's type nor whether the call is reachable; `unsafe-safety-comment` asks \
          only whether the preceding line carried a marker. The upstream corpus the gate is \
          named after is not fetched, parsed or consulted anywhere in this binary, and its size \
          was published on every pull request as a literal -- including on pull requests \
          changing no Rust at all, where the same literal was published beside the sentence \
          that the check had passed. `rules_evaluated_count` is now the length of the ruleset \
          that actually ran (rust_language_policy/mod.rs::evaluate_rust_quality), and zero when nothing was scanned (rust_language_policy/mod.rs::evaluate_rust_quality). Scope \
          is added lines, so Rust this pull request does not touch is never examined and a \
          clean verdict here is not a statement about the repository.",
    blocked_on: None,
};

pub const ATTESTATION_STATUS: GateFidelity = GateFidelity {
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
          (attestation_guard.rs::ANVIL_RECEIPTS_DIR and attestation_guard.rs::VERDICT_PENDING-211). A hash-chained receipt log was considered and \
          rejected rather than shipped: the chain would be unkeyed, so recomputing it after \
          an edit is the write path rather than an attack on it, and receipts are per-pull-\
          request files overwritten in place inside a per-run clone, so there is no \
          append-only log to chain in the first place. The receipt was also swept onto the \
          pull request by the certification pipeline's own staging sweep; all four staging \
          sites now share `stage_excluding_receipts` (git_manager/mod.rs::ANVIL_OWNED_PATHS).",
    blocked_on: Some(
        "a signing identity and a log to publish to -- a key or an OIDC issuer plus Fulcio, \
         and a transparency log; none is reachable from here",
    ),
};

pub const CEDAR_STATUS: GateFidelity = GateFidelity {
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
          an `ends_with` test on the extension a parser can read (cedar_guard.rs::policy_files_in_scope). The verdict \
          used to be a literal on all three exits, including the one reached after the model this \
          gate paid had answered non-compliant; `verify` is now total over what the checker \
          returned, and the model is deleted (cedar_guard.rs::verify). Where the checker is not \
          installed, and where it rejects Anvil own invocation rather than the policy, the gate \
          measures nothing and says so: `interpret_cedar_outcome` keeps those two exit codes apart \
          so a flag renamed here cannot read as a policy defect there (cedar_guard.rs::interpret_cedar_outcome).",
    blocked_on: Some(
        "a Cedar schema, and an entity store to decide a request against; this repository has \
         neither, and both validate and symcc require the schema",
    ),
};

pub const SCHEMA_EVOLUTION_STATUS: GateFidelity = GateFidelity {
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
          beginning `openapi` or `swagger` (compatibility_checker.rs::MAX_FIELD_NUMBER), and every other file \
          is skipped before a line of it is read (schema_evolution/mod.rs::evaluate_schema_evolution). Inside that \
          scope it covers two of buf's fifteen WIRE rules, \
          `FIELD_NO_DELETE_UNLESS_NUMBER_RESERVED` (compatibility_checker.rs::CompatibilityChecker) and \
          `MESSAGE_SAME_REQUIRED_FIELDS`, plus reuse of a deleted field number, and exactly one \
          of oasdiff's 219 checks -- `api-path-removed`, read off removed path keys rather than \
          off two documents (compatibility_checker.rs::CompatibilityChecker). A file the pull request creates -- one whose \
          diff section carries `new file mode` -- is skipped outright \
          (schema_evolution/mod.rs::evaluate_schema_evolution): it has no previous revision, and alleging a \
          break against no baseline is the same defect narrowed to one file type. A narrowed \
          response type, a newly required request property, a removed operation under a \
          surviving path, and any schema change outside a diff hunk are all invisible to it. A deleted enum value it does \
          report, which is the right verdict under buf's enum-deletion rule and the wrong noun: \
          it is published as a field. No tracked file in this repository is a protobuf \
          definition, so `NO_SCHEMA_IN_SCOPE` rather than a pass is its ordinary verdict on a \
          Rust change; the one OpenAPI description here does put it in scope, and reverting the \
          commit that published this repository's health endpoint is caught.",
    blocked_on: Some("a descriptor set or registry baseline; one diff is not a published schema"),
};

pub const ZERO_DAY_STATUS: GateFidelity = GateFidelity {
    gate_id: "zero_day_status",
    aspiration: "Detect upstream zero-day advisories against the workspace lockfiles and open the \
                 patch that closes them.",
    reference: "RustSec advisory-db; Dependabot security updates; Renovate",
    fidelity: Fidelity::Aspirational,
    gap: "Reads no advisory feed and writes no patch. The evaluation matched an empty advisory list \
          against the pull request diff, never against a lockfile, so every pull request was \
          certified clean; nothing in the module edits a manifest or opens a pull request. It now \
          publishes `NO_PATCH_SYNTHESIS` instead (zero_day_patcher/mod.rs::GATE_ID). Advisory detection \
          against the locked dependency graph moved to gate 6, which is real.",
    blocked_on: Some(
        "a manifest writer and a bot identity with write access; detection alone is \
                      already covered by gate 6",
    ),
};

pub const FEATURE_FLAG_STATUS: GateFidelity = GateFidelity {
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
          (feature_flag_ratchet/mod.rs::scan_flag_references), which is what ld-find-code-refs does. It is a proxy \
          in both directions. The call names are a fixed list -- `is_feature_enabled`, \
          `useFeatureFlag` and two more -- so a wrapper spelled any other way is invisible, and \
          a key passed as a variable, a constant or an enum is invisible to any text scan; \
          equally, a map lookup spelled the same way is counted as a toggle. \
          Whether a key it finds is stale is answered from a ledger the repository under \
          review may keep, `LEDGER_PATHS` (feature_flag_ratchet/mod.rs::GATE_ID), matched by `ledger_records_stale`, \
          which asks whether the key appears between backticks on a line \
          (feature_flag_ratchet/mod.rs::evaluate_feature_flags). That ledger is \
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
};
