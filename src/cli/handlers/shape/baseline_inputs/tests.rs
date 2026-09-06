use super::{after_presence, existing_regular, load};
use std::io::{self, ErrorKind};

const BASELINE: &[u8] = br#"{"schema":"anvil/ratchet-baseline/v1","measured_at":"0123456789012345678901234567890123456789","rules":{}}"#;
const SIGNOFF: &[u8] = br#"{"schema":"anvil/ratchet-signoff/v1"}"#;

#[test]
fn post_presence_read_failure_never_becomes_absence() {
    for kind in [ErrorKind::NotFound, ErrorKind::PermissionDenied] {
        let error = after_presence(Err(io::Error::from(kind))).unwrap_err();
        assert_eq!(error.downcast_ref::<io::Error>().unwrap().kind(), kind);
    }
    assert_eq!(
        after_presence(Ok(b"existing bytes".to_vec())).unwrap(),
        Some(b"existing bytes".to_vec())
    );
}

#[test]
fn only_known_absence_allows_bootstrap() {
    assert!(!existing_regular(Err(io::Error::from(ErrorKind::NotFound))).unwrap());
    assert!(existing_regular(Ok(true)).unwrap());
    assert!(existing_regular(Ok(false)).is_err());
    for kind in [ErrorKind::PermissionDenied, ErrorKind::Other] {
        assert!(existing_regular(Err(io::Error::from(kind))).is_err());
    }
}

#[tokio::test]
async fn absent_inputs_are_explicit_bootstrap_and_default_signoff() {
    let dir = tempfile::tempdir().unwrap();
    let baseline = dir.path().join("baseline.json");
    let signoff = dir.path().join("signoff.json");
    assert!(
        load(Some(&baseline), &signoff)
            .await
            .unwrap()
            .previous
            .is_none()
    );
    assert!(load(None, &signoff).await.unwrap().previous.is_none());
}

#[tokio::test]
async fn valid_existing_regular_documents_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let baseline = dir.path().join("baseline.json");
    let signoff = dir.path().join("signoff.json");
    std::fs::write(&baseline, BASELINE).unwrap();
    std::fs::write(&signoff, SIGNOFF).unwrap();
    let inputs = load(Some(&baseline), &signoff).await.unwrap();
    assert!(inputs.previous.is_some());
    assert_eq!(std::fs::read(&baseline).unwrap(), BASELINE);
    assert_eq!(std::fs::read(&signoff).unwrap(), SIGNOFF);
}

#[tokio::test]
async fn malformed_existing_inputs_fail_without_rewriting_them() {
    let dir = tempfile::tempdir().unwrap();
    let baseline = dir.path().join("baseline.json");
    let signoff = dir.path().join("signoff.json");
    std::fs::write(&baseline, b"invalid baseline").unwrap();
    assert!(load(Some(&baseline), &signoff).await.is_err());
    assert_eq!(std::fs::read(&baseline).unwrap(), b"invalid baseline");
    std::fs::write(&baseline, BASELINE).unwrap();
    std::fs::write(&signoff, b"invalid signoff").unwrap();
    assert!(load(Some(&baseline), &signoff).await.is_err());
    assert!(load(None, &signoff).await.is_err());
    assert_eq!(std::fs::read(&signoff).unwrap(), b"invalid signoff");
}
