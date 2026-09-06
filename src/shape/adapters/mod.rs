//! Adapters: concrete tree and dependency sources. `InMemoryTree` for
//! fixtures and as the value every loader produces; `GitTreeAtRev` reads a
//! named revision through git plumbing, never a checkout (D7, I3).

pub mod buck_label_deps;
pub mod cargo_manifest_deps;
pub mod git_tree_at_rev;
pub mod in_memory_tree;
pub mod rust_use_deps;
pub mod ts_import_deps;

pub use buck_label_deps::BuckLabelDeps;
pub use cargo_manifest_deps::CargoManifestDeps;
pub use git_tree_at_rev::GitTreeAtRev;
pub use in_memory_tree::InMemoryTree;
pub use rust_use_deps::RustUseDeps;
pub use ts_import_deps::TsImportDeps;

/// Joins path segments with `..`/`.` resolved; never escapes the root.
pub(crate) fn normalize(base_dir: &str, rel: &str) -> String {
    let mut parts: Vec<&str> = base_dir.split('/').filter(|s| !s.is_empty()).collect();
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    let mut out = parts.join("/");
    if !out.is_empty() {
        out.push('/');
    }
    out
}
