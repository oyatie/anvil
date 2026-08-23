//! The OSV.dev advisory database, reached the only way Anvil reaches anything
//! off-box: a bounded `curl`.
//!
//! # Why this file existed and did nothing
//!
//! `query_package` already built a real POST to `https://api.osv.dev/v1/query`
//! and ran it through `crate::exec`. It had no caller outside this module. The
//! gate that was supposed to use it, `supply_chain_status`, regex-scanned the
//! diff for six package names instead, and the only exercised function here was
//! the payload builder. A working client was written and left dead.
//!
//! # What changed
//!
//! One query per package is the wrong unit: a 162-package lockfile is 162 round
//! trips inside a per-PR gate. OSV publishes `/v1/querybatch` for exactly this,
//! so the single-package endpoint is gone and `build_batch_payload` sends the
//! whole lockfile in chunks of `OSV_BATCH_SIZE`.
//!
//! Batch results are minimal by design -- OSV returns `id` and `modified` per
//! vulnerability and nothing else -- so nothing here reports a severity it was
//! not sent. `results` is positional against `queries`, which is why a length
//! mismatch is an error rather than a shorter answer: a shifted list attributes
//! advisories to the wrong crates and silently clears the tail.
//!
//! # Failure is never a pass
//!
//! Every function returns `Result<_, String>` and every `Err` becomes
//! `GateStatus::NotMeasured` at the call site. curl missing, DNS down, a 429, a
//! proxy's HTML error page, a body cut off mid-object: none of them may be read
//! as "no advisories".

use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::info;

use super::LockedPackage;

/// OSV's batched query endpoint. The single-package `/v1/query` is not used:
/// one request per locked package is hundreds of round trips per pull request.
pub const OSV_BATCH_URL: &str = "https://api.osv.dev/v1/querybatch";

/// OSV's ecosystem key for crates.io packages. Any other string matches no
/// Rust advisory and returns an empty, reassuring result.
pub const OSV_ECOSYSTEM: &str = "crates.io";

/// Packages per request.
///
/// OSV documents pagination thresholds for batches (>1,000 vulnerabilities in
/// one query, >3,000 across a batch) but publishes no maximum query count, so
/// this is chosen to stay well inside the documented limits rather than to
/// match a stated one. A 162-package lockfile is one request; a 1,500-package
/// monorepo is three.
pub const OSV_BATCH_SIZE: usize = 500;

/// Anvil's kill deadline for one batch request.
///
/// Paired with `CURL_MAX_TIME` below the same way `agy_print_timeout_arg` pairs
/// with `ExecClass::Model`: curl gives up first and says why, and this only
/// fires if curl itself wedges.
pub const OSV_BUDGET: Duration = Duration::from_secs(20);

/// curl's own deadline, in seconds, five below `OSV_BUDGET`.
const CURL_MAX_TIME: &str = "15";

/// The program the transport runs. Injectable at `post_json` so the failure
/// modes can be tested against programs that are not curl, without a network.
const CURL: &str = "curl";

#[derive(Debug, Clone, Serialize)]
struct OsvBatchQuery {
    queries: Vec<OsvQuery>,
}

#[derive(Debug, Clone, Serialize)]
struct OsvQuery {
    package: OsvPackage,
    version: String,
}

#[derive(Debug, Clone, Serialize)]
struct OsvPackage {
    name: String,
    ecosystem: String,
}

#[derive(Debug, Clone, Deserialize)]
struct OsvBatchResponse {
    results: Vec<OsvResult>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct OsvResult {
    /// Absent entirely for a clean package; OSV sends `{}`, not `{"vulns":[]}`.
    #[serde(default)]
    vulns: Vec<OsvVulnerability>,
}

#[derive(Debug, Clone, Deserialize)]
struct OsvVulnerability {
    id: String,
}

/// One locked package and every advisory OSV holds against that exact version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulnerablePackage {
    pub name: String,
    pub version: String,
    pub advisory_ids: Vec<String>,
}

impl VulnerablePackage {
    /// `time 0.1.44 (RUSTSEC-2020-0071, GHSA-wcg3-cvx6-7396)`.
    pub fn describe(&self) -> String {
        format!(
            "{} {} ({})",
            self.name,
            self.version,
            self.advisory_ids.join(", ")
        )
    }
}

pub struct OsvAdvisoryStream;

impl OsvAdvisoryStream {
    /// The JSON body for one `/v1/querybatch` request.
    pub fn build_batch_payload(packages: &[LockedPackage]) -> String {
        let batch = OsvBatchQuery {
            queries: packages
                .iter()
                .map(|p| OsvQuery {
                    package: OsvPackage {
                        name: p.name.clone(),
                        ecosystem: OSV_ECOSYSTEM.to_string(),
                    },
                    version: p.version.clone(),
                })
                .collect(),
        };
        serde_json::to_string(&batch).unwrap_or_default()
    }

    /// Pairs a batch response back onto the packages it was asked about.
    ///
    /// `packages` must be the same slice `build_batch_payload` was given: OSV
    /// answers positionally and carries no package name in the result.
    pub fn parse_batch_response(
        body: &str,
        packages: &[LockedPackage],
    ) -> Result<Vec<VulnerablePackage>, String> {
        let parsed: OsvBatchResponse = serde_json::from_str(body).map_err(|e| {
            format!("the OSV advisory database returned a body this gate could not parse: {e}")
        })?;

        if parsed.results.len() != packages.len() {
            return Err(format!(
                "the OSV advisory database returned {} results for {} queries, so no \
                 advisory can be attributed to a package",
                parsed.results.len(),
                packages.len()
            ));
        }

        Ok(packages
            .iter()
            .zip(parsed.results)
            .filter(|(_, r)| !r.vulns.is_empty())
            .map(|(p, r)| VulnerablePackage {
                name: p.name.clone(),
                version: p.version.clone(),
                advisory_ids: r.vulns.into_iter().map(|v| v.id).collect(),
            })
            .collect())
    }

    /// Every advisory OSV holds against the locked versions in `packages`.
    ///
    /// Cost, per pull request: `ceil(len / OSV_BATCH_SIZE)` POSTs, each bounded
    /// at `OSV_BUDGET`. Anvil's own lockfile is one.
    pub async fn query_batch(packages: &[LockedPackage]) -> Result<Vec<VulnerablePackage>, String> {
        info!(
            "Querying the OSV advisory database for {} locked packages...",
            packages.len()
        );

        let mut found = Vec::new();
        for chunk in packages.chunks(OSV_BATCH_SIZE) {
            let payload = Self::build_batch_payload(chunk);
            let body = post_json(CURL, OSV_BATCH_URL, &payload, OSV_BUDGET).await?;
            found.extend(Self::parse_batch_response(&body, chunk)?);
        }
        Ok(found)
    }
}

/// POSTs `payload` and returns the response body, or the reason there is none.
///
/// `program` is a parameter so every way the subprocess can fail -- absent
/// binary, non-zero exit, a kill on the deadline -- is reachable from a test
/// that makes no network request. Production always passes `CURL`.
pub async fn post_json(
    program: &str,
    url: &str,
    payload: &str,
    budget: Duration,
) -> Result<String, String> {
    let mut cmd = tokio::process::Command::new(program);
    cmd.args([
        "-s",
        "-X",
        "POST",
        url,
        "-H",
        "Content-Type: application/json",
        "--max-time",
        CURL_MAX_TIME,
        // Appends the status line curl would otherwise swallow: `-s` hides the
        // transport error AND the HTTP code, so a 429 arrives looking like an
        // empty result set.
        "-w",
        "\n%{http_code}",
        "-d",
        payload,
    ]);

    let output = crate::exec::run_bounded_for(cmd, budget, "curl OSV querybatch")
        .await
        .map_err(|e| format!("the OSV advisory database could not be reached: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "the OSV advisory database could not be reached: {} exited with {}",
            program, output.status
        ));
    }

    body_of(&String::from_utf8_lossy(&output.stdout))
}

/// Splits curl's stdout into body and the `%{http_code}` trailer, and refuses
/// anything that is not a 200.
///
/// Separate from `post_json` so a rate limit, an outage and a truncated run are
/// testable without a subprocess at all.
pub fn body_of(curl_stdout: &str) -> Result<String, String> {
    let Some((body, code)) = curl_stdout.rsplit_once('\n') else {
        return Err(
            "the OSV advisory database returned no HTTP status, so the response cannot be \
             trusted to be one"
                .to_string(),
        );
    };
    let code = code.trim();
    if code != "200" {
        return Err(format!(
            "the OSV advisory database answered HTTP {code}, so no advisory data was received"
        ));
    }
    Ok(body.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, version: &str) -> LockedPackage {
        LockedPackage {
            name: name.to_string(),
            version: version.to_string(),
        }
    }

    #[test]
    fn the_payload_carries_the_ecosystem_osv_keys_rust_advisories_under() {
        let payload = OsvAdvisoryStream::build_batch_payload(&[pkg("tokio", "1.38.0")]);
        assert!(payload.contains("tokio"));
        assert!(payload.contains("crates.io"));
        assert!(payload.contains("1.38.0"));
    }

    #[test]
    fn a_clean_batch_and_a_hit_are_told_apart() {
        let pkgs = [pkg("serde", "1.0.219"), pkg("time", "0.1.44")];
        assert!(
            OsvAdvisoryStream::parse_batch_response(r#"{"results":[{},{}]}"#, &pkgs)
                .expect("parses")
                .is_empty()
        );
        let hit = OsvAdvisoryStream::parse_batch_response(
            r#"{"results":[{},{"vulns":[{"id":"RUSTSEC-2020-0071"}]}]}"#,
            &pkgs,
        )
        .expect("parses");
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].describe(), "time 0.1.44 (RUSTSEC-2020-0071)");
    }
}
