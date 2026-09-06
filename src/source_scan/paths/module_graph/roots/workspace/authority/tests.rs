use super::*;

#[test]
fn metadata_command_is_full_locked_offline_and_environment_cleared() {
    let root = tempfile::tempdir().unwrap();
    let command = metadata_command(root.path(), root.path(), root.path());
    assert_eq!(command.get_program(), "cargo");
    assert_eq!(command.get_current_dir(), Some(root.path()));
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            "metadata",
            "--locked",
            "--offline",
            "--format-version",
            "1",
            "--all-features"
        ]
    );
    let environment = command
        .get_envs()
        .collect::<std::collections::BTreeMap<_, _>>();
    assert!(environment.contains_key(std::ffi::OsStr::new("ANVIL_INTERNAL_NON_MODEL_ENV_CLEARED")));
    assert_eq!(
        environment[std::ffi::OsStr::new("RUSTUP_TOOLCHAIN")],
        Some(std::ffi::OsStr::new("1.98.0"))
    );
    assert_eq!(
        environment[std::ffi::OsStr::new("RUSTUP_AUTO_INSTALL")],
        Some(std::ffi::OsStr::new("0"))
    );
    for name in [
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTC_WRAPPER",
        "GITHUB_TOKEN",
    ] {
        assert!(!environment.contains_key(std::ffi::OsStr::new(name)));
    }
}

#[test]
fn configuration_discovery_checks_every_ancestor_and_explicit_cargo_home() {
    let tree = tempfile::tempdir().unwrap();
    let root = tree.path().join("workspace/member");
    std::fs::create_dir_all(&root).unwrap();
    let cargo_home = tree.path().join("cargo-home");
    std::fs::create_dir_all(&cargo_home).unwrap();
    assert!(configuration_absent(
        std::slice::from_ref(&root),
        &cargo_home
    ));
    std::fs::write(cargo_home.join("config.toml"), "").unwrap();
    assert!(!configuration_absent(
        std::slice::from_ref(&root),
        &cargo_home
    ));
    std::fs::remove_file(cargo_home.join("config.toml")).unwrap();
    std::fs::create_dir_all(tree.path().join(".cargo")).unwrap();
    std::fs::write(tree.path().join(".cargo/config"), "").unwrap();
    assert!(!configuration_absent(&[root], &cargo_home));
}

#[test]
fn unsupported_manifest_authority_is_not_admitted() {
    for manifest in [
        "[patch.crates-io]\nserde = { version = \"1\" }",
        "[replace]\n'serde:1.0.0' = { path = \"local\" }",
        "[dependencies]\nserde = { version = \"1\", registry = \"custom\" }",
    ] {
        assert!(!supported_manifest(&manifest.parse().unwrap()));
    }
    assert!(supported_manifest(
        &"[dependencies]\nserde = { version = \"1\", features = [\"derive\"] }"
            .parse()
            .unwrap()
    ));
}

#[test]
fn resolve_binding_preserves_kind_target_and_selected_identity() {
    let node: Node = serde_json::from_value(serde_json::json!({
        "id": "local", "features": [], "deps": [
            { "name": "codec", "pkg": "serde-id", "dep_kinds": [{"kind": null, "target": "cfg(unix)"}] },
            { "name": "codec", "pkg": "other-id", "dep_kinds": [{"kind": "build", "target": null}] }
        ]
    })).unwrap();
    assert_eq!(
        selected_dependency(&node, "codec", None, Some("cfg(unix)")),
        Some("serde-id")
    );
    assert_eq!(
        selected_dependency(&node, "codec", Some("build"), None),
        Some("other-id")
    );
    assert_eq!(selected_dependency(&node, "codec", None, None), None);
    let unavailable: Node =
        serde_json::from_value(serde_json::json!({"id":"local","features":[],"deps":[]})).unwrap();
    assert_eq!(selected_dependency(&unavailable, "codec", None, None), None);
}

#[test]
fn snapshot_reuse_requires_current_manifest_lock_and_configuration_bytes() {
    let tree = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(tree.path()).unwrap();
    let cargo_home = root.join("cargo-home");
    std::fs::create_dir(&cargo_home).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
    let read = || {
        Snapshot::read_at(
            &root,
            &[],
            root.clone(),
            cargo_home.clone(),
            root.clone(),
            Vec::new(),
        )
    };
    assert!(read().is_none(), "missing lock cannot supply evidence");
    std::fs::write(root.join("Cargo.lock"), "version = 4\n").unwrap();
    let initial = read().unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = []\n# changed\n",
    )
    .unwrap();
    assert!(read().unwrap() != initial);
    std::fs::write(root.join("Cargo.lock"), "version = 4\n# changed\n").unwrap();
    assert!(read().unwrap() != initial);
    std::fs::create_dir(root.join(".cargo")).unwrap();
    std::fs::write(root.join(".cargo/config.toml"), "").unwrap();
    assert!(
        read().is_none(),
        "new configuration invalidates admitted evidence"
    );
}

#[test]
fn reviewed_support_edges_follow_selected_package_ids_even_when_renamed() {
    let node: Node = serde_json::from_value(serde_json::json!({
        "id":"derive", "features":[], "deps":[
            {"name":"syntax", "pkg":"other-syn", "dep_kinds":[{"kind":null,"target":null}]}
        ]
    }))
    .unwrap();
    let packages: Vec<metadata::Package> = serde_json::from_value(serde_json::json!([
        {"id":"other-syn", "name":"syn", "version":"9.0.0", "source":null, "manifest_path":"unused"}
    ]))
    .unwrap();
    let selected = [("syn".to_owned(), "reviewed-syn".to_owned())].into();
    assert!(!node.reviewed_edges_match(&selected, &packages));
}

#[test]
fn tool_home_discovery_supports_userprofile_without_environment_home() {
    let root = tempfile::tempdir().unwrap();
    let profile = root.path().as_os_str().to_owned();
    assert_eq!(
        snapshot::tool_home(None, None, Some(profile.clone()), ".cargo"),
        Some(root.path().join(".cargo"))
    );
    assert_eq!(
        snapshot::tool_home(None, None, Some(profile), ".rustup"),
        Some(root.path().join(".rustup"))
    );
    assert!(snapshot::tool_home(Some("relative".into()), None, None, ".cargo").is_none());
}
