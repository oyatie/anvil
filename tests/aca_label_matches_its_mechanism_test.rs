//! Gate `automated_canary_status` may not advertise a rank test it never runs.
//!
//! # The claim and the mechanism
//!
//! The matrix row for this gate is rendered onto every pull request from
//! `pre_merge_guard::gate_labels::GATE_LABELS`. Two places in this repository
//! already record what the gate does: `StatisticalCanaryEngine`'s own doc
//! comment says the Mann-Whitney U-test the gate is named for is not
//! implemented, and `fidelity/registry.rs` records the same gap under
//! `Fidelity::Aspirational`. A third place -- the row a pull request author
//! reads -- is the one that must agree with them.
//!
//! Two facts decide what the row is allowed to say, and both are asserted here
//! rather than assumed:
//!
//! 1. The certification pipeline calls `evaluate_without_metrics_source`, which
//!    yields `NotMeasured` before any arithmetic. No distribution reaches the
//!    engine on a pull request at all.
//! 2. The engine, on the distribution a real metrics source would supply,
//!    compares arithmetic means against a fixed relative bound. A rank test
//!    would answer differently, and
//!    [`the_engine_compares_means_not_ranks`] exhibits an input where the two
//!    disagree -- so this is a demonstration, not a restatement of the doc
//!    comment.
//!
//! # Scope
//!
//! One gate. The census in issue #59 enumerates roughly a dozen further rows
//! whose detail names a mechanism the code does not reach; narrowing all of
//! them is the report-shape change that issue asks for a decision on, and is
//! deliberately not attempted here. This file pins the one row whose claim two
//! other files in the tree already contradict.
//!
//! # Both directions of I1
//!
//! The source scans below resolve their subject through
//! `source_scan::paths::module_source`, which reads `thing.rs` or every `.rs`
//! under `thing/` and panics when neither exists -- so a scan that has lost
//! its subject fails loudly instead of reporting the pipeline clean. And the
//! label assertions are paired with mechanism assertions in both directions:
//! an engine that failed every input, or one that measured nothing at all,
//! would satisfy the naming rows and is refused by the rows above them.

use anvil::automated_canary::{
    AutomatedCanaryAnalysis, CanaryVerdict, MetricDistribution, StatisticalCanaryEngine,
};
use anvil::pre_merge_guard::matrix::{MatrixRenderer, label_for};
use anvil::pre_merge_guard::report::{GateStatus, PreMergeCertificationReport};
use anvil::source_scan::paths::module_source;
use std::path::Path;

const GATE: &str = "automated_canary_status";

/// Vocabulary that names a statistical hypothesis test or its output.
///
/// A gate that reaches no test at all on a pull request, and whose engine is a
/// comparison of two means when it is reached, may use none of it about its
/// own result.
const RANK_TEST_VOCABULARY: &[&str] = &[
    "mann-whitney",
    "mann whitney",
    "u-test",
    "u test",
    "wilcoxon",
    "rank-sum",
    "rank sum",
    "kolmogorov",
    "p-value",
    "statistically significant",
];

/// The row detail a pull request reads for this gate.
fn detail() -> &'static str {
    label_for(GATE)
        .unwrap_or_else(|| panic!("`{GATE}` has no row in GATE_LABELS, so nothing is published"))
        .1
}

fn overclaims(text: &str) -> Vec<&'static str> {
    let lower = text.to_lowercase();
    RANK_TEST_VOCABULARY
        .iter()
        .filter(|w| lower.contains(**w))
        .copied()
        .collect()
}

fn distribution(baseline: &[f64], canary: &[f64]) -> MetricDistribution {
    MetricDistribution {
        metric_name: "p99_latency_ms".to_string(),
        baseline_samples: baseline.to_vec(),
        canary_samples: canary.to_vec(),
    }
}

fn production(module: &str) -> String {
    module_source(module, Path::new(env!("CARGO_MANIFEST_DIR")))
}

// ---------------------------------------------------------------------------
// 1. The mechanism, so the naming rows below are not satisfied by a deletion
// ---------------------------------------------------------------------------

/// The entry point the certification pipeline calls reaches no comparison.
///
/// `NotMeasured` rather than `Passed` or `Failed`: with no canary deployed and
/// no metrics endpoint configured there are no samples, and both a pass and an
/// accusation would be claims about a distribution nobody read.
#[test]
fn the_pipelines_entry_point_reports_not_measured() {
    let report = AutomatedCanaryAnalysis::new().evaluate_without_metrics_source();

    assert_eq!(report.verdict, CanaryVerdict::NotMeasured);
    assert!(
        !report.passed,
        "an unqueried canary must not be asserted healthy"
    );
    match &report.status {
        GateStatus::NotMeasured { gate_id, reason } => {
            assert_eq!(gate_id, GATE);
            assert!(
                reason.contains("metrics endpoint"),
                "the reason must name the missing source: {reason}"
            );
        }
        other => panic!("the pipeline's entry point must withhold, got {other:?}"),
    }
}

/// The engine is a comparison of arithmetic means, demonstrated by an input on
/// which a rank test and a mean comparison disagree.
///
/// Four tied observations and one extreme outlier. Mann-Whitney ranks the
/// samples, so one outlier against four ties is not a significant shift; the
/// mean of the canary set is 20.8 against a baseline of 1.0, which is a 1980%
/// relative regression and fails the fixed 10% bound. A `Fail` here is only
/// producible by the mean comparison.
#[test]
fn the_engine_compares_means_not_ranks() {
    let engine = StatisticalCanaryEngine::new();
    let outlier = distribution(&[1.0, 1.0, 1.0, 1.0, 1.0], &[1.0, 1.0, 1.0, 1.0, 100.0]);

    match engine.evaluate_canary_distributions(&outlier) {
        CanaryVerdict::Fail {
            relative_regression_pct,
            ..
        } => assert!(
            relative_regression_pct > 1000.0,
            "the regression reported is the ratio of the two means, got \
             {relative_regression_pct}"
        ),
        other => panic!(
            "a single outlier against four ties moves the mean and not the ranks; \
             a rank test would not fail this, got {other:?}"
        ),
    }
}

/// The other direction, so the row above is not satisfied by an engine that
/// fails everything: two identical sample sets are a pass.
#[test]
fn an_unchanged_distribution_still_passes() {
    let engine = StatisticalCanaryEngine::new();
    let unchanged = distribution(&[10.0, 10.2, 9.9], &[10.0, 10.2, 9.9]);

    assert_eq!(
        engine.evaluate_canary_distributions(&unchanged),
        CanaryVerdict::Pass,
        "an engine that fails every input would satisfy the naming rows below \
         while blocking every pull request"
    );
}

/// No production code hands this gate a distribution.
///
/// This is what makes `the_pipelines_entry_point_reports_not_measured` a fact
/// about pull requests rather than about one function. `evaluate_canary` is
/// the door a real metrics source would come through; while nothing calls it,
/// the published row may not describe what it would compute.
#[test]
fn the_certification_pipeline_hands_the_gate_no_distribution() {
    let pipeline = production("src/webhook");

    assert!(
        pipeline.contains("automated_canary"),
        "the certification pipeline no longer names this gate at all"
    );
    assert!(
        !pipeline.contains("evaluate_canary("),
        "the pipeline now supplies a distribution; the published row must be \
         revisited against what the engine actually computes"
    );
    // Windowed, not whole-file: `canary_rollout` calls a constructor of the
    // same name, so a bare `contains` would be satisfied by a different gate.
    let through_the_abstainer = pipeline.match_indices("automated_canary").any(|(i, _)| {
        pipeline[i..]
            .chars()
            .take(160)
            .collect::<String>()
            .contains("evaluate_without_metrics_source")
    });
    assert!(
        through_the_abstainer,
        "the pipeline must reach this gate through the abstaining entry point"
    );
}

// ---------------------------------------------------------------------------
// 2. The label, pinned to the mechanism above
// ---------------------------------------------------------------------------

/// The published row may not name a hypothesis test.
///
/// The gate reaches no test on any pull request, and the engine behind it is a
/// mean comparison. A row naming Mann-Whitney buys trust from a mechanism that
/// is recorded, in two other files in this tree, as not existing.
#[test]
fn the_matrix_row_does_not_name_a_test_the_gate_never_runs() {
    let over = overclaims(detail());
    assert!(
        over.is_empty(),
        "`{GATE}` publishes `{}`, which names {over:?}; the gate abstains before \
         any comparison and its engine compares two means",
        detail()
    );
}

/// The row says the gate is not measured, rather than merely dropping the
/// claim. A withdrawn claim that is silent still reads as the old one: the gate
/// id and the name both say "canary analysis" and neither can be renamed
/// without breaking `unmeasured_gates` and the fidelity registry lookup.
#[test]
fn the_matrix_row_discloses_that_nothing_is_measured() {
    let d = detail().to_lowercase();
    assert!(
        d.contains("not measured"),
        "`{GATE}` withholds on every pull request, and the row must say so; got `{}`",
        detail()
    );
}

/// The rendered scorecard carries it, since the table is what a pull request
/// author reads and `label_for` is only its input.
#[test]
fn the_rendered_scorecard_carries_the_narrowed_row() {
    let table = MatrixRenderer::render(&PreMergeCertificationReport::unmeasured("fixture"));

    assert!(
        table.contains(detail()),
        "the rendered matrix must carry the row detail"
    );
    assert!(
        overclaims(&table).is_empty(),
        "no row in the rendered matrix may name a hypothesis test for this gate"
    );
}
