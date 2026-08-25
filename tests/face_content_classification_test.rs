//! Seeded defects for the face-content classifier.
//!
//! Placement rules answer "is this crate in a legal directory". They cannot
//! answer "is this port code sitting in an adapter" -- a crate named
//! `foo-adapter` under `adapters/` passes every path rule while holding pure
//! domain logic.
//!
//! Each rule below was measured against a 438-crate reference monorepo before
//! being written; a rule the reference tree fails is a wrong rule, not a
//! finding. Every test plants a defect and asserts the classifier names it,
//! and asserts the conformant twin stays silent.

use anvil::shape::core::face_content::{CrateFacts, content_violations};
use std::collections::BTreeSet;

fn facts(owner: &str, face: &str, leaf: &str) -> CrateFacts {
    CrateFacts {
        owner: owner.into(),
        face: face.into(),
        leaf: leaf.into(),
        package_name: format!("{owner}-{leaf}"),
        dependencies: BTreeSet::new(),
        foreign_capabilities: BTreeSet::new(),
        under_app: false,
    }
}

fn deps(mut c: CrateFacts, d: &[&str]) -> CrateFacts {
    c.dependencies = d.iter().map(|s| (*s).to_string()).collect();
    c
}

fn rules(v: &[anvil::shape::core::face_content::ContentViolation]) -> Vec<&'static str> {
    v.iter().map(|x| x.rule).collect()
}

#[test]
fn a_port_that_reaches_for_a_database_is_named() {
    let planted = deps(facts("iam", "ports", "policy-api"), &["serde", "sqlx"]);
    assert_eq!(rules(&content_violations(&[planted])), ["io_in_pure_face"]);
}

#[test]
fn a_port_of_pure_value_types_is_silent() {
    let clean = deps(
        facts("audit", "ports", "emission-kernel"),
        &["serde", "thiserror"],
    );
    assert!(
        content_violations(&[clean]).is_empty(),
        "serde and thiserror say nothing about purity and must not be treated as I/O"
    );
}

#[test]
fn core_holding_an_async_runtime_is_named() {
    let planted = deps(facts("flags", "core", "server"), &["tokio"]);
    assert_eq!(rules(&content_violations(&[planted])), ["io_in_pure_face"]);
}

#[test]
fn the_same_dependency_in_an_adapter_is_correct() {
    // An adapter exists to hold exactly this. The rule must not fire on the
    // face whose whole purpose is technology.
    let ok = deps(facts("data", "adapters", "outbox-sqlx"), &["sqlx", "tokio"]);
    assert!(content_violations(&[ok]).is_empty());
}

#[test]
fn a_facade_composing_two_other_capabilities_is_a_misplaced_app() {
    let mut planted = facts("compute", "facade", "functions");
    planted.foreign_capabilities = ["data".to_string(), "network".to_string()].into();
    assert_eq!(
        rules(&content_violations(&[planted])),
        ["facade_composes_foreign_capabilities"]
    );
}

#[test]
fn a_facade_over_one_other_capability_is_still_a_facade() {
    let mut ok = facts("iam", "facade", "pdp-app");
    ok.foreign_capabilities = ["audit".to_string()].into();
    assert!(content_violations(&[ok]).is_empty());
}

#[test]
fn a_product_under_app_may_compose_many_capabilities() {
    // Composing capabilities is what a product is for; the rule targets a
    // capability facade that has quietly become one.
    let mut ok = facts("application", "facade", "application-app");
    ok.under_app = true;
    ok.foreign_capabilities = ["iam".to_string(), "billing".to_string(), "data".to_string()].into();
    assert!(content_violations(&[ok]).is_empty());
}

#[test]
fn a_name_a_move_did_not_update_is_named() {
    // Real shapes from the reference tree: a vendor/grouping prefix that
    // survived a capability rename.
    let mut planted = facts("bus", "adapters", "file");
    planted.package_name = "messaging-file-adapter".into();
    assert_eq!(
        rules(&content_violations(&[planted])),
        ["package_name_not_canonical"]
    );
}

#[test]
fn both_canonical_spellings_are_accepted() {
    let mut bare = facts("audit", "core", "event-kernel");
    bare.package_name = "event-kernel".into();
    let mut owned = facts("audit", "core", "event-kernel");
    owned.package_name = "audit-event-kernel".into();
    assert!(content_violations(&[bare, owned]).is_empty());
}

#[test]
fn one_crate_can_break_several_rules_at_once() {
    let mut planted = deps(facts("bus", "ports", "file"), &["tokio"]);
    planted.package_name = "messaging-file-adapter".into();
    let got = rules(&content_violations(&[planted]));
    assert!(got.contains(&"io_in_pure_face"), "got {got:?}");
    assert!(got.contains(&"package_name_not_canonical"), "got {got:?}");
}
