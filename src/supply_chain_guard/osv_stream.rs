use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsvQuery {
    pub package: OsvPackage,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsvPackage {
    pub name: String,
    pub ecosystem: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsvVulnerabilityResponse {
    pub vulns: Option<Vec<OsvVulnerability>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsvVulnerability {
    pub id: String,
    pub summary: Option<String>,
    pub details: Option<String>,
    pub aliases: Option<Vec<String>>,
}

pub struct OsvAdvisoryStream;

impl OsvAdvisoryStream {
    /// Builds the JSON payload for querying OSV REST API
    pub fn build_query_payload(
        package_name: &str,
        ecosystem: &str,
        version: Option<&str>,
    ) -> String {
        let query = OsvQuery {
            package: OsvPackage {
                name: package_name.to_string(),
                ecosystem: ecosystem.to_string(),
            },
            version: version.map(|v| v.to_string()),
        };
        serde_json::to_string(&query).unwrap_or_default()
    }

    /// Queries OSV API for known vulnerabilities affecting a package
    pub async fn query_package(
        package_name: &str,
        ecosystem: &str,
        version: Option<&str>,
    ) -> Result<Vec<OsvVulnerability>> {
        info!(
            "Querying OSV API for {} in ecosystem {}...",
            package_name, ecosystem
        );
        let payload = Self::build_query_payload(package_name, ecosystem, version);

        let mut curl_cmd = tokio::process::Command::new("curl");
        curl_cmd.args([
            "-s",
            "-X",
            "POST",
            "https://api.osv.dev/v1/query",
            "-H",
            "Content-Type: application/json",
            "-d",
            &payload,
            "--max-time",
            "5",
        ]);
        let output =
            crate::exec::run_bounded(curl_cmd, crate::exec::ExecClass::Api, "curl OSV API query")
                .await
                .context("Failed to send request to OSV API via curl")?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let body: OsvVulnerabilityResponse = serde_json::from_slice(&output.stdout)
            .unwrap_or(OsvVulnerabilityResponse { vulns: None });
        Ok(body.vulns.unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_query_payload() {
        let payload = OsvAdvisoryStream::build_query_payload("tokio", "crates.io", Some("1.0.0"));
        assert!(payload.contains("tokio"));
        assert!(payload.contains("crates.io"));
        assert!(payload.contains("1.0.0"));
    }
}
