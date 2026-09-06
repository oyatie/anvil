use std::collections::{BTreeMap, BTreeSet};

use super::{Alias, ResolvedPath, Symbols};

pub(super) fn resolve(
    symbols: &Symbols,
    segments: &[String],
    scope: &[String],
    locals: &BTreeMap<String, Vec<Vec<String>>>,
) -> BTreeSet<ResolvedPath> {
    resolve_inner(symbols, segments, scope, locals, &mut BTreeSet::new())
}

fn resolve_inner(
    symbols: &Symbols,
    segments: &[String],
    scope: &[String],
    locals: &BTreeMap<String, Vec<Vec<String>>>,
    visiting: &mut BTreeSet<(Vec<String>, Vec<String>)>,
) -> BTreeSet<ResolvedPath> {
    let Some(first) = segments.first() else {
        return BTreeSet::new();
    };
    let state = (scope.to_vec(), segments.to_vec());
    if !visiting.insert(state.clone()) {
        return BTreeSet::new();
    }
    let candidates = match first.as_str() {
        "crate" => BTreeSet::from([ResolvedPath {
            local: true,
            segments: segments[1..].to_vec(),
        }]),
        "self" => BTreeSet::from([ResolvedPath {
            local: true,
            segments: scope.iter().chain(&segments[1..]).cloned().collect(),
        }]),
        "super" => super_path(scope, segments, &state, visiting),
        _ => named_path(symbols, segments, scope, locals, visiting),
    };
    let mut expanded = BTreeSet::new();
    for candidate in candidates {
        if candidate.local {
            expanded.extend(expand_local(symbols, candidate, locals, visiting));
        } else {
            expanded.insert(candidate);
        }
    }
    visiting.remove(&state);
    expanded
}

fn super_path(
    scope: &[String],
    segments: &[String],
    state: &(Vec<String>, Vec<String>),
    visiting: &mut BTreeSet<(Vec<String>, Vec<String>)>,
) -> BTreeSet<ResolvedPath> {
    let mut absolute = scope.to_vec();
    let mut cursor = 0;
    while segments.get(cursor).is_some_and(|part| part == "super") {
        if absolute.pop().is_none() {
            visiting.remove(state);
            return BTreeSet::new();
        }
        cursor += 1;
    }
    absolute.extend_from_slice(&segments[cursor..]);
    BTreeSet::from([ResolvedPath {
        local: true,
        segments: absolute,
    }])
}

fn named_path(
    symbols: &Symbols,
    segments: &[String],
    scope: &[String],
    locals: &BTreeMap<String, Vec<Vec<String>>>,
    visiting: &mut BTreeSet<(Vec<String>, Vec<String>)>,
) -> BTreeSet<ResolvedPath> {
    let first = &segments[0];
    let mut candidates = BTreeSet::new();
    let mut definitely_shadowed = false;
    let mut definitely_bound = false;
    if let Some(targets) = locals.get(first) {
        for target in targets {
            if target.as_slice() == ["@shadow"] {
                definitely_shadowed = true;
                continue;
            }
            append_resolved(
                symbols,
                target,
                &segments[1..],
                scope,
                locals,
                visiting,
                &mut candidates,
            );
        }
    }
    if !definitely_shadowed {
        let mut binding = scope.to_vec();
        binding.push(first.clone());
        if let Some(aliases) = symbols.aliases.get(&binding) {
            for alias in aliases {
                let resolved =
                    resolve_inner(symbols, &alias.target, &alias.scope, locals, visiting);
                definitely_bound |= alias.definite && alias_is_namespace(symbols, alias, &resolved);
                candidates.extend(with_suffix(resolved, &segments[1..]));
            }
        }
        if symbols.modules.contains(&binding) {
            definitely_bound |= symbols.definite_modules.contains(&binding);
            binding.extend_from_slice(&segments[1..]);
            candidates.insert(ResolvedPath {
                local: true,
                segments: binding,
            });
        }
        if !definitely_bound && !scope.is_empty() {
            let root_binding = vec![first.clone()];
            if let Some(aliases) = symbols.aliases.get(&root_binding) {
                for alias in aliases.iter().filter(|alias| alias.global_extern) {
                    definitely_bound |= alias.definite;
                    append_resolved(
                        symbols,
                        &alias.target,
                        &segments[1..],
                        &alias.scope,
                        locals,
                        visiting,
                        &mut candidates,
                    );
                }
            }
        }
    }
    if !definitely_shadowed && !definitely_bound && symbols.crate_aliases.contains(first) {
        candidates.insert(ResolvedPath {
            local: true,
            segments: segments[1..].to_vec(),
        });
    }
    if !definitely_shadowed
        && !definitely_bound
        && symbols.audited_derive_crates.contains_key(first)
    {
        candidates.insert(ResolvedPath {
            local: false,
            segments: segments.to_vec(),
        });
    }
    if !definitely_shadowed
        && !definitely_bound
        && matches!(first.as_str(), "std" | "core" | "tokio")
    {
        candidates.insert(ResolvedPath {
            local: false,
            segments: segments.to_vec(),
        });
    }
    candidates
}

#[allow(clippy::too_many_arguments)]
fn append_resolved(
    symbols: &Symbols,
    target: &[String],
    suffix: &[String],
    scope: &[String],
    locals: &BTreeMap<String, Vec<Vec<String>>>,
    visiting: &mut BTreeSet<(Vec<String>, Vec<String>)>,
    candidates: &mut BTreeSet<ResolvedPath>,
) {
    candidates.extend(with_suffix(
        resolve_inner(symbols, target, scope, locals, visiting),
        suffix,
    ));
}

fn with_suffix(resolved: BTreeSet<ResolvedPath>, suffix: &[String]) -> BTreeSet<ResolvedPath> {
    resolved
        .into_iter()
        .map(|mut path| {
            path.segments.extend_from_slice(suffix);
            path
        })
        .collect()
}

fn expand_local(
    symbols: &Symbols,
    candidate: ResolvedPath,
    locals: &BTreeMap<String, Vec<Vec<String>>>,
    visiting: &mut BTreeSet<(Vec<String>, Vec<String>)>,
) -> BTreeSet<ResolvedPath> {
    for length in 1..=candidate.segments.len() {
        let prefix = &candidate.segments[..length];
        let Some(aliases) = symbols.aliases.get(prefix) else {
            continue;
        };
        let mut expanded = BTreeSet::new();
        for alias in aliases {
            append_resolved(
                symbols,
                &alias.target,
                &candidate.segments[length..],
                &alias.scope,
                locals,
                visiting,
                &mut expanded,
            );
        }
        if symbols.modules.contains(prefix) {
            expanded.insert(candidate.clone());
        }
        return expanded;
    }
    BTreeSet::from([candidate])
}

fn alias_is_namespace(symbols: &Symbols, alias: &Alias, resolved: &BTreeSet<ResolvedPath>) -> bool {
    alias.global_extern
        || (alias.target.len() == 1
            && alias.target.first().is_some_and(|first| {
                symbols.crate_aliases.contains(first)
                    || matches!(first.as_str(), "std" | "core" | "tokio")
            }))
        || (!resolved.is_empty()
            && resolved.iter().all(|path| {
                path.local && (path.segments.is_empty() || symbols.modules.contains(&path.segments))
            }))
}
