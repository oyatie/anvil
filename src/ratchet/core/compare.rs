//! Judging a candidate against the frozen reference, and checking that a
//! proposed baseline regeneration only shrinks.

use super::baseline::{Baseline, Mode};
use super::signoff::Signoff;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleVerdict {
    pub mode: Mode,
    /// Keys present now and absent from the frozen reference and the signoff.
    pub regressions: BTreeSet<String>,
    /// Keys present now, absent from the reference, covered by the signoff.
    pub signed_off: BTreeSet<String>,
    /// Keys present now and in the reference (tolerated debt).
    pub tolerated: BTreeSet<String>,
    /// Keys in the reference and absent now.
    pub fixed: BTreeSet<String>,
    pub fails: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RatchetVerdict {
    pub per_rule: BTreeMap<String, RuleVerdict>,
    /// Signoff entries that name a key not present in the candidate: the
    /// door was left open for nothing, which is itself a failure.
    pub inert_signoff: Vec<(String, String)>,
    pub fails: bool,
}

/// `current` holds every measured key per rule. A rule measured now but
/// absent from the frozen baseline takes `default_mode(rule)`; with no mode at
/// all it is advisory — a rule nobody declared cannot block.
pub fn compare(
    frozen: &Baseline,
    current: &BTreeMap<String, BTreeSet<String>>,
    signoff: &Signoff,
    default_mode: impl Fn(&str) -> Option<(Mode, bool)>,
) -> RatchetVerdict {
    let mut per_rule = BTreeMap::new();
    let mut rules: BTreeSet<&String> = frozen.rules.keys().collect();
    rules.extend(current.keys());

    for rule in rules {
        let empty = BTreeSet::new();
        let now = current.get(rule).unwrap_or(&empty);
        let (mode, reference): (Mode, &BTreeSet<String>) = match frozen.rules.get(rule) {
            Some(rb) => (rb.mode, &rb.keys),
            None => (
                default_mode(rule).map(|(m, _)| m).unwrap_or(Mode::Advisory),
                &empty,
            ),
        };
        let mut regressions = BTreeSet::new();
        let mut signed_off = BTreeSet::new();
        let mut tolerated = BTreeSet::new();
        for k in now {
            if reference.contains(k) {
                tolerated.insert(k.clone());
            } else if signoff.covers(rule, k) {
                signed_off.insert(k.clone());
            } else {
                regressions.insert(k.clone());
            }
        }
        let fixed: BTreeSet<String> = reference.difference(now).cloned().collect();
        let fails = mode == Mode::BlockOnNew && !regressions.is_empty();
        per_rule.insert(
            rule.clone(),
            RuleVerdict {
                mode,
                regressions,
                signed_off,
                tolerated,
                fixed,
                fails,
            },
        );
    }

    let mut inert_signoff = Vec::new();
    for (rule, keys) in &signoff.additions {
        for k in keys {
            let present = current.get(rule).is_some_and(|c| c.contains(k));
            if !present {
                inert_signoff.push((rule.clone(), k.clone()));
            }
        }
    }

    let fails = per_rule.values().any(|v| v.fails) || !inert_signoff.is_empty();
    RatchetVerdict {
        per_rule,
        inert_signoff,
        fails,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Growth {
    KeyAdded { rule: String, key: String },
    RuleAdded { rule: String },
    ModeDowngraded { rule: String },
    FrozenEmptyRelaxed { rule: String },
}

/// A regeneration may only shrink (I7). Anything that grows must be signed
/// off; a signed-off addition is consumed by exactly this regeneration.
pub fn regen_is_monotonic(
    frozen: &Baseline,
    proposed: &Baseline,
    signoff: &Signoff,
) -> Result<(), Vec<Growth>> {
    let mut growth = Vec::new();
    for (rule, pb) in &proposed.rules {
        match frozen.rules.get(rule) {
            None => {
                if pb.mode == Mode::BlockOnNew && !pb.keys.is_empty() {
                    let unsigned = pb.keys.iter().any(|k| !signoff.covers(rule, k));
                    if unsigned {
                        growth.push(Growth::RuleAdded { rule: rule.clone() });
                    }
                }
            }
            Some(fb) => {
                if fb.mode == Mode::BlockOnNew
                    && pb.mode == Mode::Advisory
                    && !signoff.mode_downgrades.contains(rule)
                {
                    growth.push(Growth::ModeDowngraded { rule: rule.clone() });
                }
                if fb.frozen_empty && !pb.frozen_empty {
                    growth.push(Growth::FrozenEmptyRelaxed { rule: rule.clone() });
                }
                if pb.mode == Mode::BlockOnNew {
                    for k in pb.keys.difference(&fb.keys) {
                        if !signoff.covers(rule, k) {
                            growth.push(Growth::KeyAdded {
                                rule: rule.clone(),
                                key: k.clone(),
                            });
                        }
                    }
                }
            }
        }
    }
    if growth.is_empty() {
        Ok(())
    } else {
        Err(growth)
    }
}
