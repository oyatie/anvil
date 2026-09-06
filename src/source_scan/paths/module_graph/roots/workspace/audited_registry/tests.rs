use super::*;

#[test]
fn complete_source_fingerprint_is_content_and_file_set_bound() {
    let root = tempfile::tempdir().unwrap();
    let canonical = std::fs::canonicalize(root.path()).unwrap();
    std::fs::write(root.path().join("one.rs"), "pub struct One;").unwrap();
    let first = source_fingerprint(&canonical).unwrap();
    std::fs::write(root.path().join(".cargo-ok"), "generated").unwrap();
    assert_eq!(source_fingerprint(&canonical).unwrap(), first);
    std::fs::write(root.path().join("two.rs"), "pub struct Two;").unwrap();
    assert_ne!(source_fingerprint(&canonical).unwrap(), first);
    std::fs::remove_file(root.path().join("two.rs")).unwrap();
    std::fs::write(root.path().join("one.rs"), "pub struct Changed;").unwrap();
    assert_ne!(source_fingerprint(&canonical).unwrap(), first);
}

#[test]
fn source_fingerprint_uses_archive_relative_path_normalization() {
    use sha2::{Digest, Sha256};
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    std::fs::write(root.path().join("src/lib.rs"), "text").unwrap();
    let expected = hex::encode(Sha256::digest(format!(
        "src/lib.rs\0{}\n",
        hex::encode(Sha256::digest(b"text"))
    )));
    assert_eq!(
        source_fingerprint(&std::fs::canonicalize(root.path()).unwrap()).unwrap(),
        expected
    );
}

#[test]
fn selected_manifest_identity_is_canonical_and_cache_contained() {
    let root = tempfile::tempdir().unwrap();
    let cache = std::fs::canonicalize(root.path()).unwrap();
    let package = cache.join("registry-package");
    std::fs::create_dir(&package).unwrap();
    std::fs::write(package.join("Cargo.toml"), "[package]").unwrap();
    assert_eq!(
        canonical_manifest(&package.join("Cargo.toml"), &cache),
        Some(package.join("Cargo.toml"))
    );
    assert!(canonical_manifest(&cache.join("missing/Cargo.toml"), &cache).is_none());
    assert!(canonical_manifest(&package.join("Cargo.toml"), &cache.join("other-cache")).is_none());
}

#[cfg(windows)]
#[test]
fn windows_metadata_drive_paths_bind_to_the_same_verbatim_canonical_manifest() {
    let root = tempfile::tempdir().unwrap();
    let cache = std::fs::canonicalize(root.path()).unwrap();
    let manifest = cache.join("Cargo.toml");
    std::fs::write(&manifest, "[package]").unwrap();
    let ordinary = std::path::PathBuf::from(manifest.to_str().unwrap().trim_start_matches(r"\\?\"));
    assert_eq!(canonical_manifest(&ordinary, &cache), Some(manifest));
}
