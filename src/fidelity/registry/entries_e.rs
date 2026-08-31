//! One bin of `AUDITED_GATES` entries.
//!
//! The split is by size, not by subject: `registry.rs` held every entry and
//! was 1166 lines against a 300-line budget. The order of the corpus lives in
//! `registry::AUDITED_GATES`, so nothing here is meant to be read as a grouping.

use crate::fidelity::{Fidelity, GateFidelity};

pub const LOCAL_PROBE_STATUS: GateFidelity = GateFidelity {
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
          pipeline already holds, by `commit_subjects` (git_manager/mod.rs::commit_subjects), and judged \
          against `CONVENTIONAL_HEADER` (harness/judgement.rs::CONVENTIONAL_HEADER) with commitlint's default type \
          list plus this repository's own promote type -- type-enum is configuration, not \
          specification, and hardcoding the default made the check red on the convention the \
          project follows. Two \
          gaps remain there: only the subject line is read, so a breaking-change footer and \
          a body are not checked, and none of commitlint's other default rules -- length, \
          case, trailing stop -- is enforced. Subjects git generates rather than the author \
          writes are skipped -- `GENERATED_SUBJECT_PREFIXES` (harness/judgement.rs::CONVENTIONAL_HEADER), as commitlint's own defaultIgnores skip \
          them; a pull request made entirely of those is reported unmeasured rather than \
          clean. \
          The credential half delegates to `PreMergeScanner::scan_for_secrets` \
          (fast_validator.rs::scan_staged_diff), which matches whole credentials on added lines only. It \
          used to be four bare vendor prefixes tested against the whole diff, so a change \
          that DELETED a leaked key was refused for containing one and any change touching \
          this repository's own AWS-key regex blocked itself. Six regexes is still not a \
          secret scanner: no entropy check, no bare token without a recognised shape, and \
          most vendors' formats pass it. \
          `latency_ms` is now this call's own elapsed time (local_inner_loop/mod.rs::evaluate_local_probe) \
          rather than a constant; it times the gate, and says nothing about the pull request \
          or about any developer's machine.",
    blocked_on: Some(
        "nothing external -- the remaining commitlint rules and a real secret scanner are \
         unwritten, not blocked",
    ),
};

pub const CHAOS_INJECTION_STATUS: GateFidelity = GateFidelity {
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
          `.await.unwrap()` once whitespace is removed (chaos_injector/mod.rs::ChaosFaultInjector). That is \
          the property `clippy::unwrap_used` checks, which upstream files under the opt-in \
          restriction group rather than correctness -- so a hit is a Warning, not a refused \
          merge. It blocked once, and was red on ten lines of its own diff with no true \
          positive among them. It is text, not syntax, but only over `code_only` \
          (chaos_injector/mod.rs::ChaosFaultInjector), which drops a comment and empties a string literal, so \
          prose about the property is no longer counted as the property. That is one line at \
          a time with no memory of the last, so the continuation line of a multi-line string \
          literal -- this sentence, for one -- carries no opening quote and is still counted; \
          an unwrap split across lines is still invisible, and an expect on the same await is \
          not matched at all. Only added lines are read, as `code_line` \
          (chaos_injector/mod.rs::ChaosFaultInjector), so an unwrap this change leaves untouched is invisible, \
          and a line in a test module is indistinguishable from one in production code -- which \
          is the other reason this warns rather than blocks. A diff \
          with no such line is reported unmeasured, not resilient: nothing was made to fail, so \
          nothing survived failing.",
    blocked_on: Some("a running deployment a fault injector can act on"),
};

pub const ADR_STATUS: GateFidelity = GateFidelity {
    gate_id: "adr_status",
    aspiration: "Bind every architectural change to a decision record, and hold each record to the \
                 field schema its repository requires.",
    reference: "Nygard's ADR format; MADR 4.0; adr-tools; Structured MADR's JSON-Schema CI action",
    fidelity: Fidelity::Heuristic,
    gap: "Presence of a key, not conformance of a decision. The five field names were a Rust literal \
          matched word-by-word against the whole pull request diff, so achieves, origin, rule and \
          ensure were satisfied by ordinary English in any file the change touched, and only \
          overturn-when was rare enough to ever go red. The list is now read from the repository \
          under review, from one of `SCHEMA_PATHS` (adr_drift_ratchet.rs::declared_schema); a repository \
          declaring none reports `GateStatus::NotMeasured` (adr_drift_ratchet.rs::evaluate_adr_parity); and a field is a key before a colon rather than a word, \
          which is what `declared_key` (adr_drift_ratchet.rs::declared_key) decides -- it strips heading, \
          list and bold marks and compares alphanumerics only, so Overturn-When: is a field and \
          the sentence this rule achieves parity is not. What no part of that reads is the \
          decision. Whether the rule line states a rule, whether the change under review obeys it, \
          and whether the overturn-when condition has already occurred are all outside what a key \
          scan can see; this is the presence lint Structured MADR is, not the fitness function \
          Ford and Parsons describe, and no tool in the survey derives one from an ADR \
          mechanically. The record is read off disk when it is there and off the hunks via \
          `added_lines_for` when it is not (adr_drift_ratchet.rs::evaluate_adr_parity), and only when the read \
          failed with `NotFound`, so an ADR that exists but is untouched by this pull request \
          is never checked at all and a record this diff deletes is skipped rather than charged \
          five missing fields. A change arriving with no \
          record fills `architectural_changes_without_adr` (adr_drift_ratchet.rs::evaluate_adr_parity) and is not \
          charged: the predicate is a filename guess -- lib.rs, ports, adapters -- that this \
          repository's own history trips without any decision going unrecorded, and the branch it \
          replaces published an auto-scaffolded verdict naming a file nothing wrote.",
    blocked_on: None,
};

pub const COMPLIANCE_STATUS: GateFidelity = GateFidelity {
    gate_id: "compliance_status",
    aspiration: "Evaluate a change against the statutes in force for it, across jurisdictions, on the \
                 date it is reviewed.",
    reference: "Google Cloud Sensitive Data Protection infoTypes; Microsoft Presidio; Semgrep registry rulesets",
    fidelity: Fidelity::Heuristic,
    gap: "A regex scan of added lines against five rules, presented as an engine spanning five \
          jurisdictions. The evaluation date is now `chrono::Utc::now()` \
          (compliance_guard/mod.rs::evaluate_compliance) rather than a literal that had stopped moving; \
          `read_rule_pack` is called on the tree under review on every run \
          (compliance_guard/mod.rs::evaluate_compliance_at) rather than by nothing; `statutes_evaluated` is built from \
          the rules that ran (compliance_guard/mod.rs::evaluate_compliance_at) rather than from a list advertising \
          seventeen; and `r.pattern_regex.is_some()` now keeps a rule the engine cannot \
          evaluate out of the count published beside the verdict \
          (compliance_guard/upstream_sync.rs::enforceable_rules). The pack is returned by `load_rule_pack` \
          (compliance_guard/upstream_sync.rs::load_rule_pack) rather than written into shared state, so one \
          repository's rules do not judge the next, and a pack rule claiming a rule id already \
          enforced is rejected into `pack.rejected` (compliance_guard/upstream_sync.rs::load_rule_pack) \
          rather than \
          replacing the statute that would have judged the change adding it. A match can be \
          waived by a line naming the rule, `SUPPRESSION_MARKER` \
          (compliance_guard/engine.rs::SUPPRESSION_MARKER) -- the escape hatch every oracle here has and without \
          which a repository cannot carry a test PAN in a fixture -- and each waiver is counted \
          into the published sentence rather than being silent. What remains is still pattern \
          matching. \
          Sensitive Data Protection reaches a graded likelihood by combining a pattern with a \
          checksum and surrounding context, and Presidio routes every regex hit through a \
          validator that can zero the score; neither step exists here, so \
          `4[0-9]{12}(?:[0-9]{3})?` (compliance_guard/upstream_sync.rs::build_dynamic_living_baseline) accepts any \
          sixteen-digit number opening with a four as a card number, with no Luhn check and no \
          context word. The ePHI rule is three literal column names, \
          `patient_icd10|medical_record_number|clinical_diagnosis` \
          (compliance_guard/upstream_sync.rs::build_dynamic_living_baseline), which is a spelling list rather than a \
          detector: a schema abbreviating the medical record number is invisible to it. Scope is \
          added lines only, so a statute violated by code this pull request leaves alone is never \
          seen, and the pack is plain files with no version, no signature and none of the \
          staleness bound Grype imposes on its database.",
    blocked_on: None,
};

pub const CROSS_SERVICE_STATUS: GateFidelity = GateFidelity {
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
          (cross_service_impact/contract_scan.rs::removed_required_fields), read by `required_names` \
          (cross_service_impact/contract_scan.rs::required_names), which reads no column, so re-indenting a \
          block is not a break. That is a text scan of hunks, not the model comparison the oracles \
          perform: buf compiles both sides to a descriptor set and oasdiff parses both to an \
          OpenAPI model, so both see a reference resolved, a schema moved between files, and a \
          type narrowed, and none of those is visible here. Request and response direction are not \
          told apart, so a relaxed request schema is reported alongside a broken response one. \
          Scope is whatever `is_wire_contract` (cross_service_impact/contract_scan.rs::is_wire_contract) admits, \
          a filename and extension guess -- narrowed to YAML, because `.proto` and `.json` were \
          admitted by a parser that reads neither spelling, so a JSON Schema losing a required \
          field produced no finding under a sentence saying the file had been read. And the consumer set is not derived at all: \
          `NO_CONSUMER_REGISTRY` (cross_service_impact/contract_scan.rs::NO_CONSUMER_REGISTRY) is the sentence the \
          finding carries instead, because Pact learns consumers from published pacts, Confluent \
          from a subject's registered versions and buf from a stored image, and none of those is \
          configured here.",
    blocked_on: Some("a Pact broker, schema registry or module graph naming the consumers"),
};

pub const SECURITY_SCAN_STATUS: GateFidelity = GateFidelity {
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
          working one. Seven regexes over added lines in SECRET_RULES (harness/judgement.rs::PLACEHOLDER_WORDS). Four \
          carry a provider-issued prefix and are conclusive on their own, so they run with \
          min_entropy: 0.0 and no filtering (harness/judgement.rs::PLACEHOLDER_WORDS). Two had no anchor and were the \
          two that produced this gate's false merge blocks; each now captures the candidate \
          rather than the line and filters it -- `sk-[A-Za-z0-9]{24,}` at min_entropy: 3.5 \
          (harness/judgement.rs::PLACEHOLDER_WORDS) and a quoted value of eight or more characters after the word \
          password at min_entropy: 3.0 (harness/judgement.rs::PLACEHOLDER_WORDS). shannon_entropy is a real \
          logarithm (harness/judgement.rs::PLACEHOLDER_WORDS) and the file had none before, but it is the last filter \
          and not the decision: is_credential_shaped rejects a candidate made only of \
          is_ascii_alphabetic characters and identifier punctuation before entropy is ever \
          consulted (harness/judgement.rs::is_credential_shaped), because entropy alone cannot reject a \
          kebab-case identifier. The reference config additionally anchors its own key rule on \
          the literal marker T3BlbkFJ that every issued key of that vendor embeds; this rule \
          does not, so a long enough base62 run behind the prefix is a finding whoever issued \
          it. Nothing here reads git history, so a credential added by an earlier commit and \
          merely retained by this one is outside the scan.",
    blocked_on: Some("network egress to the issuing providers, for verification"),
};

pub const CANARY_STATUS: GateFidelity = GateFidelity {
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
          (canary_rollout/mod.rs::evaluate_without_metrics_source). The circuit breaker survives as the seam a real \
          query plugs into, and it is honest but narrower than the name: it compares \
          burn_rate_5m and p99_latency_ms against caller-supplied bounds \
          (canary_rollout/circuit_breaker.rs::evaluate_metrics), which is a single-window rule. The SRE \
          Workbook walks that shape through as its Approach 4 and rejects it for recall, \
          recommending a long window paired with a short one and a threshold expressed as a \
          factor of the error budget rather than as a bare ratio; neither the pairing nor an \
          SLO target exists here, so what survives is not dimensionally a burn rate.",
    blocked_on: Some(
        "a canary deployment and a reachable Prometheus or OpenTelemetry endpoint; this crate \
         has no HTTP client to reach one with",
    ),
};
