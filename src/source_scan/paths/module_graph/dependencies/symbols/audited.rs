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
    ) -> bool {
        self.audited_macro_path(path, scope, locals, |package, symbol| {
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
    ) -> bool {
        self.audited_macro_path(path, scope, locals, |package, symbol| {
            matches!(
                (package, symbol),
                ("async-trait", "async_trait") | ("tokio", "main")
            )
        })
    }

    pub(in crate::source_scan::paths::module_graph::dependencies) fn is_audited_function_macro(
        &self,
        path: &[String],
        scope: &[String],
        locals: &BTreeMap<String, Vec<Vec<String>>>,
    ) -> bool {
        self.audited_macro_path(path, scope, locals, |package, symbol| {
            matches!(
                (package, symbol),
                ("tracing", "error" | "info" | "warn" | "info_span")
                    | ("anyhow", "anyhow" | "bail")
                    | ("serde_json", "json")
                    | ("tokio", "join" | "select")
            )
        })
    }

    fn audited_macro_path(
        &self,
        path: &[String],
        scope: &[String],
        locals: &BTreeMap<String, Vec<Vec<String>>>,
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
                    .is_some_and(|package| allowed(package, symbol))
            })
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
