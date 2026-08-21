//! Repository-root rules: the default-deny root-file allowlist, growth under
//! legacy roots, and workspace membership declared by glob rather than by
//! hand-maintained list.

use super::report::{Finding, RuleId};
use super::resolve::ResolvedSpec;
use super::spec::RootRuleKind;
use super::tree::TreeSource;
use crate::shape::core::profile::LanguageProfile;

fn root_file_allowed(spec: &ResolvedSpec, name: &str) -> bool {
    spec.spec.root_files.rules.iter().any(|r| match r.kind {
        RootRuleKind::Exact => name == r.value,
        RootRuleKind::Prefix => name.starts_with(r.value.as_str()),
        RootRuleKind::Suffix => name.ends_with(r.value.as_str()),
        RootRuleKind::PrefixDot => {
            name == r.value
                || name
                    .strip_prefix(r.value.as_str())
                    .is_some_and(|rest| rest.starts_with('.'))
        }
    })
}

pub fn root_findings(
    spec: &ResolvedSpec,
    tree: &dyn TreeSource,
    declared: &dyn Fn(&str) -> bool,
) -> Vec<Finding> {
    let mut out = Vec::new();

    if declared("root_file_unallowlisted") {
        for path in tree.paths().iter().filter(|p| !p.contains('/')) {
            if !root_file_allowed(spec, path) {
                out.push(Finding {
                    rule: RuleId::new("root_file_unallowlisted"),
                    key: path.clone(),
                    path: path.clone(),
                    unit: None,
                    detail: format!("root file {path:?} is not on the allowlist"),
                    fix: None,
                });
            }
        }
    }

    if declared("legacy_root_growth") {
        for root in &spec.spec.legacy_roots {
            for path in tree.under(root).iter() {
                out.push(Finding {
                    rule: RuleId::new("legacy_root_growth"),
                    key: path.to_string(),
                    path: path.to_string(),
                    unit: None,
                    detail: format!("lives under legacy root {root}"),
                    fix: None,
                });
            }
        }
    }

    if declared("workspace_members_not_glob")
        && spec.spec.profiles.contains(&LanguageProfile::RustCargo)
    {
        let marker = LanguageProfile::RustCargo.unit_marker();
        if let Ok(Some(bytes)) = tree.read(marker) {
            for member in workspace_members(bytes) {
                if !member.contains('*') {
                    out.push(Finding {
                        rule: RuleId::new("workspace_members_not_glob"),
                        key: member.clone(),
                        path: marker.to_string(),
                        unit: None,
                        detail: format!(
                            "workspace member {member:?} is listed by hand; adding a crate then edits the root manifest"
                        ),
                        fix: None,
                    });
                }
            }
        }
    }

    out
}

/// Entries of `[workspace] members = [...]`, possibly multi-line.
pub fn workspace_members(manifest: &[u8]) -> Vec<String> {
    let Ok(text) = std::str::from_utf8(manifest) else {
        return vec![];
    };
    let mut in_ws = false;
    let mut in_members = false;
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.split('#').next().unwrap_or("").trim();
        if t.starts_with('[') {
            in_ws = t == "[workspace]";
            in_members = false;
            continue;
        }
        if !in_ws {
            continue;
        }
        if let Some(rest) = t.strip_prefix("members") {
            let rest = rest.trim_start().trim_start_matches('=').trim();
            in_members = true;
            collect_quoted(rest, &mut out);
            if rest.contains(']') {
                in_members = false;
            }
            continue;
        }
        if in_members {
            collect_quoted(t, &mut out);
            if t.contains(']') {
                in_members = false;
            }
        }
    }
    out
}

fn collect_quoted(s: &str, out: &mut Vec<String>) {
    let mut rest = s;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else { break };
        out.push(after[..end].to_string());
        rest = &after[end + 1..];
    }
}
