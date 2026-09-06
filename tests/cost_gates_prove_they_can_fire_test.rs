//! Two cost gates demonstrate both halves.
//!
//! Neither refuses a merge — both publish `Warning` — which is exactly why they
//! need proving. A gate whose worst outcome is advisory is one nobody notices
//! has stopped working, and "it never fires" and "it cannot fire" look identical
//! from outside.

use anvil::git_manager::{PrDiffContext, SubjectRoot, Uncloned};

fn ctx(diff: &str, files: Vec<&str>) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: diff.to_string(),
        changed_files: files.into_iter().map(str::to_string).collect(),
        repo_working_dir: SubjectRoot::asserted(
            std::path::PathBuf::from("."),
            Uncloned::TestFixture,
        ),
        is_incremental: false,
        previous_head_sha: None,
    }
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("anvil-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("scratch");
    d
}

// ---------------------------------------------------------------------------
// compile_profile_status — CompileTimeProfiler
// ---------------------------------------------------------------------------
//
// A `build.rs` with no `cargo:rerun-if-changed` re-executes on every compile,
// so it charges every later build for one change.

#[test]
fn compile_profile_fires_on_a_build_script_with_no_rerun_trigger() {
    let dir = scratch("cprofile-red");
    let diff = concat!(
        "diff --git a/build.rs b/build.rs\n",
        "--- a/build.rs\n",
        "+++ b/build.rs\n",
        "@@ -0,0 +1,1 @@\n",
        "+fn main() { generate_bindings(); }\n",
    );
    let report = anvil::compile_time_profiler::CompileTimeProfiler::new()
        .evaluate_compile_profile(&dir, &ctx(diff, vec!["build.rs"]))
        .expect("the profiler runs");
    assert!(
        !report.is_lean,
        "a build script with no rerun trigger runs on every compile, and the \
         gate did not see it"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compile_profile_spares_a_build_script_that_declares_its_trigger() {
    let dir = scratch("cprofile-green");
    let diff = concat!(
        "diff --git a/build.rs b/build.rs\n",
        "--- a/build.rs\n",
        "+++ b/build.rs\n",
        "@@ -0,0 +1,2 @@\n",
        "+fn main() {\n",
        "+    println!(\"cargo:rerun-if-changed=schema.proto\");\n",
        "+}\n",
    );
    let report = anvil::compile_time_profiler::CompileTimeProfiler::new()
        .evaluate_compile_profile(&dir, &ctx(diff, vec!["build.rs"]))
        .expect("the profiler runs");
    assert!(
        report.is_lean,
        "the script declares what it depends on, which is the remedy this gate \
         wants; flagging it would refuse the fix along with the defect: {}",
        report.summary
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// runner_economics_status — CiRunnerEconomics
// ---------------------------------------------------------------------------
//
// The rule has a subject only when the workflow triggers on `pull_request`: a
// macOS runner on a nightly release job is a deliberate cost, and on every
// pull request it is an accidental one. Both fixtures keep the trigger.

#[test]
fn runner_economics_fires_on_a_costly_runner_for_every_pull_request() {
    let dir = scratch("runner-red");
    let diff = concat!(
        "diff --git a/.github/workflows/ci.yml b/.github/workflows/ci.yml\n",
        "--- a/.github/workflows/ci.yml\n",
        "+++ b/.github/workflows/ci.yml\n",
        "@@ -0,0 +1,3 @@\n",
        "+on:\n",
        "+  pull_request:\n",
        "+    runs-on: macos-14-xlarge\n",
    );
    let report = anvil::ci_runner_economics::CiRunnerEconomicsOptimizer::new()
        .evaluate_runner_economics(&dir, &ctx(diff, vec![".github/workflows/ci.yml"]))
        .expect("the gate runs");
    assert!(
        !report.is_cost_optimal,
        "every pull request now pays for a macOS runner and the gate did not \
         see it"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn runner_economics_spares_a_standard_runner_on_the_same_trigger() {
    let dir = scratch("runner-green");
    let diff = concat!(
        "diff --git a/.github/workflows/ci.yml b/.github/workflows/ci.yml\n",
        "--- a/.github/workflows/ci.yml\n",
        "+++ b/.github/workflows/ci.yml\n",
        "@@ -0,0 +1,3 @@\n",
        "+on:\n",
        "+  pull_request:\n",
        "+    runs-on: ubuntu-24.04\n",
    );
    let report = anvil::ci_runner_economics::CiRunnerEconomicsOptimizer::new()
        .evaluate_runner_economics(&dir, &ctx(diff, vec![".github/workflows/ci.yml"]))
        .expect("the gate runs");
    assert!(
        report.is_cost_optimal,
        "the same trigger with a standard runner is the conformant case, so the \
         rule had a subject and spared it: {}",
        report.summary
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// bench_status — CriterionBenchRatchet
// ---------------------------------------------------------------------------
//
// The rule has a subject only in a hot-path file — one whose name carries
// `bench`, `hotpath`, `proto`, `serialize`, `hash` or `crypto`. Both fixtures
// keep that name, so the green half passes the rule rather than its
// precondition.

#[test]
fn bench_fires_on_a_clone_marked_hot_path() {
    let dir = scratch("bench-red");
    let diff = concat!(
        "diff --git a/src/serialize.rs b/src/serialize.rs\n",
        "--- a/src/serialize.rs\n",
        "+++ b/src/serialize.rs\n",
        "@@ -0,0 +1,1 @@\n",
        "+    let owned = buf.clone(); // hotpath\n",
    );
    let report = anvil::criterion_bench_ratchet::CriterionBenchRatchet::new()
        .evaluate_benchmarks(&dir, &ctx(diff, vec!["src/serialize.rs"]))
        .expect("the ratchet runs");
    assert!(
        !report.is_within_budget,
        "a clone the author marked as hot-path was added and the ratchet did \
         not see it"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bench_spares_a_borrow_in_the_same_hot_path_file() {
    let dir = scratch("bench-green");
    let diff = concat!(
        "diff --git a/src/serialize.rs b/src/serialize.rs\n",
        "--- a/src/serialize.rs\n",
        "+++ b/src/serialize.rs\n",
        "@@ -0,0 +1,1 @@\n",
        "+    let borrowed = &buf; // hotpath\n",
    );
    let report = anvil::criterion_bench_ratchet::CriterionBenchRatchet::new()
        .evaluate_benchmarks(&dir, &ctx(diff, vec!["src/serialize.rs"]))
        .expect("the ratchet runs");
    assert!(
        report.is_within_budget,
        "borrowing instead of cloning is the remedy; flagging it would refuse \
         the fix along with the defect: {}",
        report.summary
    );
    let _ = std::fs::remove_dir_all(&dir);
}
