//! Buck2 labels as edges: every `"//pkg/path:target"` string in a BUCK file
//! is a dependency of that BUCK file's package on `pkg/path/`.

use crate::shape::ports::{
    DepEdge, DependencySource, LanguageProfile, ResolvedSpec, ResolvedUnit, SourceError, TreeSource,
};

pub struct BuckLabelDeps;

impl DependencySource for BuckLabelDeps {
    fn profile(&self) -> LanguageProfile {
        LanguageProfile::RustBuck2
    }

    fn edges(
        &self,
        tree: &dyn TreeSource,
        _spec: &ResolvedSpec,
        _units: &[ResolvedUnit],
    ) -> Result<Vec<DepEdge>, SourceError> {
        let marker = LanguageProfile::RustBuck2.unit_marker();
        let mut edges = Vec::new();
        let mut any = false;
        for (path, bytes) in tree.loaded() {
            if path.rsplit('/').next() != Some(marker) {
                continue;
            }
            any = true;
            let text = String::from_utf8_lossy(bytes);
            let dir = path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            let from = if dir.is_empty() {
                String::new()
            } else {
                format!("{dir}/")
            };
            let mut rest: &str = &text;
            while let Some(i) = rest.find("\"//") {
                let after = &rest[i + 3..];
                let Some(end) = after.find('"') else { break };
                let label = &after[..end];
                let pkg = label.split(':').next().unwrap_or(label);
                if !pkg.is_empty() {
                    edges.push(DepEdge {
                        from: from.clone(),
                        to: format!("{pkg}/"),
                        via: "buck label".to_string(),
                    });
                }
                rest = &after[end + 1..];
            }
        }
        if !any {
            return Err(SourceError::Unavailable(format!("no {marker} loaded")));
        }
        Ok(edges)
    }
}
