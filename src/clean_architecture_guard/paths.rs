//! Which unit and which layer a path belongs to.

use super::report::ArchLayer;

/// Classifies a file path into an architectural layer, or `None` when the path
/// carries no layer information at all.
/// Whether a source line is an import statement (Rust `use`/`extern crate`,
/// TS/JS `import`, Python `from ... import`). Comments and strings are not
/// dependency edges, however many layer names they mention.
pub(super) fn is_import_line(trimmed: &str) -> bool {
    let t = trimmed.trim_start();
    if t.starts_with("//") || t.starts_with("/*") || t.starts_with('*') || t.starts_with('#') {
        return false;
    }
    let t = t
        .strip_prefix("pub(crate) ")
        .or_else(|| t.strip_prefix("pub(super) "))
        .or_else(|| t.strip_prefix("pub "))
        .unwrap_or(t);
    t.starts_with("use ")
        || t.starts_with("extern crate ")
        || t.starts_with("import ")
        || t.starts_with("from ")
}

/// Whether a path is a test source rather than shipped code.
///
/// `tests/` for integration tests, `_test.rs`/`_tests.rs` by convention, and
/// any nested `/tests/` directory. `#[cfg(test)]` modules inside a production
/// file are stripped separately by `without_test_modules`.
pub(super) fn is_test_source(path: &str) -> bool {
    path.starts_with("tests/")
        || path.contains("/tests/")
        || path.ends_with("_test.rs")
        || path.ends_with("_tests.rs")
}

pub(super) fn classify_layer(file_path: &str) -> Option<ArchLayer> {
    // Normalise so a tree-relative path such as `core/x.rs` matches the same
    // `/core/` convention a repo-relative path uses.
    let path = format!("/{}", file_path.trim_start_matches('/').to_lowercase());

    if path.contains("/core/")
        || path.contains("/domain/")
        || path.ends_with("/core.rs")
        || path.ends_with("/domain.rs")
    {
        return Some(ArchLayer::Core);
    }
    if path.contains("/ports/")
        || path.contains("/application/")
        || path.ends_with("/ports.rs")
        || path.ends_with("/application.rs")
    {
        return Some(ArchLayer::Ports);
    }
    if path.contains("/adapter") || path.ends_with("/adapters.rs") || path.ends_with("/adapter.rs")
    {
        return Some(ArchLayer::Adapters);
    }
    if path.contains("/facade/")
        || path.contains("/rest/")
        || path.ends_with("/facade.rs")
        || path.ends_with("/rest.rs")
    {
        return Some(ArchLayer::Facade);
    }
    None
}

/// The unit a file belongs to: the segment its `crate::` path is rooted at.
///
/// `crate::` is rooted at a crate's `src/`, in a flat repo and a Cargo
/// workspace alike -- `crates/alpha/src/beta/mod.rs` answers to
/// `crate::beta::…`, never `crate::alpha::…`. Taking the first path segment
/// instead made every file in a workspace a member of a unit called `crates`,
/// so a unit using its OWN adapters was reported as reaching into someone
/// else's. Workspaces are the common layout for exactly the repositories that
/// have faces to check.
pub(super) fn unit_of(file_path: &str) -> Option<String> {
    let norm = file_path.replace('\\', "/");
    let segs: Vec<&str> = norm
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    let start = segs
        .iter()
        .rposition(|s| *s == "src")
        .map(|i| i + 1)
        .unwrap_or(0);
    let unit = segs.get(start)?;
    if unit.ends_with(".rs") {
        return None; // a file at the crate root owns no faces
    }
    Some((*unit).to_string())
}

pub(super) fn layer_name(layer: Option<ArchLayer>) -> &'static str {
    match layer {
        Some(ArchLayer::Core) => "CORE/DOMAIN",
        Some(ArchLayer::Ports) => "PORTS/APPLICATION",
        Some(ArchLayer::Adapters) => "ADAPTERS",
        Some(ArchLayer::Facade) => "FACADE/REST",
        None => "UNLAYERED",
    }
}
