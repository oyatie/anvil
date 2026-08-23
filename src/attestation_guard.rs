//! The lane receipt — a record of what was certified, not an attestation of it.
//!
//! # What was here
//!
//! `AttestationReport::is_attested` had exactly one production construction
//! site and it was the literal `true`. Behind the gate titled "Cryptographic
//! Provenance Attestation" sat `serde_json::to_string_pretty` and `fs::write`
//! and nothing else: no digest over a named subject, no signing key, no
//! envelope, no link to any previous receipt, and no verifier anywhere in the
//! crate. `evaluator.rs` widened that literal into `GateStatus::Passed`, so the
//! gate passed on every pull request, and its `Failed` arm was unreachable --
//! the only non-`true` path out of here is `Err`, and `certify.rs` propagates
//! that with `?`, aborting certification before the matrix is built. The word
//! "cryptographic" appeared in the summary string and in the row title, and
//! nowhere in the code.
//!
//! # What the claim requires
//!
//! An in-toto v1 statement names a `subject` -- an artefact and its digest --
//! and a `predicate` recording how it was produced; the spec is explicit that
//! subject artefacts are matched purely by digest, so the digest of the real
//! bytes is the binding and the name is only a label. That statement is wrapped
//! in a DSSE envelope, whose signature covers a Pre-Authentication Encoding of
//! the payload together with its declared payload type, so a signature cannot
//! be replayed under another type. `cosign attest` signs the envelope with a
//! key bound to an identity -- under the Sigstore keyless flow, a short-lived
//! Fulcio certificate carrying an OIDC identity in its SubjectAlternativeName
//! -- and uploads it to the Rekor transparency log, which returns an inclusion
//! proof against a signed checkpoint.
//!
//! SLSA sets the bar and states the consequence of missing it in its own words:
//! provenance may be incomplete and unsigned at Build L1, and Build L1 is
//! summarised as trivial to bypass or forge, useful to prevent mistakes and
//! nothing more. Build L2 requires a build platform on dedicated
//! infrastructure, not an individual's workstation, with the provenance tied to
//! that infrastructure through a digital signature. Provenance whose signature
//! a verifier cannot chain to a root of trust it configured out of band falls
//! back to L1.
//!
//! The property all of that exists to produce is third-party verifiability:
//! verifying needs a public key the verifier obtained independently of the
//! producer, and forging needs a private key the producer cannot claim not to
//! have used. NIST SP 800-175B classes digital signatures, and only digital
//! signatures, as supporting non-repudiation -- whether a third party can be
//! convinced about who was the source of the information.
//!
//! Anvil has none of the ingredients: no signing key, no X.509 or ECDSA
//! dependency, no HTTP client to reach a log, and no verifier.
//!
//! # Why there is no hash chain here
//!
//! The tempting middle path is a hash-chained receipt log -- each receipt
//! committing to the digest of its predecessor, checked with `sha2`, which this
//! crate already carries for webhook HMAC. It was rejected twice over.
//!
//! It proves nothing against the adversary that matters. The chain rule would
//! be public and unkeyed, so rewriting a receipt and recomputing the tail is
//! not an attack on it -- it is the write path, available to anyone who can
//! edit a receipt, which is the producer, the party whose claims an attestation
//! exists to make checkable. NIST puts the limit plainly: used without a secret
//! key there is no assurance that the data has not been altered by an adversary
//! and a new hash value computed, and the unkeyed hash is sanctioned alone only
//! where the risk is a degraded transmission medium. Keying it with HMAC would
//! not rescue it, because the key would live in the process writing the
//! receipts, so everyone able to verify would be equally able to forge and no
//! verifier could convince a third party of anything. Nor would the result be
//! tamper-evident in the sense the literature uses: Crosby and Wallach define
//! that property against an auditing process in which at least one auditor is
//! assumed to be incorruptible, and a log its own producer checks has no such
//! auditor. Shipping it would leave this gate publishing a pass under a name
//! that still reads as verifiable provenance.
//!
//! And there is no log here to chain. Receipts are written into a per-run clone
//! at `.anvil/receipts/pr-{n}-attestation.json`, one file per pull request,
//! overwritten in place -- once by `certify.rs` with a pending verdict and
//! again by `review.rs` with the final one. A chain over a single file that is
//! overwritten twice per run is a hash of itself.
//!
//! # What is here now
//!
//! The receipt is still written, because a record of what was certified is
//! useful on its own terms, and the stamper is the seam a real signer plugs
//! into. What is gone is the claim: the gate reports `GateStatus::NotMeasured`
//! naming the absent capabilities, so merge admission is withheld through
//! `unmeasured_gates` rather than granted by a literal. Not `Failed`, which
//! would accuse every pull request in the fleet of a signing failure nobody
//! attempted (invariant I1 cuts both ways).
//!
//! `AttestationReport::status` is public and the evaluator reads it unchanged,
//! so wiring a real signer later needs no change to the report, the matrix or
//! the scorecard: it supplies a verdict here and the verdict is what is
//! published.

use anyhow::Result;

/// Where lane receipts live: agent-neutral, alongside `.anvil/policy.json`.
///
/// Deliberately NOT inside `.claude/`, `.cursor/`, `.grok/` or any other
/// agent-specific directory -- those hold regenerable tool config and are being
/// drained of load-bearing content.
pub const ANVIL_RECEIPTS_DIR: &str = ".anvil/receipts";

/// Paths Anvil writes into somebody else's checkout. A commit Anvil pushes
/// carries what the change produced, never Anvil's own bookkeeping
/// (`.cursor/receipts` is the legacy location, still present in older
/// checkouts).
const ANVIL_OWNED_PATHS: &[&str] = &[ANVIL_RECEIPTS_DIR, ".cursor/receipts"];

use crate::pre_merge_guard::report::GateStatus;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;
use tracing::info;

/// Matches the `PreMergeCertificationReport` field name, so `unmeasured_gates`
/// names a gate a reader can look up in the fidelity registry.
const GATE_ID: &str = "attestation_status";

/// What must exist before any provenance claim can be made. Published verbatim
/// as the `NotMeasured` reason, so it names the capabilities that are absent
/// rather than merely saying nothing was measured.
const NO_PROVENANCE_BACKEND: &str = "no provenance backend is configured: the receipt is serialised and written to disk, and \
     nothing else -- no in-toto subject digest is computed over the artefact, no DSSE envelope \
     is built, no signature is produced because the crate holds no signing key, and no \
     transparency log records it. Nothing here could be checked by a third party who does not \
     already trust the producer";

/// Arguments that stage a change but leave Anvil's own receipts out of it.
///
/// One spelling for both staging sites. `certify.rs` wrote a receipt into the
/// clone and then staged it with a bare sweep of the whole tree, sixteen lines
/// under a comment saying it must never do that, while `QueueHealer` had the
/// exclusion; the two drifted precisely because each spelled its own arguments.
pub fn git_add_args_excluding_receipts() -> Vec<String> {
    let mut args = vec![
        "add".to_string(),
        "-A".to_string(),
        "--".to_string(),
        ".".to_string(),
    ];
    for p in ANVIL_OWNED_PATHS {
        args.push(format!(":(exclude){}", p));
    }
    args
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaneReceipt {
    pub schema_version: String,
    pub commit_sha: String,
    pub pr_number: u64,
    /// What wrote this file. It was `attestation_engine`, which asserted in the
    /// published JSON exactly what the gate above it has stopped asserting.
    pub recorded_by: String,
    pub timestamp_utc: String,
    pub gates_verified: Vec<String>,
    pub verdict: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationReport {
    /// The gate's own verdict, read unchanged by `evaluator.rs`. This replaced
    /// an `is_attested: bool` whose only production value was `true`.
    pub status: GateStatus,
    pub stamped_receipt_path: Option<String>,
    pub summary: String,
}

pub struct AttestationGuard;

impl Default for AttestationGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl AttestationGuard {
    pub fn new() -> Self {
        Self
    }

    /// Verdict written at stamp time, before the gate matrix has been evaluated.
    ///
    /// The receipt previously hardcoded "CERTIFIED_READY" here, while being
    /// stamped *before* `evaluate_pre_merge_gates` ran -- asserting a
    /// certification that had not been computed. Invariant I2: never report a
    /// value you did not measure.
    pub const VERDICT_PENDING: &'static str = "PENDING_CERTIFICATION";

    /// Writes a lane receipt into `.anvil/receipts/`, and reports that nothing
    /// about it is attested.
    ///
    /// Receipts previously landed in `.cursor/receipts/`. A durable record is
    /// load-bearing and agent-neutral; putting it inside a tool's dot directory
    /// couples it to one vendor's config layout, and those directories are
    /// being drained rather than extended. Anvil was itself the largest single
    /// producer of load-bearing content in an agent directory.
    pub async fn stamp_lane_receipt(
        &self,
        repo_dir: &Path,
        repo: &str,
        pr_number: u64,
        head_sha: &str,
        verdict: &str,
        gates_verified: Vec<String>,
    ) -> Result<AttestationReport> {
        info!(
            "Recording lane receipt for {}#{} (SHA: {})...",
            repo, pr_number, head_sha
        );

        let receipts_dir = repo_dir.join(ANVIL_RECEIPTS_DIR);
        if !receipts_dir.exists() {
            let _ = fs::create_dir_all(&receipts_dir).await;
        }

        let receipt = LaneReceipt {
            // Bumped alongside the `attestation_engine` -> `recorded_by`
            // rename, so a reader holding an older receipt can tell the two
            // shapes apart.
            schema_version: "2.0.0".to_string(),
            commit_sha: head_sha.to_string(),
            pr_number,
            recorded_by: "Oyatie Autonomous Engineering Pipeline".to_string(),
            timestamp_utc: chrono::Utc::now().to_rfc3339(),
            // Both fields are now supplied by the caller from actual results,
            // rather than being a fixed list asserting gates that had not run.
            gates_verified,
            verdict: verdict.to_string(),
        };

        let filename = format!("pr-{}-attestation.json", pr_number);
        let target_path = receipts_dir.join(&filename);
        let receipt_json = serde_json::to_string_pretty(&receipt)?;

        fs::write(&target_path, &receipt_json).await?;

        let relative_path = format!("{}/{}", ANVIL_RECEIPTS_DIR, filename);
        info!("Recorded lane receipt at {}", relative_path);

        Ok(AttestationReport {
            status: GateStatus::NotMeasured {
                gate_id: GATE_ID.to_string(),
                reason: NO_PROVENANCE_BACKEND.to_string(),
            },
            stamped_receipt_path: Some(relative_path.clone()),
            summary: format!("Lane receipt recorded at `{}`", relative_path),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stamp_receipt() {
        let guard = AttestationGuard::new();
        let temp_dir = tempfile::tempdir().expect("tempdir");

        let res = guard
            .stamp_lane_receipt(
                temp_dir.path(),
                "oyatie/console",
                106,
                "abcdef1234567890",
                AttestationGuard::VERDICT_PENDING,
                Vec::new(),
            )
            .await
            .expect("Stamps receipt");

        // The file is written; the claim about it is withheld. Both halves,
        // because either one alone is a different defect.
        assert_eq!(res.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(res.stamped_receipt_path.is_some());

        let receipt_file = temp_dir.path().join(
            "_anvil_receipts_/pr-106-attestation.json"
                .replace("_anvil_receipts_", ANVIL_RECEIPTS_DIR),
        );
        assert!(receipt_file.exists());
    }

    /// The receipt must record the verdict it was given, never a fixed value.
    /// It previously hardcoded "CERTIFIED_READY" while being stamped before the
    /// gate matrix ran, asserting a certification nothing had computed (I2).
    #[tokio::test]
    async fn receipt_records_the_supplied_verdict_not_a_hardcoded_one() {
        let guard = AttestationGuard::new();

        for (verdict, gates) in [
            (AttestationGuard::VERDICT_PENDING, Vec::new()),
            ("BLOCKED_NOT_CERTIFIED", vec!["gate-0".to_string()]),
            (
                "CERTIFIED_READY",
                vec!["gate-0".to_string(), "gate-1".to_string()],
            ),
        ] {
            let temp_dir = tempfile::tempdir().expect("tempdir");
            guard
                .stamp_lane_receipt(
                    temp_dir.path(),
                    "oyatie/console",
                    7,
                    "deadbeef",
                    verdict,
                    gates.clone(),
                )
                .await
                .expect("stamps");

            let body = std::fs::read_to_string(
                temp_dir.path().join(
                    "_anvil_receipts_/pr-7-attestation.json"
                        .replace("_anvil_receipts_", ANVIL_RECEIPTS_DIR),
                ),
            )
            .expect("receipt readable");
            let parsed: serde_json::Value = serde_json::from_str(&body).expect("valid json");

            assert_eq!(parsed["verdict"], verdict, "verdict must round-trip");
            assert_eq!(
                parsed["gates_verified"].as_array().map(|a| a.len()),
                Some(gates.len()),
                "gate list must reflect what was supplied"
            );
        }
    }

    /// Guards the specific regression: a freshly stamped receipt, before any
    /// gate has run, must not claim certification.
    #[tokio::test]
    async fn pending_stamp_does_not_claim_certification() {
        assert_ne!(AttestationGuard::VERDICT_PENDING, "CERTIFIED_READY");
        let guard = AttestationGuard::new();
        let temp_dir = tempfile::tempdir().expect("tempdir");
        guard
            .stamp_lane_receipt(
                temp_dir.path(),
                "oyatie/console",
                9,
                "cafe",
                AttestationGuard::VERDICT_PENDING,
                Vec::new(),
            )
            .await
            .expect("stamps");
        let body = std::fs::read_to_string(
            temp_dir.path().join(
                "_anvil_receipts_/pr-9-attestation.json"
                    .replace("_anvil_receipts_", ANVIL_RECEIPTS_DIR),
            ),
        )
        .expect("readable");
        assert!(!body.contains("CERTIFIED_READY"));
    }

    /// The exclusion has to name every path Anvil owns, not just the current
    /// one: a checkout carried over from before the move still has the legacy
    /// directory, and staging that is the same defect.
    #[test]
    fn the_add_pathspec_excludes_every_path_anvil_owns() {
        let args = git_add_args_excluding_receipts();
        assert_eq!(&args[..4], &["add", "-A", "--", "."]);
        for p in ANVIL_OWNED_PATHS {
            assert!(
                args.contains(&format!(":(exclude){}", p)),
                "{p} would be staged into somebody else's commit: {args:?}"
            );
        }
    }
}

#[cfg(test)]
mod agent_dir_tests {
    use super::*;

    /// Receipts are load-bearing records and must never live inside a
    /// tool-specific agent directory. Anvil was previously the largest producer
    /// of load-bearing content in one.
    #[test]
    fn receipts_do_not_live_in_an_agent_directory() {
        for forbidden in [
            ".claude", ".cursor", ".grok", ".codex", ".agents", ".gemini", ".aider",
        ] {
            assert!(
                !ANVIL_RECEIPTS_DIR.starts_with(forbidden),
                "receipts must not live under {forbidden}: {ANVIL_RECEIPTS_DIR}"
            );
        }
        assert!(ANVIL_RECEIPTS_DIR.starts_with(".anvil/"));
    }

    /// No agent directory is used to CONSTRUCT a path here.
    ///
    /// Checks for path construction specifically, not prose: the doc comment
    /// above legitimately names the old location to explain the move. The
    /// repo-wide equivalent is a pipeline gate (plan section 23.6), since a unit
    /// test cannot portably read the whole tree.
    #[test]
    fn no_path_is_constructed_under_an_agent_directory() {
        let src = include_str!("attestation_guard.rs");
        for forbidden in [".claude/", ".cursor/", ".grok/", ".codex/"] {
            let constructed = format!("join(\"{}", forbidden);
            assert!(
                !src.contains(&constructed),
                "a path is constructed under {forbidden}"
            );
        }
    }
}
