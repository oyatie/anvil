//! Lane `attestation-provenance`: the gate that published "Cryptographic
//! Provenance Attestation" from `fs::write`.
//!
//! # The defect, restated from source
//!
//! `AttestationReport::is_attested` had exactly one production construction
//! site and it was the literal `true` (`attestation_guard.rs:102`). The whole
//! mechanism behind the claim was `serde_json::to_string_pretty` followed by
//! `fs::write` (`:94-96`). No digest of the subject, no signing key, no
//! signature, no envelope, no link to a previous receipt, and no verifier
//! anywhere in the crate. The word "cryptographic" appeared in the summary
//! string and in the matrix row title, and nowhere in the code.
//!
//! `evaluator.rs:583-587` widened that literal into `GateStatus::Passed`, so
//! the gate passed on every pull request. Its `Failed` arm was unreachable:
//! the only non-`true` path out of the stamper is `Err`, and `certify.rs`
//! propagates that with `?`, which aborts certification before the matrix is
//! ever built.
//!
//! # What the oracle actually does
//!
//! An in-toto v1 attestation names a `subject` -- an artefact and its digest --
//! and a `predicate` describing how it was produced. That statement is wrapped
//! in a DSSE envelope: the signature covers a Pre-Authentication Encoding of
//! the payload *and* its declared payload type, so a signature cannot be
//! replayed under a different type. `cosign attest` signs that envelope with a
//! key bound to an identity -- under Sigstore's keyless flow, a short-lived
//! Fulcio certificate issued against an OIDC token -- and uploads it to the
//! Rekor transparency log, which returns an inclusion proof against a signed
//! tree head. SLSA sets the level: provenance that is not signed and not
//! produced by a build platform the consumer trusts does not reach Build L2 at
//! all.
//!
//! The property all of that exists to produce is third-party verifiability: a
//! consumer who does not trust the producer can check the claim, because the
//! secret that authenticates it is not one the verifier needs, and because the
//! log is witnessed by parties other than the producer.
//!
//! Anvil has none of the ingredients. There is no signing key, no X.509 or
//! ECDSA dependency, no HTTP client to reach a log, and no verifier.
//!
//! # Why a self-produced hash chain was rejected
//!
//! The tempting middle path is a hash-chained receipt: each receipt commits to
//! the digest of its predecessor, verified with `sha2`, which the crate already
//! carries for webhook HMAC. It was rejected, on two grounds.
//!
//! The first is that it proves nothing against the adversary that matters. An
//! unkeyed chain the producer writes and the producer verifies is recomputable
//! by anyone who can edit a receipt -- which is the producer. It detects a
//! corrupted byte; it does not detect a motivated forger, and the forger is the
//! party whose claims an attestation exists to make checkable. Keying it with
//! HMAC does not help either, because the key would live in the process that
//! writes the receipts, so every party able to verify is equally able to forge.
//! Shipping it would leave gate 64 publishing a green tick under a name that
//! still reads as verifiable provenance -- the same fabrication in a humbler
//! costume.
//!
//! The second is that there is no log here to chain. Receipts are written into
//! a per-run clone at `.anvil/receipts/pr-{n}-attestation.json`, one file per
//! pull request, overwritten in place -- once by `certify.rs` with a pending
//! verdict and again by `review.rs` with the final one. A hash chain over a
//! single file that is overwritten twice per run is a hash of itself.
//!
//! So the receipt keeps being written, because a record of what was certified
//! is useful on its own, and the gate stops calling it an attestation.
//!
//! # Premortem -- how this fix can already have failed
//!
//! P1. The verdict is relabelled and the literal survives somewhere a caller
//!     can reach again.
//!     -> `no_fabricated_attestation_claim_survives_anywhere_in_src`.
//! P2. Over-correction: with nothing signed the gate reports `Failed`, accusing
//!     every pull request in the fleet of an attestation failure nobody
//!     attempted.
//!     -> `an_unsigned_receipt_is_not_an_accusation`.
//! P3. Cosmetic honesty: the status says `NotMeasured` and nothing acts on it,
//!     and a real verdict would be swallowed the same way after a backend
//!     lands.
//!     -> `the_attestation_gate_id_is_recorded_only_while_it_is_unmeasured`,
//!        which asserts both directions, and
//!        `the_registry_records_what_this_gate_is_blocked_on`.
//! P4. Over-deletion: the guard is gutted to a stub that returns `NotMeasured`
//!     and writes nothing, so the record the pipeline is built around silently
//!     disappears. Every absence assertion in this file would still pass.
//!     -> `the_receipt_is_still_written_and_records_what_it_was_given`, which
//!        is the measuring counterpart to all of them.
//! P5. The row keeps its old title, so the scorecard publishes "Cryptographic
//!     Provenance Attestation" over an honest `NotMeasured` and the reader
//!     believes the gate merely failed to run this time.
//!     -> `the_matrix_row_claims_no_cryptography`.
//! P6. The receipt is written into the clone and then swept into a commit by
//!     a `git add -A`, under the comment saying it must not be. Four production
//!     sites staged that clone and each spelled its own arguments; a fifth
//!     spelling lands the same way.
//!     -> `no_production_site_spells_its_own_whole_tree_git_add` for the
//!        crate-wide rule and
//!        `the_receipt_exclusion_pathspec_excludes_receipts_and_nothing_else`
//!        for the command itself, run against a real repository.

use anvil::attestation_guard::{ANVIL_RECEIPTS_DIR, AttestationGuard};
use anvil::git_manager::stage_excluding_receipts;
use anvil::pre_merge_guard::{GateStatus, PreMergeCertificationReport};
use std::path::{Path, PathBuf};
use std::process::Command;

const GATE_ID: &str = "attestation_status";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The production half of a source file: everything before the first
/// `#[cfg(test)]`. A fixture a test writes down is legitimate; only production
/// code can carry this defect.
fn production_source(rel: &str) -> String {
    let p = repo_root().join(rel);
    let s = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    match s.find("#[cfg(test)]") {
        Some(i) => s[..i].to_string(),
        None => s,
    }
}

/// The code in `src`, with every `//` comment dropped and every string literal
/// kept. Line-by-line, tracking `"` so a `//` inside a literal is not read as a
/// comment opener.
fn code_only(src: &str) -> String {
    src.lines()
        .map(|line| {
            let bytes = line.as_bytes();
            let mut in_string = false;
            let mut i = 0;
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' if in_string => i += 1,
                    b'"' => in_string = !in_string,
                    b'/' if !in_string && bytes.get(i + 1) == Some(&b'/') => return &line[..i],
                    _ => {}
                }
                i += 1;
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn rs_files(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{} must be readable: {e}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let body = std::fs::read_to_string(&path).expect("source file is readable");
            out.push((path, body));
        }
    }
}

/// Stamps one receipt into a fresh directory and hands back the report and the
/// directory, which must outlive the report's path.
fn stamp(
    verdict: &str,
    gates: Vec<String>,
) -> (
    anvil::attestation_guard::AttestationReport,
    tempfile::TempDir,
) {
    let dir = tempfile::tempdir().expect("tempdir");
    let report = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(AttestationGuard::new().stamp_lane_receipt(
            dir.path(),
            "oyatie/console",
            42,
            "1a2b3c4d5e6f",
            verdict,
            gates,
        ))
        .expect("the receipt is written");
    (report, dir)
}

// ---------------------------------------------------------------------------
// The absent path: nothing is signed, so nothing is attested.
// ---------------------------------------------------------------------------

/// False Green prevention, and the headline defect. Writing a JSON file is not
/// attesting to it, so the gate must not report a pass for having done so.
#[test]
fn an_unsigned_receipt_is_not_an_attestation() {
    let (report, _dir) = stamp(AttestationGuard::VERDICT_PENDING, Vec::new());

    assert_eq!(
        report.status.unmeasured_gate_id(),
        Some(GATE_ID),
        "no digest is signed, no envelope is built and no log is written, so \
         the gate must report NotMeasured naming itself: {:?}",
        report.status
    );
    assert!(
        !matches!(report.status, GateStatus::Passed | GateStatus::AutoUpdated),
        "a receipt nobody signed is not provenance: {:?}",
        report.status
    );

    let GateStatus::NotMeasured { reason, .. } = &report.status else {
        unreachable!("asserted above");
    };
    // Published verbatim on the scorecard, so it has to name the capabilities
    // that are missing rather than merely say "not measured".
    for needle in ["signature", "DSSE", "in-toto", "transparency log"] {
        assert!(
            reason.contains(needle),
            "the reason must name the absent capability so a reader can close \
             the gap; `{needle}` is not in: {reason}"
        );
    }
}

/// False Red prevention (I1, symmetric half). Nobody attempted to sign this
/// artefact, so nothing failed to sign.
#[test]
fn an_unsigned_receipt_is_not_an_accusation() {
    let (report, _dir) = stamp(
        "BLOCKED_NOT_CERTIFIED",
        vec!["doc_parity_status".to_string()],
    );

    assert!(
        !matches!(
            report.status,
            GateStatus::Failed(_) | GateStatus::Errored(_) | GateStatus::Warning(_)
        ),
        "absent evidence is not an accusation: {:?}",
        report.status
    );
    assert!(
        report.status.is_acceptable(),
        "an unconfigured gate must not fail certification on its own; it is \
         withheld through unmeasured_gates instead"
    );
}

/// The published sentence is the other half of the claim. "Cryptographic lane
/// receipt stamped at ..." was the summary over `fs::write`.
#[test]
fn the_published_summary_claims_no_cryptography() {
    let (report, _dir) = stamp(AttestationGuard::VERDICT_PENDING, Vec::new());
    let lowered = report.summary.to_lowercase();
    for banned in ["cryptograph", "signed", "attested", "provenance"] {
        assert!(
            !lowered.contains(banned),
            "the summary claims `{banned}` over a plain file write: {}",
            report.summary
        );
    }
    assert!(
        report.summary.contains(ANVIL_RECEIPTS_DIR),
        "the summary must still say what was actually done and where: {}",
        report.summary
    );
}

// ---------------------------------------------------------------------------
// P4. The measuring counterpart: the honest half must survive.
// ---------------------------------------------------------------------------

/// Every assertion above is satisfied by a guard that does nothing at all. This
/// is the one that is not: the receipt is still written, and it still records
/// the verdict, the gate list and the commit it was handed rather than a fixed
/// value.
#[test]
fn the_receipt_is_still_written_and_records_what_it_was_given() {
    for (verdict, gates) in [
        (AttestationGuard::VERDICT_PENDING, Vec::new()),
        ("BLOCKED_UNMEASURED", vec!["cedar_status".to_string()]),
        (
            "CERTIFIED_READY",
            vec!["cedar_status".to_string(), "bench_status".to_string()],
        ),
    ] {
        let (report, dir) = stamp(verdict, gates.clone());

        let rel = report
            .stamped_receipt_path
            .as_deref()
            .expect("the receipt path is reported");
        assert!(
            rel.starts_with(ANVIL_RECEIPTS_DIR),
            "the receipt must land in the agent-neutral receipts directory: {rel}"
        );

        let body = std::fs::read_to_string(dir.path().join(rel))
            .unwrap_or_else(|e| panic!("the receipt at {rel} must exist and be readable: {e}"));
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("valid json");

        assert_eq!(parsed["verdict"], verdict, "the verdict must round-trip");
        assert_eq!(
            parsed["commit_sha"], "1a2b3c4d5e6f",
            "the receipt must name the commit it was stamped for"
        );
        assert_eq!(
            parsed["pr_number"], 42,
            "the receipt must name the pull request it was stamped for"
        );
        assert_eq!(
            parsed["gates_verified"]
                .as_array()
                .map(|a| a.len())
                .expect("gates_verified is an array"),
            gates.len(),
            "the gate list must reflect what was supplied"
        );
    }
}

/// The receipt is a record, and a record that calls itself an attestation
/// engine is making the same claim the gate just stopped making.
#[test]
fn the_receipt_body_names_no_attestation_engine() {
    let (_report, dir) = stamp(AttestationGuard::VERDICT_PENDING, Vec::new());
    let body = std::fs::read_to_string(
        dir.path()
            .join(ANVIL_RECEIPTS_DIR)
            .join("pr-42-attestation.json"),
    )
    .expect("receipt readable");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("valid json");
    assert!(
        parsed.get("attestation_engine").is_none(),
        "the receipt still publishes an `attestation_engine` field over a file \
         write: {body}"
    );
    assert!(
        parsed.get("recorded_by").is_some(),
        "the receipt must still say what wrote it: {body}"
    );
}

// ---------------------------------------------------------------------------
// P1. The fabrication is deleted, not parked.
// ---------------------------------------------------------------------------

/// The flag had one production construction site and it was `true`. A boolean
/// whose only value is `true` is a constant with a field name, and the usual
/// shape of this failure is that it moves into a helper, a `Default` impl or a
/// sibling file the caller can still reach. It also cannot be rebuilt into a
/// `GateStatus` by the evaluator once it no longer exists.
///
/// Comments are stripped, on the same reasoning as
/// `tests/fidelity_registry_citations_test.rs`: naming the deleted field in
/// prose is how this repository explains what it removed, and a scan that
/// forbids the explanation would be paid for by deleting the history. String
/// literals are kept, because the fabricated summary sentence was one.
#[test]
fn no_fabricated_attestation_claim_survives_anywhere_in_src() {
    let mut files = Vec::new();
    rs_files(&repo_root().join("src"), &mut files);
    assert!(!files.is_empty(), "src must be readable");

    let mut found: Vec<String> = Vec::new();
    for (path, body) in &files {
        let production = match body.find("#[cfg(test)]") {
            Some(i) => &body[..i],
            None => body.as_str(),
        };
        let code = code_only(production);
        for needle in ["is_attested", "Cryptographic lane receipt"] {
            if code.contains(needle) {
                found.push(format!("{}: {needle}", path.display()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "the fabricated attestation flag must be deleted, not kept where a \
         caller can reach it again: {found:?}"
    );
}

/// P5. The row label is what a reader sees next to the verdict. An honest
/// `NotMeasured` under the title "Cryptographic Provenance Attestation" reads
/// as a gate that failed to run, not as a capability that does not exist.
///
/// Scoped to this one row: `zero_trust_workload_status` legitimately describes
/// SPIFFE mTLS as cryptographic, and a crate-wide ban on the word would be a
/// guard someone has to weaken to do honest work.
#[test]
fn the_matrix_row_claims_no_cryptography() {
    let (title, detail) = anvil::pre_merge_guard::matrix::label_for(GATE_ID)
        .expect("the attestation gate must have a matrix row");
    for text in [title, detail] {
        let lowered = text.to_lowercase();
        for banned in ["cryptograph", "signed", "signature", "verification"] {
            assert!(
                !lowered.contains(banned),
                "the matrix row claims `{banned}` over a plain file write: {text}"
            );
        }
    }
    // The row's *title* is the line a reader sees on the scorecard, and it must
    // not keep the word the summary is forbidden to use. The detail may say
    // "nothing signs or attests it", which is the denial, not the claim.
    let lowered_title = title.to_lowercase();
    for banned in ["provenance", "attest"] {
        assert!(
            !lowered_title.contains(banned),
            "the scorecard row title claims `{banned}` over a plain file write: {title}"
        );
    }
    assert!(
        !title.contains('🔏'),
        "a padlock over a gate that locks nothing: {title}"
    );
}

// ---------------------------------------------------------------------------
// P3. NotMeasured has to do something, in both directions.
// ---------------------------------------------------------------------------

/// Naming a gate `NotMeasured` is honest only if the report acts on it, and
/// useful only if a real measurement can still get through. Both directions
/// from one fixture, so the gate can neither pass by measuring nothing nor be
/// pinned to absence for ever.
#[test]
fn the_attestation_gate_id_is_recorded_only_while_it_is_unmeasured() {
    let base = PreMergeCertificationReport::unmeasured("fixture baseline");
    let names: Vec<&'static str> = base.named_statuses().into_iter().map(|(n, _)| n).collect();
    assert!(
        names.contains(&GATE_ID),
        "the gate id must match the report field name, or unmeasured_gates \
         names a gate nobody can look up"
    );

    let with = |attestation: GateStatus| {
        let outcomes: Vec<(&str, GateStatus)> = names
            .iter()
            .map(|n| {
                let s = if *n == GATE_ID {
                    attestation.clone()
                } else {
                    GateStatus::Passed
                };
                (*n, s)
            })
            .collect();
        let mut r = PreMergeCertificationReport::from_gate_outcomes(&outcomes)
            .expect("the fixture hands over an outcome for every gate");
        r.recompute_unmeasured();
        r
    };

    let (report, _dir) = stamp(AttestationGuard::VERDICT_PENDING, Vec::new());
    let unsigned = with(report.status.clone());
    assert!(
        unsigned.unmeasured_gates.iter().any(|g| g == GATE_ID),
        "an unsigned receipt must be recorded as unmeasured so merge admission \
         is withheld: {:?}",
        unsigned.unmeasured_gates
    );

    // The positive half. A gate hardwired to report absence still reports
    // absence after a real signing backend lands.
    let signed = with(GateStatus::Passed);
    assert!(
        !signed.unmeasured_gates.iter().any(|g| g == GATE_ID),
        "a measured attestation verdict must not be recorded as absent: {:?}",
        signed.unmeasured_gates
    );
}

/// P3, on the registry side. A gate that measures nothing has to say what would
/// end that, and may not simultaneously be declared able to report a pass.
#[test]
fn the_registry_records_what_this_gate_is_blocked_on() {
    let entry = anvil::fidelity::registry::AUDITED_GATES
        .iter()
        .find(|e| e.gate_id == GATE_ID)
        .expect("attestation_status must have a fidelity registry entry");

    assert!(
        !entry.fidelity.may_report_pass(),
        "attestation_status signs nothing but the registry declares it {}",
        entry.fidelity.label()
    );
    assert!(
        entry.blocked_on.is_some(),
        "the gap must be closable: name the signing backend that is missing"
    );
    for needle in ["in-toto", "DSSE", "SLSA"] {
        assert!(
            entry.reference.contains(needle),
            "the entry must cite the systems it is measured against; `{needle}` \
             is not in: {}",
            entry.reference
        );
    }
}

// ---------------------------------------------------------------------------
// P6. The receipt must not be swept into a commit on the pull request.
// ---------------------------------------------------------------------------

/// `certify.rs` wrote the receipt into the clone and then, a few lines later
/// and directly under a comment saying "NEVER push attestation receipts in a
/// loop", ran `git add -A` over that same clone. So did `fixer` and
/// `pr_self_healer`, over the same clone, from the same `ensure_repo_cloned`.
/// Only `QueueHealer` carried the exclusion.
///
/// The rule is crate-wide rather than scoped to the site that was reported,
/// because the defect is a *spelling*: four copies of `["add", "-A"]` drifted
/// because there were four copies. Staging now goes through
/// `git_manager::stage_excluding_receipts`, which hands back the built
/// `Command` -- a caller cannot take the arguments and drop half of them --
/// and the only production files permitted to write the whole-tree flag
/// themselves are the two listed here, each of which must carry an exclusion.
///
/// A scan is a proxy; what the command actually stages is measured for real by
/// `the_receipt_exclusion_pathspec_excludes_receipts_and_nothing_else`.
#[test]
fn no_production_site_spells_its_own_whole_tree_git_add() {
    /// The only production files allowed to spell `-A` themselves. Adding a
    /// file here is a deliberate act a reviewer sees; adding a staging site
    /// that reaches for `["add", "-A"]` is the mistake this pins shut.
    const MAY_SPELL_THEIR_OWN_STAGING: &[&str] = &[
        // The shared builder every other site calls.
        "src/git_manager/mod.rs",
        // Lane staging: excludes the receipts dir *and* the lane lease file,
        // through a different exec path (`LaneError`, not `anyhow`).
        "src/change_delivery/adapters/git_vcs.rs",
    ];

    let mut files = Vec::new();
    rs_files(&repo_root().join("src"), &mut files);
    assert!(files.len() > 50, "the src scan found almost nothing");

    let mut offenders = Vec::new();
    for (path, _) in &files {
        let rel = path
            .strip_prefix(repo_root())
            .expect("under repo root")
            .to_string_lossy()
            .replace('\\', "/");
        let src = code_only(&production_source(&rel));
        if !src.contains("\"-A\"") {
            continue;
        }
        if !MAY_SPELL_THEIR_OWN_STAGING.contains(&rel.as_str()) {
            offenders.push(format!("{rel}: stages a whole tree without going through git_manager::stage_excluding_receipts"));
        } else if !src.contains(":(exclude)") {
            offenders.push(format!(
                "{rel}: spells its own `git add -A` with no exclusion"
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "a staging site sweeps the clone Anvil writes its receipts into, so the \
         receipt is committed onto the pull request: {offenders:#?}"
    );
}

/// The pathspec itself, against a real repository. A source scan proves the
/// exclusion is spelled; only git proves it is spelled correctly, and a
/// pathspec that excluded everything would satisfy the scan while quietly
/// staging nothing at all.
#[test]
fn the_receipt_exclusion_pathspec_excludes_receipts_and_nothing_else() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    let git = |args: &[&str]| {
        let out = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).expect("git output is utf-8")
    };

    git(&["init", "--quiet"]);
    git(&["config", "user.email", "test@example.invalid"]);
    git(&["config", "user.name", "test"]);

    std::fs::create_dir_all(root.join(ANVIL_RECEIPTS_DIR)).expect("receipts dir");
    std::fs::write(root.join(ANVIL_RECEIPTS_DIR).join("pr-1.json"), "{}").expect("receipt");
    std::fs::create_dir_all(root.join("docs")).expect("docs dir");
    std::fs::write(root.join("docs/policy.md"), "# policy\n").expect("doc");

    let staging = stage_excluding_receipts(root);
    let out = staging
        .as_std()
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    git(&out.iter().map(String::as_str).collect::<Vec<_>>());

    let staged = git(&["diff", "--cached", "--name-only"]);
    let staged: Vec<&str> = staged.lines().collect();

    assert!(
        staged.contains(&"docs/policy.md"),
        "the exclusion must still stage the auto-synced governance files it \
         exists to commit: {staged:?}"
    );
    assert!(
        !staged.iter().any(|p| p.starts_with(ANVIL_RECEIPTS_DIR)),
        "Anvil's own receipt was staged onto the pull request: {staged:?}"
    );
}
