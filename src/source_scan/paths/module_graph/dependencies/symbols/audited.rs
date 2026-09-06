use std::collections::BTreeMap;

use super::Symbols;

impl Symbols {
    pub(in crate::source_scan::paths::module_graph::dependencies) fn add_audited_derive_crate(
        &mut self,
        alias: &str,
        package: &str,
    ) {
        self.audited_derive_crates
            .insert(alias.to_owned(), package.to_owned());
    }

    pub(in crate::source_scan::paths::module_graph::dependencies) fn has_audited_package_alias(
        &self,
        alias: &str,
        package: &str,
    ) -> bool {
        self.audited_derive_crates
            .get(alias)
            .is_some_and(|candidate| candidate == package)
    }

    pub(in crate::source_scan::paths::module_graph::dependencies) fn is_audited_derive(
        &self,
        path: &[String],
        scope: &[String],
        locals: &BTreeMap<String, Vec<Vec<String>>>,
        macros: &BTreeMap<String, Vec<proc_macro2::TokenStream>>,
    ) -> bool {
        self.audited_macro_path(path, scope, locals, macros, |package, symbol| {
            matches!(
                (package, symbol),
                ("serde", "Serialize" | "Deserialize") | ("clap", "Parser" | "Subcommand")
            )
        })
    }

    pub(in crate::source_scan::paths::module_graph::dependencies) fn is_audited_attribute(
        &self,
        path: &[String],
        scope: &[String],
        locals: &BTreeMap<String, Vec<Vec<String>>>,
        macros: &BTreeMap<String, Vec<proc_macro2::TokenStream>>,
    ) -> bool {
        self.audited_macro_path(path, scope, locals, macros, |package, symbol| {
            matches!(
                (package, symbol),
                ("async-trait", "async_trait") | ("tokio", "main")
            )
        })
    }

    pub(in crate::source_scan::paths::module_graph::dependencies) fn audited_function_macro(
        &self,
        path: &[String],
        scope: &[String],
        locals: &BTreeMap<String, Vec<Vec<String>>>,
        macros: &BTreeMap<String, Vec<proc_macro2::TokenStream>>,
    ) -> Option<(String, String)> {
        let mut identity = None;
        for candidate in self.explicit_path_candidates(path, scope, locals) {
            let [alias, symbol] = candidate.as_slice() else {
                return None;
            };
            let package = self.audited_derive_crates.get(alias)?;
            if !matches!(
                (package.as_str(), symbol.as_str()),
                ("tracing", "error" | "info" | "warn" | "info_span")
                    | ("anyhow", "anyhow" | "bail")
                    | ("serde_json", "json")
                    | ("tokio", "join" | "select" | "try_join")
            ) || !self.generated_names_known(package, scope, locals, macros)
            {
                return None;
            }
            let resolved = (package.clone(), symbol.clone());
            // Multiple spellings of one identity are equivalent; distinct
            // possible entrypoints do not establish a single finite contract.
            if identity.as_ref().is_some_and(|known| known != &resolved) {
                return None;
            }
            identity = Some(resolved);
        }
        identity
    }

    fn audited_macro_path(
        &self,
        path: &[String],
        scope: &[String],
        locals: &BTreeMap<String, Vec<Vec<String>>>,
        macros: &BTreeMap<String, Vec<proc_macro2::TokenStream>>,
        allowed: impl Fn(&str, &str) -> bool,
    ) -> bool {
        let candidates = self.explicit_path_candidates(path, scope, locals);
        !candidates.is_empty()
            && candidates.iter().all(|candidate| {
                let [alias, symbol] = candidate.as_slice() else {
                    return false;
                };
                self.audited_derive_crates
                    .get(alias)
                    .is_some_and(|package| {
                        allowed(package, symbol)
                            && self.generated_names_known(package, scope, locals, macros)
                    })
            })
    }

    fn generated_names_known(
        &self,
        package: &str,
        scope: &[String],
        locals: &BTreeMap<String, Vec<Vec<String>>>,
        macros: &BTreeMap<String, Vec<proc_macro2::TokenStream>>,
    ) -> bool {
        // The default derive emits `extern crate serde as _serde`; a renamed
        // derive alias alone does not bind that generated facade.
        if package == "serde" && !self.has_audited_package_alias("serde", "serde") {
            return false;
        }
        let builtins: &[&str] = match package {
            "clap" => &["format", "concat"],
            // unimplemented also closes the pinned try_join cfg(doc) stub;
            // unknown cfg branches are not silently dropped from the universe.
            "tokio" => &["panic", "unreachable", "compile_error", "unimplemented"],
            "tracing" => &["module_path", "file", "line", "format_args", "concat"],
            _ => &[],
        };
        if !builtins.iter().all(|name| {
            !macros.contains_key(*name)
                && !locals.contains_key(*name)
                && !locals.contains_key("*")
                && !self.macro_shadowed(scope, name)
                && !self.alias_declared(scope, name)
                && !self.glob_imported(scope)
                && !self.unknown_macro_prelude()
        }) {
            return false;
        }
        if package == "clap" {
            if locals.contains_key("core") || self.binding_declared(scope, "core") {
                return false;
            }
            let candidates = self.explicit_path_candidates(&["clap".to_owned()], scope, locals);
            return !candidates.is_empty()
                && candidates.iter().all(|path| {
                    let [alias] = path.as_slice() else {
                        return false;
                    };
                    self.has_audited_package_alias(alias, "clap")
                });
        }
        true
    }

    fn explicit_path_candidates(
        &self,
        path: &[String],
        scope: &[String],
        locals: &BTreeMap<String, Vec<Vec<String>>>,
    ) -> Vec<Vec<String>> {
        let Some(first) = path.first() else {
            return Vec::new();
        };
        if let Some(targets) = locals.get(first) {
            return targets
                .iter()
                .map(|target| target.iter().chain(&path[1..]).cloned().collect())
                .collect();
        }
        let mut binding = scope.to_vec();
        binding.push(first.clone());
        if self.modules.contains(&binding) {
            return vec![vec!["@local-module".to_owned()]];
        }
        if let Some(aliases) = self.aliases.get(&binding) {
            return aliases
                .iter()
                .map(|alias| alias.target.iter().chain(&path[1..]).cloned().collect())
                .collect();
        }
        if !scope.is_empty()
            && let Some(aliases) = self.aliases.get(&vec![first.clone()])
        {
            let globals = aliases
                .iter()
                .filter(|alias| alias.global_extern)
                .map(|alias| alias.target.iter().chain(&path[1..]).cloned().collect())
                .collect::<Vec<_>>();
            if !globals.is_empty() {
                return globals;
            }
        }
        vec![path.to_vec()]
    }
}
