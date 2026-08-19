use serde::{Deserialize, Serialize};

pub mod sigstore_attestor;
pub use sigstore_attestor::{CosignSignatureBundle, SigstoreAttestor};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CosignReport {
    pub passed: bool,
    pub bundle: CosignSignatureBundle,
}

#[derive(Debug, Clone, Default)]
pub struct CosignProvenanceSigner {
    attestor: SigstoreAttestor,
}

impl CosignProvenanceSigner {
    pub fn new() -> Self {
        Self {
            attestor: SigstoreAttestor::new(),
        }
    }

    pub fn generate_cosign_attestation(
        &self,
        artifact_digest: &str,
    ) -> CosignReport {
        let bundle = self.attestor.sign_artifact_digest(artifact_digest);
        let passed = bundle.is_valid;

        CosignReport { passed, bundle }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosign_signer_nominal() {
        let signer = CosignProvenanceSigner::new();
        let report = signer.generate_cosign_attestation("sha256:1122334455667788");
        assert!(report.passed);
    }
}
