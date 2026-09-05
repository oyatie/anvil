//! Exact lockfile evidence for the small reviewed proc-macro surface.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

struct Evidence {
    package: &'static str,
    version: &'static str,
    checksum: &'static str,
}

const SERDE: &[Evidence] = &[
    Evidence {
        package: "serde",
        version: "1.0.229",
        checksum: "4148590afebada386688f18773da617792bf2ef03ffc1e4cbd2b1d45b023e0ba",
    },
    Evidence {
        package: "serde_derive",
        version: "1.0.229",
        checksum: "e7a5d71263a5a7d47b41f6b3f06ba276f10cc18b0931f1799f710578e2309348",
    },
];
const CLAP: &[Evidence] = &[
    Evidence {
        package: "clap",
        version: "4.6.6",
        checksum: "473c7e07f409a8d772161724aa8db6a765a2532a70f9667eeb7b49d3d02fbdca",
    },
    Evidence {
        package: "clap_derive",
        version: "4.6.4",
        checksum: "d012d2b9d65aca7f18f4d9878a045bc17899bba951561ba5ec3c2ba1eed9a061",
    },
];
const ASYNC_TRAIT: &[Evidence] = &[Evidence {
    package: "async-trait",
    version: "0.1.92",
    checksum: "82f6aeea286b8eb4dd3431a1be1b59d290ace00f5bfd8e2a159bc2a05e2c1667",
}];
const TOKIO: &[Evidence] = &[
    Evidence {
        package: "tokio",
        version: "1.53.1",
        checksum: "202caea871b69668250d242070849eb495be178ed697a3e98aebce5bc81a0bed",
    },
    Evidence {
        package: "tokio-macros",
        version: "2.7.2",
        checksum: "78773a2a397f451582ce068015985c33193cf6dea8b74d2a639fe457b2f07b0e",
    },
];
const TRACING: &[Evidence] = &[Evidence {
    package: "tracing",
    version: "0.1.44",
    checksum: "63e71662fa4b2a2c3a26f570f037eb95bb1f85397f3cd8076caed2f026a6d100",
}];
const ANYHOW: &[Evidence] = &[Evidence {
    package: "anyhow",
    version: "1.0.104",
    checksum: "330a5ed07fa54e4702c9d6c4174f74427fc0ef6e214bbd677ae50a5099946470",
}];
const SERDE_JSON: &[Evidence] = &[Evidence {
    package: "serde_json",
    version: "1.0.151",
    checksum: "c841b55ecdae098c80dcae9cf767f6f8a0c2cdb3416bbef72181df4d0fe73f14",
}];

pub(super) fn packages(root: &Path) -> Result<BTreeSet<String>, String> {
    let cargo = root.join(".cargo");
    match fs::symlink_metadata(&cargo) {
        Ok(metadata) if !metadata.file_type().is_dir() => {
            return Err(format!(
                "Cargo authority {} is not a directory",
                cargo.display()
            ));
        }
        Ok(_) => {
            for name in ["config", "config.toml"] {
                let path = cargo.join(name);
                match fs::symlink_metadata(&path) {
                    Ok(_) => return Ok(BTreeSet::new()),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(format!("cannot inspect {}: {error}", path.display()));
                    }
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot inspect {}: {error}", cargo.display())),
    }
    let path = root.join("Cargo.lock");
    if !path.exists() {
        return Ok(BTreeSet::new());
    }
    let canonical = fs::canonicalize(&path)
        .map_err(|error| format!("cannot resolve {}: {error}", path.display()))?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err(format!(
            "lockfile {} escapes the repository",
            path.display()
        ));
    }
    let bytes =
        fs::read(&canonical).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let text = String::from_utf8(bytes)
        .map_err(|error| format!("{} is not UTF-8: {error}", path.display()))?;
    let lock = text
        .parse::<toml::Value>()
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))?;
    let entries = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{} has no package records", path.display()))?;

    let mut audited = BTreeSet::new();
    for (package, evidence) in [
        ("serde", SERDE),
        ("clap", CLAP),
        ("async-trait", ASYNC_TRAIT),
        ("tokio", TOKIO),
        ("tracing", TRACING),
        ("anyhow", ANYHOW),
        ("serde_json", SERDE_JSON),
    ] {
        if evidence
            .iter()
            .all(|expected| uniquely_present(entries, expected))
        {
            audited.insert(package.to_owned());
        }
    }
    Ok(audited)
}

fn uniquely_present(entries: &[toml::Value], expected: &Evidence) -> bool {
    let matching_name = entries
        .iter()
        .filter(|entry| entry.get("name").and_then(toml::Value::as_str) == Some(expected.package))
        .collect::<Vec<_>>();
    let [entry] = matching_name.as_slice() else {
        return false;
    };
    entry.get("version").and_then(toml::Value::as_str) == Some(expected.version)
        && entry.get("checksum").and_then(toml::Value::as_str) == Some(expected.checksum)
        && entry.get("source").and_then(toml::Value::as_str)
            == Some("registry+https://github.com/rust-lang/crates.io-index")
}
