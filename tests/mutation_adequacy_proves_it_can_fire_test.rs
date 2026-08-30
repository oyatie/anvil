//! The mutation adequacy gate demonstrates both halves.
//!
//! The subprocess that generates mutants is not the gate. The gate is the
//! accounting that turns cargo-mutants' outcome lists into a verdict, and that
//! accounting is what decides whether a pull request certifies. Feeding it the
//! tool's own output shape exercises it without a ten-minute mutation run, and
//! without making the proof depend on cargo-mutants being installed.

use anvil::chaos_mutation_guard::{ChaosMutationGuard, MutantsOutcome};
use anvil::git_manager::{PrDiffContext, SubjectRoot, Uncloned};
use anvil::pre_merge_guard::report::GateStatus;

fn rust_change() -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: String::new(),
        changed_files: vec!["src/lib.rs".to_string()],
        repo_working_dir: SubjectRoot::asserted(
            std::path::PathBuf::from("."),
            Uncloned::TestFixture,
        ),
        is_incremental: false,
        previous_head_sha: None,
    }
}

#[test]
fn mutation_fires_on_a_mutant_the_suite_failed_to_kill() {
    let outcome = MutantsOutcome::Reported {
        caught: "src/lib.rs:10:5: replace add -> i32 with 0\n".to_string(),
        missed: "src/lib.rs:22:9: replace is_ready -> bool with true\n".to_string(),
        timed_out: String::new(),
    };
    let report = ChaosMutationGuard::new().report_from_outcome(outcome, &rust_change());

    assert!(
        matches!(report.gate_status(), GateStatus::Failed(_)),
        "a mutant survived: the program ran with changed behaviour and no test \
         noticed. Certifying that is the failure this gate exists to prevent. \
         Got {:?}",
        report.gate_status()
    );
    assert_eq!(
        report.surviving_findings.len(),
        1,
        "the survivor has to reach the reviewer as a location, not just a count"
    );
}

#[test]
fn mutation_spares_a_diff_whose_mutants_the_suite_all_killed() {
    let outcome = MutantsOutcome::Reported {
        caught: "src/lib.rs:10:5: replace add -> i32 with 0\n\
                 src/lib.rs:22:9: replace is_ready -> bool with true\n"
            .to_string(),
        missed: String::new(),
        timed_out: String::new(),
    };
    let report = ChaosMutationGuard::new().report_from_outcome(outcome, &rust_change());

    assert_eq!(
        report.gate_status(),
        GateStatus::Passed,
        "every mutant died. A gate that refuses this refuses every change: {}",
        report.summary
    );
}

#[test]
fn mutation_withholds_rather_than_passing_when_no_run_happened() {
    let outcome = MutantsOutcome::Unavailable("cargo-mutants is not installed".to_string());
    let report = ChaosMutationGuard::new().report_from_outcome(outcome, &rust_change());

    assert!(
        matches!(report.gate_status(), GateStatus::NotMeasured { .. }),
        "absent evidence is not a pass and not an accusation (I1). Got {:?}",
        report.gate_status()
    );
}
