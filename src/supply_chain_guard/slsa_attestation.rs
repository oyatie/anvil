use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlsaProvenancePredicate {
    pub builder_id: String,
    pub build_type: String,
    pub invocation_entrypoint: String,
    pub slsa_level: String,
    pub materials: Vec<SlsaMaterial>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlsaMaterial {
    pub uri: String,
    pub digest_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlsaProvenanceBundle {
    #[serde(rename = "_type")]
    pub predicate_type: String,
    pub predicate: SlsaProvenancePredicate,
}

pub struct SlsaAttestor;

impl SlsaAttestor {
    pub fn new() -> Self {
        Self
    }

    /// Generates in-toto SLSA Level 2+ provenance predicate for the build commit
    pub fn generate_slsa_l2_provenance(
        &self,
        repo: &str,
        commit_sha: &str,
    ) -> Result<SlsaProvenanceBundle> {
        let predicate = SlsaProvenancePredicate {
            builder_id: "https://github.com/oyatie/anvil/builders/rust-v1".to_string(),
            build_type: "https://slsa.dev/provenance/v1".to_string(),
            invocation_entrypoint: "cargo build --release".to_string(),
            slsa_level: "SLSA_LEVEL_2_PLUS".to_string(),
            materials: vec![SlsaMaterial {
                uri: format!("git+https://github.com/{}.git@{}", repo, commit_sha),
                digest_sha256: commit_sha.to_string(),
            }],
        };

        Ok(SlsaProvenanceBundle {
            predicate_type: "https://in-toto.io/attestation/provenance/v1".to_string(),
            predicate,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_slsa_provenance() {
        let attestor = SlsaAttestor::new();
        let bundle = attestor
            .generate_slsa_l2_provenance(
                "oyatie/oyatie",
                "572ebdce8f9fd80f704b88be2b92f97aaf3ec414",
            )
            .expect("Generates provenance");

        assert_eq!(bundle.predicate.slsa_level, "SLSA_LEVEL_2_PLUS");
        assert_eq!(
            bundle.predicate.materials[0].digest_sha256,
            "572ebdce8f9fd80f704b88be2b92f97aaf3ec414"
        );
    }
}
