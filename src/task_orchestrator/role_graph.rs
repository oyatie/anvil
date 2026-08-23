//! Role graph is a DAG with fan-out. It is not a single sequential pipeline.

use super::delivery_role::DeliveryRole;
use std::collections::BTreeSet;

/// Direct predecessors that must complete before `role` is ready on a slice.
pub fn deps(role: DeliveryRole) -> &'static [DeliveryRole] {
    use DeliveryRole::*;
    match role {
        Experiment | Plan | TrunkAudit | ContractAmend => &[],
        PlanReview => &[Plan],
        Prd => &[PlanReview],
        Spec => &[Prd],
        SpecReview => &[Spec],
        Tdd => &[SpecReview],
        TestReview => &[Tdd],
        Implement => &[TestReview],
        ImplReview | Coverage | SecurityHarden | WhiteBox | GreyBox | BlackBox | Docs => {
            &[Implement]
        }
        CoverageReview => &[Coverage],
        SecurityReview => &[SecurityHarden],
        Simplify => &[ImplReview],
        QualityReview => &[Simplify],
        PrBabysit => &[
            ImplReview,
            CoverageReview,
            SecurityReview,
            QualityReview,
            Docs,
        ],
    }
}

pub fn is_unblocked(role: DeliveryRole, completed: &BTreeSet<DeliveryRole>) -> bool {
    !completed.contains(&role) && deps(role).iter().all(|d| completed.contains(d))
}

pub fn transitive_deps(role: DeliveryRole) -> BTreeSet<DeliveryRole> {
    let mut out = BTreeSet::new();
    fn walk(role: DeliveryRole, out: &mut BTreeSet<DeliveryRole>) {
        for d in deps(role) {
            if out.insert(*d) {
                walk(*d, out);
            }
        }
    }
    walk(role, &mut out);
    out
}

/// After Implement, several successor roles become ready together.
pub fn fan_out_after_implement() -> &'static [DeliveryRole] {
    use DeliveryRole::*;
    &[
        ImplReview,
        Coverage,
        SecurityHarden,
        WhiteBox,
        GreyBox,
        BlackBox,
        Docs,
    ]
}
