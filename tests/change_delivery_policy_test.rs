//! Every refusal to enlist names its reason; a broken policy pauses; the
//! structure profile relaxes I1 only by a visible waiver list (D8).

use anvil::change_delivery::core::{
    Admission, LandingInputs, LandingMode, LandingPolicy, Withheld, admit,
};
use std::collections::BTreeSet;

fn green() -> LandingInputs {
    LandingInputs {
        rule_id: "satellite_alias_used".into(),
        purity_passed: true,
        required_checks_all_passed: true,
        required_checks_pending: false,
        human_changes_requested: false,
        unresolved_threads: false,
        human_approved: false,
        open_shape_prs: 0,
        merged_today: 0,
        evictions_today: 0,
        in_cooldown: false,
        conflicts_with_queued_shard: false,
        unmeasured_gates: BTreeSet::new(),
        failed_gates: BTreeSet::new(),
        kill_switch: false,
    }
}

fn auto() -> LandingPolicy {
    LandingPolicy {
        mode: LandingMode::AutoEnlistWhenGreen,
        ..LandingPolicy::default()
    }
}

#[test]
fn defaults_are_propose_only_and_never_enlist() {
    assert_eq!(
        admit(&LandingPolicy::default(), &green()),
        Err(Withheld::ProposeOnly)
    );
}

#[test]
fn a_fully_green_shard_is_admitted_under_auto_enlist() {
    assert_eq!(admit(&auto(), &green()), Ok(()));
}

#[test]
fn every_withholding_reason_is_named() {
    let p = auto();
    type Mutation = Box<dyn Fn(&mut LandingInputs)>;
    let cases: Vec<(Mutation, Withheld)> = vec![
        (Box::new(|i| i.kill_switch = true), Withheld::KillSwitch),
        (Box::new(|i| i.in_cooldown = true), Withheld::Cooldown),
        (
            Box::new(|i| i.evictions_today = 2),
            Withheld::BackpressureEvictions { today: 2 },
        ),
        (
            Box::new(|i| i.merged_today = 3),
            Withheld::BudgetExhausted {
                what: "max_merged_per_day",
            },
        ),
        (
            Box::new(|i| i.purity_passed = false),
            Withheld::PurityFailed,
        ),
        (
            Box::new(|i| i.required_checks_pending = true),
            Withheld::CiPending,
        ),
        (
            Box::new(|i| i.required_checks_all_passed = false),
            Withheld::CiFailed,
        ),
        (
            Box::new(|i| i.human_changes_requested = true),
            Withheld::HumanChangesRequested,
        ),
        (
            Box::new(|i| i.unresolved_threads = true),
            Withheld::UnresolvedThreads,
        ),
        (
            Box::new(|i| i.conflicts_with_queued_shard = true),
            Withheld::ConflictsWithQueuedShard,
        ),
        (
            Box::new(|i| {
                i.failed_gates.insert("slo_status".into());
            }),
            Withheld::GatesFailed(["slo_status".to_string()].into_iter().collect()),
        ),
        (
            Box::new(|i| {
                i.unmeasured_gates.insert("shadow_traffic_status".into());
            }),
            Withheld::UnmeasuredGates(["shadow_traffic_status".to_string()].into_iter().collect()),
        ),
    ];
    for (mutate, expected) in cases {
        let mut i = green();
        mutate(&mut i);
        assert_eq!(admit(&p, &i), Err(expected));
    }
    let paused = LandingPolicy {
        paused: true,
        ..auto()
    };
    assert_eq!(admit(&paused, &green()), Err(Withheld::Paused));
    let approval = LandingPolicy {
        require_human_approval: true,
        ..auto()
    };
    assert_eq!(
        admit(&approval, &green()),
        Err(Withheld::AwaitingHumanApproval)
    );
    let mut po = auto();
    po.propose_only_rules.insert("satellite_alias_used".into());
    assert_eq!(admit(&po, &green()), Err(Withheld::ProposeOnly));
}

#[test]
fn structure_profile_waives_only_the_named_unmeasured_gates() {
    let p = LandingPolicy {
        admission: Admission::StructureProfile {
            waived_gates: ["shadow_traffic_status".to_string()].into_iter().collect(),
        },
        ..auto()
    };
    let mut i = green();
    i.unmeasured_gates.insert("shadow_traffic_status".into());
    assert_eq!(admit(&p, &i), Ok(()), "waived gate does not withhold");
    i.unmeasured_gates.insert("slo_status".into());
    assert_eq!(
        admit(&p, &i),
        Err(Withheld::UnmeasuredGates(
            ["slo_status".to_string()].into_iter().collect()
        ))
    );
    i.failed_gates.insert("shadow_traffic_status".into());
    assert!(
        matches!(admit(&p, &i), Err(Withheld::GatesFailed(_))),
        "a waiver covers unmeasured, never failed"
    );
}

#[test]
fn an_unparseable_policy_pauses_the_repository_and_says_why() {
    let (p, problem) = LandingPolicy::load(Some(b"{ not json"));
    assert!(p.paused);
    assert!(problem.unwrap().contains("paused"));
    let (p, problem) = LandingPolicy::load(None);
    assert!(!p.paused && problem.is_none());
    assert_eq!(p.mode, LandingMode::ProposeOnly);
    let (_, problem) = LandingPolicy::load(Some(
        br#"{"mode":"auto_enlist_when_green","unknown_knob":1}"#,
    ));
    assert!(problem.is_some(), "unknown keys are refused, not ignored");
}
