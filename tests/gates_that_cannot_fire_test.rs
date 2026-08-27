//! Lane `fix/gates-that-cannot-fire`: three gates whose vocabulary is invented
//! or expired, so no input in the last twelve months could have turned them red.
//!
//! # The defect, restated from source at `origin/main` (207e3c7)
//!
//! ## 1. `feature_flag_status` -- "Zero stale or dead toggle fallback branches"
//!
//! `feature_flag_ratchet.rs:57` matched `@deprecated_flag`, `@stale_flag` and
//! `EXPIRATION:\s*202[0-5]`. Checked against the oracles:
//!
//!   - **LaunchDarkly** computes staleness server-side from flag age plus
//!     evaluation status (temporary, created >=30d ago, inactive-or-launched
//!     for >=7d). `ld-find-code-refs` searches source for the flag *key* inside
//!     quote delimiters and posts the hits back to the API; it reads no
//!     annotation.
//!   - **Unleash** carries `stale` as a boolean on the flag object, set through
//!     the admin API (`/api/admin/features-batch/stale`) or automatically by a
//!     per-flag-type expected lifetime. Nothing is expected in source.
//!   - **OpenFeature** is an evaluation API spec; it defines no staleness and no
//!     annotation. Its `STALE` token is a provider-connection reason, unrelated.
//!   - **Statsig** marks a temporary gate stale from rollout state, server-side.
//!   - **Uber `piranha`** takes the flag key AND its expected behaviour as
//!     *input from the operator* -- staleness is decided outside the tool -- and
//!     then deletes the dead branch by tree-sitter AST rewriting.
//!   - The nearest real "flags expire" system, Chromium's, keeps expiry in
//!     `chrome/browser/flag-metadata.json`, not in a source comment.
//!
//! No flag system decides staleness from an annotation, and neither
//! `@deprecated_flag` nor `@stale_flag` is a convention of any of them. Both
//! occur exactly twice in this repository: in the regex, and in the module's own
//! fixture. `EXPIRATION:\s*202[0-5]` stops at 2025 and today is 2026, so it aged
//! out of its own window; it occurs once, in the regex. `permanent_true_re`
//! (`:55`) required the literal source `if true && ... is_feature_enabled`,
//! which rustc and clippy both object to; zero occurrences anywhere.
//!
//! ## 2. `local_probe_status` -- "Instant pre-commit AST linting"
//!
//! `local_inner_loop/mod.rs:46-48` passed the hardcoded literal
//! `"feat: update codebase"` to a validator whose entire check was
//! `commit_msg.starts_with("feat")`. `PrDiffContext` carries no commit message,
//! so that half was a constant answering a constant. Against the oracle:
//! Conventional Commits 1.0.0 requires `<type>[(scope)][!]: <description>` with
//! a REQUIRED colon-space and a non-empty description, so `feat` alone is
//! invalid and `starts_with("feat")` also admits `feature:` and `feats:`.
//! `latency_ms: 18` (`:61`) was a literal too, and the module's own test
//! asserted `rep.latency_ms < 100` -- an assertion over a constant, the eighth
//! of its kind found in this codebase.
//!
//! Commit messages ARE obtainable: `git log base..head` in the clone the
//! pipeline already has, or GitHub's pull-request commits endpoint. There is no
//! AST anywhere in the module, no parser crate is a dependency, and
//! `syn::parse_file` needs a whole valid file -- a unified diff's added lines
//! are not one.
//!
//! ## 3. `chaos_injection_status` -- "Synthetic packet loss, DNS jitter & DB
//! failover certification"
//!
//! `chaos_injector/mod.rs:25-29` declared three faults and
//! `fault_simulator.rs:28` never read its `fault` parameter, so the same
//! two-substring test ran three times and produced three identical verdicts.
//! `recovery_time_ms: 45` was a literal for an experiment that did not run, and
//! the blocking sentence named a "preview sandbox" that does not exist. Every
//! real chaos tool -- Chaos Monkey (terminates running EC2/Titus instances via
//! Spinnaker), AWS FIS, Gremlin, LitmusChaos -- injects faults into a RUNNING
//! system, and the steady-state hypothesis at principlesofchaos.org presupposes
//! one. No established chaos tool decides resilience by pattern-matching a diff.
//! The honest adjacent category is a lint: `clippy::unwrap_used`, which upstream
//! files under the opt-in `restriction` group.
//!
//! # Premortem -- how this change can already have failed
//!
//! P1. The invented vocabulary is deleted and the gate still publishes a green
//!     on every PR, so nothing changed. -> the `*_publishes_not_measured_*`
//!     tests.
//! P2. Over-correction: with no source the gates report `Failed`, accusing every
//!     PR in the fleet. -> `*_does_not_fabricate_an_accusation`.
//! P3. "Moved, not removed": the invented tokens, the hardcoded commit message
//!     or the fabricated milliseconds reappear one file sideways, behind a
//!     `const`, or merely re-valued (`18` -> `22`, `45` -> `40`), which defeats
//!     any list of literals while leaving the gate exactly as unfailable.
//!     -> the `*_source_*` scans, over the production half of every file in
//!     each gate's module directory.
//! P4. The honest verdict is produced by the guard and thrown away by the
//!     wiring, exactly as it was for six gates before.
//!     -> `the_evaluator_reads_these_three_verdicts_instead_of_rebuilding_them`.
//! P5. The measuring path is deleted along with the invented one, so there is
//!     nothing for a real source to plug into and the gate can never be more
//!     than an abstention. -> every `*_still_*` / boundary counterpart.
//! P6. The published titles keep claiming AST linting, packet loss and DNS
//!     jitter after the code that would have to do them is gone.
//!     -> `the_matrix_claims_no_capability_these_three_gates_do_not_have`.
//! P7. A `NotMeasured` naming a gate_id nobody can look up, or a registry entry
//!     that lets an unmeasuring gate publish a pass.
//!     -> `the_three_gate_ids_are_registered_and_name_what_is_missing`.
//! P8. The commit source is plumbed and then permanently fed the empty slice,
//!     which is the same absence of information dressed as an evaluated PR.
//!     -> `the_pipeline_reads_commit_subjects_rather_than_passing_an_empty_slice`.

use anvil::chaos_injector::ChaosFaultInjector;
use anvil::feature_flag_ratchet::FeatureFlagRatchet;
use anvil::git_manager::PrDiffContext;
use anvil::local_inner_loop::{FastValidator, LocalInnerLoopProbe};
use anvil::pre_merge_guard::GateStatus;
use std::path::{Path, PathBuf};

// -------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------

fn ctx(diff: &str, changed: &[&str]) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "main".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        previous_head_sha: None,
        repo_working_dir: PathBuf::from("."),
        diff_content: diff.to_string(),
        changed_files: changed.iter().map(|s| s.to_string()).collect(),
        is_incremental: false,
    }
}

/// The production half of a source file: everything before the first
/// `#[cfg(test)]`, with every `//` comment removed. A fixture a test writes is
/// legitimate -- it is what a real source will supply later -- so only the
/// production half can carry the defect.
///
/// Commentary is stripped for the reason
/// `tests/fidelity_registry_citations_test.rs` strips it: a scan of the source
/// must be answerable by the code. Every module below documents the constant it
/// deleted, in prose, so that the next reader knows why it is gone -- and a
/// scan that counted those sentences would make writing that history
/// impossible, which is the opposite of what these checks are for. String
/// literals are kept: a published sentence is code, and it is where a
/// fabricated claim would live.
fn production_source(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    let s = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    let production = match s.find("#[cfg(test)]") {
        Some(i) => &s[..i],
        None => &s[..],
    };
    production
        .lines()
        .map(code_only)
        .collect::<Vec<_>>()
        .join("\n")
}

use anvil::source_scan::without_commentary as code_only;

#[test]
fn code_only_strips_commentary_but_keeps_published_sentences() {
    assert_eq!(
        code_only("    let x = 1; // recovery_time_ms: 45").trim(),
        "let x = 1;"
    );
    assert_eq!(code_only("//! `latency_ms: 18` was a literal").trim(), "");
    // A published sentence is code: this is where a fabricated claim lives.
    let published = r#"    GateStatus::Failed("provoked outage in preview sandbox".to_string())"#;
    assert_eq!(code_only(published), published);
}

/// Every `.rs` file under a module directory (or the single file, if the path
/// names one), as (repo-relative path, production half). The directory rather
/// than the named file, because the cheapest evasion of P3 is moving the
/// constant one file sideways.
fn module_sources(module_dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(module_dir);
    if root.is_file() {
        return vec![(module_dir.to_string(), production_source(module_dir))];
    }
    let mut out = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("module dir is readable") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let rel = format!(
                    "{}/{}",
                    module_dir,
                    path.strip_prefix(&root).expect("under root").display()
                );
                let src = production_source(&rel);
                out.push((rel, src));
            }
        }
    }
    assert!(
        !out.is_empty(),
        "no sources found under {module_dir} -- the scan silently covered nothing"
    );
    out.sort();
    out
}

fn assert_unmeasured(status: &GateStatus, gate_id: &str, must_name: &[&str]) {
    let GateStatus::NotMeasured {
        gate_id: id,
        reason,
    } = status
    else {
        panic!(
            "{gate_id}: with no source this gate must report NotMeasured. \
             Passed makes absent evidence a pass; Failed fabricates an \
             accusation. Got: {status:?}"
        );
    };
    assert_eq!(
        id, gate_id,
        "the gate_id must match the PreMergeCertificationReport field name, so \
         `unmeasured_gates` names a gate a human can look up"
    );
    let lower = reason.to_lowercase();
    assert!(
        must_name.iter().any(|n| lower.contains(&n.to_lowercase())),
        "{gate_id}: the reason must NAME the missing source (one of {must_name:?}), the \
         way automated_canary names Prometheus. \"not configured\" tells a reader nothing \
         about what would close the gap. Got: {reason}"
    );
}

fn assert_no_accusation(status: &GateStatus, gate_id: &str) {
    assert!(
        !matches!(status, GateStatus::Failed(_) | GateStatus::Errored(_)),
        "{gate_id}: a gate with no source must not accuse a clean PR of a defect \
         nobody can reproduce. Got: {status:?}"
    );
}

// =========================================================================
// 1. feature_flag_status
// =========================================================================

/// Catches P1. Staleness is a fact the flag-management system owns; Anvil talks
/// to none, and with no ledger in the tree the gate has nothing to judge a flag
/// reference against.
#[test]
fn feature_flag_publishes_not_measured_without_a_flag_lifecycle_source() {
    let dir = tempfile::tempdir().expect("tempdir");
    let diff = "+++ b/src/features.ts\n\
                + if (is_feature_enabled('new_billing_v2')) { doNew(); } else { doOld(); }";
    let report = FeatureFlagRatchet::new()
        .evaluate_feature_flags(dir.path(), &ctx(diff, &["src/features.ts"]))
        .expect("the ratchet reads the diff");

    assert_unmeasured(
        &report.status,
        "feature_flag_status",
        &[
            "launchdarkly",
            "unleash",
            "statsig",
            "ledger",
            "stale-flags",
        ],
    );
    assert!(
        !report.is_clean,
        "a flag whose lifecycle was never looked up is not a retired one"
    );
}

/// Catches P2 for gate 62.
#[test]
fn feature_flag_does_not_fabricate_an_accusation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let report = FeatureFlagRatchet::new()
        .evaluate_feature_flags(dir.path(), &ctx("+ let x = 1;", &["src/lib.rs"]))
        .expect("the ratchet reads the diff");
    assert_no_accusation(&report.status, "feature_flag_status");
}

/// Catches P5: the honest half must survive. Locating a flag by its **key string
/// at the call site** is exactly what `ld-find-code-refs` does, and it is the
/// seam a real LaunchDarkly / Unleash / Statsig lookup plugs into. Deleting it
/// along with the invented annotations would leave nothing to wire a source to.
#[test]
fn feature_flag_still_locates_a_flag_reference_by_its_key() {
    let refs = FeatureFlagRatchet::scan_flag_references(
        "+++ b/src/features.ts\n\
         + if (is_feature_enabled('new_billing_v2')) { doNew(); }\n\
         +++ b/src/other.ts\n\
         + const on = flags.get(\"legacy_checkout\");",
    );

    let keys: Vec<&str> = refs.iter().map(|r| r.flag_key.as_str()).collect();
    assert_eq!(keys, vec!["new_billing_v2", "legacy_checkout"]);
    assert_eq!(refs[0].file_path, "src/features.ts");
    assert_eq!(refs[1].file_path, "src/other.ts");
}

/// The measuring-path counterpart to the absence test: given a ledger, the gate
/// reaches a real verdict, and that verdict is not green for a change that adds
/// a reference to a flag the ledger records as stale. This is the Chromium
/// `flag-metadata.json` shape reduced to what this repository already does for
/// `REORG-DRAIN.md`.
#[test]
fn feature_flag_reports_a_new_reference_to_a_flag_the_ledger_records_as_stale() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("STALE-FLAGS.md"),
        "# Stale flags\n\n- `new_billing_v2` -- launched 100% since 2025-11, delete on sight\n",
    )
    .expect("ledger");

    let diff = "+++ b/src/features.ts\n\
                + if (is_feature_enabled('new_billing_v2')) { doNew(); } else { doOld(); }";
    let report = FeatureFlagRatchet::new()
        .evaluate_feature_flags(dir.path(), &ctx(diff, &["src/features.ts"]))
        .expect("the ratchet reads the diff");

    assert!(
        matches!(
            report.status,
            GateStatus::Warning(_) | GateStatus::Failed(_)
        ),
        "a new reference to a flag the ledger records as stale must be reported; \
         got {:?}",
        report.status
    );
    assert!(!report.is_clean);
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].flag_name, "new_billing_v2");
    assert_eq!(report.violations[0].file_path, "src/features.ts");
    assert!(
        report.summary.contains("new_billing_v2"),
        "the published sentence must name the flag, not merely count it: {}",
        report.summary
    );
}

/// The other side of the boundary, so the gate above cannot satisfy itself by
/// reporting everything: a flag the ledger does not record is a live flag, and a
/// live flag is a pass -- a real one, because a ledger really was read.
#[test]
fn feature_flag_passes_a_reference_to_a_flag_the_ledger_does_not_record() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("STALE-FLAGS.md"),
        "# Stale flags\n\n- `ancient_toggle`\n",
    )
    .expect("ledger");

    let diff = "+++ b/src/features.ts\n\
                + if (is_feature_enabled('new_billing_v2')) { doNew(); } else { doOld(); }";
    let report = FeatureFlagRatchet::new()
        .evaluate_feature_flags(dir.path(), &ctx(diff, &["src/features.ts"]))
        .expect("the ratchet reads the diff");

    assert!(
        matches!(report.status, GateStatus::Passed),
        "got {:?}",
        report.status
    );
    assert!(report.is_clean);
    assert_eq!(report.flags_scanned_count, 1);
}

/// A ledger with no flag reference in the diff is still nothing measured: the
/// gate observed no toggle, not a clean one. Without this, one ledger file in a
/// repository would turn every unrelated PR green.
#[test]
fn feature_flag_with_a_ledger_but_no_flag_in_the_diff_is_still_unmeasured() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("STALE-FLAGS.md"), "- `ancient_toggle`\n").expect("ledger");
    let report = FeatureFlagRatchet::new()
        .evaluate_feature_flags(dir.path(), &ctx("+ let x = 1;", &["src/lib.rs"]))
        .expect("the ratchet reads the diff");

    assert_unmeasured(
        &report.status,
        "feature_flag_status",
        &["flag reference", "no flag"],
    );
}

/// Only added lines are the change's own -- and here the direction matters more
/// than usual. A diff that *deletes* the last reference to a stale flag is the
/// cleanup the gate exists to encourage; counting the removed line as a new
/// reference would report the fix as the defect. A context line the diff merely
/// carries past is not this change's either.
///
/// Found by mutation: narrowing the added-line filter to `starts_with("+++")`
/// alone left every test in this file green.
#[test]
fn feature_flag_reads_only_the_lines_the_change_adds() {
    let refs = FeatureFlagRatchet::scan_flag_references(
        "+++ b/src/features.ts\n\
         - if (is_feature_enabled('new_billing_v2')) { doNew(); }\n\
           if (is_feature_enabled('untouched_toggle')) { doOld(); }\n\
         + doNew();",
    );
    assert!(
        refs.is_empty(),
        "a removed reference is the cleanup, not the debt, and a context line is \
         not this change's: {refs:?}"
    );
}

/// The same property at the gate: deleting the last call site of a flag the
/// ledger records as stale must not be reported as a violation.
#[test]
fn feature_flag_does_not_report_the_deletion_of_a_stale_flag_reference() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("STALE-FLAGS.md"), "- `new_billing_v2`\n").expect("ledger");

    let diff = "+++ b/src/features.ts\n\
                - if (is_feature_enabled('new_billing_v2')) { doNew(); } else { doOld(); }\n\
                + doNew();";
    let report = FeatureFlagRatchet::new()
        .evaluate_feature_flags(dir.path(), &ctx(diff, &["src/features.ts"]))
        .expect("the ratchet reads the diff");

    assert!(
        report.violations.is_empty(),
        "the change retires the flag; accusing it of referencing one inverts the \
         gate: {:?}",
        report.violations
    );
}

/// The ledger is read from the repository under review, so a gate that ignored
/// `repo_dir` and answered from Anvil's own tree would judge the wrong project.
#[test]
fn feature_flag_reads_the_ledger_of_the_repository_under_review() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("governance")).expect("governance dir");
    std::fs::write(
        dir.path().join("governance/STALE-FLAGS.md"),
        "- `new_billing_v2`\n",
    )
    .expect("ledger");

    let diff = "+++ b/src/features.ts\n+ if (is_feature_enabled('new_billing_v2')) {}";
    let report = FeatureFlagRatchet::new()
        .evaluate_feature_flags(dir.path(), &ctx(diff, &["src/features.ts"]))
        .expect("the ratchet reads the diff");

    assert!(!report.is_clean, "a governance/ ledger must be read too");
}

/// Catches P3. The invented tokens must be gone from production code, not
/// re-spelled.
#[test]
fn feature_flag_source_carries_no_invented_flag_vocabulary() {
    const INVENTED: &[&str] = &["@deprecated_flag", "@stale_flag", "EXPIRATION:", "202[0-5]"];

    let mut offenders = Vec::new();
    for (rel, src) in module_sources("src/feature_flag_ratchet.rs") {
        for (n, line) in src.lines().enumerate() {
            for token in INVENTED {
                if line.contains(token) {
                    offenders.push(format!("{rel}:{}: `{}`", n + 1, line.trim()));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "no flag system -- LaunchDarkly, Unleash, OpenFeature, Statsig, piranha -- \
         decides staleness from a source annotation, and a year window ending in 2025 \
         is expired in 2026. {} site(s):\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}

// =========================================================================
// 2. local_probe_status
// =========================================================================

/// Catches P1. `PrDiffContext` carries no commit message, so with no commit
/// source the conventional-commit half has nothing to judge -- and a diff with
/// no secret in it is not a pre-commit run that passed.
#[test]
fn local_probe_publishes_not_measured_without_a_commit_message_source() {
    let report = LocalInnerLoopProbe::new()
        .evaluate_local_probe(Path::new("."), &ctx("+ fn ok() {}", &["src/lib.rs"]), &[])
        .expect("the probe reads the diff");

    assert_unmeasured(
        &report.status,
        "local_probe_status",
        &["commit message", "git log", "commit"],
    );
    assert!(!report.is_valid);
}

/// Catches P2 for gate 38.
#[test]
fn local_probe_does_not_fabricate_an_accusation() {
    let report = LocalInnerLoopProbe::new()
        .evaluate_local_probe(Path::new("."), &ctx("+ fn ok() {}", &["src/lib.rs"]), &[])
        .expect("the probe reads the diff");
    assert_no_accusation(&report.status, "local_probe_status");
}

/// The measured half survives even with no commit source: the staged-diff secret
/// scan reads the real diff, so it can turn this gate red on its own. This is
/// the shape `hermetic_build` already uses -- one half measured, one absent.
#[test]
fn local_probe_still_fails_a_diff_carrying_a_secret_with_no_commit_source() {
    let report = LocalInnerLoopProbe::new()
        .evaluate_local_probe(
            Path::new("."),
            &ctx(
                &format!("+ let t = \"{}\";", github_token_shaped()),
                &["src/lib.rs"],
            ),
            &[],
        )
        .expect("the probe reads the diff");

    assert!(
        matches!(report.status, GateStatus::Failed(_)),
        "a token in the staged diff is measured evidence, not an absent one; got {:?}",
        report.status
    );
    assert!(!report.is_valid);
}

/// The secret scan used to read the WHOLE diff with `staged_diff.contains(m)`,
/// which is the same sign error mutation testing found in `scan_flag_references`
/// -- fixed there, live here. Removing a committed credential is the fix, not
/// the defect, and it was reported as the defect.
#[test]
fn the_secret_scan_reads_only_the_lines_the_change_adds() {
    let v = FastValidator::new();
    let key = aws_key_shaped();
    for (label, sigil) in [("deleting a leaked key", '-'), ("a context line", ' ')] {
        let diff = format!("+++ b/src/net.rs\n{sigil} let k = \"{key}\";");
        assert!(
            v.scan_staged_diff(&diff).is_valid,
            "{label} is not this change adding a credential"
        );
    }
    assert!(
        !v.scan_staged_diff(&format!("+++ b/src/net.rs\n+ let k = \"{key}\";"))
            .is_valid,
        "a key on an added line is exactly what this scan is for"
    );
}

/// Credential-SHAPED fixtures, assembled at run time.
///
/// Written as one literal they would be a whole credential on a line this pull
/// request adds, and the scan under test would -- correctly -- refuse the merge
/// that introduces its own tests. Splitting the prefix from the body is the
/// same trick a real scanner's allowlist pragma performs, with no allowlist to
/// go stale. The body below is AWS's own published example key.
fn aws_key_shaped() -> String {
    format!("AKIA{}", "IOSFODNN7EXAMPLE")
}

fn github_token_shaped() -> String {
    format!("ghp_{}", "0123456789abcdefghij0123456789abcdef")
}

/// `"AKIA"` as a bare four-character substring made every change that touched
/// this repository's own AWS-key regex block itself. A credential is the whole
/// token, which is what `pre_merge_guard::scanner` already matched.
#[test]
fn the_secret_scan_matches_a_credential_and_not_a_prefix() {
    let v = FastValidator::new();
    assert!(
        v.scan_staged_diff(
            "+++ b/src/pre_merge_guard/scanner.rs\n+            (r\"(?i)AKIA[0-9A-Z]{16}\", \"AWS Access Key ID\"),"
        )
        .is_valid,
        "the pattern that finds AWS keys is not an AWS key"
    );
    assert!(
        v.scan_staged_diff("+++ b/src/net.rs\n+ let bucket = \"AKIA-owned-artifacts\";")
            .is_valid,
        "a four-character prefix is not a credential"
    );
}

/// The measuring-path counterpart: given real commit subjects the gate reaches a
/// real verdict, and it is RED for a message that is not a conventional commit.
#[test]
fn local_probe_fails_a_real_commit_subject_that_is_not_conventional() {
    let subjects = vec!["updated some stuff".to_string()];
    let report = LocalInnerLoopProbe::new()
        .evaluate_local_probe(
            Path::new("."),
            &ctx("+ fn ok() {}", &["src/lib.rs"]),
            &subjects,
        )
        .expect("the probe reads the diff");

    assert!(
        matches!(report.status, GateStatus::Failed(_)),
        "got {:?}",
        report.status
    );
    assert!(!report.is_valid);
    assert!(
        report.summary.contains("updated some stuff"),
        "the published sentence must name the offending subject: {}",
        report.summary
    );
}

/// And green for subjects that are conventional, so the gate above cannot pass
/// by failing everything.
#[test]
fn local_probe_passes_real_conventional_commit_subjects() {
    let subjects = vec![
        "feat(auth): add cedar pdp check".to_string(),
        "fix!: drop the legacy header".to_string(),
    ];
    let report = LocalInnerLoopProbe::new()
        .evaluate_local_probe(
            Path::new("."),
            &ctx("+ fn ok() {}", &["src/lib.rs"]),
            &subjects,
        )
        .expect("the probe reads the diff");

    assert!(
        matches!(report.status, GateStatus::Passed),
        "got {:?}",
        report.status
    );
    assert!(report.is_valid);
}

/// Catches the specific defect the old check could not see.
/// `commit_msg.starts_with("feat")` is true of every string below, and
/// Conventional Commits 1.0.0 admits none of them: the colon-space and a
/// non-empty description are both REQUIRED, and `feature` is not `feat`.
#[test]
fn the_commit_header_check_implements_the_conventional_commits_grammar() {
    let v = FastValidator::new();

    for bad in [
        "feat",                 // no colon, no description
        "feat:",                // colon, no description
        "feat: ",               // colon, blank description
        "feat:no space",        // the space after the colon is required
        "feature: add a thing", // `feature` is not a type
        "feats: add a thing",
        "Feat: add a thing", // commitlint type-case is lower-case
        "featadd a thing",
        "add a thing",
        "feat(: add a thing", // unbalanced scope
    ] {
        let f = v
            .check_commit_header(bad)
            .unwrap_or_else(|| panic!("`{bad}` must be judged, not ignored"));
        assert!(
            !f.is_valid,
            "`{bad}` is not a conventional commit header but was accepted"
        );
    }

    for good in [
        "feat: add a thing",
        "fix(parser): stop dropping the last hunk",
        "feat!: change the wire format",
        "refactor(shape)!: move the baseline",
        "chore: bump the toolchain",
        "ci: pin the runner",
        "revert: feat: add a thing",
        // `type-enum` is configuration, not specification, and this repository's
        // promotion ladder writes these. Hardcoding commitlint's default made
        // the check red on the convention the project actually follows.
        "promote(dev): fast-forward integration trunk from main",
        "promote(staging): fast-forward from dev",
    ] {
        let f = v
            .check_commit_header(good)
            .unwrap_or_else(|| panic!("`{good}` must be judged, not ignored"));
        assert!(f.is_valid, "`{good}` is a valid conventional commit header");
    }
}

/// commitlint's own `defaultIgnores` skip these, and so must this: a merge or a
/// fixup subject is generated by git, not written by the author, and failing a
/// PR for one is a false red the author cannot fix.
#[test]
fn the_commit_header_check_ignores_subjects_git_generates() {
    let v = FastValidator::new();
    for ignored in [
        "Merge branch 'main' into feature-x",
        "Merge pull request #12 from oyatie/x",
        "Merge remote-tracking branch 'origin/main'",
        "fixup! feat: add a thing",
        "squash! feat: add a thing",
        "Revert \"feat: add a thing\"",
    ] {
        assert!(
            v.check_commit_header(ignored).is_none(),
            "`{ignored}` is generated by git and must not be judged"
        );
    }
}

/// A PR whose every subject is a merge commit has nothing judgeable in it, and
/// "nothing to judge" is not "hygiene verified".
#[test]
fn local_probe_is_unmeasured_when_every_subject_is_ignored() {
    let subjects = vec!["Merge branch 'main' into x".to_string()];
    let report = LocalInnerLoopProbe::new()
        .evaluate_local_probe(
            Path::new("."),
            &ctx("+ fn ok() {}", &["src/lib.rs"]),
            &subjects,
        )
        .expect("the probe reads the diff");
    assert_unmeasured(
        &report.status,
        "local_probe_status",
        &["commit message", "git log", "commit"],
    );
}

/// Catches P3, and replaces the eighth constant-certifying assertion in this
/// codebase. `assert!(rep.latency_ms < 100)` in `local_inner_loop/mod.rs`
/// compared a literal `18` against `100` and would have held for as long as the
/// literal did. A duration is a measurement or it is nothing, and the mechanical
/// way to say so is that no numeric literal is assigned into the field.
#[test]
fn local_probe_source_assigns_no_literal_latency_or_commit_message() {
    let mut offenders = Vec::new();
    // The WHOLE tree, not `src/local_inner_loop`. This guard was correct and
    // scoped to where the defect had been found rather than where it can occur,
    // so it watched the module while the surviving instance sat in
    // `src/cli/handlers.rs`: the `probe` subcommand passed the literal
    // "chore: probe check" to `validate_pre_commit` and graded it. A caller
    // outside the module is exactly the caller the module cannot police, which
    // makes the narrow scope the one scope guaranteed to miss.
    for (rel, src) in module_sources("src") {
        for (n, line) in src.lines().enumerate() {
            let squashed: String = line.chars().filter(|c| !c.is_whitespace()).collect();
            let literal_latency = squashed
                .split("latency_ms:")
                .nth(1)
                .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()));
            if literal_latency {
                offenders.push(format!(
                    "{rel}:{}: `{}` -- a duration nobody timed",
                    n + 1,
                    line.trim()
                ));
            }
            if line.contains("validate_pre_commit(\"") || line.contains("check_commit_header(\"") {
                offenders.push(format!(
                    "{rel}:{}: `{}` -- a commit message written by the caller answers \
                     only the caller",
                    n + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "{} fabricated input(s) in the local inner-loop probe:\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}

/// The behavioural half of the same property: the published latency is bounded
/// by the wall clock measured around the call, which a declared constant is not.
/// The input is deliberately tiny so the surrounding clock reads at or near
/// zero -- a hardcoded `18` cannot fit under it.
#[test]
fn local_probe_latency_is_timed_rather_than_declared() {
    let started = std::time::Instant::now();
    let report = LocalInnerLoopProbe::new()
        .evaluate_local_probe(Path::new("."), &ctx("+ fn ok() {}", &["src/lib.rs"]), &[])
        .expect("the probe reads the diff");
    let outer = started.elapsed().as_millis() as u64;

    assert!(
        report.latency_ms <= outer,
        "the published latency ({}ms) exceeds the wall clock around the call ({}ms), \
         so it was declared rather than timed",
        report.latency_ms,
        outer
    );
}

// =========================================================================
// 3. chaos_injection_status
// =========================================================================

/// Catches P1. Nothing here runs a system, so no fault can be injected into one
/// and no steady state can be observed returning to normal.
#[test]
fn chaos_publishes_not_measured_without_a_running_system() {
    let report =
        ChaosFaultInjector::new().scan_for_unhandled_await_without_a_running_system("+ let n = 1;");

    assert_unmeasured(
        &report.status,
        "chaos_injection_status",
        &["running", "deployment", "fault injector"],
    );
    assert!(
        report.unhandled_awaits.is_empty(),
        "a system nothing was injected into is not a resilient one, and nothing was linted"
    );
}

/// Catches P2 for gate 48.
#[test]
fn chaos_does_not_fabricate_an_accusation() {
    let report =
        ChaosFaultInjector::new().scan_for_unhandled_await_without_a_running_system("+ let n = 1;");
    assert_no_accusation(&report.status, "chaos_injection_status");
}

/// Catches P5 and is the measuring-path counterpart: the one honest computation
/// in the module was a scan of the diff for an `.unwrap()` on an awaited call --
/// a lint (`clippy::unwrap_used`, upstream's opt-in `restriction` group), not a
/// chaos experiment. It reads the real diff, so it can turn the gate red on real
/// input, and it survives.
#[test]
fn chaos_still_reports_an_unwrap_on_an_awaited_call() {
    let report = ChaosFaultInjector::new().scan_for_unhandled_await_without_a_running_system(
        "+++ b/src/net.rs\n+ let resp = client.send().await.unwrap();",
    );

    assert!(
        matches!(report.status, GateStatus::Warning(_)),
        "got {:?}",
        report.status
    );
    assert_eq!(report.unhandled_awaits.len(), 1);
    assert_eq!(report.unhandled_awaits[0].file_path, "src/net.rs");
    assert!(
        report.summary.contains("src/net.rs"),
        "the published sentence must name where it looked: {}",
        report.summary
    );
}

/// The old scan matched exactly two literal spellings,
/// `.send().await.unwrap()` and `.query().await.unwrap()`, so a panic on any
/// other awaited call was invisible. The property is the unwrap on the await,
/// not the receiver's name.
#[test]
fn chaos_reports_an_unwrap_on_an_awaited_call_whatever_the_receiver_is_called() {
    let report = ChaosFaultInjector::new().scan_for_unhandled_await_without_a_running_system(
        "+++ b/src/db.rs\n+ let row = pool.fetch_one(sql).await.unwrap();",
    );
    assert!(
        matches!(report.status, GateStatus::Warning(_)),
        "an unwrap on an awaited call is the property, not the two receiver names the \
         old scan hardcoded; got {:?}",
        report.status
    );
}

/// The gate blocked on a lint whose first run was red on ten lines of its own
/// diff, none of them an unwrapped await, and which cannot tell a test module
/// from production code. `clippy::unwrap_used` is in the opt-in `restriction`
/// group for the same reason. Debt is surfaced, not refused -- the conclusion
/// `feature_flag_status` in this same change already reaches.
#[test]
fn chaos_surfaces_an_unwrapped_await_without_refusing_the_merge() {
    let report = ChaosFaultInjector::new().scan_for_unhandled_await_without_a_running_system(
        "+++ b/src/net.rs\n+ let resp = client.send().await.unwrap();",
    );
    assert!(
        report.status.is_acceptable(),
        "a lint clippy files under `restriction` may not block a merge; got {:?}",
        report.status
    );
    assert!(
        !matches!(
            report.status,
            GateStatus::Passed | GateStatus::NotMeasured { .. }
        ),
        "it is still a published finding, not silence; got {:?}",
        report.status
    );
}

/// The first version of this scan read the raw line, so it was red on its own
/// implementation, on its own tests' fixture strings, and on the registry
/// sentence describing it. Prose about the property is not the property.
#[test]
fn chaos_does_not_count_the_lint_written_about_it_as_a_hit() {
    let own_diff = concat!(
        "+++ b/src/chaos_injector/mod.rs\n",
        "+    /// `.send().await.unwrap()` and `.query().await.unwrap()`, so a panic on any\n",
        "+        if squashed.contains(\".await.unwrap()\") {\n",
        "+++ b/tests/gates_that_cannot_fire_test.rs\n",
        "+        \"+++ b/src/net.rs\\n+ let resp = client.send().await.unwrap();\",\n",
    );
    let report =
        ChaosFaultInjector::new().scan_for_unhandled_await_without_a_running_system(own_diff);
    assert!(
        report.unhandled_awaits.is_empty(),
        "comments and string literals describe the lint; they are not unwrapped awaits: {:?}",
        report.unhandled_awaits
    );
    assert_unmeasured(
        &report.status,
        "chaos_injection_status",
        &["running", "deployment", "fault injector"],
    );
}

/// The ceiling of a line-oriented scan, pinned rather than claimed away.
///
/// `code_only` reads one line with no memory of the previous one, and a diff
/// hunk is not contiguous file text, so the CONTINUATION line of a multi-line
/// Rust string literal carries no opening quote and is read as code. The
/// registry gap sentence describing this lint is exactly such a line, so it
/// still counts itself. A Warning is the reason that is affordable; it is the
/// reason this may not block.
#[test]
fn chaos_still_counts_a_continuation_line_of_a_multi_line_string_literal() {
    let report = ChaosFaultInjector::new().scan_for_unhandled_await_without_a_running_system(
        "+++ b/src/fidelity/registry.rs\n+              `.await.unwrap()` once whitespace is removed. \\",
    );
    assert_eq!(
        report.unhandled_awaits.len(),
        1,
        "known ceiling: a line-oriented scan cannot see it is inside a string"
    );
    assert!(
        report.status.is_acceptable(),
        "which is precisely why a hit may not refuse a merge; got {:?}",
        report.status
    );
}

/// Only added lines are the change's own. A `.await.unwrap()` the diff merely
/// carries past as context, or removes, is not something this PR did.
#[test]
fn chaos_reads_only_the_lines_the_change_adds() {
    let report = ChaosFaultInjector::new().scan_for_unhandled_await_without_a_running_system(
        "+++ b/src/net.rs\n  let resp = client.send().await.unwrap();\n- let old = x.go().await.unwrap();\n+ let ok = 1;",
    );
    assert!(
        report.unhandled_awaits.is_empty(),
        "a context or removed line is not an accusation this change earned: {:?}",
        report.unhandled_awaits
    );
}

/// A handled await is not a resilience certification either -- nothing was made
/// to fail -- so the clean case is `NotMeasured`, not `Passed`. Without this the
/// gate could satisfy the tests above by reporting everything, or return to
/// publishing a green for every PR that happens not to spell `.unwrap()`.
#[test]
fn chaos_reports_a_handled_await_as_unmeasured_rather_than_resilient() {
    let report = ChaosFaultInjector::new().scan_for_unhandled_await_without_a_running_system(
        "+++ b/src/net.rs\n+ let resp = client.send().await.map_err(AppError::from)?;",
    );
    assert_unmeasured(
        &report.status,
        "chaos_injection_status",
        &["running", "deployment", "fault injector"],
    );
}

/// Catches P3. The three fault declarations produced three identical verdicts
/// because the simulator never read its `fault` argument, and `recovery_time_ms`
/// timed a recovery that never happened. Neither may come back, in this module
/// or one file sideways.
#[test]
fn chaos_source_declares_no_fault_it_cannot_inject_and_no_recovery_it_did_not_time() {
    const BANNED: &[&str] = &[
        "NetworkPacketDrop",
        "DnsResolutionLatency",
        "DatabaseLeaderFailover",
        "ServiceWorkerPanic",
        "recovery_time_ms",
        "drop_pct",
        "delay_ms",
        "preview sandbox",
    ];

    let mut offenders = Vec::new();
    for (rel, src) in module_sources("src/chaos_injector") {
        for (n, line) in src.lines().enumerate() {
            for banned in BANNED {
                if line.contains(banned) {
                    offenders.push(format!("{rel}:{}: `{}`", n + 1, line.trim()));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "Chaos Monkey terminates running instances, AWS FIS and Gremlin act on live \
         resources, LitmusChaos on live workloads. Nothing here runs a system, so a \
         declared fault is a fault nobody injected and a recovery time is a recovery \
         nobody observed. {} site(s):\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}

/// The blocking sentence named a "preview sandbox" that exists nowhere in this
/// repository, so an author reading a blocked PR was sent to look for a thing
/// they could not find. The evaluator is where it was written.
#[test]
fn no_gate_blocks_a_merge_by_naming_a_sandbox_that_does_not_exist() {
    let src = production_source("src/pre_merge_guard/evaluator.rs");
    assert!(
        !src.contains("preview sandbox"),
        "the chaos gate blocked with \"provoked unhandled panic/outage in preview \
         sandbox\"; no preview sandbox is deployed, spawned or configured anywhere in \
         this repository"
    );
}

// =========================================================================
// 4. Wiring, titles and registry -- P4, P6, P7, P8
// =========================================================================

/// Catches P4, the failure mode that has already undone this work once for six
/// other gates: the guard is made honest and the evaluator rebuilds the verdict
/// from a boolean, which collapses `NotMeasured` into `Passed`.
#[test]
fn the_evaluator_reads_these_three_verdicts_instead_of_rebuilding_them() {
    let code: String = production_source("src/pre_merge_guard/evaluator.rs")
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut missing = Vec::new();
    for (binding, gate_id) in [
        ("local_probe_report", "local_probe_status"),
        ("chaos_injection_report", "chaos_injection_status"),
        ("feature_flag_report", "feature_flag_status"),
    ] {
        if !code.contains(&format!("let {gate_id} = {binding}.status.clone();")) {
            missing.push(format!("`let {gate_id} = {binding}.status.clone();`"));
        }
        if code.contains(&format!("= if {binding}.")) {
            missing.push(format!(
                "`= if {binding}.` -- the verdict is rebuilt in the wiring"
            ));
        }
    }

    assert!(
        missing.is_empty(),
        "these gates own their GateStatus and the evaluator must carry it through \
         verbatim, as gates 11, 14 and 47 already do: {missing:?}"
    );
}

/// Catches P8. Plumbing a commit source and then permanently handing it the
/// empty slice is the `evaluate_stack_synchronization(&[])` defect wearing a new
/// coat: the argument is a literal that is empty on every pull request forever.
#[test]
fn the_pipeline_reads_commit_subjects_rather_than_passing_an_empty_slice() {
    let src = production_source("src/webhook/pipelines/certify.rs");
    let flat: String = src.chars().filter(|c| !c.is_whitespace()).collect();

    assert!(
        !flat.contains("evaluate_local_probe(repo_dir,diff_ctx,&[])"),
        "the probe is handed an empty slice on every pull request, so the commit \
         half can never be measured"
    );
    assert!(
        flat.contains("commit_subjects"),
        "the certification pipeline must obtain the commit subjects it hands the \
         probe; `PrDiffContext` carries none, and `git log <base>..<head>` in the \
         clone the pipeline already has is where they come from"
    );
}

/// Catches P6. A title is published on the PR scorecard, so it is a claim. None
/// of these three capabilities exists: there is no AST anywhere in
/// `src/local_inner_loop`, and no packet is dropped, no DNS query delayed and no
/// database leader failed over anywhere in `src/chaos_injector`.
#[test]
fn the_matrix_claims_no_capability_these_three_gates_do_not_have() {
    let src = production_source("src/pre_merge_guard/matrix.rs");
    let offenders: Vec<&str> = [
        "AST linting",
        "Synthetic packet loss, DNS jitter & DB failover certification",
        "Zero stale or dead toggle fallback branches",
    ]
    .into_iter()
    .filter(|claim| src.contains(claim))
    .collect();

    assert!(
        offenders.is_empty(),
        "the scorecard publishes {} capability claim(s) with no implementation behind \
         them: {offenders:?}",
        offenders.len()
    );
}

/// Catches P7. The gate_id is the join key between the published status, the
/// scorecard field and the fidelity registry; a `NotMeasured` nobody can resolve
/// blocks a merge for a reason the author cannot act on.
#[test]
fn the_three_gate_ids_are_registered_and_name_what_is_missing() {
    for gate_id in [
        "feature_flag_status",
        "local_probe_status",
        "chaos_injection_status",
    ] {
        let entry = anvil::fidelity::registry::AUDITED_GATES
            .iter()
            .find(|e| e.gate_id == gate_id)
            .unwrap_or_else(|| {
                panic!(
                    "`{gate_id}` has no entry in the fidelity registry, so \
                     `unmeasured_gates` names a gate nobody can look up"
                )
            });
        assert!(
            !entry.gap.is_empty(),
            "{gate_id}: the gap is what the scorecard publishes in place of the claim; \
             it may not be blank"
        );
        assert!(
            entry.blocked_on.is_some(),
            "{gate_id} cannot measure what it is named for and must name what it is \
             blocked on, so the gap is closable rather than merely admitted"
        );
    }
}

/// The three gate ids must be reachable from the report, so an honest verdict
/// that survives the evaluator also survives the scorecard.
#[test]
fn the_three_verdicts_reach_the_published_report() {
    let report = anvil::pre_merge_guard::PreMergeCertificationReport::unmeasured("fixture");
    let named: Vec<&str> = report
        .named_statuses()
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| {
            matches!(
                *id,
                "feature_flag_status" | "local_probe_status" | "chaos_injection_status"
            )
        })
        .collect();
    assert_eq!(named.len(), 3, "got {named:?}");
}
