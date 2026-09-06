//! Cosign / Sigstore provenance — the gate that published a certificate nobody
//! issued.
//!
//! # What was here
//!
//! A sibling file held an attestor whose doc comment claimed it signed an
//! artefact digest through the Sigstore Fulcio OIDC keyless flow and recorded
//! it in the Rekor transparency log. It returned a struct literal: a
//! hardcoded PEM string with an elided body as the certificate chain, a
//! transparency-log id formatted from the first eight characters of the
//! artefact digest, and a validity flag fixed at `true`. No OIDC token was
//! requested, no certificate was issued, no signature was computed and nothing
//! was submitted to any log. `CosignProvenanceSigner` returned that constant,
//! `evaluator.rs` turned it into `GateStatus::Passed`, and the scorecard
//! published it as a signing attestation.
//!
//! Every other fabricated gate in this repository published a number that was
//! merely wrong. This one published a claim of cryptographic provenance — that
//! the artefact traces to an identity and to an append-only log entry — with
//! nothing whatsoever behind it.
//!
//! # What the aspiration requires
//!
//! Keyless signing needs an OIDC identity token, an ephemeral keypair, a CSR
//! exchanged at Fulcio for a short-lived certificate binding that key to the
//! workflow identity, a signature over the artefact digest, and an upload to
//! Rekor returning an entry id, a log index, a signed entry timestamp and an
//! inclusion proof. Verification re-checks the chain to the Fulcio root, the
//! certificate identity policy, the signature, and the inclusion proof against
//! a trusted checkpoint.
//!
//! Anvil has none of it: no OIDC client, no HTTP client for Fulcio or Rekor, no
//! signing key, no X.509 or ECDSA dependency, and it runs no external signing
//! binary.
//!
//! # What is here now
//!
//! The attestor is deleted rather than retained as a fixture — unlike
//! `StatisticalCanaryEngine` or `StackedDagManager`, which are honest
//! computations over caller-supplied data and are the seam a real source plugs
//! into, it computed nothing and there was no honest half to keep.
//!
//! With no signing backend the gate reports `GateStatus::NotMeasured` naming
//! what is absent. Not `Passed`, which is the defect being removed, and not
//! `Failed`, which would accuse every pull request in the fleet of a signing
//! failure that was never attempted (invariant I1 cuts both ways).
//!
//! `CosignReport::status` is public and the evaluator reads it unchanged, so
//! wiring a real Sigstore backend later needs no change to the report, the
//! matrix or the scorecard: it supplies a verdict here and the verdict is what
//! is published.

use serde::{Deserialize, Serialize};

use crate::pre_merge_guard::report::GateStatus;

/// Matches the `PreMergeCertificationReport` field name, so `unmeasured_gates`
/// names a gate a reader can look up in the fidelity registry.
const GATE_ID: &str = "cosign_status";

/// What must exist before any provenance claim can be made at all. Published
/// verbatim as the `NotMeasured` reason.
const MISSING_SIGNING_BACKEND: &str = "no Sigstore signing backend is configured: no OIDC identity token is requested, no Fulcio \
     certificate is issued, no signature is computed over the artefact digest and no Rekor \
     inclusion proof is fetched, so this artefact carries no attestation to verify";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CosignReport {
    pub status: GateStatus,
}

#[derive(Debug, Clone, Default)]
pub struct CosignProvenanceSigner;

impl CosignProvenanceSigner {
    pub fn new() -> Self {
        Self
    }

    /// The review pipeline's entry point. Nothing here signs, so nothing is
    /// claimed; see the module docs.
    pub fn evaluate_without_signing_backend(&self) -> CosignReport {
        CosignReport {
            status: GateStatus::NotMeasured {
                gate_id: GATE_ID.to_string(),
                reason: MISSING_SIGNING_BACKEND.to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unsigned_artefact_carries_no_attestation() {
        let report = CosignProvenanceSigner::new().evaluate_without_signing_backend();
        assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(
            report.status.is_acceptable(),
            "absent evidence is not an accusation"
        );
    }
}
