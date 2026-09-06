use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use syn::Item;

use super::{RoleMap, Roles, nested};
use crate::source_scan::paths::module_graph::child_module_dir;
use crate::source_scan::paths::module_graph::dependencies::{
    LexicalContext, SourceModule, Symbols, item_is_non_production,
    scan_expression_for_classification, scan_syntax_for_classification, symbols_for_classification,
};

mod modules;

type Seen = (PathBuf, bool, Vec<String>, PathBuf, String);

#[cfg(test)]
mod tests;

pub(super) fn module_roles_from_roots(
    repo_root: &Path,
    roots: &[PathBuf],
) -> Result<RoleMap, String> {
    let canonical_repo = fs::canonicalize(repo_root).map_err(|error| {
        format!(
            "cannot resolve repository {} while classifying test modules: {error}",
            repo_root.display()
        )
    })?;
    let contexts =
        crate::source_scan::paths::module_graph::roots::crate_roots_with_context(&canonical_repo)?;
    roles_with_contexts(&canonical_repo, roots, &contexts)
}

fn roles_with_contexts(
    canonical_repo: &Path,
    roots: &[PathBuf],
    contexts: &[crate::source_scan::paths::module_graph::roots::CrateRoot],
) -> Result<RoleMap, String> {
    let mut roles = BTreeMap::new();
    let mut complete = true;
    for root in roots {
        let canonical_root = contained(root, canonical_repo, "crate root")?;
        let mut matching = contexts
            .iter()
            .filter(|context| context.path == canonical_root)
            .map(Some)
            .collect::<Vec<_>>();
        if matching.is_empty() {
            matching.push(None);
        }
        for context in matching {
            let empty_aliases = BTreeSet::new();
            let empty_audited = BTreeMap::new();
            let (symbols, root_complete) = symbols_for_classification(
                &canonical_root,
                canonical_repo,
                context.map_or(&empty_aliases, |context| &context.aliases),
                context.map_or(&empty_audited, |context| &context.audited_derive_crates),
            )?;
            complete &= root_complete;
            let mut seen = BTreeSet::new();
            let mut active = BTreeSet::new();
            visit_file(
                &canonical_root,
                canonical_repo,
                false,
                true,
                &[],
                LexicalContext::default(),
                &symbols,
                &mut seen,
                &mut active,
                &mut roles,
            )?;
        }
    }
    Ok(RoleMap { roles, complete })
}

#[allow(clippy::too_many_arguments)]
fn visit_file(
    path: &Path,
    repo: &Path,
    inherited_test: bool,
    is_crate_root: bool,
    scope: &[String],
    lexical: LexicalContext,
    symbols: &Symbols,
    seen: &mut BTreeSet<Seen>,
    active: &mut BTreeSet<PathBuf>,
    roles: &mut BTreeMap<PathBuf, Roles>,
) -> Result<(), String> {
    let context = if is_crate_root {
        path.parent().unwrap_or(Path::new(".")).to_path_buf()
    } else {
        child_module_dir(path)
    };
    let path_context = path.parent().unwrap_or(Path::new("."));
    visit_source(
        path,
        repo,
        inherited_test,
        false,
        scope,
        &context,
        path_context,
        lexical,
        symbols,
        seen,
        active,
        roles,
    )
}

#[allow(clippy::too_many_arguments)]
fn visit_source(
    path: &Path,
    repo: &Path,
    inherited_test: bool,
    allow_expression: bool,
    scope: &[String],
    context: &Path,
    path_context: &Path,
    lexical: LexicalContext,
    symbols: &Symbols,
    seen: &mut BTreeSet<Seen>,
    active: &mut BTreeSet<PathBuf>,
    roles: &mut BTreeMap<PathBuf, Roles>,
) -> Result<(), String> {
    let canonical = contained(path, repo, "declared source")?;
    if !active.insert(canonical.clone()) {
        return Err(format!(
            "production source cycle reaches {}",
            canonical.display()
        ));
    }
    let key = (
        canonical.clone(),
        inherited_test,
        scope.to_vec(),
        context.to_path_buf(),
        lexical.identity(),
    );
    if !seen.insert(key) {
        active.remove(&canonical);
        return Ok(());
    }
    record_role(roles.entry(canonical.clone()).or_default(), inherited_test);
    let result = parse_and_visit(
        &canonical,
        repo,
        inherited_test,
        allow_expression,
        scope,
        context,
        path_context,
        lexical,
        symbols,
        seen,
        active,
        roles,
    );
    active.remove(&canonical);
    result
}

mod source;
use source::parse_and_visit;

#[allow(clippy::too_many_arguments)]
fn visit_items(
    items: &[Item],
    file: &Path,
    repo: &Path,
    context: &Path,
    path_context: &Path,
    scope: &[String],
    inherited_test: bool,
    lexical: LexicalContext,
    symbols: &Symbols,
    seen: &mut BTreeSet<Seen>,
    active: &mut BTreeSet<PathBuf>,
    roles: &mut BTreeMap<PathBuf, Roles>,
) -> Result<(), String> {
    for item in items {
        if let Item::Mod(module) = item {
            modules::visit_module(
                SourceModule {
                    module: module.clone(),
                    context: LexicalContext::default(),
                    block_local: false,
                },
                file,
                repo,
                context,
                path_context,
                scope,
                inherited_test,
                symbols,
                seen,
                active,
                roles,
            )?;
        }
    }

    let (syntax, _) = scan_syntax_for_classification(items, scope, symbols, lexical);
    modules::visit_syntax(
        syntax.modules,
        syntax.includes,
        file,
        repo,
        context,
        path_context,
        scope,
        inherited_test,
        symbols,
        seen,
        active,
        roles,
    )?;

    for item in items
        .iter()
        .filter(|item| !matches!(item, Item::Mod(_)) && item_is_non_production(item))
    {
        let nested = nested::NestedSources::in_test_item(item);
        modules::visit_test_sources(
            nested.modules,
            nested.includes,
            file,
            repo,
            context,
            path_context,
            scope,
            symbols,
            seen,
            active,
            roles,
        )?;
    }
    Ok(())
}

fn contained(path: &Path, repo: &Path, label: &str) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("cannot resolve {label} {}: {error}", path.display()))?;
    if !canonical.starts_with(repo) {
        return Err(format!(
            "{label} {} escapes repository {}",
            path.display(),
            repo.display()
        ));
    }
    Ok(canonical)
}

fn record_role(role: &mut Roles, test: bool) {
    if test {
        role.test = true;
    } else {
        role.production = true;
    }
}
