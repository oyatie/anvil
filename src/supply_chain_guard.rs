//! Gate 6: the dependency graph, matched against a live advisory database.
//!
//! # What this was
//!
//! Two regexes over the diff, matching a six-name list -- `node-ipc`,
//! `event-stream`, `flatmap-stream`, `net2`, `ws2_32`, `winapi` -- and only when
//! a manifest filename was among the changed files. It never opened `Cargo.lock`,
//! so it audited nothing that the pull request did not literally spell out, and
//! it could not see a transitive dependency at all. `winapi` on the list also
//! made it a source of spurious blocks: it is a maintained, widely used crate.
//!
//! `slsa_provenance_generated` was assigned the audit's own boolean, so the
//! report claimed provenance had been generated whenever nothing had been
//! flagged. Nothing generated any. That claim is gone rather than repaired:
//! the SLSA build track puts the signing party outside the build, so a check
//! running inside a pull request cannot self-attest above L1 whatever it emits.
//!
//! # What it is
//!
//! Resolve `Cargo.lock` -- the exact locked versions, transitive included --
//! and ask OSV.dev whether any of them carries an advisory. This is the oracle
//! `osv-scanner` uses; `cargo-audit` and `cargo-deny check advisories` read the
//! same lockfile against RustSec, which OSV mirrors.
//!
//! Whole-lockfile semantics, matching `cargo audit`: the gate reports what the
//! merged tree would ship, not only what this diff introduced.
//!
//! # What it still does not do
//!
//! No SBOM is produced (`syft`/`cargo-cyclonedx` are not invoked), no
//! provenance is signed, and no license or ban policy from `deny.toml` is
//! evaluated. The registry entry says so.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

pub mod osv_stream;
pub mod slsa_attestation;
pub use osv_stream::{OsvAdvisoryStream, VulnerablePackage};
pub use slsa_attestation::{SlsaAttestor, SlsaProvenanceBundle};

use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::GateStatus;

const GATE_ID: &str = "supply_chain_status";

/// One `[[package]]` table of a `Cargo.lock`.
///
/// Name and version only. `source` and `checksum` decide nothing here, and a
/// path or git dependency has no OSV ecosystem to be queried under anyway --
/// it is sent and comes back clean, which is the honest answer for a package
/// no advisory database indexes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedPackage {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
struct CargoLock {
    #[serde(default)]
    package: Vec<LockedPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyChainReport {
    pub status: GateStatus,
    /// Locked packages actually queried. Previously the count of files the
    /// pull request touched, which is not a number of packages.
    pub audited_packages: usize,
    pub vulnerable_packages: Vec<VulnerablePackage>,
    pub summary: String,
}

#[derive(Default)]
pub struct SupplyChainGuard;

impl SupplyChainGuard {
    pub fn new() -> Self {
        Self
    }

    /// Resolves every `[[package]]` entry in a `Cargo.lock`.
    ///
    /// An unreadable lockfile and a lockfile listing nothing are both errors,
    /// never an empty package list: an empty list runs the whole gate to
    /// completion and reports "no advisories", which is how `vec![]` made the
    /// zero-day gate unfailable.
    pub fn parse_lockfile(content: &str) -> Result<Vec<LockedPackage>, String> {
        let lock: CargoLock =
            toml::from_str(content).map_err(|e| format!("Cargo.lock could not be parsed: {e}"))?;
        if lock.package.is_empty() {
            return Err(
                "Cargo.lock lists no package entries, so no dependency graph was \
                        resolved to audit"
                    .to_string(),
            );
        }
        Ok(lock.package)
    }

    /// The gate's verdict, given a resolved lockfile and whatever the advisory
    /// query produced.
    ///
    /// Split out so the pass, the failure and every abstention are reachable
    /// from a test without a network request.
    pub fn report(
        packages: &[LockedPackage],
        queried: Result<Vec<VulnerablePackage>, String>,
    ) -> SupplyChainReport {
        let vulnerable = match queried {
            Ok(v) => v,
            Err(reason) => return Self::not_measured(reason, packages.len()),
        };

        let summary = if vulnerable.is_empty() {
            format!(
                "{} locked packages carry no advisory in the OSV database.",
                packages.len()
            )
        } else {
            format!(
                "{} of {} locked packages carry an OSV advisory: {}",
                vulnerable.len(),
                packages.len(),
                vulnerable
                    .iter()
                    .map(|v| v.describe())
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };

        SupplyChainReport {
            status: if vulnerable.is_empty() {
                GateStatus::Passed
            } else {
                GateStatus::Failed(summary.clone())
            },
            audited_packages: packages.len(),
            vulnerable_packages: vulnerable,
            summary,
        }
    }

    /// The gate's answer when no audit could be performed.
    ///
    /// Absent evidence, never a pass and never an accusation (invariant I1).
    pub fn not_measured(reason: String, audited_packages: usize) -> SupplyChainReport {
        SupplyChainReport {
            status: GateStatus::NotMeasured {
                gate_id: GATE_ID.to_string(),
                reason: reason.clone(),
            },
            audited_packages,
            vulnerable_packages: Vec::new(),
            summary: reason,
        }
    }

    /// Audits the working tree's locked dependency graph against OSV.dev.
    pub async fn audit_supply_chain(
        &self,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<SupplyChainReport> {
        info!(
            "Running SupplyChainGuard dependency security audit on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let path = repo_dir.join("Cargo.lock");
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                // Not `path.display()`: that put an ephemeral absolute runner
                // path onto a scorecard with a pinned size budget.
                return Ok(Self::not_measured(
                    format!("no Cargo.lock in the working tree: {e}"),
                    0,
                ));
            }
        };

        let packages = match Self::parse_lockfile(&content) {
            Ok(p) => p,
            Err(reason) => return Ok(Self::not_measured(reason, 0)),
        };

        let queried = OsvAdvisoryStream::query_batch(&packages).await;
        Ok(Self::report(&packages, queried))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = r#"
version = 4

[[package]]
name = "time"
version = "0.1.44"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;

    #[test]
    fn a_lockfile_resolves_to_its_locked_versions() {
        let p = SupplyChainGuard::parse_lockfile(LOCK).expect("parses");
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].version, "0.1.44");
    }

    #[test]
    fn an_advisory_against_a_locked_version_fails_the_gate() {
        let p = SupplyChainGuard::parse_lockfile(LOCK).expect("parses");
        let report = SupplyChainGuard::report(
            &p,
            Ok(vec![VulnerablePackage {
                name: "time".to_string(),
                version: "0.1.44".to_string(),
                advisory_ids: vec!["RUSTSEC-2020-0071".to_string()],
            }]),
        );
        assert!(matches!(report.status, GateStatus::Failed(_)));
    }
}
