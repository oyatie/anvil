//! Applying fixes, not describing them.
//!
//! `Fix` shipped as an enum nothing executed. A repair only a human can apply
//! is a repair applied 47 times by hand, and at 251 sites it is not applied at
//! all -- it becomes a report someone reads and closes.
//!
//! This is the codemod half: the rule that FINDS the defect and the transform
//! that REPAIRS it are one declaration, and the runner is written once. The
//! author of a rule never writes a migration script.
//!
//! # What this refuses to do
//!
//! Silently. Every path here either performs an edit it can describe exactly,
//! or refuses and says which fix it could not apply and why. A codemod that
//! skips what it cannot handle produces a tree that looks migrated and is not
//! -- the same defect as a check that reads absence as success, one layer over.
//!
//! # Order and conflict
//!
//! Fixes are grouped by the file they touch and applied per file, so two fixes
//! to one file are seen together rather than racing. A file whose fixes
//! disagree is refused whole: a partially-migrated file is worse than an
//! untouched one, because the next run sees a tree in neither state.

use super::{Finding, Fix};
use std::collections::BTreeMap;

/// A single edit, resolved against real content. Nothing is written until every
/// edit in the plan has been resolved, so a failure late does not leave a tree
/// half-migrated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// Replace the whole body of a file.
    Rewrite { path: String, body: String },
    /// Move a file, contents unchanged.
    Move { from: String, to: String },
    /// Create a file that does not exist.
    Create { path: String, body: String },
}

/// Why a fix could not be turned into an edit. Named per cause so a caller can
/// tell "this needs a human" from "the tree moved under us".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The finding carried no fix. Not a failure: most findings need judgement.
    NoFixOffered { rule: &'static str, subject: String },
    /// The file the fix names is not in the corpus we were handed.
    SubjectAbsent { path: String },
    /// The text the fix expects to replace is not present. The tree moved, or
    /// the fix was computed against a different revision.
    AnchorNotFound { path: String, expected: String },
    /// Two fixes want to change the same file in ways that cannot both hold.
    Conflict { path: String, detail: String },
    /// Creating a path that already exists would destroy content.
    WouldOverwrite { path: String },
}

/// The result of planning. Both halves are always present: a plan that reports
/// only what it will do, and not what it declined, invites the caller to read
/// silence as completeness.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub edits: Vec<Edit>,
    pub refused: Vec<Refused>,
}

impl Plan {
    /// Every fix became an edit.
    pub fn is_complete(&self) -> bool {
        self.refused.is_empty() && !self.edits.is_empty()
    }

    /// Fixes that need a human, separated from fixes that failed.
    ///
    /// `NoFixOffered` is the expected case for a rule whose repair requires
    /// judgement; the others mean something went wrong. Kept distinct so a
    /// caller does not treat "needs judgement" as "broken".
    pub fn needs_judgement(&self) -> usize {
        self.refused
            .iter()
            .filter(|r| matches!(r, Refused::NoFixOffered { .. }))
            .count()
    }

    pub fn failed(&self) -> usize {
        self.refused.len() - self.needs_judgement()
    }
}

/// Turn findings into edits against the given file contents.
///
/// Pure: reads the map it is handed, writes nothing. The caller decides whether
/// to apply, which is what makes a dry run identical to a real one rather than
/// a separate code path that can drift.
pub fn plan(findings: &[&Finding], files: &BTreeMap<String, String>) -> Plan {
    let mut plan = Plan::default();
    let mut by_file: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();

    for finding in findings {
        match &finding.fix {
            None => plan.refused.push(Refused::NoFixOffered {
                rule: finding.rule,
                subject: finding.subject.clone(),
            }),
            Some(_) => by_file.entry(&finding.subject).or_default().push(finding),
        }
    }

    for (path, group) in by_file {
        match plan_file(path, &group, files) {
            Ok(mut edits) => plan.edits.append(&mut edits),
            Err(refused) => plan.refused.push(refused),
        }
    }
    plan.edits.sort_by(|a, b| edit_path(a).cmp(edit_path(b)));
    plan
}

fn edit_path(e: &Edit) -> &str {
    match e {
        Edit::Rewrite { path, .. } | Edit::Create { path, .. } => path,
        Edit::Move { from, .. } => from,
    }
}

/// All fixes for one file, resolved together.
///
/// Whole-file, deliberately: two fixes to one file must be seen together, and a
/// file that cannot take all of its fixes takes none of them.
fn plan_file(
    path: &str,
    findings: &[&Finding],
    files: &BTreeMap<String, String>,
) -> Result<Vec<Edit>, Refused> {
    let mut moves: Vec<(&String, &String)> = Vec::new();
    let mut creates: Vec<(&String, &String)> = Vec::new();
    let mut body: Option<String> = None;

    for finding in findings {
        match finding.fix.as_ref().expect("filtered above") {
            Fix::MovePath { from, to } => moves.push((from, to)),
            Fix::CreatePath { path, template } => creates.push((path, template)),
            Fix::RenameSymbol { from, to } | Fix::RetargetDependency { from, to } => {
                let current = match body.take() {
                    Some(b) => b,
                    None => files
                        .get(path)
                        .cloned()
                        .ok_or_else(|| Refused::SubjectAbsent {
                            path: path.to_string(),
                        })?,
                };
                if !current.contains(from.as_str()) {
                    return Err(Refused::AnchorNotFound {
                        path: path.to_string(),
                        expected: from.clone(),
                    });
                }
                body = Some(current.replace(from.as_str(), to.as_str()));
            }
        }
    }

    if moves.len() > 1 {
        return Err(Refused::Conflict {
            path: path.to_string(),
            detail: format!("{} fixes each want to move this file", moves.len()),
        });
    }
    if body.is_some() && !moves.is_empty() {
        return Err(Refused::Conflict {
            path: path.to_string(),
            detail: "one fix rewrites this file while another moves it".to_string(),
        });
    }

    let mut edits = Vec::new();
    if let Some(b) = body {
        edits.push(Edit::Rewrite {
            path: path.to_string(),
            body: b,
        });
    }
    for (from, to) in moves {
        edits.push(Edit::Move {
            from: from.clone(),
            to: to.clone(),
        });
    }
    for (p, template) in creates {
        if files.contains_key(p) {
            return Err(Refused::WouldOverwrite { path: p.clone() });
        }
        edits.push(Edit::Create {
            path: p.clone(),
            body: template.clone(),
        });
    }
    Ok(edits)
}
