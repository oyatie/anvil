//! Lane `cosign-attestation`: the gate that published a certificate nobody
//! issued and a transparency-log entry nobody logged.
//!
//! # The defect, restated from source
//!
//! `SigstoreAttestor::sign_artifact_digest` in `cosign_signer/sigstore_attestor.rs`
//! returned a struct literal. Its certificate chain was one hardcoded PEM
//! string with an elided body, its Rekor entry id was formatted from the first
//! eight characters of the artefact digest, and its validity flag was the
//! constant `true`. No OIDC token was requested, no Fulcio certificate was
//! issued, no signature was computed over anything, and nothing was submitted
//! to Rekor. `CosignProvenanceSigner::generate_cosign_attestation` returned
//! that constant as its verdict, so the gate was unfailable, and `evaluator.rs`
//! turned it into `GateStatus::Passed` for the scorecard to publish as a
//! signing attestation.
//!
//! That is the worst class of claim in this repository: a published assertion
//! of cryptographic provenance with no cryptography behind it. A reader who
//! trusts it believes the artefact can be traced to an identity and to an
//! append-only log entry, and neither exists.
//!
//! # What the oracle actually does
//!
//! Sigstore keyless signing: request an OIDC identity token, generate an
//! ephemeral keypair, exchange a CSR plus that token at Fulcio for a
//! short-lived X.509 certificate binding the key to the workflow identity,
//! sign the artefact digest, discard the key, and upload signature and
//! certificate to Rekor, which returns an entry id, a log index, a signed
//! entry timestamp and an inclusion proof. Verification re-checks the chain to
//! the Fulcio root, the certificate identity policy, the signature over the
//! digest, and the inclusion proof against a trusted checkpoint. Anvil has none
//! of that: no OIDC client, no HTTP client for Fulcio or Rekor, no signing key,
//! no X.509 or ECDSA dependency in `Cargo.toml`, and it invokes no external
//! signing binary.
//!
//! # Premortem -- how this fix can already have failed
//!
//! P1. The status is relabelled `NotMeasured` but the fabricated bundle is
//!     kept as a fixture, so the next caller signs with it again.
//!     -> `no_fabricated_signing_material_survives_in_the_module` and
//!        `no_fabricated_signing_material_survives_anywhere_in_src`.
//! P2. The guard is made honest and the *wiring* throws the honesty away:
//!     `evaluator.rs` rebuilds `GateStatus` from a boolean, exactly the pattern
//!     already corrected for six other gates. With a false boolean that rebuild
//!     publishes `Failed`, accusing every pull request in the fleet of a
//!     signing failure -- absent evidence turned into an accusation, the
//!     symmetric half of I1.
//!     -> `the_evaluator_reads_the_cosign_verdict_instead_of_rebuilding_it`
//!        and `an_absent_signing_backend_is_not_an_accusation`.
//! P3. Everything is named `NotMeasured` and nothing else changes, so the gate
//!     is cosmetically honest and mechanically inert: the id is not recorded,
//!     merge admission is not withheld, and a measured verdict -- the one a
//!     real Sigstore backend will produce -- would be swallowed the same way.
//!     -> `the_cosign_gate_id_is_recorded_only_while_it_is_unmeasured`, which
//!        asserts BOTH directions, and
//!        `the_registry_records_what_this_gate_is_blocked_on`.
//! P4. The fidelity registry keeps publishing its quotation of the deleted
//!     literal. `gap` is a `&'static str`; the compiler is silent when the code
//!     it quotes disappears.
//!     -> covered by the whole-`src` scan, which reaches `registry.rs`, and by
//!        `tests/fidelity_registry_citations_test.rs`.

use anvil::cosign_signer::CosignProvenanceSigner;
use anvil::pre_merge_guard::{GateStatus, PreMergeCertificationReport};
use std::path::{Path, PathBuf};

const GATE_ID: &str = "cosign_status";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every `.rs` file under `dir`, as (path, contents).
fn rs_files(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{} must be readable: {e}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(rs_files(&path));
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let body = std::fs::read_to_string(&path).expect("source file is readable");
            out.push((path, body));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The absent path: nothing signs, so nothing is claimed.
// ---------------------------------------------------------------------------

/// False Green prevention. Anvil drives no signing backend, so the only honest
/// answer is that no attestation exists -- not a pass built on a literal.
#[test]
fn an_absent_signing_backend_is_not_a_signed_artefact() {
    let report = CosignProvenanceSigner::new().evaluate_without_signing_backend();

    assert_eq!(
        report.status.unmeasured_gate_id(),
        Some(GATE_ID),
        "no OIDC token, no Fulcio certificate and no Rekor entry exist, so the \
         gate must report NotMeasured naming itself"
    );
    assert!(
        !matches!(report.status, GateStatus::Passed),
        "an unsigned artefact is not a signed one"
    );

    let GateStatus::NotMeasured { reason, .. } = &report.status else {
        unreachable!("asserted above");
    };
    // The reason is published verbatim on the scorecard, so it has to name the
    // capability that is missing rather than merely say "not measured".
    for needle in ["Fulcio", "Rekor"] {
        assert!(
            reason.contains(needle),
            "the reason must name the absent capability so a reader can close \
             the gap; it says: {reason}"
        );
    }
}

/// False Red prevention (I1, symmetric half). An artefact nobody tried to sign
/// has not failed to sign. Reporting `Failed` here would accuse every pull
/// request in the fleet of a signing failure nobody can reproduce.
#[test]
fn an_absent_signing_backend_is_not_an_accusation() {
    let report = CosignProvenanceSigner::new().evaluate_without_signing_backend();

    assert!(
        !matches!(
            report.status,
            GateStatus::Failed(_) | GateStatus::Errored(_)
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

// ---------------------------------------------------------------------------
// The fabrication is deleted, not parked.
// ---------------------------------------------------------------------------

/// P1. "Moved, not removed" is the usual shape of this failure: the struct
/// literal is pushed into a `const`, a `Default` impl, a sibling file or a
/// test fixture that production code can still reach. A comment cannot stop
/// that; only a scan can.
#[test]
fn no_fabricated_signing_material_survives_in_the_module() {
    let dir = repo_root().join("src/cosign_signer");
    let files = rs_files(&dir);
    assert!(!files.is_empty(), "the module must still exist");

    // Field and item names that only ever existed to carry invented evidence.
    let banned = [
        "certificate_chain",
        "rekor_entry_uuid",
        "oidc_issuer",
        "is_valid",
        "sign_artifact_digest",
        "SigstoreAttestor",
        "CosignSignatureBundle",
    ];
    let mut found: Vec<String> = Vec::new();
    for (path, body) in &files {
        for needle in banned {
            if body.contains(needle) {
                found.push(format!("{}: {needle}", path.display()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "the fabricated attestation must be deleted, not kept as a fixture a \
         caller can reach again: {found:?}"
    );

    // A digest this module cannot sign has nothing to derive from it: any value
    // formed here would be another invented identifier.
    for (path, body) in &files {
        assert!(
            !body.contains("format!"),
            "{} formats a value out of inputs it cannot verify; nothing here \
             may derive published evidence",
            path.display()
        );
    }
}

/// The same needles across the whole crate, because deleting a file is not the
/// same as deleting a claim: `fidelity/registry.rs` quoted the PEM literal in a
/// `&'static str` that renders onto the pull-request scorecard, and the
/// compiler says nothing when the code it quotes disappears.
#[test]
fn no_fabricated_signing_material_survives_anywhere_in_src() {
    let mut found: Vec<String> = Vec::new();
    for (path, body) in rs_files(&repo_root().join("src")) {
        for needle in ["BEGIN CERTIFICATE", "rekor-log-uuid"] {
            if body.contains(needle) {
                found.push(format!("{}: {needle}", path.display()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "an invented certificate or transparency-log id is still published \
         somewhere in the crate: {found:?}"
    );
}

// ---------------------------------------------------------------------------
// The wiring: a verdict the guard owns must reach the report unchanged.
// ---------------------------------------------------------------------------

/// P2. The bug this repository has already shipped six times: the guard is made
/// honest and `evaluator.rs` rebuilds the status from a boolean, so
/// `NotMeasured` becomes `Failed` on the way to the report. A guard-level test
/// cannot see it, and `evaluate_pre_merge_gates` takes sixty-nine reports, so
/// the wiring is checked where it is written.
#[test]
fn the_evaluator_reads_the_cosign_verdict_instead_of_rebuilding_it() {
    let src = std::fs::read_to_string(repo_root().join("src/pre_merge_guard/evaluator.rs"))
        .expect("evaluator.rs must exist");
    let code: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        code.contains("let cosign_status = cosign_report.status.clone();"),
        "the evaluator must carry the gate's own verdict through unchanged"
    );
    assert!(
        !code.contains("cosign_report.passed"),
        "rebuilding the status from a boolean discards NotMeasured and \
         publishes a fabricated accusation instead"
    );
    assert!(
        !code.contains("Cosign keyless OIDC transparency log signing failed."),
        "no signing was attempted, so no signing failure may be reported"
    );
    // The absence is the guard's finding, not the wiring's opinion. An
    // evaluator that minted `NotMeasured` here would report absence for ever,
    // including after a real Sigstore backend starts producing verdicts.
    assert!(
        !code.contains("gate_id: \"cosign_status\""),
        "the evaluator must not mint this gate's NotMeasured itself; the guard \
         owns the verdict so a real backend can replace it"
    );
}

// ---------------------------------------------------------------------------
// The counterpart: NotMeasured has to do something, in both directions.
// ---------------------------------------------------------------------------

/// P3. Naming a gate `NotMeasured` is only honest if the report acts on it, and
/// only useful if a real measurement can still get through. Both directions are
/// asserted from one fixture, so the gate cannot pass by measuring nothing and
/// cannot be pinned to `NotMeasured` for ever either.
#[test]
fn the_cosign_gate_id_is_recorded_only_while_it_is_unmeasured() {
    let base = PreMergeCertificationReport::unmeasured("fixture baseline");
    let names: Vec<&'static str> = base.named_statuses().into_iter().map(|(n, _)| n).collect();
    assert!(
        names.contains(&GATE_ID),
        "the gate id must match the report field name, or unmeasured_gates \
         names a gate nobody can look up"
    );

    let with = |cosign: GateStatus| {
        let outcomes: Vec<(&str, GateStatus)> = names
            .iter()
            .map(|n| {
                let s = if *n == GATE_ID {
                    cosign.clone()
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

    let unsigned = with(
        CosignProvenanceSigner::new()
            .evaluate_without_signing_backend()
            .status,
    );
    assert!(
        unsigned.unmeasured_gates.iter().any(|g| g == GATE_ID),
        "an unsigned artefact must be recorded as unmeasured so merge \
         admission is withheld: {:?}",
        unsigned.unmeasured_gates
    );

    // The positive half. When a real Sigstore backend supplies a verdict the
    // report must carry it: a gate hardwired to report absence is a gate that
    // will still report absence after the backend lands.
    let signed = with(GateStatus::Passed);
    assert!(
        !signed.unmeasured_gates.iter().any(|g| g == GATE_ID),
        "a measured cosign verdict must not be recorded as absent: {:?}",
        signed.unmeasured_gates
    );
}

/// P3, on the registry side. A gate that reports no measurement has to say what
/// would end that, and may not simultaneously be declared able to report a
/// pass. Both halves are the registry's contract with `unmeasured_gates`.
#[test]
fn the_registry_records_what_this_gate_is_blocked_on() {
    let entry = anvil::fidelity::registry::AUDITED_GATES
        .iter()
        .find(|e| e.gate_id == GATE_ID)
        .expect("cosign_status must have a fidelity registry entry");

    assert!(
        !entry.fidelity.may_report_pass(),
        "cosign_status measures nothing but the registry declares it {}",
        entry.fidelity.label()
    );
    assert!(
        entry.blocked_on.is_some(),
        "the gap must be closable: name the signing backend that is missing"
    );
    assert!(
        entry.reference.contains("Sigstore"),
        "the entry must cite the system it is measured against"
    );
}
