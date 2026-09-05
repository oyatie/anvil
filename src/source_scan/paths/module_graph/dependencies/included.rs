use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use syn::visit::Visit;

use super::symbols::Symbols;
use super::syntax::LexicalContext;
use super::{SeenModule, Syntax, contained_source, include_path, scan_items, walk};

#[allow(clippy::too_many_arguments)]
pub(super) fn scan_included_file(
    path: &Path,
    subject: &str,
    logical_module: &[String],
    _context: &Path,
    repo_root: &Path,
    symbols: &Symbols,
    seen: &mut BTreeSet<SeenModule>,
    active_files: &mut BTreeSet<std::path::PathBuf>,
    deps: &mut BTreeSet<String>,
    lexical: LexicalContext,
) -> Result<(), String> {
    let canonical = contained_source(path, repo_root)?;
    // `include!` preserves the logical Rust scope, but conventional module
    // lookup inside the included token file starts at that file's directory.
    let context = path.parent().unwrap_or(Path::new("."));
    if !active_files.insert(canonical.clone()) {
        return Err(format!(
            "production include cycle reaches {}",
            canonical.display()
        ));
    }
    let identity = (
        subject.to_owned(),
        logical_module.to_vec(),
        canonical.clone(),
        context.to_path_buf(),
        lexical.identity(),
    );
    if !seen.insert(identity) {
        active_files.remove(&canonical);
        return Ok(());
    }
    let source = fs::read_to_string(path)
        .map_err(|error| format!("cannot read included source {}: {error}", path.display()))?;
    if let Ok(file) = syn::parse_file(&source) {
        let result = scan_items(
            &file.items,
            subject,
            path,
            context,
            path.parent().unwrap_or(Path::new(".")),
            logical_module,
            repo_root,
            symbols,
            seen,
            active_files,
            deps,
            lexical,
        );
        active_files.remove(&canonical);
        return result;
    }
    let expression = syn::parse_str::<syn::Expr>(&source).map_err(|error| {
        format!(
            "included Rust {} is neither an item source nor an expression: {error}",
            path.display()
        )
    })?;
    let mut syntax = Syntax::with_context(logical_module, symbols, lexical);
    syntax.visit_expr(&expression);
    if let Some(reason) = syntax.uncertainties.first() {
        return Err(format!(
            "production syntax in {} cannot be measured: {reason}",
            path.display()
        ));
    }
    deps.extend(syntax.dependencies);
    for include in syntax.includes {
        let (nested, lexical) = include_path(include, path)?;
        scan_included_file(
            &nested,
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
            path,
            context,
            path.parent().unwrap_or(Path::new(".")),
            &nested_logical,
            repo_root,
            symbols,
            seen,
            active_files,
            deps,
            local.context,
        )?;
    }
    active_files.remove(&canonical);
    Ok(())
}
