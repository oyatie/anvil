use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinnedImageManifest {
    pub image_repo: String,
    pub digest: String,
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestPinFinding {
    pub file_path: String,
    pub line_number: usize,
    pub unpinned_image: String,
    pub issue: String,
}

pub struct DigestPinner;

impl DigestPinner {
    pub fn new() -> Self {
        Self
    }

    /// Scans manifest diff for unpinned mutable tags like `:latest` or missing sha256 digests
    pub fn scan_unpinned_images(&self, file_path: &str, content: &str) -> Vec<DigestPinFinding> {
        let mut findings = Vec::new();
        let image_line_re = Regex::new(r#"(?i)image:\s*["']?([^\s"']+)["']?"#).unwrap();
        let mutable_tag_re = Regex::new(r":(?:latest|main|master|dev)$").unwrap();
        let has_sha_re = Regex::new(r"@sha256:[a-f0-9]{64}").unwrap();

        for (idx, line) in content.lines().enumerate() {
            if let Some(caps) = image_line_re.captures(line) {
                let image_str = caps.get(1).map(|m| m.as_str()).unwrap_or("");

                if mutable_tag_re.is_match(image_str)
                    || (!has_sha_re.is_match(image_str) && !image_str.contains("localhost"))
                {
                    findings.push(DigestPinFinding {
                        file_path: file_path.to_string(),
                        line_number: idx + 1,
                        unpinned_image: image_str.to_string(),
                        issue: "Image reference is not pinned to an immutable `@sha256:...` digest. Mutable tags violate GitOps reproducibility.".to_string(),
                    });
                }
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_unpinned_latest_tag() {
        let pinner = DigestPinner::new();
        let yaml = "image: ghcr.io/oyatie/console:latest";
        let findings = pinner.scan_unpinned_images("infra/gitops/app.yaml", yaml);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn test_accepts_pinned_sha256_digest() {
        let pinner = DigestPinner::new();
        let yaml = "image: ghcr.io/oyatie/console@sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let findings = pinner.scan_unpinned_images("infra/gitops/app.yaml", yaml);
        assert_eq!(findings.len(), 0);
    }
}
