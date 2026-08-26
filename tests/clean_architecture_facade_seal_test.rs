//! The facade seal, exercised against the spellings that defeated it.
//!
//! Every case here failed a first implementation that split on `crate::` and
//! read one path out of the result. They are kept as cases, rather than as a
//! sentence in a doc comment, because each one is a spelling somebody will
//! write by accident and two of them silently ACCUSED conformant code.
//!
//! The first version of this rule was reviewed by running it, not by reading
//! it, and that is what found them.

use anvil::clean_architecture_guard::CleanArchitectureGuard;
use anvil::git_manager::PrDiffContext;

fn diff_of(path: &str, body: &str) -> PrDiffContext {
    let mut d = format!("+++ b/{path}\n");
    for l in body.lines() {
        d.push('+');
        d.push_str(l);
        d.push('\n');
    }
    PrDiffContext {
        repo: "seal/seal".into(),
        pr_number: 1,
        base_branch: "dev".into(),
        base_sha: String::new(),
        head_sha: String::new(),
        is_incremental: false,
        previous_head_sha: None,
        diff_content: d,
        changed_files: Vec::new(),
        repo_working_dir: std::path::PathBuf::from("."),
    }
}

fn bypasses(path: &str, body: &str) -> Vec<String> {
    let mut v: Vec<String> = CleanArchitectureGuard::new()
        .evaluate_architecture(&diff_of(path, body))
        .unwrap()
        .violations
        .iter()
        .filter(|x| x.description.contains("reaches past"))
        .map(|x| x.target_layer.clone())
        .collect();
    v.sort();
    v
}

/// A grouped `use` names several units at once. Reading only the first path
/// missed the rest, and left the brace glued to the unit name.
#[test]
fn a_grouped_use_names_every_unit_in_the_group() {
    assert_eq!(
        bypasses(
            "src/alpha/mod.rs",
            "use crate::{beta::adapters::X, gamma::core::Y};"
        ),
        vec!["beta::adapters", "gamma::core"]
    );
}

/// A nested group puts the brace exactly where the `::` between unit and face
/// would be, so neither name exists in the text in matchable form.
#[test]
fn a_nested_use_group_is_expanded_before_matching() {
    assert_eq!(
        bypasses("src/alpha/mod.rs", "use crate::beta::{core::X, ports::Y};"),
        vec!["beta::core", "beta::ports"]
    );
}

/// The evasion: reading one path per line means a legal reference placed first
/// hides an illegal one after it. One line, no cleverness required.
#[test]
fn a_legal_reference_first_on_the_line_does_not_hide_a_later_one() {
    assert_eq!(
        bypasses(
            "src/alpha/mod.rs",
            "let z = f(crate::beta::facade::A, crate::gamma::adapters::B);"
        ),
        vec!["gamma::adapters"]
    );
}

/// Sparing conformant code is half the rule. A unit reaching into its own
/// interior is the normal case -- it is how a facade uses its adapters.
#[test]
fn a_unit_using_its_own_interior_is_spared() {
    assert!(bypasses("src/alpha/mod.rs", "use crate::alpha::adapters::X;").is_empty());
    assert!(
        bypasses(
            "src/alpha/mod.rs",
            "use crate::{alpha::adapters::X, alpha::core::Y};"
        )
        .is_empty(),
        "a unit grouping its OWN faces was accused, because `{{alpha` can never equal `alpha`"
    );
}

/// `crate::` is rooted at a crate's `src/`. In a Cargo workspace, taking the
/// first path segment made every file a member of a unit called `crates`, so
/// conformant code was accused and cross-unit edges were compared against the
/// wrong name. Workspaces are the common layout for repositories with faces.
#[test]
fn workspace_layout_resolves_units_the_way_crate_paths_do() {
    assert!(
        bypasses(
            "crates/alpha/src/beta/mod.rs",
            "use crate::beta::adapters::X;"
        )
        .is_empty(),
        "a workspace unit using its own adapters was accused"
    );
    assert_eq!(
        bypasses(
            "crates/alpha/src/gamma/mod.rs",
            "use crate::beta::adapters::X;"
        ),
        vec!["beta::adapters"]
    );
}

/// A found violation must never be reported as an absence.
///
/// The verdict was keyed on whether any file sat in a layer, which is the
/// layer-direction rules' subject, not the seal's. So a real bypass in an
/// unlayered file -- the common case, and the case the seal exists for -- was
/// returned as "NOT MEASURED ... no inward-dependency claim can be made", with
/// the finding dropped from the summary entirely.
#[test]
fn a_bypass_in_an_unlayered_file_is_reported_not_swallowed() {
    let r = CleanArchitectureGuard::new()
        .evaluate_architecture(&diff_of(
            "src/alpha/mod.rs",
            "use crate::beta::adapters::X;",
        ))
        .unwrap();
    assert_eq!(r.violations.len(), 1);
    assert!(!r.is_clean);
    assert!(
        r.measurement.is_measured(),
        "a run that FOUND a violation reported itself unmeasured: {:?}",
        r.measurement
    );
    assert!(
        r.summary.contains("beta") && r.summary.contains("adapters"),
        "the finding is missing from the summary: {}",
        r.summary
    );
}

/// A subject the rule cannot read must be reported as unmeasured, not as zero.
/// The seal is spelled in Rust paths; a TypeScript tree offers it nothing.
#[test]
fn a_tree_with_no_rust_source_is_unmeasured_rather_than_clean() {
    let r = CleanArchitectureGuard::new()
        .evaluate_architecture(&diff_of(
            "src/alpha/service.ts",
            "import { X } from '../beta/adapters/x';",
        ))
        .unwrap();
    assert!(
        !r.is_clean,
        "a tree the rule cannot read was reported clean: {}",
        r.summary
    );
    assert!(
        !r.measurement.is_measured(),
        "claimed to measure a tree with no Rust source: {:?}",
        r.measurement
    );
}
