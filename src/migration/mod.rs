//! Migration destiny per component. See [`registry`] for the ledger itself.

pub mod boundary;
pub mod registry;

pub use boundary::{BoundaryViolation, check_edge, edge_is_allowed, verdict_for};
pub use registry::{Confidence, MIGRATION_LEDGER, MigrationEntry, Verdict};

/// Counts by verdict. Returns (migrating, rewired, superseded, scaffolding).
pub fn verdict_counts() -> (usize, usize, usize, usize) {
    let mut m = 0;
    let mut r = 0;
    let mut s = 0;
    let mut f = 0;
    for e in MIGRATION_LEDGER {
        match e.verdict {
            Verdict::Migrating => m += 1,
            Verdict::Rewired => r += 1,
            Verdict::Superseded => s += 1,
            Verdict::Scaffolding => f += 1,
        }
    }
    (m, r, s, f)
}

/// Components that survive absorption in some form -- the only ones worth
/// renaming or restructuring. Superseded and scaffolding code is both deleted,
/// so applying the naming law to either is waste.
pub fn surviving_surface() -> Vec<&'static MigrationEntry> {
    MIGRATION_LEDGER
        .iter()
        .filter(|e| !matches!(e.verdict, Verdict::Superseded | Verdict::Scaffolding))
        .collect()
}

/// Superseded components whose evidence is strong enough to act on. Deliberately
/// narrower than "everything marked Superseded": a probable verdict must not
/// delete working code.
pub fn deletable_today() -> Vec<&'static MigrationEntry> {
    MIGRATION_LEDGER
        .iter()
        .filter(|e| e.deletion_is_authorised())
        .collect()
}

/// Scans the live tree for forbidden dependency edges.
///
/// Returns `Err(reason)` when the source tree cannot be read, so the caller can
/// report `NotMeasured` rather than an absence of violations. A gate that
/// cannot see the code must not report that the code is clean.
pub fn live_tree_violations(
    repo_root: &crate::git_manager::SubjectRoot,
) -> Result<Vec<BoundaryViolation>, String> {
    let repo_root = repo_root.as_path();
    use std::collections::BTreeSet;

    let root = repo_root.join("src");
    if !root.is_dir() {
        return Err(format!("source tree not readable at {}", root.display()));
    }

    let mut modules: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let p = entry.path();
        let Some(name) = p.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if name == "main" || name == "lib" {
            continue;
        }
        if p.is_dir() || p.extension().is_some_and(|e| e == "rs") {
            modules.push(name.to_string());
        }
    }
    modules.sort();

    let mut out = Vec::new();
    for module in &modules {
        let dir = root.join(module);
        let file = root.join(format!("{module}.rs"));
        let mut text = String::new();
        if dir.is_dir() {
            let mut stack = vec![dir];
            while let Some(d) = stack.pop() {
                for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p.extension().is_some_and(|x| x == "rs") {
                        text.push_str(&std::fs::read_to_string(&p).unwrap_or_default());
                    }
                }
            }
        } else if file.is_file() {
            text = std::fs::read_to_string(&file).unwrap_or_default();
        }

        // Comments are stripped so a doc example cannot fabricate an edge.
        let code: String = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        let mut deps: BTreeSet<String> = BTreeSet::new();
        for (i, _) in code.match_indices("crate::") {
            let rest = &code[i + 7..];
            let first: String = rest
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
                .collect();
            if first.is_empty() {
                continue;
            }
            let after = &rest[first.len()..];
            if let Some(tail) = after.strip_prefix("::") {
                let second: String = tail
                    .chars()
                    .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
                    .collect();
                if !second.is_empty() {
                    deps.insert(format!("{first}/{second}"));
                    continue;
                }
            }
            deps.insert(first);
        }

        for dep in deps {
            if let Some(v) = check_edge(module, &dep) {
                out.push(v);
            }
        }
    }
    Ok(out)
}
