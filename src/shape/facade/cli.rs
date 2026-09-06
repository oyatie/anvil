//! `anvil shape validate-spec`: parse, validate and resolve a spec file and
//! say what it declares. Exits non-zero on any problem.

use crate::shape::ports::{ResolvedSpec, RuleMode, ShapeSpec, SpecError, resolve};
use std::path::Path;

/// The one path literal Anvil carries for the Shape Program: its own config
/// location inside a tenant repository, not a rule about that tenant's layout.
pub const SPEC_PATH: &str = ".anvil/shape.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationSummary {
    pub schema: String,
    pub profiles: Vec<String>,
    pub unit_kinds: usize,
    pub units_resolved: usize,
    pub discovery_rules: usize,
    pub rules_blocking: usize,
    pub rules_advisory: usize,
    pub registry_supplied: bool,
}

impl ValidationSummary {
    pub fn from_resolved(resolved: &ResolvedSpec, registry_supplied: bool) -> Self {
        let spec = &resolved.spec;
        ValidationSummary {
            schema: spec.schema.clone(),
            profiles: spec.profiles.iter().map(|p| p.name().to_string()).collect(),
            unit_kinds: spec.unit_kinds.len(),
            units_resolved: resolved.units.len(),
            discovery_rules: resolved.discovery.len(),
            rules_blocking: spec
                .rules
                .values()
                .filter(|r| r.mode == RuleMode::BaselineBlockOnNew)
                .count(),
            rules_advisory: spec
                .rules
                .values()
                .filter(|r| r.mode == RuleMode::AdvisoryUntilInfra)
                .count(),
            registry_supplied,
        }
    }

    pub fn render(&self) -> String {
        format!(
            "shape spec OK\n  schema:          {}\n  profiles:        {}\n  unit kinds:      {}\n  units resolved:  {}{}\n  discovery rules: {}\n  rules:           {} blocking, {} advisory",
            self.schema,
            self.profiles.join(", "),
            self.unit_kinds,
            self.units_resolved,
            if self.registry_supplied {
                ""
            } else {
                " (no registry supplied; registry-backed kinds not enumerated)"
            },
            self.discovery_rules,
            self.rules_blocking,
            self.rules_advisory,
        )
    }
}

/// Reads and validates `path`; `registry` is the tenant's unit registry when
/// the spec refers to one and the caller has it.
pub fn validate_spec_file(
    path: &Path,
    registry: Option<&Path>,
) -> Result<ValidationSummary, SpecError> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| SpecError::Parse(format!("{}: {e}", path.display())))?;
    let spec = ShapeSpec::parse(&raw)?;
    let registry_doc = match registry {
        Some(p) => {
            let raw = std::fs::read_to_string(p)
                .map_err(|e| SpecError::Registry(format!("{}: {e}", p.display())))?;
            Some(
                serde_json::from_str::<serde_json::Value>(&raw)
                    .map_err(|e| SpecError::Registry(format!("{}: {e}", p.display())))?,
            )
        }
        None => None,
    };
    let resolved = match resolve(&spec, registry_doc.as_ref()) {
        Ok(r) => r,
        // A spec that needs a registry the caller did not supply is still a
        // valid spec; report it as unresolved rather than invalid.
        Err(SpecError::Registry(_)) if registry_doc.is_none() => {
            let mut discovery_only = spec.clone();
            discovery_only
                .unit_kinds
                .retain(|_, k| k.members.starts_with("discover:"));
            resolve(&discovery_only, None)?
        }
        Err(e) => return Err(e),
    };
    Ok(ValidationSummary::from_resolved(
        &resolved,
        registry_doc.is_some(),
    ))
}
