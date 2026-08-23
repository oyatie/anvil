//! Default engineering practice Anvil admits on a product monorepo.
//!
//! Every capability and every `app/<product>/` is hexagonal:
//! `core/`, `ports/`, `adapters/`, `facade/`. Extra children are a closed
//! set. Unknown names are RED. Missing allowed names are not RED.
//! Anvil's own tree is the control plane, not this layout.

use std::collections::BTreeSet;

pub const ALLOWED_ROOT_DIRS: &[&str] = &[
    "app",
    "audit",
    "billing",
    "build",
    "bus",
    "cell",
    "compliance",
    "compute",
    "data",
    "docs",
    "flags",
    "gateway",
    "iac",
    "iam",
    "intelligence",
    "k8s",
    "marketplace",
    "network",
    "observability",
    "packs",
    "pipeline",
    "secrets",
    "storage",
    "templates",
    "tenancy",
    "third-party",
];

pub const ALLOWED_ROOT_FILES: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "README.md",
    "LICENSE",
    "OWNERS",
    "AGENTS.md",
    "CLAUDE.md",
    "rust-toolchain.toml",
    "rustfmt.toml",
    "deny.toml",
    "reindeer.toml",
];

/// Hexagonal faces. Default engineering practice for every cap and app.
pub const FACES: &[&str] = &["core", "ports", "adapters", "facade"];

/// Additional children allowed beside faces.
pub const CAP_EXTRAS: &[&str] = &["cedar", "observability", "iac", "docs"];

pub const CAP_CHILDREN: &[&str] = &[
    "core",
    "ports",
    "adapters",
    "facade",
    "cedar",
    "observability",
    "iac",
    "docs",
];

pub const FORBIDDEN_NAMES: &[&str] = &[
    "plan",
    "tasks",
    "contracts",
    "specs",
    "libs",
    "tools",
    "infra",
    "kernel",
    "os",
    "governance",
    "console",
];

const META_ROOTS: &[&str] = &[
    "app",
    "build",
    "docs",
    "iac",
    "observability",
    "templates",
    "third-party",
];

pub fn path_parts(path: &str) -> Vec<&str> {
    path.trim_start_matches("./")
        .split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn first_component(path: &str) -> Option<&str> {
    path_parts(path).into_iter().next()
}

fn is_meta_root(root: &str) -> bool {
    META_ROOTS.contains(&root)
}

pub fn is_capability_root(root: &str) -> bool {
    ALLOWED_ROOT_DIRS.contains(&root) && !is_meta_root(root)
}

pub fn cap_root_file_ok(name: &str) -> bool {
    matches!(
        name,
        "OWNERS" | "README.md" | "BUCK" | "PRD.md" | "Cargo.toml" | "Cargo.lock" | "LICENSE"
    )
}

pub fn face_dir_ok(child: &str) -> bool {
    FACES.contains(&child) || CAP_EXTRAS.contains(&child)
}

pub fn cap_child_ok(child: &str) -> bool {
    face_dir_ok(child) || cap_root_file_ok(child)
}

/// Violations on a PR file list against default engineering practice.
pub fn layout_violations(changed_files: &[String]) -> Vec<String> {
    let allowed_dirs: BTreeSet<&str> = ALLOWED_ROOT_DIRS.iter().copied().collect();
    let allowed_files: BTreeSet<&str> = ALLOWED_ROOT_FILES.iter().copied().collect();
    let forbidden: BTreeSet<&str> = FORBIDDEN_NAMES.iter().copied().collect();
    let mut out = Vec::new();
    for file in changed_files {
        let parts = path_parts(file);
        let Some(root) = parts.first().copied() else {
            continue;
        };
        if root.starts_with('.') || root == "target" {
            continue;
        }
        if forbidden.contains(root) {
            out.push(format!("{file}: forbidden root `{root}`"));
            continue;
        }
        if parts.len() == 1 {
            if !allowed_files.contains(root) && !allowed_dirs.contains(root) {
                out.push(format!("{file}: unknown root file `{root}`"));
            }
            continue;
        }
        if !allowed_dirs.contains(root) {
            out.push(format!("{file}: unknown root `{root}`"));
            continue;
        }
        if root == "app" {
            out.extend(app_child_violations(file, &parts, &forbidden));
        } else if is_capability_root(root) {
            out.extend(cap_child_violations(file, &parts, &forbidden));
        }
    }
    out
}

fn cap_child_violations(file: &str, parts: &[&str], forbidden: &BTreeSet<&str>) -> Vec<String> {
    let child = parts[1];
    if forbidden.contains(child) {
        return vec![format!("{file}: forbidden child `{child}`")];
    }
    if parts.len() == 2 {
        if !cap_child_ok(child) {
            return vec![format!(
                "{file}: cap-root file `{child}` is not a face, extra, or allowed file"
            )];
        }
        return Vec::new();
    }
    if !face_dir_ok(child) {
        return vec![format!(
            "{file}: `{child}` is not core/ports/adapters/facade (or cedar/observability/iac/docs)"
        )];
    }
    Vec::new()
}

fn app_child_violations(file: &str, parts: &[&str], forbidden: &BTreeSet<&str>) -> Vec<String> {
    if parts.len() == 2 {
        if cap_root_file_ok(parts[1]) {
            return Vec::new();
        }
        return vec![format!(
            "{file}: app-root file `{}` is not an allowed owner file",
            parts[1]
        )];
    }
    let child = parts[2];
    if forbidden.contains(child) {
        return vec![format!("{file}: forbidden child `{child}`")];
    }
    if parts.len() == 3 {
        if !cap_child_ok(child) {
            return vec![format!(
                "{file}: `app/<product>/{child}` is not a face, extra, or allowed file"
            )];
        }
        return Vec::new();
    }
    if !face_dir_ok(child) {
        return vec![format!(
            "{file}: `app/<product>/{child}` is not core/ports/adapters/facade"
        )];
    }
    Vec::new()
}

/// Anvil's own tree (`src/`, `tests/`, …) is not a product monorepo.
pub fn enforce_on_repo(repo: &str) -> bool {
    let name = repo.rsplit('/').next().unwrap_or(repo);
    name != "anvil"
}
