//! TypeScript imports are not read in this build. Declaring the profile
//! makes the dependency rules NotMeasured for the tree, with this reason,
//! rather than silently measuring only the Rust half.

use crate::shape::ports::{
    DepEdge, DependencySource, LanguageProfile, ResolvedSpec, ResolvedUnit, SourceError, TreeSource,
};

pub struct TsImportDeps;

impl DependencySource for TsImportDeps {
    fn profile(&self) -> LanguageProfile {
        LanguageProfile::TsWorkspace
    }

    fn edges(
        &self,
        _tree: &dyn TreeSource,
        _spec: &ResolvedSpec,
        _units: &[ResolvedUnit],
    ) -> Result<Vec<DepEdge>, SourceError> {
        Err(SourceError::Unavailable(
            "TypeScript import adapter is not available in this build".into(),
        ))
    }
}
