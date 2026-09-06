use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use syn::Item;
use syn::visit::Visit;

use super::read_parsed;
use super::roots::crate_roots_with_context;
use crate::source_scan::cfg::excludes_when_test_is_false;

mod attribute_provenance;
pub(super) mod attrs;
mod collect;
mod containment;
mod imports;
mod include_provenance;
mod included;
mod macro_paths;
mod macro_reachability;
mod root;
mod symbols;
mod syntax;
mod walk;
pub(super) use collect::{symbols_for_classification, symbols_for_root};
use containment::contained_source;
pub(super) use symbols::Symbols;
pub(super) use syntax::{LexicalContext, SourceInclude, SourceModule, Syntax};

type SeenModule = (String, Vec<String>, PathBuf, PathBuf, String);
type SeenRoot = (PathBuf, String);

/// Production dependencies, grouped by top-level logical module subject.
pub fn production_module_dependencies(
    repo_root: &Path,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let canonical_root = fs::canonicalize(repo_root)
        .map_err(|error| format!("cannot resolve repository {}: {error}", repo_root.display()))?;
    let mut edges = BTreeMap::new();
    for root in crate_roots_with_context(repo_root)? {
        let symbols = symbols_for_root(
            &root.path,
            &canonical_root,
            &root.aliases,
            &root.audited_derive_crates,
        )?;
        // Alias provenance is crate-root-relative. The same physical module
        // included by two roots must be measured once in each symbol universe.
        let mut seen = BTreeSet::new();
        let mut active_files = BTreeSet::new();
        root::scan_root_file(
            &root.path,
            &canonical_root,
            &symbols,
            &mut BTreeSet::new(),
            &mut seen,
            &mut active_files,
            &mut edges,
        )?;
    }
    if edges.is_empty() {
        return Err(format!(
            "no production Rust modules are declared under {}",
            repo_root.display()
        ));
    }
    Ok(edges)
}

#[allow(clippy::too_many_arguments)]
fn scan_items(
    items: &[Item],
    subject: &str,
    containing_file: &Path,
    context: &Path,
    path_context: &Path,
    logical_module: &[String],
    repo_root: &Path,
    symbols: &Symbols,
    seen: &mut BTreeSet<SeenModule>,
    active_files: &mut BTreeSet<PathBuf>,
    deps: &mut BTreeSet<String>,
    lexical: LexicalContext,
) -> Result<(), String> {
    let syntax = scan_syntax(items, containing_file, logical_module, symbols, lexical)?;
    deps.extend(syntax.dependencies);
    for include in syntax.includes {
        let (path, lexical) = include_path(include, containing_file)?;
        included::scan_included_file(
            &path,
            subject,
            logical_module,
            context,
            repo_root,
            symbols,
            seen,
            active_files,
            deps,
            lexical,
        )?;
    }
    for local in syntax.modules {
        let mut nested_logical = logical_module.to_vec();
        nested_logical.push(local.module.ident.to_string());
        walk::scan_module(
            &local.module,
            subject,
            containing_file,
            context,
            path_context,
            &nested_logical,
            repo_root,
            symbols,
            seen,
            active_files,
            deps,
            local.context,
        )?;
    }
    for item in items {
        if item_is_non_production(item) {
            continue;
        }
        let Item::Mod(module) = item else { continue };
        let mut nested_logical = logical_module.to_vec();
        nested_logical.push(module.ident.to_string());
        walk::scan_module(
            module,
            subject,
            containing_file,
            context,
            path_context,
            &nested_logical,
            repo_root,
            symbols,
            seen,
            active_files,
            deps,
            LexicalContext::default(),
        )?;
    }
    Ok(())
}

pub(super) fn scan_syntax<'symbols>(
    items: &[Item],
    containing_file: &Path,
    logical_module: &[String],
    symbols: &'symbols Symbols,
    lexical: LexicalContext,
) -> Result<Syntax<'symbols>, String> {
    require_measured(
        syntax_from_items(items, logical_module, symbols, lexical),
        containing_file,
    )
}

pub(super) fn scan_syntax_for_classification<'symbols>(
    items: &[Item],
    logical_module: &[String],
    symbols: &'symbols Symbols,
    lexical: LexicalContext,
) -> (Syntax<'symbols>, bool) {
    let syntax = syntax_from_items(items, logical_module, symbols, lexical);
    let complete = syntax.uncertainties.is_empty();
    (syntax, complete)
}

fn syntax_from_items<'symbols>(
    items: &[Item],
    logical_module: &[String],
    symbols: &'symbols Symbols,
    lexical: LexicalContext,
) -> Syntax<'symbols> {
    let block_scope = lexical.is_block_scope();
    let mut syntax = Syntax::with_context(logical_module, symbols, lexical);
    for item in items {
        if !item_is_non_production(item)
            && let Item::Use(item) = item
        {
            syntax.prepare_use(item);
        }
    }
    syntax.finish_imports(block_scope);
    for item in items {
        if item_is_non_production(item) {
            continue;
        }
        if let Item::Mod(module) = item {
            for attribute in &module.attrs {
                syntax.visit_attribute(attribute);
            }
        } else {
            syntax.visit_item(item);
        }
    }
    syntax
}

pub(super) fn scan_expression_syntax<'symbols>(
    expression: &syn::Expr,
    containing_file: &Path,
    logical_module: &[String],
    symbols: &'symbols Symbols,
    lexical: LexicalContext,
) -> Result<Syntax<'symbols>, String> {
    require_measured(
        syntax_from_expression(expression, logical_module, symbols, lexical),
        containing_file,
    )
}

pub(super) fn scan_expression_for_classification<'symbols>(
    expression: &syn::Expr,
    logical_module: &[String],
    symbols: &'symbols Symbols,
    lexical: LexicalContext,
) -> (Syntax<'symbols>, bool) {
    let syntax = syntax_from_expression(expression, logical_module, symbols, lexical);
    let complete = syntax.uncertainties.is_empty();
    (syntax, complete)
}

fn syntax_from_expression<'symbols>(
    expression: &syn::Expr,
    logical_module: &[String],
    symbols: &'symbols Symbols,
    lexical: LexicalContext,
) -> Syntax<'symbols> {
    let mut syntax = Syntax::with_context(logical_module, symbols, lexical);
    syntax.visit_expr(expression);
    syntax
}

fn require_measured<'symbols>(
    syntax: Syntax<'symbols>,
    containing_file: &Path,
) -> Result<Syntax<'symbols>, String> {
    if let Some(reason) = syntax.uncertainties.first() {
        return Err(format!(
            "production syntax in {} cannot be measured: {reason}",
            containing_file.display()
        ));
    }
    Ok(syntax)
}

fn include_path(
    include: SourceInclude,
    containing_file: &Path,
) -> Result<(PathBuf, LexicalContext), String> {
    let relative = include.path.map_err(|tokens| {
        format!(
            "dynamic source include in {} cannot be measured: include!({tokens})",
            containing_file.display()
        )
    })?;
    Ok((
        containing_file
            .parent()
            .unwrap_or(Path::new("."))
            .join(relative),
        include.context,
    ))
}

pub(super) fn item_is_non_production(item: &Item) -> bool {
    let attrs = match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        _ => return false,
    };
    excludes_when_test_is_false(attrs)
}
