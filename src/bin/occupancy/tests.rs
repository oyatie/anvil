use super::*;

fn set(paths: &[&str]) -> BTreeSet<String> {
    paths.iter().map(|s| (*s).to_string()).collect()
}

fn occupied(id: &str, paths: &[&str]) -> (String, BTreeSet<String>) {
    (id.to_owned(), set(paths))
}

fn args(raw: &[&str]) -> Vec<String> {
    raw.iter().map(|s| (*s).to_string()).collect()
}

/// This hop, at number `pr`, against the open set.
fn admit(
    pr: u64,
    this: &[&str],
    in_flight: &[(String, BTreeSet<String>)],
    at_trunk: bool,
) -> GateStatus {
    verdict(&set(this), pr, in_flight, at_trunk, false)
}

#[test]
fn an_unreadable_list_is_errored_not_passed() {
    let fixture = tempfile::tempdir().expect("ordinary freshness fixture");
    let proof_path = fixture.path().join("freshness.txt");
    std::fs::write(&proof_path, evidence("dev", "feature", true, true))
        .expect("valid evidence before the unreadable-list assertion");
    let status = run(&args(&[
        "--this",
        "/nonexistent/this.txt",
        "--in-flight",
        "/nonexistent/in-flight.txt",
        "--freshness-file",
        proof_path.to_str().expect("UTF-8 fixture path"),
        "--this-pr",
        "7",
    ]));
    assert!(
        matches!(status, GateStatus::Errored(_)),
        "a list the check could not read is absent evidence, not an empty path-set: {status:?}"
    );
    assert!(!admits(&status));
}

#[test]
fn an_overlap_is_failed_and_names_the_pull_request_holding_the_path() {
    let status = admit(
        9,
        &["tests/lane_a.rs"],
        &[occupied("pr-7", &["tests/lane_a.rs"])],
        true,
    );
    let GateStatus::Failed(reason) = &status else {
        panic!("an overlap is a defect this gate measured: {status:?}");
    };
    assert!(reason.contains("tests/lane_a.rs"), "{reason}");
    assert!(
        reason.contains("pr-7"),
        "the refusal must name the occupant: {reason}"
    );
    assert!(!admits(&status));
}

/// An owner the collecting step wrote in a shape this binary does not
/// understand is a hop that would silently stop being compared against.
#[test]
fn an_unparseable_owner_is_errored_not_a_hop_quietly_dropped() {
    let status = verdict(
        &set(&["tests/lane_a.rs"]),
        9,
        &[occupied("branch-foo", &["tests/lane_a.rs"])],
        true,
        false,
    );
    let GateStatus::Errored(reason) = &status else {
        panic!("an owner that does not parse is absent evidence: {status:?}");
    };
    assert!(reason.contains("branch-foo"), "{reason}");
    assert!(!admits(&status));
}

/// The override is visible in the status, so nothing downstream can read it
/// as a measured disjointness.
#[test]
fn the_override_label_admits_an_overlap_as_a_warning_never_as_a_pass() {
    let file = &["tests/shared.rs"];
    let status = verdict(&set(file), 9, &[occupied("pr-7", file)], true, true);
    let GateStatus::Warning(reason) = &status else {
        panic!("an audited override admits, and says so: {status:?}");
    };
    assert!(
        reason.contains(OVERRIDE_LABEL),
        "the warning must name the label that admitted it: {reason}"
    );
    assert!(reason.contains("pr-7"), "and what it overrode: {reason}");
    assert!(admits(&status));
    assert_ne!(
        status,
        GateStatus::Passed,
        "an overridden admission is not a measurement"
    );
}

/// The one refusal the label may not lift.
#[test]
fn the_override_label_does_not_admit_a_hub_off_a_stale_base() {
    let status = verdict(&set(&["src/main.rs"]), 7, &[], false, true);
    assert!(
        matches!(status, GateStatus::Failed(_)),
        "the other refusals order hops that were each measured; this one says \
         the measurement was taken against a combination the queue will not \
         build, and admitting it publishes a verdict about a tree that does \
         not exist: {status:?}"
    );
    assert!(!admits(&status));
}

#[test]
fn errored_is_the_state_for_a_forge_that_did_not_answer_and_it_does_not_admit() {
    let status = GateStatus::Errored("rate limited".to_owned());
    assert!(
        !admits(&status),
        "a check that could not measure must not read as no overlap"
    );
}

#[test]
fn not_measured_would_admit_which_is_why_occupancy_never_reports_it() {
    let unmeasured = GateStatus::NotMeasured {
        gate_id: "occupancy".to_owned(),
        reason: "no data source".to_owned(),
    };
    assert!(
        unmeasured.is_acceptable(),
        "NotMeasured is acceptable by construction, so occupancy must never emit it"
    );
    assert!(!admits(&unmeasured), "and it is not an admission either");
}

fn evidence(base: &str, head: &str, at_tip: bool, same_tree: bool) -> String {
    let fields = [
        "occupancy-freshness-v1",
        "owner/repo",
        "owner/repo",
        "owner/repo",
        base,
        head,
    ]
    .into_iter()
    .map(str::to_owned)
    .chain(
        [
            'a',
            'a',
            'b',
            if at_tip { 'b' } else { 'c' },
            'd',
            if same_tree { 'd' } else { 'e' },
        ]
        .map(|c| c.to_string().repeat(40)),
    )
    .collect::<Vec<_>>();
    fields.join("\n")
}

// Preserve legacy test assertions while constructing their strict proof through
// the same private parser as production; false cannot mint promotion authority.
fn verdict(
    this: &BTreeSet<String>,
    pr: u64,
    open: &[(String, BTreeSet<String>)],
    at_tip: bool,
    label: bool,
) -> GateStatus {
    let proof =
        freshness::FreshnessProof::parse(&evidence("dev", "feature", at_tip, true)).unwrap();
    super::verdict(this, pr, open, &proof, label)
}

fn from_record(
    record: &str,
    paths: &[&str],
    open: Vec<(String, BTreeSet<String>)>,
    label: bool,
) -> GateStatus {
    evaluate(
        freshness::FreshnessProof::parse(record).map(|freshness| inputs::Inputs {
            this: set(paths),
            this_pr: 9,
            in_flight: open,
            freshness,
            override_label: label,
        }),
    )
}

#[test]
fn actual_proof_to_verdict_only_equates_verified_promotion_trees() {
    let record = evidence("staging", "dev", false, true);
    let proof = freshness::FreshnessProof::parse(&record).unwrap();
    assert!(proof.diagnostic().contains("EquivalentPromotionTree"));
    assert_eq!(
        from_record(&record, &["src/main.rs"], vec![], false),
        GateStatus::Passed
    );
    for (base, head, same_tree) in [("dev", "feature", true), ("staging", "dev", false)] {
        for label in [false, true] {
            let status = from_record(
                &evidence(base, head, false, same_tree),
                &["src/main.rs"],
                vec![],
                label,
            );
            assert!(matches!(status, GateStatus::Failed(_)));
            assert!(!admits(&status));
        }
    }
}

#[test]
fn promotion_proof_preserves_overlap_hub_order_and_recorded_override() {
    let record = evidence("staging", "dev", false, true);
    for (other, reason) in [("src/main.rs", "occupied"), ("Cargo.lock", "N=1")] {
        let status = from_record(
            &record,
            &["src/main.rs"],
            vec![occupied("pr-7", &[other])],
            false,
        );
        let GateStatus::Failed(message) = status else {
            panic!("must refuse: {status:?}")
        };
        assert!(message.contains(reason));
        let overridden = from_record(
            &record,
            &["src/main.rs"],
            vec![occupied("pr-7", &[other])],
            true,
        );
        assert!(matches!(overridden, GateStatus::Warning(_)));
    }
    assert_eq!(
        from_record(
            &record,
            &["src/main.rs"],
            vec![occupied("pr-10", &["Cargo.lock"])],
            false
        ),
        GateStatus::Passed
    );
}

#[test]
fn invalid_actual_evidence_is_errored_and_never_not_measured() {
    let status = from_record("missing evidence", &["src/main.rs"], vec![], true);
    assert!(matches!(status, GateStatus::Errored(_)));
    assert!(!admits(&status));
}

#[test]
fn legacy_public_boolean_entries_remain_strict_for_hubs() {
    use anvil::change_delivery::facade::occupancy::{SpawnKind, admit_in_queue, admit_spawn};
    for at_tip in [false, true] {
        let expected = if at_tip {
            Ok(SpawnKind::Hub)
        } else {
            Err(SpawnRefused::HubOnStaleBase)
        };
        assert_eq!(
            admit_spawn(&set(&["src/main.rs"]), &anvil_hubs(), &[], at_tip),
            expected
        );
        assert_eq!(
            admit_in_queue(&set(&["src/main.rs"]), 9, &anvil_hubs(), &[], at_tip),
            expected
        );
    }
}
