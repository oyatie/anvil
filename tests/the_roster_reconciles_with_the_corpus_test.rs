//! ADR-0002 claims its roster can be checked against the live corpus. This is
//! the check.
//!
//! The ADR says it in its own words: "Roster names are the live report fields
//! minus the `_status` suffix, so every `Today:` line can be checked
//! mechanically against `PreMergeCertificationReport`. Every gate in the live
//! corpus is named by exactly one seat."
//!
//! Nothing checked it. A decision record whose central claim is mechanical and
//! unchecked drifts silently: a gate deleted from the corpus stays named by a
//! seat, and a gate added is owned by nobody. Both have happened.
//!
//! ADR-0002:108 forbids the other shape this could take -- "Do not add twenty
//! named gates. Give each hole a real artifact and a measurement." So this adds
//! no gate. It reconciles the two lists that already exist.

use std::collections::BTreeMap;
use std::path::Path;

const ADR: &str = "docs/adr/0002-agentic-roster-and-delivery-fabric.md";

/// Seat name -> the gates its `Today:` line claims.
///
/// A seat with `Today: nothing` claims none, which is the ADR's own way of
/// saying the seat exists and owns no gate yet. That is not drift.
fn seats() -> BTreeMap<String, Vec<String>> {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(ADR))
        .expect("ADR-0002 is readable");
    let mut out = BTreeMap::new();
    let mut seat = String::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t
            .split_once(". ")
            .filter(|(n, _)| n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty())
            .map(|(_, r)| r)
        {
            seat = rest.split('.').next().unwrap_or("").trim().to_string();
        }
        if seat.is_empty() {
            continue;
        }
        let Some(today) = t.split_once("Today: ") else {
            continue;
        };
        // Each entry is a bare gate name, but one seat writes
        // `review_verdict + \`is_certified_ready\``. Taking the leading
        // identifier of each fragment reads that seat correctly; requiring the
        // WHOLE fragment to be an identifier silently dropped it, and reported
        // a gate the roster does own as owned by nobody. A parse that cannot
        // read its subject accusing that subject is I1 pointed the other way.
        let claimed = today
            .1
            .split('.')
            .next()
            .unwrap_or("")
            .split(',')
            .map(|g| {
                g.trim()
                    .trim_start_matches('`')
                    .chars()
                    .take_while(|c| c.is_ascii_lowercase() || *c == '_')
                    .collect::<String>()
            })
            .filter(|g| !g.is_empty() && g != "nothing")
            .collect::<Vec<_>>();
        out.entry(seat.clone())
            .or_insert_with(Vec::new)
            .extend(claimed);
    }
    out
}

/// Every gate the live corpus holds, as the roster spells them.
fn corpus() -> Vec<String> {
    anvil::pre_merge_guard::matrix::GATE_LABELS
        .iter()
        .map(|(id, _, _)| id.trim_end_matches("_status").to_string())
        .collect()
}

/// The scan must be able to find its subjects, or it reconciles nothing.
#[test]
fn both_lists_are_non_empty() {
    let seats = seats();
    assert!(
        seats.len() >= 10,
        "parsed only {} seat(s) from the ADR; the roster has twenty, so this \
         parse is broken rather than the roster",
        seats.len()
    );
    assert!(
        seats.values().any(|g| !g.is_empty()),
        "no seat claims any gate, so every reconciliation below is vacuous"
    );
    assert!(!corpus().is_empty(), "the live corpus is empty");
}

/// A seat naming a gate the corpus does not have.
///
/// This is the direction that rots quietly. Deleting a gate does not touch the
/// ADR, so the roster keeps claiming a job nobody does -- and the ADR's
/// Overturn-When says a seat is cut by Jason, not by a deletion elsewhere.
#[test]
fn no_seat_claims_a_gate_the_corpus_does_not_have() {
    let live = corpus();
    let mut orphans: Vec<String> = Vec::new();
    for (seat, gates) in seats() {
        for g in gates {
            if !live.contains(&g) {
                orphans.push(format!("{seat} claims `{g}`"));
            }
        }
    }
    assert!(
        orphans.is_empty(),
        "{} roster entr(ies) name a gate the live corpus does not have:\n  {}\n\
         ADR-0002 says roster names ARE the live report fields. A seat still \
         claiming a deleted gate is a decision record describing a system that \
         no longer exists.",
        orphans.len(),
        orphans.join("\n  ")
    );
}

/// A gate no seat names, or one that two seats do.
#[test]
fn every_gate_is_named_by_exactly_one_seat() {
    let owners: BTreeMap<String, Vec<String>> =
        seats()
            .into_iter()
            .fold(BTreeMap::new(), |mut acc, (seat, gates)| {
                for g in gates {
                    acc.entry(g).or_insert_with(Vec::new).push(seat.clone());
                }
                acc
            });

    let mut unowned = Vec::new();
    let mut contested = Vec::new();
    for gate in corpus() {
        match owners.get(&gate).map(Vec::as_slice) {
            None => unowned.push(gate),
            Some([_]) => {}
            Some(many) => contested.push(format!("`{gate}` claimed by {}", many.join(" and "))),
        }
    }

    assert!(
        contested.is_empty(),
        "{} gate(s) are claimed by more than one seat:\n  {}",
        contested.len(),
        contested.join("\n  ")
    );
    // Bounded, not zero, and only in this direction.
    //
    // The other two findings this reconciliation made are mechanical: a gate
    // deleted from the corpus, and a seat naming a field by its old name. Both
    // are the ADR conforming to its own rule that "roster names ARE the live
    // report fields", so both are fixed.
    //
    // These two are not mechanical. Deciding which seat owns `cloud_native` and
    // `stack_whitelist` is an ownership judgement, and ADR-0002's Overturn-When
    // reserves that: "Jason cuts a seat, or a measurement proves a named job is
    // already owned by another seat." A measurement has not proven it, and
    // assigning them here would be a decision taken by whoever happened to be
    // writing the test.
    //
    // So the gap is recorded at what it is and may not grow: a gate added
    // without an owner fails this, which is the property the ADR claims.
    const UNOWNED_TODAY: &[&str] = &["cloud_native", "stack_whitelist"];
    let unexpected: Vec<&String> = unowned
        .iter()
        .filter(|g| !UNOWNED_TODAY.contains(&g.as_str()))
        .collect();
    assert!(
        unexpected.is_empty(),
        "{} gate(s) in the live corpus are named by no seat:\n  {unexpected:#?}\n\
         ADR-0002: \"Every gate in the live corpus is named by exactly one \
         seat.\" A gate nobody owns is a job nobody has.",
        unexpected.len()
    );
    let stale: Vec<&&str> = UNOWNED_TODAY
        .iter()
        .filter(|g| !unowned.iter().any(|u| u == *g))
        .collect();
    assert!(
        stale.is_empty(),
        "{stale:?} now has an owner. Remove it from UNOWNED_TODAY in this same \
         change -- a list of known gaps that outlives the gap stops bounding \
         anything."
    );
}
