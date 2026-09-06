use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use syn::ItemUse;

use super::imports::{Import, collect_imports, ident_name};
use super::symbols::Symbols;

#[cfg(test)]
mod audited_tests;
mod macro_contract;
mod scope;
mod visitor;

#[derive(Clone, Default)]
pub(in crate::source_scan::paths::module_graph) struct LexicalContext {
    aliases: BTreeMap<String, Vec<Vec<String>>>,
    macros: BTreeMap<String, Vec<proc_macro2::TokenStream>>,
    block_depth: usize,
}

impl LexicalContext {
    pub(in crate::source_scan::paths::module_graph) fn identity(&self) -> String {
        let aliases = self
            .aliases
            .iter()
            .map(|(name, targets)| format!("{name}={targets:?}"));
        let macros = self
            .macros
            .iter()
            .flat_map(|(name, bodies)| bodies.iter().map(move |body| format!("{name}={}", body)));
        std::iter::once(format!("block_depth={}", self.block_depth))
            .chain(aliases)
            .chain(macros)
            .collect::<Vec<_>>()
            .join(";")
    }

    pub(in crate::source_scan::paths::module_graph) fn is_block_scope(&self) -> bool {
        self.block_depth > 0
    }

    pub(in crate::source_scan::paths::module_graph) fn module_child(&self) -> Self {
        Self {
            aliases: BTreeMap::new(),
            macros: self.macros.clone(),
            block_depth: 0,
        }
    }
}

pub(in crate::source_scan::paths::module_graph) struct SourceInclude {
    pub(in crate::source_scan::paths::module_graph) path: Result<PathBuf, String>,
    pub(in crate::source_scan::paths::module_graph) context: LexicalContext,
}

pub(in crate::source_scan::paths::module_graph) struct SourceModule {
    pub(in crate::source_scan::paths::module_graph) module: syn::ItemMod,
    pub(in crate::source_scan::paths::module_graph) context: LexicalContext,
    pub(in crate::source_scan::paths::module_graph) block_local: bool,
}

pub(in crate::source_scan::paths::module_graph) struct Syntax<'symbols> {
    pub(in crate::source_scan::paths::module_graph) dependencies: BTreeSet<String>,
    pub(in crate::source_scan::paths::module_graph) includes: Vec<SourceInclude>,
    pub(in crate::source_scan::paths::module_graph) modules: Vec<SourceModule>,
    pub(in crate::source_scan::paths::module_graph) uncertainties: Vec<String>,
    logical_module: Vec<String>,
    symbols: &'symbols Symbols,
    lexical: LexicalContext,
    imports: Vec<Import>,
    reachable_macro_bodies: BTreeSet<String>,
}

impl<'symbols> Syntax<'symbols> {
    pub(super) fn new(logical_module: &[String], symbols: &'symbols Symbols) -> Self {
        Self {
            dependencies: BTreeSet::new(),
            includes: Vec::new(),
            modules: Vec::new(),
            uncertainties: Vec::new(),
            logical_module: logical_module.to_vec(),
            symbols,
            lexical: LexicalContext::default(),
            imports: Vec::new(),
            reachable_macro_bodies: BTreeSet::new(),
        }
    }

    pub(super) fn with_context(
        logical_module: &[String],
        symbols: &'symbols Symbols,
        lexical: LexicalContext,
    ) -> Self {
        Self {
            lexical,
            ..Self::new(logical_module, symbols)
        }
    }

    pub(super) fn prepare_use(&mut self, item: &ItemUse) {
        collect_imports(&item.tree, &mut Vec::new(), &mut self.imports);
    }

    pub(super) fn finish_imports(&mut self, lexical: bool) {
        let imports = std::mem::take(&mut self.imports);
        self.install_imports(&imports, lexical);
    }

    fn install_imports(&mut self, imports: &[Import], lexical: bool) {
        if lexical {
            for import in imports {
                match &import.binding {
                    Some(binding) if binding != "_" => self
                        .lexical
                        .aliases
                        .entry(binding.clone())
                        .or_default()
                        .push(import.target.clone()),
                    None => self
                        .lexical
                        .aliases
                        .entry("*".to_owned())
                        .or_default()
                        .push(import.target.clone()),
                    _ => {}
                }
            }
        }
        for import in imports {
            self.record_segments(&import.target);
            if import.binding.is_none() {
                for target in self.symbols.glob_targets(
                    &import.target,
                    &self.logical_module,
                    &self.lexical.aliases,
                ) {
                    if target.local {
                        self.record_absolute(&target.segments);
                    }
                }
            }
        }
    }

    fn resolve(&self, segments: &[String]) -> BTreeSet<super::symbols::ResolvedPath> {
        self.symbols
            .resolve(segments, &self.logical_module, &self.lexical.aliases)
    }

    fn record_segments(&mut self, segments: &[String]) {
        for resolved in self.resolve(segments) {
            if resolved.local {
                self.record_absolute(&resolved.segments);
            }
        }
    }

    fn record_absolute(&mut self, absolute: &[String]) {
        let Some(first) = absolute.first() else {
            return;
        };
        if self.logical_module.first() == Some(first) {
            return;
        }
        let mut dependency = first.clone();
        if let Some(second) = absolute.get(1) {
            dependency.push('/');
            dependency.push_str(second);
        }
        self.dependencies.insert(dependency);
    }

    fn record_path(&mut self, path: &syn::Path) {
        self.record_segments(
            &path
                .segments
                .iter()
                .map(|segment| ident_name(&segment.ident))
                .collect::<Vec<_>>(),
        );
    }

    fn record_macro_paths(&mut self, tokens: proc_macro2::TokenStream) {
        let paths = super::macro_paths::analyze(tokens);
        if paths.dynamic_local {
            self.uncertainties
                .push("macro constructs a crate-relative path from a metavariable".to_owned());
        }
        for segments in paths.literal {
            self.record_segments(&segments);
        }
    }

    fn record_reachable_macro_body(&mut self, tokens: proc_macro2::TokenStream) {
        if !self.reachable_macro_bodies.insert(tokens.to_string()) {
            return;
        }
        self.record_macro_paths(tokens.clone());
        let reachable = super::macro_reachability::analyze(tokens);
        if reachable.forwards_syntax {
            self.uncertainties.push(
                "reachable macro forwards metavariables that may synthesize an include, module, or local path"
                    .to_owned(),
            );
        }
        if reachable.declares_module {
            self.uncertainties.push(
                "reachable macro expansion declares a module whose source cannot be proven"
                    .to_owned(),
            );
        }
        for invocation in reachable.invocations {
            match super::include_provenance::classify_include_segments(
                &invocation.path,
                false,
                self.symbols,
                &self.logical_module,
                &self.lexical.aliases,
                &self.lexical.macros,
            ) {
                super::include_provenance::IncludeProvenance::Builtin => {
                    self.includes.push(SourceInclude {
                        path: syn::parse2::<syn::LitStr>(invocation.tokens)
                            .map(|literal| PathBuf::from(literal.value()))
                            .map_err(|error| error.to_string()),
                        context: self.lexical.clone(),
                    });
                }
                super::include_provenance::IncludeProvenance::Ambiguous => self
                    .uncertainties
                    .push("reachable macro has ambiguous include provenance".to_owned()),
                super::include_provenance::IncludeProvenance::Other => {
                    let bodies = self.symbols.macro_bodies(
                        &self.logical_module,
                        &invocation.path,
                        &self.lexical.aliases,
                    );
                    let mut locally_measured = !bodies.is_empty();
                    for body in bodies {
                        self.record_reachable_macro_body(body);
                    }
                    if invocation.path.len() == 1
                        && let Some(bodies) = self.lexical.macros.get(&invocation.path[0]).cloned()
                    {
                        locally_measured = true;
                        for body in bodies {
                            self.record_reachable_macro_body(body);
                        }
                    }
                    if self
                        .symbols
                        .audited_function_macro(
                            &invocation.path,
                            &self.logical_module,
                            &self.lexical.aliases,
                            &self.lexical.macros,
                        )
                        .is_some_and(|(package, symbol)| {
                            macro_contract::public_function_tokens(
                                (&package, &symbol),
                                invocation.tokens.clone(),
                            )
                        })
                    {
                        locally_measured = true;
                    }
                    if !locally_measured
                        && !visitor::known_unshadowed_builtin_macro(self, &invocation.path)
                    {
                        self.uncertainties.push(format!(
                            "reachable macro {}! has no locally measurable expansion",
                            invocation.path.join("::")
                        ));
                    }
                    self.record_reachable_macro_body(invocation.tokens);
                }
            }
        }
    }
}
