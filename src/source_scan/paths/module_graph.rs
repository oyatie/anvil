//! Fallible, declaration-driven Rust module aggregation.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use syn::Item;

use crate::source_scan::cfg::excludes_when_test_is_false;

mod declaration;
pub use declaration::{
    TestSourceClassifier, declared_production_module_files_from_roots, declared_test_module_files,
    declared_test_module_files_from_roots, try_is_test_source,
};
mod dependencies;
pub use dependencies::production_module_dependencies;
mod lookup;
use lookup::resolve_external;
mod path_attrs;
use path_attrs::path_overrides;
mod roots;
use roots::crate_roots;
pub use roots::production_top_level_modules;
mod source_file;
use source_file::{child_module_dir, existing_file, read_parsed};

#[cfg(test)]
mod tests;

/// Production Rust for a declared module, asserting that the corpus exists.
///
/// This convenience is for structural tests whose missing fixture is itself a
/// test failure. Production gates must use [`try_module_source`] and preserve
/// its error as `NotMeasured`.
pub fn module_source(module: &str, repo_root: &Path) -> String {
    try_module_source(module, repo_root).unwrap_or_else(|reason| panic!("{reason}"))
}

/// Production Rust for a module and every production child it declares.
pub fn try_module_source(module: &str, repo_root: &Path) -> Result<String, String> {
    let canonical_repo = fs::canonicalize(repo_root)
        .map_err(|error| format!("cannot resolve repository {}: {error}", repo_root.display()))?;
    let normalized = module.replace('\\', "/");
    let normalized = normalized.trim_end_matches(".rs").trim_end_matches("/mod");
    let segments = normalized.split('/').collect::<Vec<_>>();
    if segments.len() < 2 || segments[0] != "src" || segments.iter().any(|s| s.is_empty()) {
        return Err(format!(
            "`{module}` is not a source module rooted under src/"
        ));
    }
    // `lib.rs`, Cargo's `main.rs`, and an explicitly named `mod.rs` are
    // physical root files. Their child modules live beside them, not beneath
    // a directory named `lib`, `main`, or `mod`, so naming that file preserves
    // the file-sized subject used by the original module-source contract.
    let root_file_only = module.replace('\\', "/").ends_with("/mod")
        || module.replace('\\', "/").ends_with("/mod.rs")
        || normalized == "src/lib"
        || normalized == "src/main"
        || (segments.len() == 4 && segments[..2] == ["src", "bin"] && segments[3] == "main");

    let (mut current, child_start, crate_root_paths) = if segments[1] == "bin" {
        let (paths, child_start) = roots::binary_subject_root(repo_root, &segments)?;
        (paths.clone(), child_start, paths)
    } else {
        let roots = crate_roots(repo_root)?;
        if matches!(segments[1], "lib" | "main") {
            let selected = roots
                .iter()
                .filter(|path| path.file_stem().and_then(|stem| stem.to_str()) == Some(segments[1]))
                .cloned()
                .collect::<Vec<_>>();
            (selected, 2, roots)
        } else {
            (
                declared_paths_in_files(&roots, segments[1], &roots, &canonical_repo)?,
                2,
                roots,
            )
        }
    };
    if current.is_empty() {
        return Err(format!("no crate root source for `{module}`"));
    }
    for segment in &segments[child_start..] {
        current =
            match declared_paths_in_files(&current, segment, &crate_root_paths, &canonical_repo) {
                Ok(paths) => paths,
                Err(logical_error) => {
                    // A `#[path = "..."]` module's filesystem spelling need not
                    // match its logical module path. Source-scanning callers name
                    // the file they are reporting, so accept that spelling only
                    // when the parsed production graph actually reaches it.
                    let stem = repo_root.join(normalized);
                    let mut candidates = BTreeSet::new();
                    for candidate in [stem.with_extension("rs"), stem.join("mod.rs")] {
                        if existing_file(&candidate)? {
                            candidates.insert(fs::canonicalize(&candidate).map_err(|error| {
                                format!(
                                    "cannot resolve module source {}: {error}",
                                    candidate.display()
                                )
                            })?);
                        }
                    }
                    let paths = lookup::declared_sources_at(
                        &crate_root_paths,
                        &candidates,
                        &canonical_repo,
                    )?;
                    if paths.is_empty() {
                        return Err(logical_error);
                    }
                    current = paths;
                    break;
                }
            };
    }

    let mut seen = BTreeSet::new();
    let mut sources = Vec::new();
    for path in current {
        if root_file_only {
            let (source, _) = read_parsed(&path)?;
            let production = super::super::try_without_test_modules(&source)
                .map_err(|error| format!("cannot classify source {}: {error}", path.display()))?;
            sources.push((path, production));
        } else {
            discover_file(
                &path,
                crate_root_paths.contains(&path),
                &mut seen,
                &mut sources,
                &canonical_repo,
            )?;
        }
    }
    if sources.is_empty() {
        return Err(format!(
            "no production source for `{module}` under {}",
            repo_root.display()
        ));
    }
    sources.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(sources
        .into_iter()
        .map(|(_, source)| source)
        .collect::<Vec<_>>()
        .join("\n"))
}

fn declared_paths_in_files(
    parents: &[PathBuf],
    name: &str,
    crate_roots: &[PathBuf],
    repo_root: &Path,
) -> Result<Vec<PathBuf>, String> {
    let mut paths = BTreeSet::new();
    let mut saw_test_only = false;
    for parent in parents {
        let (_, parsed) = read_parsed(parent)?;
        for item in parsed.items {
            let Item::Mod(module) = item else { continue };
            if module.ident != name {
                continue;
            }
            if excludes_when_test_is_false(&module.attrs) {
                saw_test_only = true;
                continue;
            }
            if module.content.is_some() {
                return Err(format!(
                    "production module `{name}` is inline in {}; module aggregation cannot read it as a separate subject",
                    parent.display()
                ));
            }
            paths.extend(contained_module_paths(
                resolve_external(
                    &module,
                    &if crate_roots.contains(parent) {
                        parent.parent().unwrap_or(Path::new(".")).to_path_buf()
                    } else {
                        child_module_dir(parent)
                    },
                    parent.parent().unwrap_or(Path::new(".")),
                )?,
                repo_root,
            )?);
        }
    }
    if paths.is_empty() {
        let qualifier = if saw_test_only {
            "production (non-test) "
        } else {
            ""
        };
        return Err(format!(
            "no {qualifier}declaration or source for module `{name}`"
        ));
    }
    Ok(paths.into_iter().collect())
}

fn discover_file(
    path: &Path,
    is_crate_root: bool,
    seen: &mut BTreeSet<PathBuf>,
    sources: &mut Vec<(PathBuf, String)>,
    repo_root: &Path,
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
    let (source, parsed) = read_parsed(path)?;
    let production = super::super::try_without_test_modules(&source)
        .map_err(|error| format!("cannot classify source {}: {error}", path.display()))?;
    sources.push((path.to_path_buf(), production));
    let context = if is_crate_root {
        path.parent().unwrap_or(Path::new(".")).to_path_buf()
    } else {
        child_module_dir(path)
    };
    discover_items(
        &parsed.items,
        &context,
        path.parent().unwrap_or(Path::new(".")),
        seen,
        sources,
        repo_root,
    )
}

fn discover_items(
    items: &[Item],
    context: &Path,
    path_context: &Path,
    seen: &mut BTreeSet<PathBuf>,
    sources: &mut Vec<(PathBuf, String)>,
    repo_root: &Path,
) -> Result<(), String> {
    for item in items {
        let Item::Mod(module) = item else { continue };
        if excludes_when_test_is_false(&module.attrs) {
            continue;
        }
        if let Some((_, nested)) = &module.content {
            let nested_context = context.join(module.ident.to_string());
            discover_items(
                nested,
                &nested_context,
                &nested_context,
                seen,
                sources,
                repo_root,
            )?;
        } else {
            for path in
                contained_module_paths(resolve_external(module, context, path_context)?, repo_root)?
            {
                discover_file(&path, false, seen, sources, repo_root)?;
            }
        }
    }
    Ok(())
}

fn contained_module_paths(paths: Vec<PathBuf>, repo_root: &Path) -> Result<Vec<PathBuf>, String> {
    for path in &paths {
        if !path.starts_with(repo_root) {
            return Err(format!(
                "module source {} escapes repository {}",
                path.display(),
                repo_root.display()
            ));
        }
    }
    Ok(paths)
}
