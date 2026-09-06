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
/// OSV publishes no maximum query count, so this is a self-imposed bound on
/// request size, nothing more. It is deliberately *not* justified by OSV's
/// pagination thresholds (>1,000 vulnerabilities in one query, >3,000 across a
/// batch): those count vulnerabilities, not queries, so no choice of chunk size
/// controls them. Pagination is handled where it happens, in
/// `parse_batch_response`. A 162-package lockfile is one request; a
/// 1,500-package monorepo is three.
pub const OSV_BATCH_SIZE: usize = 500;

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
    /// Present when OSV truncated this result and holds the rest behind another
    /// request. Deserialised so the truncation is visible: serde would
    /// otherwise ignore the field and the advisory list would be short with no
    /// signal at all.
    #[serde(default)]
    next_page_token: Option<String>,
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
        // Not `unwrap_or_default`: that turned a serialisation failure into an
        // empty POST body and a wrong request. This payload is Strings only and
        // cannot fail to serialise; if it ever can, the panic names the day.
        serde_json::to_string(&batch).expect("an OSV batch query of owned Strings serialises")
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

        // A paginated result is a partial one, and this gate's whole value is
        // naming which advisories to fix. Publishing the first page as though
        // it were the answer is a claim past the evidence, so the audit
        // abstains instead (invariant I1). This gate does not follow the token:
        // paging is a second request shape with its own failure modes, and
        // nothing in this fleet has reached the threshold.
        if parsed
            .results
            .iter()
            .any(|r| r.next_page_token.as_deref().is_some_and(|t| !t.is_empty()))
        {
            return Err(
                "the OSV advisory database paginated this batch, so the advisory list \
                 is incomplete and no complete verdict can be published"
                    .to_string(),
            );
        }

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
    /// at the transport's fixed budget. Anvil's own lockfile is one. The budget is per chunk and
    /// there is no aggregate deadline, so a 1,500-package lockfile is three
    /// requests and up to 60s worst case added to a certification.
    pub async fn query_batch(packages: &[LockedPackage]) -> Result<Vec<VulnerablePackage>, String> {
        info!(
            "Querying the OSV advisory database for {} locked packages...",
            packages.len()
        );

        let mut found = Vec::new();
        for chunk in packages.chunks(OSV_BATCH_SIZE) {
            let body = post_batch(chunk).await?;
            found.extend(Self::parse_batch_response(&body, chunk)?);
        }
        Ok(found)
    }
}

/// Executes the fixed OSV transport and returns its authenticated response
/// body. There is intentionally no public program/URL/payload transport API.
async fn post_batch(packages: &[LockedPackage]) -> Result<String, String> {
    let output = crate::exec::post_osv_batch(packages)
        .await
        .map_err(|e| format!("the OSV advisory database could not be reached: {e}"))?;

    decode_output(output)
}

fn decode_output(output: std::process::Output) -> Result<String, String> {
    if !output.status.success() {
        return Err(format!(
            "the OSV advisory database could not be reached: curl exited with {}",
            output.status
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("the OSV advisory database returned non-UTF-8 output: {error}"))?;
    body_of(&stdout)
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

    #[cfg(unix)]
    #[test]
    fn response_decode_accepts_200_and_rejects_nonzero_or_non_utf8_output() {
        use std::os::unix::process::ExitStatusExt;

        let output = |code, stdout| std::process::Output {
            status: std::process::ExitStatus::from_raw(code),
            stdout,
            stderr: Vec::new(),
        };
        assert_eq!(
            decode_output(output(
                0,
                br#"{"results":[{}]}
200"#
                    .to_vec()
            ))
            .expect("valid offline OSV response"),
            r#"{"results":[{}]}"#
        );

        assert!(
            decode_output(output(
                7 << 8,
                br#"{"results":[]}
200"#
                    .to_vec()
            ))
            .expect_err("nonzero curl status cannot become advisory evidence")
            .contains('7')
        );

        let mut invalid = vec![0xff];
        invalid.extend_from_slice(b"\n200");
        assert!(
            decode_output(output(0, invalid))
                .expect_err("lossy OSV output cannot become advisory evidence")
                .contains("non-UTF-8")
        );
    }
}
