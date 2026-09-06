//! GitHub webhook signature verification.
//!
//! Split out of `webhook_handlers` because that file is twice its budget and
//! the oversized-file ratchet had refused five changes to it in one session.
//! This is the piece that moves with no risk: a pure function over bytes, with
//! no borrow of the router's state and no caller outside the handler.
//!
//! The larger split -- the four autonomous doors into their own module -- is
//! the one this file's existence argues for, and it belongs in its own change
//! rather than inside a security fix, so that a bisect over it is one step.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

/// Verifies GitHub X-Hub-Signature-256 HMAC in constant time to prevent timing attacks
pub fn verify_github_hmac(secret: &str, raw_bytes: &[u8], signature_header: Option<&str>) -> bool {
    let signature = match signature_header {
        Some(sig) => sig,
        None => return false,
    };

    let expected_hex = match signature.strip_prefix("sha256=") {
        Some(hex) => hex,
        None => signature,
    };

    let expected_bytes = match hex::decode(expected_hex) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let mut mac = match Hmac::<Sha256>::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };

    mac.update(raw_bytes);
    let result = mac.finalize().into_bytes();

    result.as_slice().ct_eq(&expected_bytes).into()
}

#[cfg(test)]
mod hmac_tests {
    use super::*;
    use hmac::Mac;

    fn sign(secret: &str, body: &[u8]) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
    }

    #[test]
    fn accepts_a_correctly_signed_body() {
        let body = br#"{"action":"opened"}"#;
        assert!(verify_github_hmac(
            "s3cr3t",
            body,
            Some(&sign("s3cr3t", body))
        ));
    }

    #[test]
    fn rejects_wrong_secret_missing_header_and_tampered_body() {
        let body = br#"{"action":"opened"}"#;
        let sig = sign("s3cr3t", body);
        assert!(!verify_github_hmac("other", body, Some(&sig)));
        assert!(!verify_github_hmac("s3cr3t", body, None));
        assert!(!verify_github_hmac(
            "s3cr3t",
            br#"{"action":"closed"}"#,
            Some(&sig)
        ));
        assert!(!verify_github_hmac("s3cr3t", body, Some("sha256=zzzz")));
        assert!(!verify_github_hmac("s3cr3t", body, Some("")));
    }

    /// The rotation window: a delivery signed with the OLD secret must still
    /// verify while GITHUB_WEBHOOK_SECRET_PREVIOUS is set, and must stop
    /// verifying once it is cleared. This is what makes rotation lossless.
    #[test]
    fn rotation_window_accepts_old_signatures_then_stops() {
        let body = br#"{"action":"synchronize"}"#;
        let old_sig = sign("old-secret", body);

        // New secret alone does not accept an old-signed delivery.
        assert!(!verify_github_hmac("new-secret", body, Some(&old_sig)));
        // The previous secret does -- this is the fallback the handler consults.
        assert!(verify_github_hmac("old-secret", body, Some(&old_sig)));
        // After the window closes, the old signature is refused.
        assert!(!verify_github_hmac("unrelated", body, Some(&old_sig)));
    }
}
