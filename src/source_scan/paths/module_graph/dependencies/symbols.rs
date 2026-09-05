use std::collections::{BTreeMap, BTreeSet};

use super::imports::{Import, collect_imports, ident_name};

mod audited;
mod resolution;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ResolvedPath {
    pub(super) local: bool,
    pub(super) segments: Vec<String>,
}

#[derive(Clone)]
struct Alias {
    scope: Vec<String>,
    target: Vec<String>,
    definite: bool,
    global_extern: bool,
}

#[derive(Default)]
pub(in crate::source_scan::paths::module_graph) struct Symbols {
    aliases: BTreeMap<Vec<String>, Vec<Alias>>,
    modules: BTreeSet<Vec<String>>,
    definite_modules: BTreeSet<Vec<String>>,
    crate_aliases: BTreeSet<String>,
    audited_derive_crates: BTreeMap<String, String>,
    macros: BTreeMap<Vec<String>, Vec<proc_macro2::TokenStream>>,
    unknown_macro_prelude: bool,
}

impl Symbols {
    pub(super) fn add_crate_alias(&mut self, name: &str) {
        self.crate_aliases.insert(name.to_owned());
    }

    pub(super) fn add_module(&mut self, scope: &[String], module: &syn::ItemMod) {
        let mut qualified = scope.to_vec();
        qualified.push(ident_name(&module.ident));
        self.modules.insert(qualified.clone());
        if crate::source_scan::cfg::availability_when_test_is_false(&module.attrs)
            == crate::source_scan::cfg::Truth::AlwaysTrue
        {
            self.definite_modules.insert(qualified);
        }
    }

    pub(super) fn add_use(&mut self, scope: &[String], item: &syn::ItemUse) -> Vec<Import> {
        let mut imports = Vec::new();
        collect_imports(&item.tree, &mut Vec::new(), &mut imports);
        let definite = crate::source_scan::cfg::availability_when_test_is_false(&item.attrs)
            == crate::source_scan::cfg::Truth::AlwaysTrue;
        for import in &imports {
            let Some(binding) = &import.binding else {
                self.add_alias(scope, "*", import.target.clone(), definite, false);
                continue;
            };
            if binding == "_" {
                continue;
            }
            self.add_alias(scope, binding, import.target.clone(), definite, false);
        }
        imports
    }

    pub(super) fn add_extern_crate(&mut self, scope: &[String], item: &syn::ItemExternCrate) {
        if item.ident != "self"
            && item
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("macro_use"))
        {
            self.unknown_macro_prelude = true;
        }
        let binding = item
            .rename
            .as_ref()
            .map(|(_, rename)| ident_name(rename))
            .unwrap_or_else(|| ident_name(&item.ident));
        let target = if item.ident == "self" {
            vec!["crate".to_owned()]
        } else {
            vec![ident_name(&item.ident)]
        };
        let definite = crate::source_scan::cfg::availability_when_test_is_false(&item.attrs)
            == crate::source_scan::cfg::Truth::AlwaysTrue;
        self.add_alias(scope, &binding, target, definite, scope.is_empty());
    }

    pub(super) fn add_macro(&mut self, scope: &[String], item: &syn::ItemMacro) {
        if let Some(name) = &item.ident {
            // `#[macro_export]` exports at the crate root even when the
            // definition is physically nested in a module.
            let mut qualified = if item
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("macro_export"))
            {
                Vec::new()
            } else {
                scope.to_vec()
            };
            qualified.push(ident_name(name));
            let bodies = self.macros.entry(qualified).or_default();
            let identity = item.mac.tokens.to_string();
            if !bodies.iter().any(|body| body.to_string() == identity) {
                bodies.push(item.mac.tokens.clone());
            }
        }
    }

    pub(super) fn hoist_macros(&mut self, from_scope: &[String], to_scope: &[String]) {
        let exports = self
            .macros
            .iter()
            .filter(|(path, _)| path.len() == from_scope.len() + 1 && path.starts_with(from_scope))
            .map(|(path, bodies)| (path.last().cloned().expect("macro name"), bodies.clone()))
            .collect::<Vec<_>>();
        for (name, exported) in exports {
            let mut target = to_scope.to_vec();
            target.push(name);
            let bodies = self.macros.entry(target).or_default();
            for body in exported {
                if !bodies
                    .iter()
                    .any(|existing| existing.to_string() == body.to_string())
                {
                    bodies.push(body);
                }
            }
        }
    }

    fn add_alias(
        &mut self,
        scope: &[String],
        binding: &str,
        target: Vec<String>,
        definite: bool,
        global_extern: bool,
    ) {
        let mut qualified = scope.to_vec();
        qualified.push(binding.to_owned());
        self.aliases
            .entry(qualified.clone())
            .or_default()
            .push(Alias {
                scope: scope.to_vec(),
                target,
                definite,
                global_extern,
            });
    }

    pub(super) fn resolve(
        &self,
        segments: &[String],
        scope: &[String],
        locals: &BTreeMap<String, Vec<Vec<String>>>,
    ) -> BTreeSet<ResolvedPath> {
        resolution::resolve(self, segments, scope, locals)
    }

    pub(super) fn macro_shadowed(&self, scope: &[String], name: &str) -> bool {
        (0..=scope.len()).rev().any(|depth| {
            let mut qualified = scope[..depth].to_vec();
            qualified.push(name.to_owned());
            self.macros.contains_key(&qualified)
        })
    }

    pub(super) fn macro_bodies(
        &self,
        scope: &[String],
        path: &[String],
        locals: &BTreeMap<String, Vec<Vec<String>>>,
    ) -> Vec<proc_macro2::TokenStream> {
        let mut candidates = BTreeSet::new();
        if path.first().is_some_and(|part| part == "crate") {
            candidates.insert(path[1..].to_vec());
        } else if path.len() == 1 {
            for depth in 0..=scope.len() {
                let mut qualified = scope[..depth].to_vec();
                qualified.push(path[0].clone());
                candidates.insert(qualified);
            }
        }
        candidates.extend(
            self.resolve(path, scope, locals)
                .into_iter()
                .filter(|resolved| resolved.local)
                .map(|resolved| resolved.segments),
        );
        candidates
            .into_iter()
            .filter_map(|candidate| self.macros.get(&candidate))
            .flatten()
            .cloned()
            .collect()
    }

    pub(super) fn binding_declared(&self, scope: &[String], name: &str) -> bool {
        self.alias_declared(scope, name) || {
            let mut qualified = scope.to_vec();
            qualified.push(name.to_owned());
            self.modules.contains(&qualified)
        }
    }

    pub(super) fn alias_declared(&self, scope: &[String], name: &str) -> bool {
        let mut qualified = scope.to_vec();
        qualified.push(name.to_owned());
        self.aliases.contains_key(&qualified)
            || (!scope.is_empty()
                && self
                    .aliases
                    .get(&vec![name.to_owned()])
                    .is_some_and(|aliases| aliases.iter().any(|alias| alias.global_extern)))
    }

    pub(super) fn glob_imported(&self, scope: &[String]) -> bool {
        self.alias_declared(scope, "*")
    }

    pub(super) fn unknown_macro_prelude(&self) -> bool {
        self.unknown_macro_prelude
    }

    pub(super) fn glob_targets(
        &self,
        target: &[String],
        scope: &[String],
        locals: &BTreeMap<String, Vec<Vec<String>>>,
    ) -> BTreeSet<ResolvedPath> {
        let mut targets = BTreeSet::new();
        for base in self.resolve(target, scope, locals) {
            if !base.local {
                continue;
            }
            for binding in self
                .aliases
                .keys()
                .chain(self.modules.iter())
                .filter(|binding| {
                    binding.len() == base.segments.len() + 1 && binding.starts_with(&base.segments)
                })
            {
                let mut path = vec!["crate".to_owned()];
                path.extend(binding.iter().cloned());
                targets.extend(self.resolve(&path, scope, locals));
            }
        }
        targets
    }
}
