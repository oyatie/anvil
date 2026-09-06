use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use syn::{Item, ItemMod};

use super::{child_module_dir, existing_file, path_overrides, read_parsed};
use crate::source_scan::cfg::excludes_when_test_is_false;

pub(super) fn resolve_external(
    module: &ItemMod,
    context: &Path,
    path_context: &Path,
) -> Result<Vec<PathBuf>, String> {
    let overrides = path_overrides(&module.attrs)?;
    if let Some(path) = overrides.certain.iter().next() {
        // rustc interprets a nested inline module's `#[path]` relative to the
        // module's logical directory. Normalize `.`/`..` lexically before the
        // filesystem lookup: the logical directory itself need not exist when
        // the override walks back out of it.
        let path = normalize_lexically(&path_context.join(path));
        if existing_file(&path)? {
            return fs::canonicalize(&path)
                .map(|path| vec![path])
                .map_err(|error| {
                    format!("cannot resolve module source {}: {error}", path.display())
                });
        }
        return Err(format!(
            "declared module source is missing at {}",
            path.display()
        ));
    }
    let name = module.ident.to_string();
    let conventional = [
        context.join(format!("{name}.rs")),
        context.join(name).join("mod.rs"),
    ];
    let mut found = BTreeSet::new();
    let mut conventional_found = 0;
    for path in conventional {
        if existing_file(&path)? {
            conventional_found += 1;
            found.insert(fs::canonicalize(&path).map_err(|error| {
                format!("cannot resolve module source {}: {error}", path.display())
            })?);
        }
    }
    if conventional_found > 1 {
        return Err(format!(
            "declared module `{}` has both file and directory sources beneath {}",
            module.ident,
            context.display()
        ));
    }
    for relative in &overrides.possible {
        let path = normalize_lexically(&path_context.join(relative));
        if existing_file(&path)? {
            found.insert(fs::canonicalize(&path).map_err(|error| {
                format!("cannot resolve module source {}: {error}", path.display())
            })?);
        }
    }
    if found.is_empty() {
        return Err(format!(
            "declared module `{}` has no source beneath {}",
            module.ident,
            context.display()
        ));
    }
    Ok(found.into_iter().collect())
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                let can_pop = normalized
                    .file_name()
                    .is_some_and(|name| name != std::ffi::OsStr::new(".."));
                if can_pop {
                    normalized.pop();
                } else if !normalized.has_root() {
                    normalized.push("..");
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

/// Finds requested filesystem spellings only when a production declaration
/// graph reaches them. This covers `#[path = "..."]` without turning an
/// arbitrary `.rs` file under `src/` into production source.
pub(super) fn declared_sources_at(
    roots: &[PathBuf],
    requested: &BTreeSet<PathBuf>,
    repo_root: &Path,
) -> Result<Vec<PathBuf>, String> {
    let mut seen = BTreeSet::new();
    let mut found = BTreeSet::new();
    for root in roots {
        visit_file(root, true, requested, repo_root, &mut seen, &mut found)?;
    }
    Ok(found.into_iter().collect())
}

fn visit_file(
    path: &Path,
    is_crate_root: bool,
    requested: &BTreeSet<PathBuf>,
    repo_root: &Path,
    seen: &mut BTreeSet<PathBuf>,
    found: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    if !path.starts_with(repo_root) {
        return Err(format!(
            "module source {} escapes repository {}",
            path.display(),
            repo_root.display()
        ));
    }
    if !seen.insert(path.to_path_buf()) {
        return Ok(());
    }
    if requested.contains(path) {
        found.insert(path.to_path_buf());
        return Ok(());
    }
    let (_, parsed) = read_parsed(path)?;
    let context = if is_crate_root {
        path.parent().unwrap_or(Path::new(".")).to_path_buf()
    } else {
        child_module_dir(path)
    };
    visit_items(
        &parsed.items,
        &context,
        path.parent().unwrap_or(Path::new(".")),
        requested,
        repo_root,
        seen,
        found,
    )
}

fn visit_items(
    items: &[Item],
    context: &Path,
    path_context: &Path,
    requested: &BTreeSet<PathBuf>,
    repo_root: &Path,
    seen: &mut BTreeSet<PathBuf>,
    found: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    for item in items {
        let Item::Mod(module) = item else { continue };
        if excludes_when_test_is_false(&module.attrs) {
            continue;
        }
        if let Some((_, nested)) = &module.content {
            let nested_context = context.join(module.ident.to_string());
            visit_items(
                nested,
                &nested_context,
                &nested_context,
                requested,
                repo_root,
                seen,
                found,
            )?;
        } else {
            for path in resolve_external(module, context, path_context)? {
                visit_file(&path, false, requested, repo_root, seen, found)?;
            }
        }
    }
    Ok(())
}
