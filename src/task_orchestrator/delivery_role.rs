//! Hyperscaler delivery roles. Lanes, not a serial mega-agent.

use crate::ai_driver::AgenticStage;
use serde::{Deserialize, Serialize};

/// Who receives a verified intake package. Never the implement lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HandoffAgent {
    Product,
    Program,
}

/// One hop on a slice. Each hop is a fresh agent; lanes do not fold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DeliveryRole {
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
}

impl DeliveryRole {
    pub fn mutates_paths(self) -> bool {
        matches!(
            self,
            Self::Experiment
                | Self::Plan
                | Self::Prd
                | Self::Spec
                | Self::Tdd
                | Self::Implement
                | Self::Coverage
                | Self::SecurityHarden
                | Self::Docs
                | Self::Simplify
                | Self::ContractAmend
        )
    }

    pub fn lane(self) -> &'static str {
        match self {
            Self::Experiment => "experiment",
            Self::Plan | Self::PlanReview | Self::Prd | Self::Spec | Self::SpecReview => "plan",
            Self::Tdd | Self::TestReview | Self::Coverage | Self::CoverageReview => "test",
            Self::Implement | Self::Simplify => "implement",
            Self::ImplReview
            | Self::QualityReview
            | Self::WhiteBox
            | Self::GreyBox
            | Self::BlackBox => "review",
            Self::SecurityHarden | Self::SecurityReview => "security",
            Self::Docs => "docs",
            Self::ContractAmend => "contract",
            Self::PrBabysit => "pr",
            Self::TrunkAudit => "trunk",
        }
    }

    pub fn model_stage(self) -> AgenticStage {
        match self {
            Self::Experiment | Self::TrunkAudit => AgenticStage::Recon,
            Self::Plan | Self::Prd | Self::Docs => AgenticStage::Planning,
            Self::PlanReview => AgenticStage::PlanReview,
            Self::Spec => AgenticStage::ArchitectSpec,
            Self::SpecReview | Self::WhiteBox | Self::GreyBox | Self::BlackBox => {
                AgenticStage::SpecReview
            }
            Self::Tdd
            | Self::Implement
            | Self::Coverage
            | Self::SecurityHarden
            | Self::Simplify
            | Self::ContractAmend => AgenticStage::Implementation,
            Self::TestReview
            | Self::ImplReview
            | Self::CoverageReview
            | Self::SecurityReview
            | Self::QualityReview => AgenticStage::CodeReviewAudit,
            Self::PrBabysit => AgenticStage::GitOps,
        }
    }
}
