//! I8: a structure-only change changes structure only. The staged diff of a
//! shape shard may contain renames, wiring-line edits (`use`, `mod`, path
//! dependencies, build labels, ownership lines) and the skeleton files the
//! spec templated — and nothing else. Anything else fails closed.

use super::model::{MoveKind, Shard};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameStatus {
    Renamed {
        from: String,
        to: String,
        similarity: u8,
    },
    Modified(String),
    Added(String),
    Deleted(String),
}

impl NameStatus {
    /// Parses `git diff --cached -M --name-status` output.
    pub fn parse(text: &str) -> Vec<NameStatus> {
        text.lines()
            .filter_map(|l| {
                let mut p = l.split('\t');
                let code = p.next()?;
                let a = p.next()?;
                match code.chars().next()? {
                    'R' => {
                        let sim: u8 = code[1..].parse().unwrap_or(100);
                        Some(NameStatus::Renamed {
                            from: a.to_string(),
                            to: p.next()?.to_string(),
                            similarity: sim,
                        })
                    }
                    'M' => Some(NameStatus::Modified(a.to_string())),
                    'A' => Some(NameStatus::Added(a.to_string())),
                    'D' => Some(NameStatus::Deleted(a.to_string())),
                    _ => None,
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PurityViolation {
    LowSimilarityRename {
        from: String,
        to: String,
        similarity: u8,
    },
    BehaviourLineChanged {
        path: String,
        line: String,
    },
    UnexpectedAddition {
        path: String,
    },
    Deletion {
        path: String,
    },
    ConflictMarker {
        path: String,
    },
}

/// A changed line in a modified file is allowed when it is wiring.
pub fn is_wiring_line(line: &str) -> bool {
    let t = line.trim_start();
    let t = t
        .strip_prefix("pub(crate) ")
        .or_else(|| t.strip_prefix("pub "))
        .unwrap_or(t);
    t.starts_with("use ")
        || t.starts_with("mod ")
        || t.starts_with("extern crate ")
        || t.starts_with("path = ")
        || t.starts_with("name = ")
        || t.starts_with("members")
        || t.starts_with("\"//")
        || t.starts_with("//")
        || t.starts_with('@')
        || t.trim().is_empty()
        || t.trim() == "]"
        || t.trim() == "}"
}

pub fn diff_is_structure_only(
    name_status: &[NameStatus],
    cached_diff: &str,
    shard: &Shard,
) -> Result<(), Vec<PurityViolation>> {
    let mut v = Vec::new();
    let additions_allowed = shard
        .moves
        .iter()
        .any(|m| matches!(m.kind, MoveKind::CreateSkeleton | MoveKind::AddManifest));
    for ns in name_status {
        match ns {
            NameStatus::Renamed {
                from,
                to,
                similarity,
            } => {
                if *similarity < 50 {
                    v.push(PurityViolation::LowSimilarityRename {
                        from: from.clone(),
                        to: to.clone(),
                        similarity: *similarity,
                    });
                }
            }
            NameStatus::Added(p) => {
                let templated = shard.moves.iter().any(|m| &m.to == p);
                if !(additions_allowed && templated) {
                    v.push(PurityViolation::UnexpectedAddition { path: p.clone() });
                }
            }
            NameStatus::Deleted(p) => v.push(PurityViolation::Deletion { path: p.clone() }),
            NameStatus::Modified(_) => {}
        }
    }
    let mut current: Option<String> = None;
    for line in cached_diff.lines() {
        if let Some(rest) = line.strip_prefix("+++ b/") {
            current = Some(rest.trim().to_string());
            continue;
        }
        if line.starts_with("+++")
            || line.starts_with("---")
            || line.starts_with("@@")
            || line.starts_with("diff ")
        {
            continue;
        }
        let Some(path) = &current else { continue };
        if line.starts_with("<<<<<<< ")
            || line.starts_with(">>>>>>> ")
            || line.starts_with("+<<<<<<< ")
            || line.starts_with("+>>>>>>> ")
        {
            v.push(PurityViolation::ConflictMarker { path: path.clone() });
            continue;
        }
        let is_modified = name_status
            .iter()
            .any(|n| matches!(n, NameStatus::Modified(p) if p == path));
        if !is_modified {
            continue;
        }
        if let Some(body) = line.strip_prefix('+').or_else(|| line.strip_prefix('-'))
            && !is_wiring_line(body)
        {
            v.push(PurityViolation::BehaviourLineChanged {
                path: path.clone(),
                line: body.trim().to_string(),
            });
        }
    }
    if v.is_empty() { Ok(()) } else { Err(v) }
}
