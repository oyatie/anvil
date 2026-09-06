use std::collections::BTreeSet;
use std::path::Path;

use syn::ItemMod;

use super::symbols::Symbols;
use super::syntax::LexicalContext;
use super::{SeenModule, contained_source, scan_items};
use crate::source_scan::paths::module_graph::{child_module_dir, read_parsed, resolve_external};

#[allow(clippy::too_many_arguments)]
pub(super) fn scan_module(
    module: &ItemMod,
    subject: &str,
    containing_file: &Path,
    context: &Path,
    path_context: &Path,
    logical_module: &[String],
    repo_root: &Path,
    symbols: &Symbols,
    seen: &mut BTreeSet<SeenModule>,
    active_files: &mut BTreeSet<std::path::PathBuf>,
    deps: &mut BTreeSet<String>,
    lexical: LexicalContext,
) -> Result<(), String> {
    if let Some((_, items)) = &module.content {
        let nested_context = context.join(module.ident.to_string());
        scan_items(
            items,
            subject,
            containing_file,
            &nested_context,
            &nested_context,
            logical_module,
            repo_root,
            symbols,
            seen,
            active_files,
            deps,
            lexical,
        )
    } else {
        for path in resolve_external(module, context, path_context)? {
            scan_module_file(
                &path,
                subject,
                logical_module,
                repo_root,
                symbols,
                seen,
                active_files,
                deps,
                lexical.clone(),
            )?;
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn scan_module_file(
    path: &Path,
    subject: &str,
    logical_module: &[String],
    repo_root: &Path,
    symbols: &Symbols,
    seen: &mut BTreeSet<SeenModule>,
    active_files: &mut BTreeSet<std::path::PathBuf>,
    deps: &mut BTreeSet<String>,
    lexical: LexicalContext,
) -> Result<(), String> {
    let context = child_module_dir(path);
    let path_context = path.parent().unwrap_or(Path::new("."));
    scan_file_with_context(
        path,
        subject,
        logical_module,
        &context,
        path_context,
        repo_root,
        symbols,
        seen,
        active_files,
        deps,
        lexical,
    )
}

#[allow(clippy::too_many_arguments)]
fn scan_file_with_context(
    path: &Path,
    subject: &str,
    logical_module: &[String],
    context: &Path,
    path_context: &Path,
    repo_root: &Path,
    symbols: &Symbols,
    seen: &mut BTreeSet<SeenModule>,
    active_files: &mut BTreeSet<std::path::PathBuf>,
    deps: &mut BTreeSet<String>,
    lexical: LexicalContext,
) -> Result<(), String> {
    let canonical = contained_source(path, repo_root)?;
    if !active_files.insert(canonical.clone()) {
        return Err(format!(
            "production source cycle reaches {}",
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
    let (_, parsed) = read_parsed(path)?;
    let result = scan_items(
        &parsed.items,
        subject,
        path,
        context,
        path_context,
        logical_module,
        repo_root,
        symbols,
        seen,
        active_files,
        deps,
        lexical,
    );
    active_files.remove(&canonical);
    result
}
