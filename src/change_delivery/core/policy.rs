//! The per-repository landing policy (`.anvil/landing.json` on the tenant's
//! default branch) and the pure admission decision. A policy that does not
//! parse pauses the repository (I12: a broken file never widens behaviour).

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LandingMode {
    ProposeOnly,
    AutoEnlistWhenGreen,
}

/// D8: structure-only shape PRs may land under a named waiver list; every
/// other PR keeps the full matrix.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "profile", rename_all = "snake_case")]
pub enum Admission {
    FullMatrix,
    StructureProfile {
        #[serde(default)]
        waived_gates: BTreeSet<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LandingPolicy {
    #[serde(default = "d_mode")]
    pub mode: LandingMode,
    #[serde(default)]
    pub paused: bool,
    #[serde(default = "d_open")]
    pub max_open_shape_prs: u32,
    #[serde(default = "d_merged")]
    pub max_merged_per_day: u32,
    #[serde(default = "d_files")]
    pub max_files_per_pr: u32,
    #[serde(default = "d_true")]
    pub one_unit_at_a_time: bool,
    #[serde(default = "d_true")]
    pub require_destination_stable: bool,
    #[serde(default = "d_cooldown")]
    pub cooldown_after_revert_hours: u32,
    #[serde(default = "d_evictions")]
    pub max_evictions_per_day: u32,
    #[serde(default = "d_admission")]
    pub admission: Admission,
    #[serde(default)]
    pub require_human_approval: bool,
    /// Files whose edit serialises shards: at most one in flight touches any
    /// of them. Tenant-declared; defaults to the root build manifests the
    /// spec's profiles name.
    #[serde(default)]
    pub hot_files: BTreeSet<String>,
    /// Rules whose shards are always proposed, never enlisted, whatever the
    /// mode says.
    #[serde(default)]
    pub propose_only_rules: BTreeSet<String>,
}

fn d_mode() -> LandingMode {
    LandingMode::ProposeOnly
}
fn d_open() -> u32 {
    2
}
fn d_merged() -> u32 {
    3
}
fn d_files() -> u32 {
    40
}
fn d_true() -> bool {
    true
}
fn d_cooldown() -> u32 {
    24
}
fn d_evictions() -> u32 {
    2
}
fn d_admission() -> Admission {
    Admission::FullMatrix
}

impl Default for LandingPolicy {
    fn default() -> Self {
        serde_json::from_str("{}").expect("defaults parse")
    }
}

impl LandingPolicy {
    /// Absent file -> defaults (propose_only). Unparseable -> paused, with
    /// the reason carried so the dashboard can show it.
    pub fn load(bytes: Option<&[u8]>) -> (LandingPolicy, Option<String>) {
        match bytes {
            None => (LandingPolicy::default(), None),
            Some(b) => match serde_json::from_slice::<LandingPolicy>(b) {
                Ok(p) => (p, None),
                Err(e) => (
                    LandingPolicy {
                        paused: true,
                        ..LandingPolicy::default()
                    },
                    Some(format!(
                        "landing policy does not parse; repository paused: {e}"
                    )),
                ),
            },
        }
    }
}

/// Everything the admission decision looks at, gathered by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandingInputs {
    pub rule_id: String,
    pub purity_passed: bool,
    pub required_checks_all_passed: bool,
    pub required_checks_pending: bool,
    pub human_changes_requested: bool,
    pub unresolved_threads: bool,
    pub human_approved: bool,
    pub open_shape_prs: u32,
    pub merged_today: u32,
    pub evictions_today: u32,
    pub in_cooldown: bool,
    pub conflicts_with_queued_shard: bool,
    pub unmeasured_gates: BTreeSet<String>,
    pub failed_gates: BTreeSet<String>,
    pub kill_switch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Withheld {
    ProposeOnly,
    Paused,
    KillSwitch,
    Cooldown,
    BudgetExhausted { what: &'static str },
    BackpressureEvictions { today: u32 },
    CiPending,
    CiFailed,
    HumanChangesRequested,
    UnresolvedThreads,
    AwaitingHumanApproval,
    ConflictsWithQueuedShard,
    PurityFailed,
    GatesFailed(BTreeSet<String>),
    UnmeasuredGates(BTreeSet<String>),
}

/// Pure: may this shard be enlisted right now? Every refusal names its reason.
pub fn admit(policy: &LandingPolicy, i: &LandingInputs) -> Result<(), Withheld> {
    if i.kill_switch {
        return Err(Withheld::KillSwitch);
    }
    if policy.paused {
        return Err(Withheld::Paused);
    }
    if policy.mode == LandingMode::ProposeOnly || policy.propose_only_rules.contains(&i.rule_id) {
        return Err(Withheld::ProposeOnly);
    }
    if i.in_cooldown {
        return Err(Withheld::Cooldown);
    }
    if i.evictions_today >= policy.max_evictions_per_day {
        return Err(Withheld::BackpressureEvictions {
            today: i.evictions_today,
        });
    }
    if i.merged_today >= policy.max_merged_per_day {
        return Err(Withheld::BudgetExhausted {
            what: "max_merged_per_day",
        });
    }
    if !i.purity_passed {
        return Err(Withheld::PurityFailed);
    }
    if i.required_checks_pending {
        return Err(Withheld::CiPending);
    }
    if !i.required_checks_all_passed {
        return Err(Withheld::CiFailed);
    }
    if i.human_changes_requested {
        return Err(Withheld::HumanChangesRequested);
    }
    if i.unresolved_threads {
        return Err(Withheld::UnresolvedThreads);
    }
    if policy.require_human_approval && !i.human_approved {
        return Err(Withheld::AwaitingHumanApproval);
    }
    if i.conflicts_with_queued_shard {
        return Err(Withheld::ConflictsWithQueuedShard);
    }
    if !i.failed_gates.is_empty() {
        return Err(Withheld::GatesFailed(i.failed_gates.clone()));
    }
    match &policy.admission {
        Admission::FullMatrix => {
            if !i.unmeasured_gates.is_empty() {
                return Err(Withheld::UnmeasuredGates(i.unmeasured_gates.clone()));
            }
        }
        Admission::StructureProfile { waived_gates } => {
            let blocking: BTreeSet<String> = i
                .unmeasured_gates
                .difference(waived_gates)
                .cloned()
                .collect();
            if !blocking.is_empty() {
                return Err(Withheld::UnmeasuredGates(blocking));
            }
        }
    }
    Ok(())
}
