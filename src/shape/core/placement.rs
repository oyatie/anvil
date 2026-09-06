//! The placement algorithm: ADR-0562 §3 as data, evaluated by a pure,
//! total, deterministic function. Where a step needs a fact the caller could
//! not supply, the answer is `NotMeasured` naming the fact — never a guess.

use super::glob::Glob;
use super::resolve::{ResolvedSpec, ResolvedUnit};
use super::spec::{PlacementStep, Skeleton};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathFacts {
    pub rel: String,
    /// True for a unit-level subject (a crate or package directory), false
    /// for a single file.
    pub is_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoleFacts {
    pub unit: Option<ResolvedUnit>,
    pub declared_face: Option<String>,
    pub crate_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DepFacts {
    pub consumer_units: Option<BTreeSet<String>>,
    pub composes_units: Option<BTreeSet<String>>,
    pub dag_rank: Option<BTreeMap<String, u32>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    Canonical {
        dest: String,
        step: String,
    },
    AlreadyCanonical {
        step: String,
    },
    Ambiguous {
        reason: String,
        candidates: Vec<String>,
    },
    NotMeasured {
        reason: String,
    },
}

fn basename(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

pub fn place(spec: &ResolvedSpec, p: &PathFacts, r: &RoleFacts, d: &DepFacts) -> Placement {
    for step in &spec.spec.placement.steps {
        match step {
            PlacementStep::MetaDir { dir } => {
                if p.rel.starts_with(dir.as_str()) {
                    return Placement::AlreadyCanonical {
                        step: "meta_dir".into(),
                    };
                }
            }
            PlacementStep::ArtifactClass {
                class,
                dest,
                patterns,
            } => {
                let hit = patterns.iter().any(|pat| {
                    let g = Glob::new(pat);
                    g.matches(&p.rel) || g.matches(basename(&p.rel))
                });
                if hit {
                    return if p.rel.starts_with(dest.as_str()) {
                        Placement::AlreadyCanonical {
                            step: format!("artifact_class:{class}"),
                        }
                    } else {
                        Placement::Canonical {
                            dest: format!("{dest}{}", basename(&p.rel)),
                            step: format!("artifact_class:{class}"),
                        }
                    };
                }
            }
            PlacementStep::SharedPrimitive {
                min_consumers,
                dest,
                ..
            } => {
                if !p.is_dir {
                    continue;
                }
                match &d.consumer_units {
                    None => return Placement::NotMeasured {
                        reason:
                            "shared_primitive needs consumer units; dependency facts unavailable"
                                .into(),
                    },
                    Some(c) if c.len() as u32 >= *min_consumers => {
                        return if p.rel.starts_with(dest.as_str()) {
                            Placement::AlreadyCanonical {
                                step: "shared_primitive".into(),
                            }
                        } else {
                            Placement::Canonical {
                                dest: format!("{dest}{}/", basename(p.rel.trim_end_matches('/'))),
                                step: "shared_primitive".into(),
                            }
                        };
                    }
                    Some(_) => {}
                }
            }
            PlacementStep::Composition { min_units, dest } => {
                if !p.is_dir {
                    continue;
                }
                match &d.composes_units {
                    None => {
                        return Placement::NotMeasured {
                            reason:
                                "composition needs composed units; dependency facts unavailable"
                                    .into(),
                        };
                    }
                    Some(c) if c.len() as u32 >= *min_units => {
                        let name = basename(p.rel.trim_end_matches('/'));
                        let want = dest.replace("<name>", name);
                        return if p.rel.starts_with(want.as_str()) {
                            Placement::AlreadyCanonical {
                                step: "composition".into(),
                            }
                        } else {
                            Placement::Canonical {
                                dest: want,
                                step: "composition".into(),
                            }
                        };
                    }
                    Some(_) => {}
                }
            }
            PlacementStep::UnitByFace => {
                return unit_by_face(spec, p, r);
            }
            PlacementStep::LowestDagNode => {}
        }
    }
    Placement::Ambiguous {
        reason: "no placement step matched".into(),
        candidates: Vec::new(),
    }
}

fn unit_by_face(spec: &ResolvedSpec, p: &PathFacts, r: &RoleFacts) -> Placement {
    let Some(unit) = &r.unit else {
        return Placement::Ambiguous {
            reason: "path belongs to no unit".into(),
            candidates: Vec::new(),
        };
    };
    let Some(skel) = spec.spec.skeletons.get(&unit.skeleton) else {
        return Placement::NotMeasured {
            reason: format!(
                "unit {} names unknown skeleton {}",
                unit.name, unit.skeleton
            ),
        };
    };
    let Some(rest) = p.rel.strip_prefix(unit.root.as_str()) else {
        return Placement::Ambiguous {
            reason: format!("path is outside unit root {}", unit.root),
            candidates: Vec::new(),
        };
    };
    place_within_unit(unit, skel, rest)
}

fn place_within_unit(unit: &ResolvedUnit, skel: &Skeleton, rest: &str) -> Placement {
    if skel.faces.values().any(|d| rest.starts_with(d.as_str())) {
        return Placement::AlreadyCanonical {
            step: "unit_by_face".into(),
        };
    }
    for (class, sat) in &skel.satellites {
        if unit.satellites_not_applicable.iter().any(|c| c == class) {
            continue;
        }
        for alias in &sat.aliases {
            if let Some(tail) = rest.strip_prefix(alias.as_str()) {
                return Placement::Canonical {
                    dest: format!("{}{}{}", unit.root, sat.dir, tail),
                    step: format!("satellite:{class}"),
                };
            }
        }
    }
    for (class, sat) in &skel.satellites {
        if let Some(tail) = rest.strip_prefix(sat.dir.as_str()) {
            if sat.excludes.iter().any(|e| tail.starts_with(e.as_str())) {
                continue;
            }
            return if Glob::new(&sat.form).matches(tail) {
                Placement::AlreadyCanonical {
                    step: format!("satellite:{class}"),
                }
            } else {
                Placement::Ambiguous {
                    reason: format!(
                        "{class} satellite holds {tail:?}, which does not match form {:?}",
                        sat.form
                    ),
                    candidates: Vec::new(),
                }
            };
        }
    }
    if !rest.contains('/') {
        return if skel.allowed_unit_root_files.iter().any(|f| f == rest) {
            Placement::AlreadyCanonical {
                step: "unit_root_file".into(),
            }
        } else {
            Placement::Ambiguous {
                reason: format!("unit root file {rest:?} is not allowlisted"),
                candidates: skel.allowed_unit_root_files.clone(),
            }
        };
    }
    let first = rest.split('/').next().unwrap_or(rest);
    let mut candidates: Vec<String> = skel.faces.values().cloned().collect();
    candidates.extend(skel.satellites.values().map(|s| s.dir.clone()));
    Placement::Ambiguous {
        reason: format!("directory {first:?} is neither a face nor a declared satellite"),
        candidates,
    }
}
