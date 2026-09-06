//! The three retired capability claims, checked against the matrix's actual data.
//!
//! Matrix source reexports a sibling table; scanning renderer source alone lost
//! these labels when that table moved. This test imports the same exported data
//! that label_for reads. It does not certify arbitrary wording or live capabilities.

use anvil::pre_merge_guard::matrix::GATE_LABELS;

const REQUIRED_GATES: [&str; 3] = [
    "local_probe_status",
    "chaos_injection_status",
    "feature_flag_status",
];

const FORBIDDEN_CLAIMS: [&str; 3] = [
    "AST linting",
    "Synthetic packet loss, DNS jitter & DB failover certification",
    "Zero stale or dead toggle fallback branches",
];

fn validate_capability_rows(rows: &[(&str, &str, &str)]) -> Result<(), String> {
    for required in REQUIRED_GATES {
        let matching: Vec<_> = rows.iter().filter(|(id, _, _)| *id == required).collect();
        if matching.len() != 1 {
            return Err(format!(
                "{required}: expected one row, found {}",
                matching.len()
            ));
        }
        let (_, label, detail) = matching[0];
        if label.trim().is_empty() || detail.trim().is_empty() {
            return Err(format!("{required}: blank label or detail"));
        }
    }

    // Preserve the old whole-table prohibition: moving a claim to another gate
    // is not a fix. Both fields feed the actual matrix renderer.
    for (id, label, detail) in rows {
        for claim in FORBIDDEN_CLAIMS {
            if label.contains(claim) || detail.contains(claim) {
                return Err(format!("{id}: forbidden capability claim {claim}"));
            }
        }
    }
    Ok(())
}

#[test]
fn the_matrix_claims_no_capability_these_three_gates_do_not_have() {
    validate_capability_rows(GATE_LABELS).expect("the actual matrix table must retain honest rows");
}

#[test]
fn absent_required_rows_are_not_an_empty_success() {
    assert!(validate_capability_rows(&[]).is_err());
    for required in REQUIRED_GATES {
        let rows: Vec<_> = GATE_LABELS
            .iter()
            .copied()
            .filter(|(id, _, _)| *id != required)
            .collect();
        let error = validate_capability_rows(&rows).expect_err("missing subject must fail");
        assert!(error.contains(required) && error.contains("found 0"));
    }
}

#[test]
fn duplicate_required_rows_cannot_choose_the_honest_twin() {
    for required in REQUIRED_GATES {
        let mut rows = GATE_LABELS.to_vec();
        let row = *rows
            .iter()
            .find(|(id, _, _)| *id == required)
            .expect("actual row");
        rows.push(row);
        let error = validate_capability_rows(&rows).expect_err("duplicate subject must fail");
        assert!(error.contains(required) && error.contains("found 2"));
    }
}

#[test]
fn required_labels_and_details_must_be_nonblank() {
    for required in REQUIRED_GATES {
        for blank in ["", " \t\n"] {
            for field in [1, 2] {
                let mut rows = GATE_LABELS.to_vec();
                let row = rows
                    .iter_mut()
                    .find(|(id, _, _)| *id == required)
                    .expect("actual row");
                if field == 1 {
                    row.1 = blank;
                } else {
                    row.2 = blank;
                }
                let error = validate_capability_rows(&rows).expect_err("blank subject must fail");
                assert!(error.contains(required) && error.contains("blank label or detail"));
            }
        }
    }
}

#[test]
fn every_retired_claim_is_rejected_in_either_field_of_any_row() {
    for required in REQUIRED_GATES.into_iter().chain(["doc_parity_status"]) {
        for claim in FORBIDDEN_CLAIMS {
            for field in [1, 2] {
                let mut rows = GATE_LABELS.to_vec();
                let row = rows
                    .iter_mut()
                    .find(|(id, _, _)| *id == required)
                    .expect("actual row");
                if field == 1 {
                    row.1 = claim;
                } else {
                    row.2 = claim;
                }
                let error = validate_capability_rows(&rows).expect_err("retired claim must fail");
                assert!(error.contains(required) && error.contains(claim));
            }
        }
    }
}

#[test]
fn nonempty_wording_is_not_pinned_to_todays_labels() {
    let mut rows = GATE_LABELS.to_vec();
    for required in REQUIRED_GATES {
        let row = rows
            .iter_mut()
            .find(|(id, _, _)| *id == required)
            .expect("actual row");
        row.1 = "Source-text check";
        row.2 = "Examines the supplied source text; no running system is observed";
    }
    assert!(validate_capability_rows(&rows).is_ok());
}
