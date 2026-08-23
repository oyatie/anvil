//! Deterministic ready hops. Anvil does not spawn agents and does not fold a lane.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::default_layout::{enforce_on_repo, layout_violations};
use super::delivery_role::{DeliveryRole, HandoffAgent};
use super::path_occupancy::path_sets_disjoint;
use super::role_graph::is_unblocked;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceState {
    pub id: String,
    pub paths: Vec<String>,
    pub handoff: HandoffAgent,
    pub completed: BTreeSet<DeliveryRole>,
    pub in_flight: BTreeSet<DeliveryRole>,
}

impl SliceState {
    pub fn done(&self) -> bool {
        self.completed.contains(&DeliveryRole::PrBabysit)
    }

    pub fn active(&self) -> bool {
        !self.done() && (!self.completed.is_empty() || !self.in_flight.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadyHop {
    pub slice_id: String,
    pub role: DeliveryRole,
    pub paths: Vec<String>,
    pub handoff: HandoffAgent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimedHop {
    pub hop_id: u64,
    pub slice_id: String,
    pub role: DeliveryRole,
    pub paths: Vec<String>,
    pub agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadySnapshot {
    pub repo: String,
    pub hops: Vec<ReadyHop>,
    pub lane_counts: BTreeMap<String, usize>,
}

#[derive(Default)]
pub struct DeliveryBoard {
    slices: BTreeMap<String, SliceState>,
    next_hop_id: u64,
    claims: BTreeMap<u64, ClaimedHop>,
    used_agents: BTreeSet<String>,
}

impl DeliveryBoard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn admit_slice(
        &mut self,
        id: impl Into<String>,
        paths: Vec<String>,
        handoff: HandoffAgent,
        repo: &str,
        completed: BTreeSet<DeliveryRole>,
    ) -> Result<()> {
        let id = id.into();
        if self.slices.contains_key(&id) {
            bail!("slice `{id}` already admitted");
        }
        if enforce_on_repo(repo) {
            let v = layout_violations(&paths);
            if !v.is_empty() {
                bail!("layout: {}", v.join("; "));
            }
        }
        self.slices.insert(
            id.clone(),
            SliceState {
                id,
                paths,
                handoff,
                completed,
                in_flight: BTreeSet::new(),
            },
        );
        Ok(())
    }

    pub fn ready_hops(&self) -> Vec<ReadyHop> {
        let mut hops = Vec::new();
        for slice in self.slices.values() {
            if slice.done() {
                continue;
            }
            for role in all_roles().iter().copied() {
                if is_unblocked(role, &slice.completed) && !slice.in_flight.contains(&role) {
                    hops.push(ReadyHop {
                        slice_id: slice.id.clone(),
                        role,
                        paths: slice.paths.clone(),
                        handoff: slice.handoff,
                    });
                }
            }
        }
        hops.sort_by(|a, b| (&a.slice_id, &a.role).cmp(&(&b.slice_id, &b.role)));
        hops
    }

    /// Ready hops that do not overlap a held path-set.
    ///
    /// A slice holds its paths once selected. At most one mutating hop per
    /// slice is schedulable in a snapshot. Overlapping admitted slices: first
    /// wins, the rest wait — that is occupancy, not a fold.
    pub fn schedulable_hops(&self) -> Vec<ReadyHop> {
        let mut held: BTreeSet<String> = BTreeSet::new();
        let mut occupied: Vec<Vec<String>> = Vec::new();
        let mut mutating_picked: BTreeSet<String> = BTreeSet::new();
        for s in self.slices.values() {
            if !s.in_flight.is_empty() {
                held.insert(s.id.clone());
                occupied.push(s.paths.clone());
                if s.in_flight.iter().any(|r| r.mutates_paths()) {
                    mutating_picked.insert(s.id.clone());
                }
            }
        }
        let mut out = Vec::new();
        for hop in self.ready_hops() {
            if !held.contains(&hop.slice_id) {
                let clash = occupied.iter().any(|p| !path_sets_disjoint(p, &hop.paths));
                if clash {
                    continue;
                }
                held.insert(hop.slice_id.clone());
                occupied.push(hop.paths.clone());
            }
            if hop.role.mutates_paths() && mutating_picked.contains(&hop.slice_id) {
                continue;
            }
            if hop.role.mutates_paths() {
                mutating_picked.insert(hop.slice_id.clone());
            }
            out.push(hop);
        }
        out
    }

    pub fn lane_ready(&self, role: DeliveryRole) -> Vec<ReadyHop> {
        self.schedulable_hops()
            .into_iter()
            .filter(|h| h.role == role)
            .collect()
    }

    /// Bind a fresh agent to one hop. Does not spawn a process.
    pub fn claim(
        &mut self,
        slice_id: &str,
        role: DeliveryRole,
        agent_id: impl Into<String>,
    ) -> Result<ClaimedHop> {
        let agent_id = agent_id.into();
        if !self.used_agents.insert(agent_id.clone()) {
            bail!("agent `{agent_id}` reused; each hop requires a fresh agent");
        }
        let ok = self
            .schedulable_hops()
            .into_iter()
            .any(|h| h.slice_id == slice_id && h.role == role);
        if !ok {
            bail!("hop {slice_id}/{role:?} is not schedulable");
        }
        let slice = self
            .slices
            .get_mut(slice_id)
            .ok_or_else(|| anyhow::anyhow!("unknown slice `{slice_id}`"))?;
        slice.in_flight.insert(role);
        self.next_hop_id += 1;
        let hop = ClaimedHop {
            hop_id: self.next_hop_id,
            slice_id: slice_id.to_string(),
            role,
            paths: slice.paths.clone(),
            agent_id,
        };
        self.claims.insert(hop.hop_id, hop.clone());
        Ok(hop)
    }

    pub fn complete(&mut self, hop_id: u64) -> Result<()> {
        let hop = self
            .claims
            .remove(&hop_id)
            .ok_or_else(|| anyhow::anyhow!("unknown hop {hop_id}"))?;
        let slice = self
            .slices
            .get_mut(&hop.slice_id)
            .ok_or_else(|| anyhow::anyhow!("unknown slice {}", hop.slice_id))?;
        slice.in_flight.remove(&hop.role);
        slice.completed.insert(hop.role);
        Ok(())
    }

    pub fn snapshot(&self, repo: &str) -> ReadySnapshot {
        let hops = self.schedulable_hops();
        let mut lane_counts = BTreeMap::new();
        for h in &hops {
            *lane_counts.entry(h.role.lane().to_string()).or_insert(0) += 1;
        }
        ReadySnapshot {
            repo: repo.to_string(),
            hops,
            lane_counts,
        }
    }

    /// N ready hops of a role must not be bound to k < N agents.
    pub fn assert_lane_not_folded(
        role: DeliveryRole,
        ready_n: usize,
        bound_n: usize,
    ) -> Result<()> {
        if bound_n < ready_n {
            bail!("lane folded: {bound_n} agents for {ready_n} ready {role:?} hops");
        }
        Ok(())
    }

    /// Consumer writes a draft port on *their* paths. Owner files stay untouched.
    pub fn file_draft_port(consumer_paths: &[String], draft_path: &str) -> Result<()> {
        if !draft_path.contains("/ports/draft/") && !draft_path.contains("/adapters/draft/") {
            bail!("draft port must live under ports/draft/ or adapters/draft/");
        }
        if !consumer_paths.iter().any(|p| p == draft_path) {
            bail!("draft `{draft_path}` is not in the consumer path-set");
        }
        Ok(())
    }

    pub fn reject_write_to_foreign_paths(
        writer_paths: &[String],
        foreign_path: &str,
    ) -> Result<()> {
        if !writer_paths.iter().any(|p| p == foreign_path) {
            bail!("no authority over `{foreign_path}`; file a draft port on owned paths");
        }
        Ok(())
    }
}

fn all_roles() -> &'static [DeliveryRole] {
    use DeliveryRole::*;
    &[
        Experiment,
        Plan,
        PlanReview,
        Prd,
        Spec,
        SpecReview,
        Tdd,
        TestReview,
        Implement,
        ImplReview,
        Coverage,
        CoverageReview,
        SecurityHarden,
        SecurityReview,
        WhiteBox,
        GreyBox,
        BlackBox,
        Docs,
        Simplify,
        QualityReview,
        ContractAmend,
        PrBabysit,
        TrunkAudit,
    ]
}
