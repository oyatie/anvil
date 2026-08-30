//! `adapter_not_port_plus_technology`: an adapter is named for the port it
//! implements plus the technology that implements it.
//!
//! The template is the tenant's, carried in `naming.adapter_name` as
//! `<a><sep><b>`; nothing here knows what the two components are called. The
//! port half is checked against the ports the same unit actually declares, so
//! a name that merely contains a separator does not pass: the first component
//! must be a port that exists.
//!
//! # What makes this a measurement rather than a spelling check
//!
//! A crate name is reduced to its `body` — the tenant's crate prefix and the
//! declared layer suffix removed — before it is read, and the ports of a unit
//! are reduced the same way. So the comparison is between the parts the
//! tenant's own template governs, on both sides.
//!
//! # I1, both directions
//!
//! A unit whose ports face carries no crate has no port names to compare
//! against. That unit is reported as not measured, naming itself, and
//! contributes no finding: a scan that cannot see its subject must not accuse
//! it. The same holds for a spec that declares the rule without declaring a
//! template, and for a tree from which no manifest was loaded.

use super::dependency::classify;
use super::naming::package_name;
use super::report::{Finding, Fix, RuleId};
use super::resolve::{ResolvedSpec, ResolvedUnit};
use super::spec::Naming;
use super::tree::TreeSource;
use crate::shape::core::profile::LanguageProfile;
use std::collections::{BTreeMap, BTreeSet};

pub const RULE: &str = "adapter_not_port_plus_technology";

/// The two hexagonal roles this rule reads.
///
/// Face names are tenant data everywhere else in the engine. These two are
/// the roles the rule is about — the same pair `port_defined_in_adapter`
/// reads — and a tenant whose skeleton names neither simply produces no
/// subjects, which is reported as not measured rather than as conformance.
const ADAPTERS_FACE: &str = "adapters";
const PORTS_FACE: &str = "ports";

/// The separator a two-component template puts between its components.
///
/// `"<a>-<b>"` yields `"-"`. `None` for anything that is not two
/// angle-bracketed components with a non-empty literal between them, which is
/// what `validate` refuses so a tenant hears about it once, at adoption.
pub fn template_separator(template: &str) -> Option<String> {
    let (lead, rest) = template.split_once('>')?;
    let first = lead.strip_prefix('<')?;
    let (sep, tail) = rest.split_once('<')?;
    let second = tail.strip_suffix('>')?;
    let ok = !first.is_empty() && !second.is_empty() && !sep.is_empty() && !second.contains('>');
    ok.then(|| sep.to_string())
}

/// A crate name split into the part the template governs and the trailing
/// `-<layer suffix>` the tenant's suffix rule governs, with the crate prefix
/// already removed. The suffix is kept so a rename can put it back.
fn body<'a>(name: &'a str, naming: &Naming) -> (&'a str, &'a str) {
    let stripped = name
        .strip_prefix(naming.crate_prefix.as_str())
        .unwrap_or(name);
    let cut = naming
        .layer_suffixes
        .iter()
        .filter(|s| {
            stripped
                .strip_suffix(s.as_str())
                .is_some_and(|h| h.ends_with('-'))
        })
        .map(|s| stripped.len() - s.len() - 1)
        .min()
        .unwrap_or(stripped.len());
    stripped.split_at(cut)
}

/// Every crate manifest loaded from the tree, with the unit and face it is in.
fn manifests_by_face<'a>(
    spec: &ResolvedSpec,
    tree: &'a dyn TreeSource,
    units: &'a [ResolvedUnit],
) -> Vec<(&'a String, String, &'a ResolvedUnit, String)> {
    let marker = LanguageProfile::RustCargo.unit_marker();
    let mut out = Vec::new();
    for (path, bytes) in tree.loaded() {
        if path.rsplit('/').next() != Some(marker) {
            continue;
        }
        let (Some(name), Some((unit, Some(face)))) =
            (package_name(bytes), classify(spec, units, path))
        else {
            continue;
        };
        out.push((path, name, unit, face));
    }
    out
}

/// Findings, and the subjects this tree could not put to the rule.
pub fn adapter_naming_findings(
    spec: &ResolvedSpec,
    tree: &dyn TreeSource,
    units: &[ResolvedUnit],
) -> (Vec<Finding>, Vec<(RuleId, String)>) {
    let naming = &spec.spec.naming;
    let id = || RuleId::new(RULE);
    let Some(template) = naming.adapter_name.as_deref() else {
        let why = "naming.adapter_name declares no template, so there is nothing to measure an \
                   adapter name against"
            .to_string();
        return (Vec::new(), vec![(id(), why)]);
    };
    let Some(sep) = template_separator(template) else {
        let why = format!("naming.adapter_name {template:?} names no two components");
        return (Vec::new(), vec![(id(), why)]);
    };

    let found = manifests_by_face(spec, tree, units);
    if found.is_empty() {
        let marker = LanguageProfile::RustCargo.unit_marker();
        let why = format!("no {marker} was loaded under a face of any unit");
        return (Vec::new(), vec![(id(), why)]);
    }

    let mut ports: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (_, name, unit, _) in found.iter().filter(|(_, _, _, f)| f == PORTS_FACE) {
        ports
            .entry(unit.name.as_str())
            .or_default()
            .insert(body(name, naming).0);
    }

    let mut findings = Vec::new();
    let mut unmeasured: BTreeSet<String> = BTreeSet::new();
    for (path, name, unit, _) in found.iter().filter(|(_, _, _, f)| f == ADAPTERS_FACE) {
        let Some(known) = ports.get(unit.name.as_str()) else {
            unmeasured.insert(format!(
                "unit {} declares no crate under its {PORTS_FACE} face, so the port half of \
                 {template} cannot be checked for the adapters it does declare",
                unit.name
            ));
            continue;
        };
        let (stem, layer) = body(name, naming);
        if known.iter().any(|p| implements(stem, p, &sep)) {
            continue;
        }
        findings.push(finding(
            path,
            name,
            (stem, layer),
            unit,
            known,
            (template, &sep),
        ));
    }
    (
        findings,
        unmeasured.into_iter().map(|why| (id(), why)).collect(),
    )
}

/// Whether `stem` is `port` followed by the separator and a technology.
///
/// The technology must be non-empty: an adapter named for its port alone says
/// which port it implements and not what implements it, which is the half of
/// the template that carries the information.
fn implements(stem: &str, port: &str, sep: &str) -> bool {
    stem.strip_prefix(port)
        .and_then(|rest| rest.strip_prefix(sep))
        .is_some_and(|tech| !tech.is_empty())
}

fn finding(
    path: &str,
    name: &str,
    split: (&str, &str),
    unit: &ResolvedUnit,
    known: &BTreeSet<&str>,
    tmpl: (&str, &str),
) -> Finding {
    let ((stem, layer), (template, sep)) = (split, tmpl);
    let listed = known.iter().copied().collect::<Vec<_>>().join(", ");
    let detail = if known.contains(stem) {
        format!("adapter crate {name} names port {stem:?} and no technology; {template} needs both")
    } else {
        format!(
            "adapter crate {name} is not {template}: {stem:?} opens with no port of unit {} ({listed})",
            unit.name
        )
    };
    // One candidate port is one unambiguous rename; more than one is a choice
    // only the author can make.
    let fix = (known.len() == 1 && !known.contains(stem)).then(|| Fix::Rename {
        from: name.to_string(),
        to: {
            let port = known.iter().next().copied().unwrap_or_default();
            let head = &name[..name.len() - stem.len() - layer.len()];
            format!("{head}{port}{sep}{stem}{layer}")
        },
    });
    Finding {
        rule: RuleId::new(RULE),
        key: name.to_string(),
        path: path.to_string(),
        unit: Some(unit.name.clone()),
        detail,
        fix,
    }
}
