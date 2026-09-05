use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::{Seen, visit_file, visit_items, visit_source};
use crate::source_scan::cfg::excludes_when_test_is_false;
use crate::source_scan::paths::module_graph::declaration::Roles;
use crate::source_scan::paths::module_graph::dependencies::{
    LexicalContext, SourceInclude, SourceModule, Symbols,
};
use crate::source_scan::paths::module_graph::resolve_external;

#[allow(clippy::too_many_arguments)]
pub(super) fn visit_syntax(
    modules: Vec<SourceModule>,
    includes: Vec<SourceInclude>,
    file: &Path,
    repo: &Path,
    context: &Path,
    path_context: &Path,
    scope: &[String],
    inherited_test: bool,
    symbols: &Symbols,
    seen: &mut BTreeSet<Seen>,
    active: &mut BTreeSet<PathBuf>,
    roles: &mut BTreeMap<PathBuf, Roles>,
) -> Result<(), String> {
    for module in modules {
        visit_module(
            module,
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
    for include in includes {
        let relative = include.path.map_err(|tokens| {
            format!(
                "dynamic production include in {} cannot be classified: include!({tokens})",
                file.display()
            )
        })?;
        let included = file.parent().unwrap_or(Path::new(".")).join(relative);
        let included_context = included.parent().unwrap_or(Path::new("."));
        visit_source(
            &included,
            repo,
            inherited_test,
            true,
            scope,
            included_context,
            included_context,
            include.context,
            symbols,
            seen,
            active,
            roles,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn visit_test_sources(
    modules: Vec<syn::ItemMod>,
    includes: Vec<Result<PathBuf, String>>,
    file: &Path,
    repo: &Path,
    context: &Path,
    path_context: &Path,
    scope: &[String],
    symbols: &Symbols,
    seen: &mut BTreeSet<Seen>,
    active: &mut BTreeSet<PathBuf>,
    roles: &mut BTreeMap<PathBuf, Roles>,
) -> Result<(), String> {
    for module in modules {
        visit_module(
            SourceModule {
                module,
                context: LexicalContext::default(),
                block_local: false,
            },
            file,
            repo,
            context,
            path_context,
            scope,
            true,
            symbols,
            seen,
            active,
            roles,
        )?;
    }
    for include in includes {
        let relative = include.map_err(|error| {
            format!(
                "dynamic test include in {} cannot be classified: {error}",
                file.display()
            )
        })?;
        let included = file.parent().unwrap_or(Path::new(".")).join(relative);
        let included_context = included.parent().unwrap_or(Path::new("."));
        visit_source(
            &included,
            repo,
            true,
            true,
            scope,
            included_context,
            included_context,
            LexicalContext::default(),
            symbols,
            seen,
            active,
            roles,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn visit_module(
    source: SourceModule,
    file: &Path,
    repo: &Path,
    context: &Path,
    path_context: &Path,
    scope: &[String],
    inherited_test: bool,
    symbols: &Symbols,
    seen: &mut BTreeSet<Seen>,
    active: &mut BTreeSet<PathBuf>,
    roles: &mut BTreeMap<PathBuf, Roles>,
) -> Result<(), String> {
    let module = source.module;
    let is_test = inherited_test || excludes_when_test_is_false(&module.attrs);
    let mut nested_scope = scope.to_vec();
    nested_scope.push(module.ident.to_string());
    if let Some((_, items)) = module.content {
        let nested_context = context.join(module.ident.to_string());
        return visit_items(
            &items,
            file,
            repo,
            &nested_context,
            &nested_context,
            &nested_scope,
            is_test,
            source.context,
            symbols,
            seen,
            active,
            roles,
        );
    }
    for path in resolve_external(&module, context, path_context)? {
        visit_file(
            &path,
            repo,
            is_test,
            false,
            &nested_scope,
            source.context.clone(),
            symbols,
            seen,
            active,
            roles,
        )?;
    }
    Ok(())
}
