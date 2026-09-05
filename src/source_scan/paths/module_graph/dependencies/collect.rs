use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use syn::Item;
use syn::visit::Visit;

use super::symbols::Symbols;
use super::syntax::{LexicalContext, SourceModule};
use super::{Syntax, contained_source, item_is_non_production};
use crate::source_scan::paths::module_graph::{child_module_dir, read_parsed, resolve_external};

mod included;

type Seen = (Vec<String>, PathBuf, PathBuf, String);

pub(in crate::source_scan::paths::module_graph) fn symbols_for_root(
    path: &Path,
    repo_root: &Path,
    aliases: &BTreeSet<String>,
    audited_derive_crates: &BTreeMap<String, String>,
) -> Result<Symbols, String> {
    collect_symbols(path, repo_root, aliases, audited_derive_crates, true)
        .map(|(symbols, _)| symbols)
}

pub(in crate::source_scan::paths::module_graph) fn symbols_for_classification(
    path: &Path,
    repo_root: &Path,
) -> Result<(Symbols, bool), String> {
    collect_symbols(path, repo_root, &BTreeSet::new(), &BTreeMap::new(), false)
}

fn collect_symbols(
    path: &Path,
    repo_root: &Path,
    aliases: &BTreeSet<String>,
    audited_derive_crates: &BTreeMap<String, String>,
    strict: bool,
) -> Result<(Symbols, bool), String> {
    let mut collector = Collector {
        symbols: Symbols::default(),
        seen: BTreeSet::new(),
        active_files: BTreeSet::new(),
        repo_root,
        strict,
        complete: true,
    };
    for alias in aliases {
        collector.symbols.add_crate_alias(alias);
    }
    for (alias, package) in audited_derive_crates {
        collector.symbols.add_audited_derive_crate(alias, package);
    }
    let context = path.parent().unwrap_or(Path::new("."));
    collector.file(path, &[], context, context, LexicalContext::default())?;
    Ok((collector.symbols, collector.complete))
}

struct Collector<'root> {
    symbols: Symbols,
    seen: BTreeSet<Seen>,
    active_files: BTreeSet<PathBuf>,
    repo_root: &'root Path,
    strict: bool,
    complete: bool,
}

impl Collector<'_> {
    fn file(
        &mut self,
        path: &Path,
        scope: &[String],
        context: &Path,
        path_context: &Path,
        lexical: LexicalContext,
    ) -> Result<(), String> {
        let canonical = contained_source(path, self.repo_root)?;
        if !self.active_files.insert(canonical.clone()) {
            return Err(format!(
                "production source cycle reaches {}",
                canonical.display()
            ));
        }
        let identity = (
            scope.to_vec(),
            canonical.clone(),
            context.to_path_buf(),
            lexical.identity(),
        );
        if !self.seen.insert(identity) {
            self.active_files.remove(&canonical);
            return Ok(());
        }
        let (_, parsed) = read_parsed(path)?;
        let result = self.items(&parsed.items, path, scope, context, path_context, lexical);
        self.active_files.remove(&canonical);
        result
    }

    fn items(
        &mut self,
        items: &[Item],
        containing_file: &Path,
        scope: &[String],
        context: &Path,
        path_context: &Path,
        lexical: LexicalContext,
    ) -> Result<(), String> {
        let block_scope = lexical.is_block_scope();
        let module_context = lexical.module_child();
        if !block_scope {
            for item in items {
                if item_is_non_production(item) {
                    continue;
                }
                match item {
                    Item::Use(item) => {
                        self.symbols.add_use(scope, item);
                    }
                    Item::ExternCrate(item) => self.symbols.add_extern_crate(scope, item),
                    Item::Macro(item) => self.symbols.add_macro(scope, item),
                    Item::Mod(item) => self.symbols.add_module(scope, item),
                    _ => {}
                }
            }
        }

        for item in items {
            if item_is_non_production(item) {
                continue;
            }
            let Item::Mod(module) = item else { continue };
            let mut nested_scope = scope.to_vec();
            nested_scope.push(module.ident.to_string());
            if let Some((_, nested)) = &module.content {
                let nested_context = context.join(module.ident.to_string());
                self.items(
                    nested,
                    containing_file,
                    &nested_scope,
                    &nested_context,
                    &nested_context,
                    module_context.clone(),
                )?;
            } else {
                for path in resolve_external(module, context, path_context)? {
                    let nested_context = child_module_dir(&path);
                    let nested_path_context = path.parent().unwrap_or(Path::new("."));
                    self.file(
                        &path,
                        &nested_scope,
                        &nested_context,
                        nested_path_context,
                        module_context.clone(),
                    )?;
                }
            }
            if module
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("macro_use"))
            {
                self.symbols.hoist_macros(&nested_scope, scope);
            }
        }

        let mut syntax = Syntax::with_context(scope, &self.symbols, lexical);
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
        if let Some(reason) = syntax.uncertainties.first() {
            if self.strict {
                return Err(format!(
                    "production syntax in {} cannot be measured: {reason}",
                    containing_file.display()
                ));
            }
            self.complete = false;
        }
        let includes = std::mem::take(&mut syntax.includes);
        let modules = std::mem::take(&mut syntax.modules);
        drop(syntax);
        for include in includes {
            let relative = include.path.map_err(|tokens| {
                format!(
                    "dynamic source include in {} cannot be measured: include!({tokens})",
                    containing_file.display()
                )
            })?;
            let path = containing_file
                .parent()
                .unwrap_or(Path::new("."))
                .join(relative);
            self.included(&path, scope, context, include.context)?;
        }
        self.local_modules(modules, containing_file, scope, context, path_context)
    }

    fn local_modules(
        &mut self,
        modules: Vec<SourceModule>,
        containing_file: &Path,
        scope: &[String],
        context: &Path,
        path_context: &Path,
    ) -> Result<(), String> {
        for local in modules {
            if !local.block_local {
                self.symbols.add_module(scope, &local.module);
            }
            let mut nested_scope = scope.to_vec();
            nested_scope.push(local.module.ident.to_string());
            if let Some((_, nested)) = &local.module.content {
                let nested_context = context.join(local.module.ident.to_string());
                self.items(
                    nested,
                    containing_file,
                    &nested_scope,
                    &nested_context,
                    &nested_context,
                    local.context,
                )?;
            } else {
                for path in resolve_external(&local.module, context, path_context)? {
                    let nested_context = child_module_dir(&path);
                    let nested_path_context = path.parent().unwrap_or(Path::new("."));
                    self.file(
                        &path,
                        &nested_scope,
                        &nested_context,
                        nested_path_context,
                        local.context.clone(),
                    )?;
                }
            }
        }
        Ok(())
    }
}
