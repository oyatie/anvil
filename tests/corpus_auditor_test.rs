use anvil::corpus_auditor::{ContinuousHygieneEngine, CorpusAuditor};
use tempfile::tempdir;

#[test]
fn test_corpus_auditor_and_hygiene_engine() {
    let dir = tempdir().unwrap();
    let tenancy = dir.path().join("tenancy");
    std::fs::create_dir_all(&tenancy).unwrap();
    std::fs::write(
        tenancy.join("policy.md"),
        "---\ncanonical_authority: true\n---\n# Tenancy Policy",
    )
    .unwrap();

    let audit_report = CorpusAuditor::audit_repository(dir.path(), 180).unwrap();
    assert_eq!(audit_report.unauthorized_ssot_claims.len(), 1);

    let batch = ContinuousHygieneEngine::generate_maintenance_batch(dir.path(), 5, false).unwrap();
    assert_eq!(batch.files_modified.len(), 0); // policy.md had no last_verified_at, but we can verify batch runs cleanly
}
