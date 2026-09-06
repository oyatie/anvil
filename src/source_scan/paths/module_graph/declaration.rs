use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::roots::all_crate_roots;

mod nested;
mod walk;

#[derive(Clone, Copy, Default)]
pub(super) struct Roles {
    pub(super) production: bool,
    pub(super) test: bool,
}

pub(super) struct RoleMap {
    pub(super) roles: BTreeMap<PathBuf, Roles>,
    pub(super) complete: bool,
}

/// Files reached only through module declarations impossible with `test = false`.
///
/// The walk starts at every Cargo production root and retains the inline
/// module directory context rustc uses. A basename is never evidence: an
/// unconditional `mod tests;` ships, while `#[cfg(test)] mod fixtures;` does
/// not, regardless of either file's name.
pub fn declared_test_module_files(repo_root: &Path) -> Result<BTreeSet<PathBuf>, String> {
    let roots = all_crate_roots(repo_root)?;
    declared_test_module_files_from_roots(repo_root, &roots)
}

/// The declaration-driven test-only set for an authoritative target-root list.
///
/// Cargo-metadata consumers use this form because a valid package may put its
/// library or binary root anywhere in the repository. An empty root list is a
/// valid absence of declaration evidence and therefore classifies nothing as
/// test-only.
pub fn declared_test_module_files_from_roots(
    repo_root: &Path,
    roots: &[PathBuf],
) -> Result<BTreeSet<PathBuf>, String> {
    let measured = module_roles_from_roots(repo_root, roots)?;
    if !measured.complete {
        return Ok(BTreeSet::new());
    }
    let roles = measured.roles;
    Ok(roles
        .into_iter()
        .filter_map(|(path, roles)| (roles.test && !roles.production).then_some(path))
        .collect())
}

/// Files which the declaration graph reaches from a production Cargo target.
///
/// This is intentionally independent of path spelling. A source under
/// `tests/`, `examples/`, or `benches/` can still ship when a production root
/// includes or declares it.
pub fn declared_production_module_files_from_roots(
    repo_root: &Path,
    roots: &[PathBuf],
) -> Result<BTreeSet<PathBuf>, String> {
    let measured = module_roles_from_roots(repo_root, roots)?;
    if !measured.complete {
        return all_contained_files(repo_root);
    }
    let roles = measured.roles;
    Ok(roles
        .into_iter()
        .filter_map(|(path, roles)| roles.production.then_some(path))
        .collect())
}

fn module_roles_from_roots(repo_root: &Path, roots: &[PathBuf]) -> Result<RoleMap, String> {
    walk::module_roles_from_roots(repo_root, roots)
}

/// Exact evidence for ownership. Unlike the conservative public production
/// set, incomplete classification never substitutes every contained file.
pub(super) fn exact_production_roles(
    repo_root: &Path,
    root: &Path,
) -> Result<(BTreeSet<PathBuf>, bool), String> {
    let measured = module_roles_from_roots(repo_root, &[root.to_path_buf()])?;
    Ok(exact_role_evidence(measured))
}

pub(super) fn exact_role_evidence(measured: RoleMap) -> (BTreeSet<PathBuf>, bool) {
    (
        measured
            .roles
            .into_iter()
            .filter_map(|(path, role)| role.production.then_some(path))
            .collect(),
        measured.complete,
    )
}

/// One declaration graph, reused while a guard classifies many changed files.
pub struct TestSourceClassifier {
    repo_root: PathBuf,
    declared_test_modules: BTreeSet<PathBuf>,
    declared_production_modules: BTreeSet<PathBuf>,
    complete: bool,
}

impl TestSourceClassifier {
    pub fn new(repo_root: &Path) -> Result<Self, String> {
        let roots = all_crate_roots(repo_root)?;
        let measured = module_roles_from_roots(repo_root, &roots)?;
        let roles = measured.roles;
        Ok(Self {
            repo_root: repo_root.to_path_buf(),
            declared_test_modules: roles
                .iter()
                .filter_map(|(path, role)| (role.test && !role.production).then_some(path.clone()))
                .collect(),
            declared_production_modules: roles
                .into_iter()
                .filter_map(|(path, role)| role.production.then_some(path))
                .collect(),
            complete: measured.complete,
        })
    }

    pub fn classify(&self, path: &Path) -> Result<bool, String> {
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.repo_root.join(path)
        };
        let relative = path.strip_prefix(&self.repo_root).map_err(|_| {
            format!(
                "source {} is outside repository {}",
                path.display(),
                self.repo_root.display()
            )
        })?;
        if !path.is_file() {
            return Ok(super::super::is_test_source(&relative.to_string_lossy()));
        }
        let canonical = fs::canonicalize(&path)
            .map_err(|error| format!("cannot resolve source {}: {error}", path.display()))?;
        let canonical_repo = fs::canonicalize(&self.repo_root).map_err(|error| {
            format!(
                "cannot resolve repository {}: {error}",
                self.repo_root.display()
            )
        })?;
        if !canonical.starts_with(&canonical_repo) {
            return Err(format!(
                "source {} resolves outside repository {}",
                path.display(),
                canonical_repo.display()
            ));
        }
        if !self.complete {
            return Ok(false);
        }
        if self.declared_production_modules.contains(&canonical) {
            return Ok(false);
        }
        if self.declared_test_modules.contains(&canonical) {
            return Ok(true);
        }
        if path.extension().is_none_or(|extension| extension != "rs") {
            return Ok(super::super::is_test_source(&relative.to_string_lossy()));
        }
        if super::super::is_test_source(&relative.to_string_lossy()) {
            return Ok(true);
        }
        Ok(false)
    }
}

fn all_contained_files(repo_root: &Path) -> Result<BTreeSet<PathBuf>, String> {
    let canonical_root = fs::canonicalize(repo_root)
        .map_err(|error| format!("cannot resolve repository {}: {error}", repo_root.display()))?;
    let mut files = BTreeSet::new();
    let mut pending = vec![canonical_root.clone()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("cannot read {}: {error}", directory.display()))?
        {
            let entry = entry.map_err(|error| {
                format!("cannot read an entry in {}: {error}", directory.display())
            })?;
            if entry.file_name() == ".git" {
                continue;
            }
            let kind = entry
                .file_type()
                .map_err(|error| format!("cannot inspect {}: {error}", entry.path().display()))?;
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() || kind.is_symlink() {
                let canonical = fs::canonicalize(entry.path()).map_err(|error| {
                    format!("cannot resolve source {}: {error}", entry.path().display())
                })?;
                if !canonical.starts_with(&canonical_root) {
                    return Err(format!(
                        "source {} resolves outside repository {}",
                        entry.path().display(),
                        canonical_root.display()
                    ));
                }
                if canonical.is_file() {
                    files.insert(canonical);
                }
            }
        }
    }
    Ok(files)
}

/// Whether a Rust path is test-only by Cargo layout or its declaration graph.
pub fn try_is_test_source(repo_root: &Path, path: &Path) -> Result<bool, String> {
    let full_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    };
    full_path.strip_prefix(repo_root).map_err(|_| {
        format!(
            "source {} is outside repository {}",
            full_path.display(),
            repo_root.display()
        )
    })?;
    if !full_path.is_file() {
        let relative = full_path.strip_prefix(repo_root).expect("validated above");
        return Ok(super::super::is_test_source(&relative.to_string_lossy()));
    }
    TestSourceClassifier::new(repo_root)?.classify(path)
}
