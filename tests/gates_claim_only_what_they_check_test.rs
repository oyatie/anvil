//! Three gates published a claim their mechanism did not support.
//!
//! # The defects, restated from source
//!
//! ## Gate 15 `adr_status` -- "Mandatory achieves, origin, rule, ensure, overturn_when"
//!
//! Two of them. The five field regexes were matched against
//! `diff_ctx.diff_content` -- the *whole pull request diff* -- rather than
//! against the ADR being validated, so `\bensure\b`, `\brule\b`, `\borigin\b`
//! and `\bachieves\b` were satisfied by any English sentence anywhere in the
//! change. Four of the five mandatory fields were therefore satisfied by prose
//! in an unrelated Rust file, and the one that was not, `overturn[-_ ]when`, is
//! the only reason the gate ever went red.
//!
//! And an architectural change arriving with no ADR was recorded as
//! `AUTO-SCAFFOLDED (Draft ADR generated ...)` naming
//! `docs/decisions/ADR-{n:04}-pr-{n}.md`. Nothing writes that file. There is no
//! `fs::write`, no `git add`, no PR comment; the path is a `format!` of the PR
//! number. The remediation was a claim about a file that does not exist.
//!
//! The oracles agree on both halves. Nygard's original format, MADR 4.0 and
//! `adr-tools` all treat the *record* as the unit -- `adr-tools` even writes
//! `doc/adr/NNNN-slug.md` to disk before opening `$EDITOR`, which is what
//! "scaffolded" means. The credible CI-side tools validate the ADR file itself:
//! Structured MADR runs a JSON Schema over frontmatter plus body sections;
//! adrkit's `adr lint` parses frontmatter. And on scaffolding they are
//! unanimous in the other direction: adrkit's CI action explicitly never
//! creates or commits an ADR, ADR Guard and Structured MADR fail or comment
//! only. CI reports; it does not author the decision.
//!
//! The five field names are also not universal. Nygard's are Title/Context/
//! Decision/Status/Consequences; MADR 4.0's required body sections are Context
//! and Problem Statement / Considered Options / Decision Outcome, and every
//! frontmatter key is optional. `achieves, origin, rule, ensure, overturn_when`
//! is one repository's house convention. A tool that hardcodes it manufactures
//! an accusation against every tenant using plain MADR -- the symmetric
//! violation of I1 -- and this repository's own ADR-0006 already settles where
//! such a rule belongs ("a rule Anvil cannot state generically belongs in the
//! tenant repository, not in the tool"). So the field list is read from the
//! repository, and a repository that declares none gets `NotMeasured`, not a
//! pass and not an accusation.
//!
//! ## Gate 3 `compliance_status` -- "Dynamic temporal KR PIPA, FSS & HIPAA engine"
//!
//! `let current_date = "2026-08-19"; // Canonical platform time`. The window a
//! "dynamic temporal" engine exists to advance was a literal. Every
//! `effective_date` and `sunset_date` on every rule was compared against a date
//! that stopped moving, so the temporal machinery -- which is otherwise real
//! and correct -- could never change its answer.
//!
//! `sync_upstream_rules` had zero callers: the hot-reload path that makes the
//! registry "dynamic" was reachable from nothing. And `active_rules_count`, the
//! number published in the pass sentence, counted a rule with
//! `pattern_regex: None`, which the engine cannot evaluate at all.
//!
//! ## Gate 33 `cross_service_status` -- "Wire contract compatibility proven across microservices"
//!
//! The whole predicate was a path containing `api/` or `proto/` and the diff
//! containing the literal `-   required:` -- a minus sign and exactly three
//! spaces. Every `required:` in this repository's own `openapi/openapi.yaml`
//! sits at eight or fourteen spaces, and no line anywhere in the tree is
//! indented by three. On a hit it emitted `service_name: "oyatie-backend"` and
//! `impacted_consumer: "oyatie-console"` -- two string literals, identical on
//! every pull request in every repository.
//!
//! `buf breaking` compiles both sides into a descriptor set; `oasdiff` parses
//! both specs into an OpenAPI model. Neither diffs text, so neither can be
//! defeated by whitespace. And on the consumer set every oracle behaves the
//! same way: Pact knows consumers because they published contracts to a broker,
//! Confluent Schema Registry checks a subject against its own registered
//! version history, buf compares against a stored image. None of them guesses a
//! downstream list. With no broker, registry or module graph configured here,
//! naming one is the thing no oracle does.

use anvil::adr_drift_ratchet::AdrDriftRatchet;
use anvil::compliance_guard::ComplianceGuard;
use anvil::cross_service_impact::CrossServiceImpactEngine;
use anvil::git_manager::PrDiffContext;
use anvil::pre_merge_guard::report::GateStatus;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A diff over one or more files, in the shape `git diff` actually emits.
fn diff_of(files: &[(&str, &str)]) -> PrDiffContext {
    let mut diff = String::new();
    for (path, body) in files {
        diff.push_str(&format!(
            "diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1,1 +1,9 @@\n{body}\n"
        ));
    }
    PrDiffContext {
        repo: "oyatie/test-repo".to_string(),
        pr_number: 4242,
        base_branch: "main".to_string(),
        base_sha: "base".to_string(),
        head_sha: "head".to_string(),
        is_incremental: false,
        previous_head_sha: None,
        diff_content: diff,
        changed_files: files.iter().map(|(p, _)| p.to_string()).collect(),
        repo_working_dir: anvil::git_manager::SubjectRoot::asserted(
            PathBuf::from("."),
            anvil::git_manager::Uncloned::TestFixture,
        ),
    }
}

fn write(root: &Path, rel: &str, body: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().expect("a parent")).expect("mkdir");
    fs::write(&p, body).expect("write");
}

/// A working tree declaring the five-field house schema this gate's label names.
fn repo_declaring_the_five_fields() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        dir.path(),
        "docs/decisions/adr-schema.json",
        r#"["achieves", "origin", "rule", "ensure", "overturn_when"]"#,
    );
    dir
}

const COMPLETE_ADR: &str = "\
# ADR-0042: Shared Receipt Store

## Schema
Achieves: One authority record per receipt
Origin: console-yw0
Rule: Bind writers via ReceiptOwner
Ensure: Zero stale shared permissions
Overturn-When: A single-writer DB role migration lands
";

fn as_added_lines(body: &str) -> String {
    body.lines()
        .map(|l| format!("+{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

// ===========================================================================
// Gate 15 -- adr_status
// ===========================================================================

/// The defect verbatim: four of the five mandatory fields are satisfied by
/// ordinary English in a file that is not the ADR.
///
/// The ADR carries `Achieves:` and nothing else. The Rust file in the same pull
/// request contains the words origin, rule, ensure and overturn-when in prose.
/// Under the old predicate -- five regexes over `diff_ctx.diff_content` -- this
/// diff is fully compliant. It must not be.
#[test]
fn a_field_is_missing_when_the_adr_lacks_it_however_the_rest_of_the_diff_reads() {
    let repo = repo_declaring_the_five_fields();
    let adr = "docs/decisions/ADR-0042-receipts.md";
    write(
        repo.path(),
        adr,
        "# ADR-0042: Shared Receipt Store\n\nAchieves: One authority record\n",
    );
    write(repo.path(), "src/receipts.rs", "fn f() {}\n");

    let ctx = diff_of(&[
        (adr, "+Achieves: One authority record"),
        (
            "src/receipts.rs",
            "+origin: receipt-store\n\
             +rule: bind writers\n\
             +ensure: no stale grants\n\
             +overturn_when: the migration lands",
        ),
    ]);

    let report = AdrDriftRatchet::new()
        .evaluate_adr_parity(repo.path(), &ctx)
        .expect("the ratchet reads the diff");

    assert!(
        !report.is_compliant,
        "prose in an unrelated file satisfied the ADR schema: {:?}",
        report.summary
    );
    for field in ["origin", "rule", "ensure", "overturn_when"] {
        assert!(
            report.violations.iter().any(|v| v.contains(field)),
            "expected a violation naming `{field}`, got {:?}",
            report.violations
        );
    }
    assert!(
        matches!(report.status, GateStatus::Failed(_)),
        "expected Failed, got {:?}",
        report.status
    );
}

/// The clean case. Every declared field is present *as a field* in the record.
#[test]
fn an_adr_carrying_every_declared_field_passes() {
    let repo = repo_declaring_the_five_fields();
    let adr = "docs/decisions/ADR-0042-receipts.md";
    write(repo.path(), adr, COMPLETE_ADR);

    let ctx = diff_of(&[(adr, &as_added_lines(COMPLETE_ADR))]);
    let report = AdrDriftRatchet::new()
        .evaluate_adr_parity(repo.path(), &ctx)
        .expect("the ratchet reads the diff");

    assert!(
        report.is_compliant,
        "a complete ADR was accused: {:?}",
        report.violations
    );
    assert_eq!(report.adrs_evaluated, 1);
    assert!(matches!(report.status, GateStatus::Passed));
}

/// Teeth: the check is for a *field*, not for a word.
///
/// This ADR uses all five words in fluent prose and declares none of them. A
/// `\bword\b` regex -- the old mechanism, merely re-scoped to the ADR file --
/// reports this as fully compliant. Scoping alone is not the fix.
#[test]
fn a_word_in_prose_is_not_a_field() {
    let repo = repo_declaring_the_five_fields();
    let adr = "docs/decisions/ADR-0043-prose.md";
    write(
        repo.path(),
        adr,
        "# ADR-0043: Prose\n\n\
         This rule achieves a stable origin for receipts. We ensure the\n\
         invariant and would overturn when a migration lands.\n",
    );

    let ctx = diff_of(&[(adr, "+This rule achieves a stable origin")]);
    let report = AdrDriftRatchet::new()
        .evaluate_adr_parity(repo.path(), &ctx)
        .expect("the ratchet reads the diff");

    assert!(
        !report.is_compliant,
        "five words in a sentence were accepted as five declared fields"
    );
    assert_eq!(
        report.violations.len(),
        5,
        "every field is undeclared here: {:?}",
        report.violations
    );
}

/// The same teeth from the other side. Dropping the colon requirement -- so any
/// line becomes a key -- is a mutant the prose case above does not kill, because
/// a whole sentence normalises to a whole sentence and matches no field. A bare
/// word on its own line does match, so this is what forbids it.
#[test]
fn a_bare_word_on_its_own_line_is_not_a_field() {
    let repo = repo_declaring_the_five_fields();
    let adr = "docs/decisions/ADR-0046-bare.md";
    write(
        repo.path(),
        adr,
        "# ADR-0046\n\nachieves\norigin\nrule\nensure\noverturn_when\n",
    );

    let ctx = diff_of(&[(adr, "+achieves")]);
    let report = AdrDriftRatchet::new()
        .evaluate_adr_parity(repo.path(), &ctx)
        .expect("the ratchet reads the diff");

    assert_eq!(
        report.violations.len(),
        5,
        "five words on five lines were accepted as five declared fields: {:?}",
        report.violations
    );
}

/// A heading *is* a declaration. Nygard's sections and MADR's required body
/// sections are headings, and Structured MADR's action checks for exactly that,
/// so refusing them would accuse every record written in either format.
#[test]
fn a_markdown_heading_declares_the_field_it_names() {
    let repo = repo_declaring_the_five_fields();
    let adr = "docs/adr/0047-headings.md";
    write(
        repo.path(),
        adr,
        "# ADR-0047\n\n## Achieves\na\n## Origin\nb\n## Rule\nc\n## Ensure\nd\n## Overturn-When\ne\n",
    );

    let ctx = diff_of(&[(adr, "+## Achieves")]);
    let report = AdrDriftRatchet::new()
        .evaluate_adr_parity(repo.path(), &ctx)
        .expect("the ratchet reads the diff");

    assert!(
        report.is_compliant,
        "a record declaring its fields as headings was accused: {:?}",
        report.violations
    );
}

/// Field spellings the record may legitimately use. `Overturn-When:` is this
/// repository's own spelling in `docs/adr/0001`; a Markdown heading and a bold
/// run are both ordinary ways to write a field in a decision record.
#[test]
fn a_field_is_recognised_under_the_spellings_real_adrs_use() {
    let repo = repo_declaring_the_five_fields();
    let adr = "docs/adr/0044-spellings.md";
    write(
        repo.path(),
        adr,
        "# ADR-0044\n\
         ### achieves: a\n\
         **Origin**: b\n\
         - Rule: c\n\
         ENSURE: d\n\
         Overturn-When: e\n",
    );

    let ctx = diff_of(&[(adr, "+### achieves: a")]);
    let report = AdrDriftRatchet::new()
        .evaluate_adr_parity(repo.path(), &ctx)
        .expect("the ratchet reads the diff");

    assert!(
        report.is_compliant,
        "a legitimately spelled field was read as missing: {:?}",
        report.violations
    );
}

/// A schema check with no schema measured nothing.
///
/// The five names are one repository's house convention, not a property of
/// decision records. Against a tenant that uses plain MADR, hardcoding them
/// manufactures five accusations per ADR. Absent evidence is not a pass and not
/// an accusation: it is `NotMeasured`.
#[test]
fn without_a_declared_field_schema_the_gate_measures_nothing() {
    let repo = tempfile::tempdir().expect("tempdir");
    let adr = "docs/decisions/ADR-0045-madr.md";
    write(
        repo.path(),
        adr,
        "# ADR-0045\n\n## Context and Problem Statement\n\n## Decision Outcome\n",
    );

    let ctx = diff_of(&[(adr, "+## Decision Outcome")]);
    let report = AdrDriftRatchet::new()
        .evaluate_adr_parity(repo.path(), &ctx)
        .expect("the ratchet reads the diff");

    match &report.status {
        GateStatus::NotMeasured { gate_id, reason } => {
            assert_eq!(gate_id, "adr_status");
            assert!(
                reason.contains("adr-schema.json"),
                "the reason must name the file that would have supplied the schema: {reason}"
            );
        }
        other => panic!("expected NotMeasured, got {other:?}"),
    }
    assert!(
        report.violations.is_empty(),
        "a repository that declared no schema was accused anyway: {:?}",
        report.violations
    );
}

/// The scaffold claim, deleted. Every path this gate names must be a path that
/// exists -- either a real changed file, or nothing at all.
#[test]
fn the_gate_names_no_file_it_did_not_write() {
    let repo = repo_declaring_the_five_fields();
    write(repo.path(), "src/lib.rs", "pub mod a;\n");

    let ctx = diff_of(&[("src/lib.rs", "+pub mod a;")]);
    let report = AdrDriftRatchet::new()
        .evaluate_adr_parity(repo.path(), &ctx)
        .expect("the ratchet reads the diff");

    let rendered = format!("{report:?}");
    assert!(
        !rendered.contains("AUTO-SCAFFOLDED") && !rendered.to_lowercase().contains("scaffold"),
        "the gate still claims to have scaffolded something: {rendered}"
    );
    assert!(
        !rendered.contains(&format!("ADR-{:04}-pr-", ctx.pr_number)),
        "the gate still names a per-PR ADR path nothing writes: {rendered}"
    );

    // The honest half of that branch survives: the architectural change is
    // still observed, and every path it reports is a path in the diff.
    for named in &report.architectural_changes_without_adr {
        assert!(
            ctx.changed_files.contains(named),
            "`{named}` is not a file this pull request changed"
        );
        assert!(
            repo.path().join(named).exists(),
            "`{named}` does not exist on disk"
        );
    }
    assert!(
        report
            .architectural_changes_without_adr
            .contains(&"src/lib.rs".to_string()),
        "the honest observation was deleted along with the fabricated one"
    );
}

/// The observation is not an accusation. `ends_with(\"lib.rs\")` is a guess
/// about spelling, and this repository's own history changes `src/lib.rs`
/// without an ADR. Recording it is right; failing on it is a fabricated
/// accusation dressed as a ratchet.
#[test]
fn an_architectural_change_without_an_adr_is_recorded_not_charged() {
    let repo = repo_declaring_the_five_fields();
    write(repo.path(), "src/lib.rs", "pub mod a;\n");
    let ctx = diff_of(&[("src/lib.rs", "+pub mod a;")]);
    let report = AdrDriftRatchet::new()
        .evaluate_adr_parity(repo.path(), &ctx)
        .expect("the ratchet reads the diff");

    assert!(report.is_compliant);
    assert!(
        matches!(report.status, GateStatus::Passed),
        "expected Passed, got {:?}",
        report.status
    );
    assert!(report.violations.is_empty());
}

/// Retiring a superseded decision is not five schema violations. `changed_files`
/// carries deletions, the record is not on disk, and the hunks add nothing, so
/// every declared field read as missing and the gate blocked any pull request
/// that removed an ADR.
#[test]
fn a_record_this_diff_deletes_is_not_charged_for_the_fields_it_no_longer_has() {
    let repo = repo_declaring_the_five_fields();
    let adr = "docs/decisions/ADR-0007-superseded.md";
    let ctx = diff_of(&[(adr, "")]);

    let report = AdrDriftRatchet::new()
        .evaluate_adr_parity(repo.path(), &ctx)
        .expect("the ratchet reads the diff");

    assert!(
        report.violations.is_empty(),
        "deleting a decision record was charged as schema drift: {:?}",
        report.violations
    );
    assert_eq!(report.adrs_evaluated, 0);
    assert!(
        matches!(report.status, GateStatus::Passed),
        "expected Passed, got {:?}",
        report.status
    );
    assert!(
        report.summary.contains(adr),
        "the record was skipped silently: {}",
        report.summary
    );
}

/// Only `NotFound` falls back to the hunks. Any other read error used to take
/// the same branch, so an unreadable record silently degraded to whatever the
/// diff happened to add and could pass.
#[test]
fn a_record_that_cannot_be_read_is_an_error_rather_than_a_silent_fallback() {
    let repo = repo_declaring_the_five_fields();
    let adr = "docs/decisions/ADR-0008-unreadable.md";
    // A directory where a file is expected: readable metadata, unreadable body,
    // and not `NotFound`.
    fs::create_dir_all(repo.path().join(adr)).expect("mkdir");
    let ctx = diff_of(&[(adr, "+Achieves: something")]);

    let err = AdrDriftRatchet::new()
        .evaluate_adr_parity(repo.path(), &ctx)
        .expect_err("an unreadable record must not degrade to the hunks");
    assert!(
        err.to_string().contains(adr),
        "the error does not name the record: {err}"
    );
}

/// A file under `docs/decisions/` that is not a decision record is not an ADR.
/// Without this the schema file accuses itself the moment it is edited.
#[test]
fn the_schema_file_is_not_itself_an_adr() {
    let repo = repo_declaring_the_five_fields();
    let ctx = diff_of(&[(
        "docs/decisions/adr-schema.json",
        "+[\"achieves\", \"origin\", \"rule\", \"ensure\", \"overturn_when\"]",
    )]);
    let report = AdrDriftRatchet::new()
        .evaluate_adr_parity(repo.path(), &ctx)
        .expect("the ratchet reads the diff");

    assert_eq!(report.adrs_evaluated, 0);
    assert!(report.violations.is_empty(), "{:?}", report.violations);
}

// ===========================================================================
// Gate 3 -- compliance_status
// ===========================================================================

const RRN_DIFF: &str = "+ const testUserRRN = \"931225-1029384\";"; // anvil-ignore: KR_PIPA_RRN_BAN

/// A minimal well-formed pack rule, so the tests below differ only in the two
/// fields they are about.
fn pack_rule(rule_id: &str, pattern: &str) -> String {
    format!(
        r#"{{
          "rule_id": "{rule_id}",
          "scope": "Global",
          "level": "InternalStandard",
          "statute_or_policy_name": "Pack Standard",
          "citation": "Pack §1",
          "temporal": {{
            "enacted_date": "2000-01-01",
            "effective_date": "2000-01-01",
            "grace_period_until": null,
            "sunset_date": null
          }},
          "official_reference_url": null,
          "title": "Banned token",
          "requirement_spec": "{pattern} may not appear.",
          "trigger_extensions": ["rs", "ts"],
          "pattern_regex": "{pattern}",
          "required_controls": ["none"],
          "severity": "CRITICAL"
        }}"#
    )
}

fn compliance_ctx(root: &Path, file: &str, added: &str) -> PrDiffContext {
    let mut ctx = diff_of(&[(file, added)]);
    ctx.repo_working_dir = anvil::git_manager::SubjectRoot::asserted(
        root.to_path_buf(),
        anvil::git_manager::Uncloned::TestFixture,
    );
    ctx
}

/// The frozen literal, gone. The window a temporal engine evaluates against is
/// the clock's, or the engine is not temporal.
#[test]
fn the_evaluation_date_comes_from_the_clock() {
    let repo = tempfile::tempdir().expect("tempdir");
    let ctx = compliance_ctx(repo.path(), "src/user.rs", "+ let x = 1;");
    let before = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let report = ComplianceGuard::new()
        .evaluate_compliance(&ctx)
        .expect("the guard reads the diff");

    // Two `Utc::now()` calls straddling UTC midnight give different dates, so
    // the assertion is membership in the window the run spanned, not equality
    // with one sample of it.
    let after = chrono::Utc::now().format("%Y-%m-%d").to_string();
    assert!(
        report.evaluation_date == before || report.evaluation_date == after,
        "the engine is evaluating against {:?}, which is neither {before} nor {after}",
        report.evaluation_date
    );
}

/// The mechanism, not the value. A literal re-valued to next Tuesday defeats
/// any fixed-needle check while leaving the engine exactly as frozen, so what
/// is forbidden is a date literal anywhere the guard decides from.
///
/// The evaluation path is two modules, named as two modules: the root and the
/// engine that walks the rules. `upstream_sync` is deliberately not among them
/// -- it carries the codified rule pack, where a statute's enactment date is a
/// fact about the statute rather than a clock the engine reads. Widening this
/// to the whole directory would accuse that data, and a gate that accuses
/// correct code gets weakened by the first author it blocks.
///
/// Commentary is stripped with `without_commentary` rather than by dropping
/// lines that open with a slash, which leaves a trailing comment on a code line
/// in the text and lets prose answer -- or trip -- a scan about code.
#[test]
fn no_date_literal_survives_in_the_compliance_guards_evaluation_path() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = format!(
        "{}\n{}",
        anvil::source_scan::paths::module_source("src/compliance_guard/mod", manifest),
        anvil::source_scan::paths::module_source("src/compliance_guard/engine", manifest),
    );
    let code_only = anvil::source_scan::without_commentary(&src);
    let re = regex::Regex::new(r#""(19|20)\d\d-\d\d-\d\d"#).expect("valid");
    assert!(
        !re.is_match(&code_only),
        "a date literal is back in the evaluation path: {:?}",
        re.find(&code_only).map(|m| m.as_str())
    );
}

/// Temporal validity, evaluated at two dates chosen by the test rather than by
/// the calendar. This is what makes the suite non-time-dependent: the clock is
/// exercised once, above, and every behavioural assertion pins an explicit day.
#[test]
fn a_rule_is_not_enforced_before_its_effective_date_and_is_after_it() {
    let repo = tempfile::tempdir().expect("tempdir");
    write(
        repo.path(),
        "policies/regulatory/future.json",
        r#"{
          "rule_id": "TEST_FUTURE_STATUTE",
          "scope": {"Jurisdiction": "KR"},
          "level": "Statute",
          "statute_or_policy_name": "Test Future Act",
          "citation": "Test Act §1",
          "temporal": {
            "enacted_date": "2029-01-01",
            "effective_date": "2030-01-01",
            "grace_period_until": null,
            "sunset_date": null
          },
          "official_reference_url": null,
          "title": "Banned token",
          "requirement_spec": "The literal FUTURE_BANNED_TOKEN may not appear.",
          "trigger_extensions": ["rs"],
          "pattern_regex": "FUTURE_BANNED_TOKEN",
          "required_controls": ["none"],
          "severity": "CRITICAL"
        }"#,
    );
    let ctx = compliance_ctx(repo.path(), "src/x.rs", "+ let t = FUTURE_BANNED_TOKEN;");
    let guard = ComplianceGuard::new();

    let before = guard
        .evaluate_compliance_at(&ctx, "2029-12-31")
        .expect("evaluates");
    assert!(
        before.is_compliant && before.violations.is_empty(),
        "a statute was enforced the day before it took effect: {:?}",
        before.violations
    );

    let after = guard
        .evaluate_compliance_at(&ctx, "2030-01-01")
        .expect("evaluates");
    assert!(
        after
            .violations
            .iter()
            .any(|v| v.rule_id == "TEST_FUTURE_STATUTE"),
        "a statute in force was not enforced: {:?}",
        after.violations
    );
}

/// `sync_upstream_rules` had zero callers. A rule pack on disk must reach the
/// engine through the ordinary evaluation path, or the "dynamic" half of the
/// label is a claim about unreachable code.
#[test]
fn a_rule_pack_on_disk_reaches_the_engine() {
    let repo = tempfile::tempdir().expect("tempdir");
    write(
        repo.path(),
        "policies/regulatory/tenant.json",
        r#"{
          "rule_id": "TENANT_PACK_RULE",
          "scope": "Global",
          "level": "InternalStandard",
          "statute_or_policy_name": "Tenant Pack Standard",
          "citation": "Tenant §1",
          "temporal": {
            "enacted_date": "2000-01-01",
            "effective_date": "2000-01-01",
            "grace_period_until": null,
            "sunset_date": null
          },
          "official_reference_url": null,
          "title": "Banned tenant token",
          "requirement_spec": "TENANT_BANNED_TOKEN may not appear.",
          "trigger_extensions": ["rs"],
          "pattern_regex": "TENANT_BANNED_TOKEN",
          "required_controls": ["none"],
          "severity": "CRITICAL"
        }"#,
    );
    let ctx = compliance_ctx(repo.path(), "src/x.rs", "+ let t = TENANT_BANNED_TOKEN;");
    let report = ComplianceGuard::new()
        .evaluate_compliance_at(&ctx, "2026-01-01")
        .expect("evaluates");

    assert_eq!(report.rules_loaded_from_pack, 1);
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.rule_id == "TENANT_PACK_RULE"),
        "the pack was loaded but never evaluated: {:?}",
        report.violations
    );
}

/// The published count is "enforceable rules". A rule with no pattern is one
/// the engine cannot evaluate, so counting it inflates the sentence.
#[test]
fn the_enforceable_count_excludes_rules_the_engine_cannot_evaluate() {
    let repo = tempfile::tempdir().expect("tempdir");
    write(
        repo.path(),
        "policies/regulatory/patternless.json",
        r#"{
          "rule_id": "PATTERNLESS_RULE",
          "scope": "Global",
          "level": "InternalStandard",
          "statute_or_policy_name": "Unevaluable Doctrine",
          "citation": "Doctrine §1",
          "temporal": {
            "enacted_date": "2000-01-01",
            "effective_date": "2000-01-01",
            "grace_period_until": null,
            "sunset_date": null
          },
          "official_reference_url": null,
          "title": "Cannot fire",
          "requirement_spec": "Has no pattern.",
          "trigger_extensions": ["rs"],
          "pattern_regex": null,
          "required_controls": ["none"],
          "severity": "CRITICAL"
        }"#,
    );
    let ctx = compliance_ctx(repo.path(), "src/x.rs", "+ let x = 1;");
    let with_pack = ComplianceGuard::new()
        .evaluate_compliance_at(&ctx, "2026-01-01")
        .expect("evaluates");

    let bare = tempfile::tempdir().expect("tempdir");
    let bare_ctx = compliance_ctx(bare.path(), "src/x.rs", "+ let x = 1;");
    let without_pack = ComplianceGuard::new()
        .evaluate_compliance_at(&bare_ctx, "2026-01-01")
        .expect("evaluates");

    assert_eq!(
        with_pack.active_rules_count, without_pack.active_rules_count,
        "a rule the engine cannot evaluate was counted as enforceable"
    );
    assert!(
        !with_pack
            .statutes_evaluated
            .iter()
            .any(|s| s.contains("Unevaluable Doctrine")),
        "an unevaluable rule's statute was published as evaluated: {:?}",
        with_pack.statutes_evaluated
    );
}

/// The label advertised roughly seventeen statutes across five jurisdictions
/// from a hardcoded `vec!`. What is published must be derived from the rules
/// that actually ran, so it cannot over-claim again.
#[test]
fn the_report_names_only_statutes_it_enforced() {
    let repo = tempfile::tempdir().expect("tempdir");
    let ctx = compliance_ctx(repo.path(), "src/x.rs", "+ let x = 1;");
    let report = ComplianceGuard::new()
        .evaluate_compliance_at(&ctx, "2026-01-01")
        .expect("evaluates");

    assert_eq!(
        report.statutes_evaluated.len(),
        report.active_rules_count,
        "one line per enforceable rule, no more: {:?}",
        report.statutes_evaluated
    );
    for absent in ["FSS", "COPPA", "DORA", "NIS2", "SOC 2", "ISO 27001", "CCPA"] {
        assert!(
            !report.statutes_evaluated.iter().any(|s| s.contains(absent)),
            "`{absent}` is published as evaluated and no rule implements it"
        );
    }
}

/// The clean case, preserved from the guard's own suite.
#[test]
fn a_diff_matching_no_rule_is_compliant() {
    let repo = tempfile::tempdir().expect("tempdir");
    let ctx = compliance_ctx(
        repo.path(),
        "src/mask.rs",
        "+ let masked = mask_token(&user.ci_token);",
    );
    let report = ComplianceGuard::new()
        .evaluate_compliance_at(&ctx, "2026-01-01")
        .expect("evaluates");
    assert!(report.is_compliant);
    assert!(report.violations.is_empty());
}

/// The baseline rules still fire, at a date the test chooses. These moved out of
/// `src/compliance_guard/mod.rs`, where they called the wall-clock entry point
/// and pointed `repo_working_dir` at `/tmp` -- so they would have loaded a rule
/// pack from a world-writable directory once the sync path acquired a caller.
#[test]
fn the_baseline_rules_still_fire_at_a_fixed_date() {
    let repo = tempfile::tempdir().expect("tempdir");
    let guard = ComplianceGuard::new();

    let rrn = compliance_ctx(repo.path(), "src/user.ts", RRN_DIFF);
    let report = guard
        .evaluate_compliance_at(&rrn, "2026-01-01")
        .expect("evaluates");
    assert!(!report.is_compliant);
    assert_eq!(report.violations[0].rule_id, "KR_PIPA_RRN_BAN");
    assert!(report.violations[0].citation.contains("개보법 §24의2"));

    let dark = compliance_ctx(
        repo.path(),
        "src/Checkout.tsx",
        "+ <input type=\"checkbox\" defaultChecked={true} name=\"marketing\" />",
    );
    // 2026-07-21 is this rule's effective date, so the date is load-bearing.
    let report = guard
        .evaluate_compliance_at(&dark, "2026-07-21")
        .expect("evaluates");
    assert_eq!(
        report.violations[0].rule_id,
        "KR_ECOM_ANTI_DARK_PATTERN_PRECHECK"
    );
    let before = guard
        .evaluate_compliance_at(&dark, "2026-07-20")
        .expect("evaluates");
    assert!(
        before.violations.is_empty(),
        "a statute was enforced the day before it took effect"
    );

    let pan = compliance_ctx(
        repo.path(),
        "src/billing.rs",
        "+ const leakedPan = \"4111111111111111\";", // anvil-ignore: GLOBAL_PCI_PLAINTEXT_PAN
    );
    let report = guard
        .evaluate_compliance_at(&pan, "2026-01-01")
        .expect("evaluates");
    assert_eq!(report.violations[0].rule_id, "GLOBAL_PCI_PLAINTEXT_PAN");
}

/// A waiver is how a repository says a match is a fixture, and it is the only
/// reason this guard can be shipped by a repository that carries the canonical
/// Visa test PAN in its own tests. Semgrep has `nosemgrep`, Presidio has
/// `validate_result`, Sensitive Data Protection has `exclusion_rules`; a
/// detector with none accuses its own author. What keeps it from being an off
/// switch is that it names one rule and is published as used.
#[test]
fn a_waiver_names_one_rule_and_is_published_as_used() {
    let repo = tempfile::tempdir().expect("tempdir");
    let guard = ComplianceGuard::new();

    let waived = compliance_ctx(
        repo.path(),
        "src/fixtures.rs",
        "+ const fixtureRRN = \"931225-1029384\"; // anvil-ignore: KR_PIPA_RRN_BAN",
    );
    let report = guard
        .evaluate_compliance_at(&waived, "2026-01-01")
        .expect("evaluates");
    assert!(report.is_compliant, "{:?}", report.violations);
    assert_eq!(report.suppressed_matches, 1);
    assert!(
        report.summary.contains("waived"),
        "a waived match was silent in the published sentence: {}",
        report.summary
    );

    // Waiving one statute does not waive another on the same line. The waiver
    // for *this* line's own PAN is the trailing comment: it is how this file is
    // shippable under the gate it tests, and the replay in
    // `gates_do_not_fire_on_anvils_own_history_test.rs` is what proves it.
    let pan = "+ const pan = \"4111111111111111\";"; // anvil-ignore: GLOBAL_PCI_PLAINTEXT_PAN
    let other = compliance_ctx(
        repo.path(),
        "src/fixtures.rs",
        &format!("{pan} // anvil-ignore: KR_PIPA_RRN_BAN"),
    );
    let report = guard
        .evaluate_compliance_at(&other, "2026-01-01")
        .expect("evaluates");
    assert_eq!(
        report.violations[0].rule_id, "GLOBAL_PCI_PLAINTEXT_PAN",
        "waiving one rule waived another: {:?}",
        report.violations
    );
    assert_eq!(report.suppressed_matches, 0);
}

/// A rule pack belongs to the repository that shipped it. It used to be written
/// into a process-global snapshot the daemon holds for its lifetime, so every
/// repository reviewed afterwards was judged by another tenant's rules.
#[test]
fn a_rule_pack_does_not_follow_the_guard_to_the_next_repository() {
    let with_pack = tempfile::tempdir().expect("tempdir");
    write(
        with_pack.path(),
        "policies/regulatory/tenant.json",
        &pack_rule("TENANT_A_ONLY", "TENANT_A_BANNED_TOKEN"),
    );
    let guard = ComplianceGuard::new();

    let a = compliance_ctx(
        with_pack.path(),
        "src/x.rs",
        "+ let t = TENANT_A_BANNED_TOKEN;",
    );
    let a_report = guard
        .evaluate_compliance_at(&a, "2026-01-01")
        .expect("evaluates");
    assert_eq!(a_report.rules_loaded_from_pack, 1);
    assert!(!a_report.is_compliant, "the pack never reached the engine");

    let no_pack = tempfile::tempdir().expect("tempdir");
    let b = compliance_ctx(
        no_pack.path(),
        "src/x.rs",
        "+ let t = TENANT_A_BANNED_TOKEN;",
    );
    let b_report = guard
        .evaluate_compliance_at(&b, "2026-01-01")
        .expect("evaluates");
    assert_eq!(
        b_report.rules_loaded_from_pack, 0,
        "another repository's rule pack was still loaded"
    );
    assert!(
        b_report.is_compliant && b_report.violations.is_empty(),
        "a repository shipping no pack was judged by another one's rules: {:?}",
        b_report.violations
    );
}

/// A pull request cannot disarm the statute judging it. The pack write was
/// `retain(id != ..); push(..)`, so a file claiming `KR_PIPA_RRN_BAN` replaced
/// the real one with a pattern matching nothing -- for that pull request, and
/// for every repository the daemon saw afterwards.
#[test]
fn a_pack_rule_cannot_claim_a_baseline_statutes_id_and_disarm_it() {
    let repo = tempfile::tempdir().expect("tempdir");
    write(
        repo.path(),
        "policies/regulatory/oops.json",
        &pack_rule("KR_PIPA_RRN_BAN", "ZZZ_NEVER_MATCHES"),
    );
    let ctx = compliance_ctx(repo.path(), "src/user.ts", RRN_DIFF);
    let report = ComplianceGuard::new()
        .evaluate_compliance_at(&ctx, "2026-01-01")
        .expect("evaluates");

    assert!(
        report
            .violations
            .iter()
            .any(|v| v.rule_id == "KR_PIPA_RRN_BAN"),
        "a rule pack file disarmed a baseline statute: {:?}",
        report.violations
    );
    assert_eq!(report.rules_loaded_from_pack, 0);
    assert!(
        report
            .pack_rules_rejected
            .iter()
            .any(|r| r.contains("oops.json") && r.contains("KR_PIPA_RRN_BAN")),
        "the rejection was silent: {:?}",
        report.pack_rules_rejected
    );
}

/// One typo'd rule file used to read exactly like shipping no pack at all:
/// `rules_loaded_from_pack: 0` and a green gate.
#[test]
fn a_rule_pack_file_that_does_not_parse_is_reported_not_swallowed() {
    let repo = tempfile::tempdir().expect("tempdir");
    write(
        repo.path(),
        "policies/regulatory/typo.json",
        "{ \"rule_id\": ",
    );
    let ctx = compliance_ctx(repo.path(), "src/x.rs", "+ let x = 1;");
    let report = ComplianceGuard::new()
        .evaluate_compliance_at(&ctx, "2026-01-01")
        .expect("evaluates");

    assert_eq!(report.rules_loaded_from_pack, 0);
    assert!(
        report
            .pack_rules_rejected
            .iter()
            .any(|r| r.contains("typo.json")),
        "an unreadable rule file was indistinguishable from no pack: {:?}",
        report.pack_rules_rejected
    );
    assert!(
        report.summary.contains("typo.json"),
        "the gate published a clean sentence over a pack it could not read: {}",
        report.summary
    );
}

// ===========================================================================
// Gate 33 -- cross_service_status
// ===========================================================================

/// The defect verbatim. This is `openapi/openapi.yaml`'s real indentation --
/// eight spaces for the key, ten for the members. `contains("-   required:")`
/// matches neither, so the gate has never been able to see a removal in the one
/// contract this repository ships.
#[test]
fn a_required_field_removed_at_the_repositorys_own_indentation_is_detected() {
    let diff = "\
diff --git a/openapi/openapi.yaml b/openapi/openapi.yaml
--- a/openapi/openapi.yaml
+++ b/openapi/openapi.yaml
@@ -30,8 +30,7 @@
             schema:
               type: object
-              required:
-                - repo
-                - pr_number
+              required:
+                - repo
";
    let mut ctx = diff_of(&[]);
    ctx.diff_content = diff.to_string();
    ctx.changed_files = vec!["openapi/openapi.yaml".to_string()];

    let report = CrossServiceImpactEngine::new()
        .evaluate_cross_service_impact(Path::new("."), &ctx)
        .expect("the engine reads the diff");

    assert!(
        !report.is_compatible,
        "a removed required field went undetected: {}",
        report.summary
    );
    let f = &report.breaking_findings[0];
    assert_eq!(f.removed_required_field, "pr_number");
    assert_eq!(f.contract_file, "openapi/openapi.yaml");
}

/// The flow-sequence spelling of the same change. `required: [repo, pr_number]`
/// is the form this repository's own contract uses in three places.
#[test]
fn a_flow_sequence_losing_a_member_is_detected() {
    let diff = "\
diff --git a/openapi/openapi.yaml b/openapi/openapi.yaml
--- a/openapi/openapi.yaml
+++ b/openapi/openapi.yaml
@@ -34,2 +34,2 @@
-              required: [repo, pr_number]
+              required: [repo]
";
    let mut ctx = diff_of(&[]);
    ctx.diff_content = diff.to_string();
    ctx.changed_files = vec!["openapi/openapi.yaml".to_string()];

    let report = CrossServiceImpactEngine::new()
        .evaluate_cross_service_impact(Path::new("."), &ctx)
        .expect("the engine reads the diff");
    assert!(!report.is_compatible, "{}", report.summary);
    assert_eq!(
        report.breaking_findings[0].removed_required_field,
        "pr_number"
    );
}

/// Teeth in the other direction. `buf` and `oasdiff` compare models, so
/// reformatting is invisible to them; a text scan that fires on re-indentation
/// is a false-accusation machine on any repository that runs a YAML formatter.
#[test]
fn reindenting_a_required_block_is_not_a_breaking_change() {
    let diff = "\
diff --git a/openapi/openapi.yaml b/openapi/openapi.yaml
--- a/openapi/openapi.yaml
+++ b/openapi/openapi.yaml
@@ -30,6 +30,6 @@
-              required:
-                - repo
-                - pr_number
+   required:
+   - repo
+   - pr_number
";
    let mut ctx = diff_of(&[]);
    ctx.diff_content = diff.to_string();
    ctx.changed_files = vec!["openapi/openapi.yaml".to_string()];

    let report = CrossServiceImpactEngine::new()
        .evaluate_cross_service_impact(Path::new("."), &ctx)
        .expect("the engine reads the diff");
    assert!(
        report.is_compatible,
        "re-indentation was reported as a wire break: {:?}",
        report.breaking_findings
    );
}

/// Adding a required field is not a removal. (It is breaking in the *request*
/// direction under oasdiff, which this gate does not distinguish and does not
/// claim to -- see the fidelity registry entry.)
#[test]
fn adding_a_required_field_is_not_reported_as_a_removal() {
    let diff = "\
diff --git a/openapi/openapi.yaml b/openapi/openapi.yaml
--- a/openapi/openapi.yaml
+++ b/openapi/openapi.yaml
@@ -34,2 +34,2 @@
-              required: [repo]
+              required: [repo, pr_number]
";
    let mut ctx = diff_of(&[]);
    ctx.diff_content = diff.to_string();
    ctx.changed_files = vec!["openapi/openapi.yaml".to_string()];

    let report = CrossServiceImpactEngine::new()
        .evaluate_cross_service_impact(Path::new("."), &ctx)
        .expect("the engine reads the diff");
    assert!(report.is_compatible, "{:?}", report.breaking_findings);
}

/// A non-contract file carrying the word `required:` is not a wire contract.
#[test]
fn a_source_file_is_not_a_wire_contract() {
    let diff = "\
diff --git a/src/schema.rs b/src/schema.rs
--- a/src/schema.rs
+++ b/src/schema.rs
@@ -1,2 +1,1 @@
-    required: [repo, pr_number]
+    required: [repo]
";
    let mut ctx = diff_of(&[]);
    ctx.diff_content = diff.to_string();
    ctx.changed_files = vec!["src/schema.rs".to_string()];
    let report = CrossServiceImpactEngine::new()
        .evaluate_cross_service_impact(Path::new("."), &ctx)
        .expect("the engine reads the diff");
    assert!(report.is_compatible, "{:?}", report.breaking_findings);
}

/// The consumer set is not guessed. No oracle names a downstream service it did
/// not learn from a registration: Pact from a published pact, Confluent from a
/// registered subject, buf from a stored image. With none of those configured,
/// the honest report names the contract and the field and abstains on who is
/// downstream.
#[test]
fn the_gate_names_no_consumer_it_did_not_derive() {
    let all = anvil::source_scan::paths::module_source(
        "src/cross_service_impact",
        Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    let code_only: String = all
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    for invented in ["oyatie-backend", "oyatie-console"] {
        assert!(
            !code_only.contains(invented),
            "`{invented}` is still hardcoded as an impacted service"
        );
    }
    assert!(
        !code_only.contains("impacted_consumer"),
        "the finding still carries a consumer field nothing derives"
    );
    assert!(
        code_only.contains("no consumer registry"),
        "the module must say, in the sentence it publishes, that the consumer \
         set was not derived"
    );
}

/// A format this parser cannot open is not admitted, and is not counted as
/// read. `required_names` parses the two YAML spellings; JSON Schema's
/// `"required": [...]` carries a quote before the key and proto2's `required
/// string name = 1;` has no key, so admitting `.json` and `.proto` produced no
/// findings under a sentence saying the file had been read.
#[test]
fn a_contract_format_the_parser_cannot_open_is_not_reported_as_read() {
    let diff = "\
diff --git a/api/schema.json b/api/schema.json
--- a/api/schema.json
+++ b/api/schema.json
@@ -1,2 +1,1 @@
-  \"required\": [\"repo\", \"pr_number\"]
+  \"required\": [\"repo\"]
";
    let mut ctx = diff_of(&[]);
    ctx.diff_content = diff.to_string();
    ctx.changed_files = vec![
        "api/schema.json".to_string(),
        "proto/user.proto".to_string(),
    ];

    let report = CrossServiceImpactEngine::new()
        .evaluate_cross_service_impact(Path::new("."), &ctx)
        .expect("the engine reads the diff");

    assert!(report.breaking_findings.is_empty());
    assert!(
        report.summary.contains("none in this diff"),
        "a file the parser skipped was published as a contract that was read: {}",
        report.summary
    );
}

/// And the published summary carries the abstention, not just the source.
#[test]
fn the_summary_states_that_the_consumer_set_was_not_derived() {
    let diff = "\
diff --git a/openapi/openapi.yaml b/openapi/openapi.yaml
--- a/openapi/openapi.yaml
+++ b/openapi/openapi.yaml
@@ -34,2 +34,1 @@
-              required: [repo, pr_number]
+              required: [repo]
";
    let mut ctx = diff_of(&[]);
    ctx.diff_content = diff.to_string();
    ctx.changed_files = vec!["openapi/openapi.yaml".to_string()];

    let report = CrossServiceImpactEngine::new()
        .evaluate_cross_service_impact(Path::new("."), &ctx)
        .expect("the engine reads the diff");
    assert!(
        report.summary.contains("no consumer registry"),
        "the failure sentence still implies a known blast radius: {}",
        report.summary
    );
}
