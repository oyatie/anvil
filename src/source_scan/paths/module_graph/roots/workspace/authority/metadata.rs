use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
pub(in crate::source_scan::paths::module_graph::roots::workspace) struct Metadata {
    pub(super) workspace_root: PathBuf,
    pub(in crate::source_scan::paths::module_graph::roots::workspace) packages: Vec<Package>,
    resolve: Resolve,
}
#[derive(Deserialize)]
struct Resolve {
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
pub(in crate::source_scan::paths::module_graph::roots::workspace) struct Package {
    pub(in crate::source_scan::paths::module_graph::roots::workspace) id: String,
    pub(in crate::source_scan::paths::module_graph::roots::workspace) name: String,
    pub(in crate::source_scan::paths::module_graph::roots::workspace) version: String,
    pub(in crate::source_scan::paths::module_graph::roots::workspace) source: Option<String>,
    pub(in crate::source_scan::paths::module_graph::roots::workspace) manifest_path: PathBuf,
}

#[derive(Deserialize)]
pub(in crate::source_scan::paths::module_graph::roots::workspace) struct Node {
    id: String,
    pub(in crate::source_scan::paths::module_graph::roots::workspace) features: Vec<String>,
    deps: Vec<Dependency>,
}
#[derive(Deserialize)]
struct Dependency {
    name: String,
    pkg: String,
    dep_kinds: Vec<Kind>,
}
#[derive(Deserialize)]
struct Kind {
    kind: Option<String>,
    target: Option<String>,
}

impl Metadata {
    pub(in crate::source_scan::paths::module_graph::roots::workspace) fn node(
        &self,
        id: &str,
    ) -> Option<&Node> {
        let mut nodes = self.resolve.nodes.iter().filter(|node| node.id == id);
        let node = nodes.next()?;
        nodes.next().is_none().then_some(node)
    }
}

impl Node {
    pub(in crate::source_scan::paths::module_graph::roots::workspace) fn reviewed_edges_match(
        &self,
        selected: &std::collections::BTreeMap<String, String>,
        packages: &[Package],
    ) -> bool {
        self.deps.iter().all(|dependency| {
            let candidates = packages
                .iter()
                .filter(|package| package.id == dependency.pkg)
                .collect::<Vec<_>>();
            let [package] = candidates.as_slice() else {
                return false;
            };
            selected
                .get(&package.name)
                .is_none_or(|id| id == &dependency.pkg)
        })
    }
}

pub(super) fn selected_dependency<'a>(
    node: &'a Node,
    alias: &str,
    kind: Option<&str>,
    target: Option<&str>,
) -> Option<&'a str> {
    let mut matches = node.deps.iter().filter(|dependency| {
        dependency.name == alias
            && dependency
                .dep_kinds
                .iter()
                .any(|entry| entry.kind.as_deref() == kind && entry.target.as_deref() == target)
    });
    let id = matches.next()?.pkg.as_str();
    matches.all(|candidate| candidate.pkg == id).then_some(id)
}
