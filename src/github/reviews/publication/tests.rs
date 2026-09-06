use super::{RecordedReview, ReviewPublication};

const HEAD: &str = "0123456789012345678901234567890123456789";

fn response(state: &str, head: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"id": 42, "state": state, "commit_id": head})).unwrap()
}

#[test]
fn only_an_actual_approved_receipt_for_this_head_certifies() {
    let receipt =
        RecordedReview::from_response(&response("APPROVED", HEAD), HEAD, "APPROVE").unwrap();
    assert!(receipt.require_approved_for(HEAD).is_ok());
    assert!(receipt.require_approved_for("another-head").is_err());
    let comment =
        RecordedReview::from_response(&response("COMMENTED", HEAD), HEAD, "COMMENT").unwrap();
    assert!(comment.require_approved_for(HEAD).is_err());
}

#[test]
fn missing_or_inconsistent_response_evidence_is_not_a_receipt() {
    for bytes in [
        b"not json".to_vec(),
        b"{}".to_vec(),
        response("COMMENTED", HEAD),
        response("PENDING", HEAD),
        response("APPROVED", "another-head"),
        serde_json::to_vec(&serde_json::json!({"id": 0, "state": "APPROVED", "commit_id": HEAD}))
            .unwrap(),
    ] {
        assert!(RecordedReview::from_response(&bytes, HEAD, "APPROVE").is_err());
    }
    assert!(RecordedReview::from_response(&response("APPROVED", HEAD), HEAD, "UNKNOWN").is_err());
}

#[test]
fn each_receipt_field_is_required_and_typed() {
    let valid = serde_json::json!({"id": 42, "state": "APPROVED", "commit_id": HEAD});
    for field in ["id", "state", "commit_id"] {
        let mut missing = valid.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            RecordedReview::from_response(&serde_json::to_vec(&missing).unwrap(), HEAD, "APPROVE")
                .is_err()
        );
        for wrong_type in [
            serde_json::Value::Null,
            serde_json::json!([]),
            serde_json::json!(true),
        ] {
            let mut wrong = valid.clone();
            wrong[field] = wrong_type;
            assert!(
                RecordedReview::from_response(
                    &serde_json::to_vec(&wrong).unwrap(),
                    HEAD,
                    "APPROVE"
                )
                .is_err()
            );
        }
    }
    let string_id = serde_json::json!({"id": "42", "state": "APPROVED", "commit_id": HEAD});
    assert!(
        RecordedReview::from_response(&serde_json::to_vec(&string_id).unwrap(), HEAD, "APPROVE")
            .is_err()
    );
}

#[test]
fn findings_can_be_published_without_a_formal_review() {
    assert!(
        ReviewPublication::SummaryCommentOnly
            .require_recorded()
            .is_err()
    );
    let receipt = RecordedReview::from_response(
        &response("CHANGES_REQUESTED", HEAD),
        HEAD,
        "REQUEST_CHANGES",
    )
    .unwrap();
    assert!(
        ReviewPublication::RecordedReview(receipt)
            .require_recorded()
            .is_ok()
    );
}
