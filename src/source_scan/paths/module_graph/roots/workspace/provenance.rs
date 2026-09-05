use std::collections::BTreeMap;

use super::{PackageManifest, TargetKind, normalize_crate_name};

impl PackageManifest {
    pub(in crate::source_scan::paths::module_graph::roots) fn audited_derive_crates(
        &self,
        target_kind: TargetKind,
    ) -> BTreeMap<String, String> {
        let mut candidates = BTreeMap::<String, Vec<_>>::new();
        for dependency in self
            .dependencies
            .iter()
            .filter(|dependency| dependency.kind.visible_to(target_kind))
        {
            candidates
                .entry(normalize_crate_name(&dependency.key))
                .or_default()
                .push(dependency);
        }
        candidates
            .into_iter()
            .filter_map(|(alias, candidates)| {
                let package = candidates.first()?.package.as_str();
                candidates
                    .iter()
                    .all(|dependency| {
                        dependency.package == package
                            && dependency.default_registry
                            && dependency.audited_macro_surface_enabled()
                            && self.audited_registry.contains(package)
                            && matches!(
                                package,
                                "serde"
                                    | "clap"
                                    | "async-trait"
                                    | "tokio"
                                    | "tracing"
                                    | "anyhow"
                                    | "serde_json"
                            )
                            && !self
                                .global_overrides
                                .iter()
                                .any(|candidate| candidate.package == package)
                    })
                    .then(|| (alias, package.to_owned()))
            })
            .collect()
    }
}
