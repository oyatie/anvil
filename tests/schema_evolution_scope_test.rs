//! Lane `schema-evolution-scope`: gate 53 read every removed line in the diff,
//! including Rust, and called it a breaking wire schema change.
//!
//! # The defect
//!
//! `compatibility_checker.rs` had no file-type scoping at all. Its whole
//! predicate was: the line starts with `-`, its lowercase form contains one of
//! `required`/`string`/`int32`/`int64`, and it contains `=`. Nothing about the
//! path the line came from ever entered the decision, so a removed Rust line
//! reading
//!
//! ```text
//! -            let mut current_file = "migration.sql".to_string();
//! ```
//!
//! matched on `to_string` and `=` and was published as
//! "Breaking deletion of wire schema field". That maps to `GateStatus::Failed`,
//! which blocks the merge queue.
//!
//! Measured against this repository's own last ten commits, with
//! `scripts/schema_regression.sh`, four of them were blocked -- 207e3c7 (2),
//! 4f573d8 (2), 5e38c72 (3), 553dd03 (14) -- in a tree containing zero `.proto`
//! files. Every one of those findings is a Rust line.
//!
//! # What the real tools do
//!
//! `buf breaking` parses both sides into `FileDescriptorSet`s and compares
//! them against an `--against` baseline; Confluent Schema Registry compares a
//! candidate schema against the registered versions of a subject; `oasdiff`
//! compares two resolved OpenAPI documents. All three share two properties this
//! gate had neither of: they parse the schema, and they are only ever pointed
//! at schema files. The parser and the stored baseline need infrastructure this
//! repository does not have. The scope does not, and it is the half that was
//! producing the false accusations.
//!
//! # Why each direction needs pinning
//!
//! A gate rewritten to report `NotMeasured` unconditionally would satisfy the
//! Rust tests below while measuring exactly as little as before -- the failure
//! mode `empty_scope_is_not_a_pass_test.rs` names for the four marker-scoped
//! gates. So every out-of-scope test here is paired with an in-scope one that
//! must still reach a real verdict: a genuine wire break to `Failed`, a
//! compatible schema edit to `Passed`.

use anvil::pre_merge_guard::GateStatus;
use anvil::schema_evolution::SchemaEvolutionRatchet;

const GATE_ID: &str = "schema_evolution_status";

fn evaluate(diff: &str) -> anvil::schema_evolution::SchemaEvolutionReport {
    SchemaEvolutionRatchet::new().evaluate_schema_evolution(diff)
}

/// A unified-diff section for one file, with the header the gate must read the
/// path out of.
fn file_diff(path: &str, body: &str) -> String {
    format!("diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1,4 +1,4 @@\n{body}\n")
}

// ---------------------------------------------------------------------------
// Out of scope: ordinary Rust is not a wire schema
// ---------------------------------------------------------------------------

/// Catches: the headline defect. Every line below was removed by a commit in
/// this repository's last ten, and every one of them was published as a
/// breaking wire schema change.
#[test]
fn removed_rust_lines_from_this_repositorys_own_history_are_not_wire_breaks() {
    // Verbatim from 207e3c7, 4f573d8, 5e38c72 and 553dd03.
    let real_removals = [
        r#"            let mut current_file = "migration.sql".to_string();"#,
        r#"                current_file = path.trim_start_matches("b/").to_string();"#,
        r#"            let stderr = String::from_utf8_lossy(&output.stderr);"#,
        r#"        let err = res.unwrap_err().to_string();"#,
        r#"pub type AffinityCacheMap = HashMap<String, (String, Instant)>;"#,
        r#"            let mut current_category = "General".to_string();"#,
    ];

    for removed in real_removals {
        let diff = file_diff("src/some_guard.rs", &format!("-{removed}"));
        let report = evaluate(&diff);

        assert_eq!(
            report.breaking_field_changes, 0,
            "a removed Rust line is not a wire schema change: {removed}"
        );
        assert_eq!(
            report.status.unmeasured_gate_id(),
            Some(GATE_ID),
            "a diff touching no schema file measured nothing, and must say so \
             rather than passing or failing: {removed}"
        );
        assert!(
            !report.passed,
            "an unmeasured gate has not passed: {removed}"
        );
    }
}

/// Catches: scope decided once for the whole diff instead of per file. A PR
/// that touches a `.proto` and a `.rs` must have the `.proto` scanned and the
/// `.rs` left alone -- not both, and not neither.
#[test]
fn a_mixed_pr_scans_the_proto_and_leaves_the_rust_alone() {
    let diff = format!(
        "{}{}",
        file_diff(
            "src/some_guard.rs",
            r#"-            let mut current_file = "migration.sql".to_string();"#
        ),
        file_diff("proto/order.proto", "-  string customer_id = 3;")
    );

    let report = evaluate(&diff);

    assert_eq!(
        report.breaking_field_changes, 1,
        "exactly the proto deletion is a finding; the Rust line is not.\n{}",
        report.summary
    );
    assert!(
        report.summary.contains("order.proto"),
        "a finding must name the schema file it came from: {}",
        report.summary
    );
    assert!(matches!(report.status, GateStatus::Failed(_)));
}

/// Catches: a `.proto`-shaped line inside a comment being read as a field.
#[test]
fn a_removed_proto_comment_is_not_a_removed_field() {
    let report = evaluate(&file_diff(
        "proto/order.proto",
        "-  // string customer_id = 3;",
    ));

    assert_eq!(report.breaking_field_changes, 0);
    assert!(
        report.passed,
        "a proto file was in scope and scanned clean: {}",
        report.summary
    );
    assert!(matches!(report.status, GateStatus::Passed));
}

// ---------------------------------------------------------------------------
// In scope: a genuine wire break still fires
// ---------------------------------------------------------------------------

/// Catches: a gate that reports `NotMeasured` for everything.
///
/// `buf breaking`'s `FIELD_NO_DELETE_UNLESS_NUMBER_RESERVED`: deleting a field
/// without reserving its number lets a later field reuse it, and old readers
/// then decode the new field into the old one.
#[test]
fn a_deleted_proto_field_whose_number_is_not_reserved_fails() {
    let report = evaluate(&file_diff(
        "proto/order.proto",
        "-  string customer_id = 3;",
    ));

    assert_eq!(report.breaking_field_changes, 1, "{}", report.summary);
    assert!(!report.passed);
    match &report.status {
        GateStatus::Failed(reason) => assert!(
            reason.contains("customer_id"),
            "the failure must name the field it is about: {reason}"
        ),
        other => panic!("a deleted unreserved field must fail the gate, got {other:?}"),
    }
}

/// Catches: the reserved half of the same rule being unimplemented, which would
/// make the gate fail every legitimate field removal.
#[test]
fn a_deleted_proto_field_whose_number_is_reserved_is_compatible() {
    let report = evaluate(&file_diff(
        "proto/order.proto",
        "-  string customer_id = 3;\n+  reserved 3;",
    ));

    assert_eq!(
        report.breaking_field_changes, 0,
        "reserving the number is exactly how buf says to delete a field: {}",
        report.summary
    );
    assert!(report.passed);
    assert!(matches!(report.status, GateStatus::Passed));
}

/// Catches: number reuse read as an ordinary deletion. Handing tag 3 to a
/// different field is the wire break the `tag_renumbering_detected` field is
/// named for.
#[test]
fn reusing_a_deleted_field_number_for_a_different_field_is_renumbering() {
    let report = evaluate(&file_diff(
        "proto/order.proto",
        "-  string customer_id = 3;\n+  string account_id = 3;",
    ));

    assert!(
        report.tag_renumbering_detected,
        "tag 3 was handed to a different field: {}",
        report.summary
    );
    assert!(!report.passed);
    assert!(matches!(report.status, GateStatus::Failed(_)));
}

/// Catches: `reserved` being read only as a number. Protobuf reserves names as
/// well, which is what stops a later field reclaiming the name in the JSON
/// mapping.
#[test]
fn a_deleted_proto_field_whose_name_is_reserved_is_compatible() {
    let report = evaluate(&file_diff(
        "proto/order.proto",
        "-  string customer_id = 3;\n+  reserved \"customer_id\";",
    ));

    assert_eq!(report.breaking_field_changes, 0, "{}", report.summary);
    assert!(report.passed);
}

/// Catches: a field line rewritten unchanged -- reindented, or moved with its
/// neighbours -- being read as a deletion followed by an unrelated addition on
/// the same number, which would report renumbering on a file nothing changed in.
#[test]
fn a_field_removed_and_re_added_unchanged_is_not_withdrawn() {
    let report = evaluate(&file_diff(
        "proto/order.proto",
        "-  string customer_id = 3;\n+    string customer_id = 3;",
    ));

    assert_eq!(report.breaking_field_changes, 0, "{}", report.summary);
    assert!(!report.tag_renumbering_detected);
    assert!(report.passed);
}

/// Catches: a summary that lists every finding unbounded, which is how a gate
/// with a hundred findings blows the GitHub comment limit for every other gate
/// on the scorecard.
#[test]
fn a_long_finding_list_is_truncated_in_the_summary() {
    let body = (1..=5)
        .map(|n| format!("-  string field_{n} = {n};"))
        .collect::<Vec<_>>()
        .join("\n");
    let report = evaluate(&file_diff("proto/order.proto", &body));

    assert_eq!(report.breaking_field_changes, 5, "{}", report.summary);
    assert!(
        report.summary.contains("and 2 more"),
        "the summary must say what it left out: {}",
        report.summary
    );
    assert!(
        !report.summary.contains("field_4"),
        "the fourth finding is past the listing budget: {}",
        report.summary
    );
}

/// Catches: proto2 `required` being added, which `buf`'s
/// `MESSAGE_SAME_REQUIRED_FIELDS` forbids in the WIRE category -- an old writer
/// emits no such field and the new reader rejects the message.
#[test]
fn adding_a_required_proto_field_fails() {
    let report = evaluate(&file_diff(
        "proto/order.proto",
        "+  required string tenant_id = 9;",
    ));

    assert_eq!(report.breaking_field_changes, 1, "{}", report.summary);
    assert!(matches!(report.status, GateStatus::Failed(_)));
}

/// Catches: the whole proto scan being replaced by "any change to a .proto is
/// breaking", which would fail every additive PR.
#[test]
fn adding_an_optional_proto_field_is_compatible() {
    let report = evaluate(&file_diff(
        "proto/order.proto",
        "+  optional string idempotency_token = 12;",
    ));

    assert_eq!(report.breaking_field_changes, 0, "{}", report.summary);
    assert!(report.passed);
    assert!(matches!(report.status, GateStatus::Passed));
}

// ---------------------------------------------------------------------------
// OpenAPI: the one schema format this repository actually has
// ---------------------------------------------------------------------------

/// Catches: `openapi/openapi.yaml`, the only schema file in this tree, being
/// out of scope -- which would leave the gate structurally unable to measure
/// anything here, or in scope but scanned with a proto predicate that no YAML
/// line can ever match, which is the vacuous pass in a new costume.
///
/// `oasdiff`'s `api-path-removed`: a consumer calling a removed endpoint gets a
/// 404 under the new contract.
#[test]
fn removing_an_openapi_endpoint_fails() {
    let report = evaluate(&file_diff("openapi/openapi.yaml", "-  /healthz:"));

    assert_eq!(report.breaking_field_changes, 1, "{}", report.summary);
    assert!(!report.passed);
    match &report.status {
        GateStatus::Failed(reason) => assert!(
            reason.contains("/healthz"),
            "the failure must name the endpoint it removed: {reason}"
        ),
        other => panic!("a removed endpoint must fail the gate, got {other:?}"),
    }
}

/// Catches: a removed *line* being read as a removed *endpoint*. Reindenting or
/// reordering moves the key without withdrawing it, and the path is still
/// served.
#[test]
fn reindenting_an_openapi_endpoint_is_not_removing_it() {
    let report = evaluate(&file_diff(
        "openapi/openapi.yaml",
        "-  /healthz:\n+    /healthz:",
    ));

    assert_eq!(report.breaking_field_changes, 0, "{}", report.summary);
    assert!(report.passed);
}

/// Catches: an OpenAPI edit that removes nothing being reported as anything
/// other than a pass -- the other end of the same wire.
#[test]
fn an_additive_openapi_change_passes() {
    let report = evaluate(&file_diff(
        "openapi/openapi.yaml",
        "+  /readyz:\n+    get:\n+      summary: readiness",
    ));

    assert_eq!(report.breaking_field_changes, 0, "{}", report.summary);
    assert!(report.passed);
    assert!(matches!(report.status, GateStatus::Passed));
}

/// Catches: any removed YAML key being read as a removed endpoint. A path item
/// starts with `/`; `externalDocs` is a sibling key of `paths` and withdrawing
/// it withdraws no route. The mapping line is here too, so the scan is pinned
/// on both the key-shaped and the value-shaped removal.
#[test]
fn removing_a_non_path_openapi_key_is_not_removing_an_endpoint() {
    let report = evaluate(&file_diff(
        "openapi/openapi.yaml",
        "-  externalDocs:\n-    url: https://example.com/docs\n-      summary: liveness probe",
    ));

    assert_eq!(report.breaking_field_changes, 0, "{}", report.summary);
    assert!(report.passed);
}

/// Catches: a removed YAML comment being read as content. `#` opens a comment
/// in YAML the way `//` does in proto.
#[test]
fn a_removed_openapi_comment_is_not_a_removed_endpoint() {
    let report = evaluate(&file_diff("openapi/openapi.yaml", "-  # /healthz: dropped"));

    assert_eq!(report.breaking_field_changes, 0, "{}", report.summary);
    assert!(report.passed);
}

/// Catches: a YAML file that is not an API description being pulled into scope
/// because it ends in `.yaml`. A CI workflow deleting a job step is not a wire
/// break.
#[test]
fn an_ordinary_yaml_file_is_not_an_api_description() {
    let report = evaluate(&file_diff(
        ".github/workflows/ci.yaml",
        "-  /healthz:\n-  string customer_id = 3;",
    ));

    assert_eq!(report.breaking_field_changes, 0);
    assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
}

// ---------------------------------------------------------------------------
// Degenerate inputs
// ---------------------------------------------------------------------------

/// Catches: a diff whose file headers never arrived being treated as scanned.
/// Without a header the gate does not know what it is holding, and "I could not
/// tell" is not "compatible".
#[test]
fn a_headerless_diff_is_not_measured() {
    let report = evaluate("-  string customer_id = 3;");

    assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
    assert!(!report.passed);
}

#[test]
fn an_empty_diff_is_not_measured() {
    let report = evaluate("");

    assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
    assert!(!report.passed);
}

/// Catches: the `---`/`+++` header lines of the unified diff itself being
/// counted as removed and added content.
#[test]
fn the_diffs_own_header_lines_are_not_schema_content() {
    let report = evaluate(&file_diff(
        "proto/order.proto",
        "   string customer_id = 3;",
    ));

    assert_eq!(
        report.breaking_field_changes, 0,
        "`--- a/proto/order.proto` is diff syntax, not a removed field: {}",
        report.summary
    );
    assert!(report.passed);
}
