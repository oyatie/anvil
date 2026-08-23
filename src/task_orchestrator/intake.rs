//! Operator interview → verified ephemeral package → Product XOR Program.
//!
//! Raw client text never reaches implement. Ambiguous or wrong needs fail
//! closed here. The package is not written under product `plan/` or `tasks/`.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::default_layout::{
    first_component, is_capability_root, layout_violations, FORBIDDEN_NAMES,
};
use super::delivery_role::HandoffAgent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterviewDraft {
    pub idea: String,
    pub research_citations: Vec<String>,
    pub constraints: Vec<String>,
    pub acceptance: Vec<String>,
    pub target_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactPackage {
    pub package_id: String,
    pub idea: String,
    pub research_citations: Vec<String>,
    pub constraints: Vec<String>,
    pub acceptance: Vec<String>,
    pub target_paths: Vec<String>,
    pub handoff: HandoffAgent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntakeVerdict {
    NeedClarification { questions: Vec<String> },
    Rejected { reason: String },
    Packaged(ArtifactPackage),
}

fn ambiguous(s: &str) -> bool {
    let t = s.trim();
    t.is_empty()
        || t.eq_ignore_ascii_case("tbd")
        || t.eq_ignore_ascii_case("todo")
        || t.contains("???")
}

fn route_handoff(paths: &[String]) -> Result<HandoffAgent, String> {
    let mut product = false;
    let mut program = false;
    for p in paths {
        let Some(root) = first_component(p) else {
            continue;
        };
        if root == "app" {
            product = true;
        } else if is_capability_root(root) {
            program = true;
        }
    }
    match (product, program) {
        (true, false) => Ok(HandoffAgent::Product),
        (false, true) => Ok(HandoffAgent::Program),
        (true, true) => Err("mixed app/ and capability paths; split into two packages".into()),
        (false, false) => Err("no app/ or capability path; operator must assign an owner".into()),
    }
}

/// Research + working tree + layout. Does not spawn. Does not pass raw client text.
pub fn interview(draft: &InterviewDraft, repo_root: &Path) -> IntakeVerdict {
    for p in &draft.target_paths {
        if let Some(root) = first_component(p) {
            if FORBIDDEN_NAMES.contains(&root) {
                return IntakeVerdict::Rejected {
                    reason: format!(
                        "client asked for dump root `{root}`; that is not engineering practice"
                    ),
                };
            }
        }
    }
    if !draft.target_paths.is_empty() {
        let layout = layout_violations(&draft.target_paths);
        if !layout.is_empty() {
            return IntakeVerdict::Rejected {
                reason: format!("wrong shape: {}", layout.join("; ")),
            };
        }
    }

    let mut questions = Vec::new();
    if ambiguous(&draft.idea) {
        questions.push("idea is empty or a placeholder; what is the actual need?".into());
    }
    if draft.acceptance.is_empty() || draft.acceptance.iter().any(|a| ambiguous(a)) {
        questions.push("acceptance is missing or TBD; what would falsify this?".into());
    }
    if draft.target_paths.is_empty() {
        questions.push("no target paths; which capability or app owns this?".into());
    }
    if draft.research_citations.is_empty() {
        questions.push("no citations; which existing docs or code were checked?".into());
    }
    for cite in &draft.research_citations {
        if !repo_root.join(cite).exists() {
            questions.push(format!("citation `{cite}` is not in the working tree"));
        }
    }
    if !draft.target_paths.is_empty() {
        if let Err(msg) = route_handoff(&draft.target_paths) {
            questions.push(msg);
        }
    }
    if !questions.is_empty() {
        return IntakeVerdict::NeedClarification { questions };
    }

    let handoff = match route_handoff(&draft.target_paths) {
        Ok(h) => h,
        Err(reason) => return IntakeVerdict::Rejected { reason },
    };
    IntakeVerdict::Packaged(ArtifactPackage {
        package_id: package_id(&draft.idea, &draft.target_paths),
        idea: draft.idea.trim().to_string(),
        research_citations: draft.research_citations.clone(),
        constraints: draft.constraints.clone(),
        acceptance: draft.acceptance.clone(),
        target_paths: draft.target_paths.clone(),
        handoff,
    })
}

fn package_id(idea: &str, paths: &[String]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    idea.hash(&mut h);
    paths.hash(&mut h);
    format!("pkg-{:x}", h.finish())
}

pub fn package_must_not_land_in_product_dump(package: &ArtifactPackage) -> Result<()> {
    for p in &package.target_paths {
        if let Some(root) = first_component(p) {
            if FORBIDDEN_NAMES.contains(&root) {
                bail!("package path `{p}` is a dump root; intake is ephemeral, not plan/");
            }
        }
    }
    Ok(())
}
