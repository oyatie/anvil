use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use syn::Item;

use super::{
    LexicalContext, SeenModule, SeenRoot, Symbols, contained_source, include_path,
    item_is_non_production, read_parsed, scan_expression_syntax, scan_syntax, walk,
};

pub(super) fn scan_root_file(
    path: &Path,
    repo_root: &Path,
    symbols: &Symbols,
    seen_roots: &mut BTreeSet<SeenRoot>,
    seen_modules: &mut BTreeSet<SeenModule>,
    active_files: &mut BTreeSet<PathBuf>,
    edges: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<(), String> {
    let canonical = contained_source(path, repo_root)?;
    if !active_files.insert(canonical.clone()) {
        return Err(format!(
            "production source cycle reaches {}",
            canonical.display()
        ));
    }
    let lexical = LexicalContext::default();
    if !seen_roots.insert((canonical.clone(), lexical.identity())) {
        active_files.remove(&canonical);
        return Ok(());
    }
    let (_, parsed) = read_parsed(path)?;
    let context = path.parent().unwrap_or(Path::new("."));
    let result = scan_root_items(
        &parsed.items,
        path,
        context,
        context,
        repo_root,
        symbols,
        seen_roots,
        seen_modules,
        active_files,
        edges,
        lexical,
    );
    active_files.remove(&canonical);
    result
}

#[allow(clippy::too_many_arguments)]
fn scan_root_items(
    items: &[Item],
    containing_file: &Path,
    context: &Path,
    path_context: &Path,
    repo_root: &Path,
    symbols: &Symbols,
    seen_roots: &mut BTreeSet<SeenRoot>,
    seen_modules: &mut BTreeSet<SeenModule>,
    active_files: &mut BTreeSet<PathBuf>,
    edges: &mut BTreeMap<String, BTreeSet<String>>,
    lexical: LexicalContext,
) -> Result<(), String> {
    let syntax = scan_syntax(items, containing_file, &[], symbols, lexical)?;
    for local in syntax.modules {
        scan_subject(
            &local.module,
            containing_file,
            context,
            path_context,
            repo_root,
            symbols,
            seen_modules,
            active_files,
            edges,
            local.context,
        )?;
    }
    for include in syntax.includes {
        let (path, lexical) = include_path(include, containing_file)?;
        scan_root_include(
            &path,
            repo_root,
            symbols,
            seen_roots,
            seen_modules,
            active_files,
            edges,
            lexical,
        )?;
    }
    for item in items {
        if item_is_non_production(item) {
            continue;
        }
        let Item::Mod(module) = item else { continue };
        scan_subject(
            module,
            containing_file,
            context,
            path_context,
            repo_root,
            symbols,
            seen_modules,
            active_files,
            edges,
            LexicalContext::default(),
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn scan_root_include(
    path: &Path,
    repo_root: &Path,
    symbols: &Symbols,
    seen_roots: &mut BTreeSet<SeenRoot>,
    seen_modules: &mut BTreeSet<SeenModule>,
    active_files: &mut BTreeSet<PathBuf>,
    edges: &mut BTreeMap<String, BTreeSet<String>>,
    lexical: LexicalContext,
) -> Result<(), String> {
    let canonical = contained_source(path, repo_root)?;
    if !active_files.insert(canonical.clone()) {
        return Err(format!(
            "production include cycle reaches {}",
            canonical.display()
        ));
    }
    let identity = (canonical.clone(), lexical.identity());
    let result = if seen_roots.insert(identity) {
        let source = fs::read_to_string(path)
            .map_err(|error| format!("cannot read included source {}: {error}", path.display()))?;
        let context = path.parent().unwrap_or(Path::new("."));
        if let Ok(parsed) = syn::parse_file(&source) {
            scan_root_items(
                &parsed.items,
                path,
                context,
                context,
                repo_root,
                symbols,
                seen_roots,
                seen_modules,
                active_files,
                edges,
                lexical,
            )
        } else {
            let expression = syn::parse_str::<syn::Expr>(&source).map_err(|error| {
                format!(
                    "included Rust {} is neither an item source nor an expression: {error}",
                    path.display()
                )
            })?;
            scan_root_expression(
                &expression,
                path,
                context,
                repo_root,
                symbols,
                seen_modules,
                active_files,
                edges,
                lexical,
            )
        }
    } else {
        Ok(())
    };
    active_files.remove(&canonical);
    result
}

#[allow(clippy::too_many_arguments)]
fn scan_root_expression(
    expression: &syn::Expr,
    containing_file: &Path,
    context: &Path,
    repo_root: &Path,
    symbols: &Symbols,
    seen_modules: &mut BTreeSet<SeenModule>,
    active_files: &mut BTreeSet<PathBuf>,
    edges: &mut BTreeMap<String, BTreeSet<String>>,
    lexical: LexicalContext,
) -> Result<(), String> {
    let syntax = scan_expression_syntax(expression, containing_file, &[], symbols, lexical)?;
    for include in syntax.includes {
        let (path, lexical) = include_path(include, containing_file)?;
        let canonical = contained_source(&path, repo_root)?;
        if !active_files.insert(canonical.clone()) {
            return Err(format!(
                "production include cycle reaches {}",
                canonical.display()
            ));
        }
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("cannot read included source {}: {error}", path.display()))?;
        let nested = syn::parse_str::<syn::Expr>(&source).map_err(|error| {
            format!(
                "expression include {} cannot be measured as Rust: {error}",
                path.display()
            )
        })?;
        let result = scan_root_expression(
            &nested,
            &path,
            path.parent().unwrap_or(Path::new(".")),
            repo_root,
            symbols,
            seen_modules,
            active_files,
            edges,
            lexical,
        );
        active_files.remove(&canonical);
        result?;
    }
    for local in syntax.modules {
        scan_subject(
            &local.module,
            containing_file,
            context,
            context,
            repo_root,
            symbols,
            seen_modules,
            active_files,
            edges,
            local.context,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn scan_subject(
    module: &syn::ItemMod,
    containing_file: &Path,
    context: &Path,
    path_context: &Path,
    repo_root: &Path,
    symbols: &Symbols,
    seen_modules: &mut BTreeSet<SeenModule>,
    active_files: &mut BTreeSet<PathBuf>,
    edges: &mut BTreeMap<String, BTreeSet<String>>,
    lexical: LexicalContext,
) -> Result<(), String> {
    let subject = module.ident.to_string();
    let deps = edges.entry(subject.clone()).or_default();
    walk::scan_module(
        module,
        &subject,
        containing_file,
        context,
        path_context,
        std::slice::from_ref(&subject),
        repo_root,
        symbols,
        seen_modules,
        active_files,
        deps,
        lexical,
    )
}
