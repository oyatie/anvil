use anvil::git_manager::PrDiffContext;
use anvil::stack_whitelist_guard::StackWhitelistGuard;
use std::path::Path;

#[test]
fn test_stack_whitelist_guard_catches_unapproved_mongodb_and_actix() {
    let guard = StackWhitelistGuard::new();

    let diff_ctx = PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 1200,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        // A real unified diff, because the rule now decides per file: a
        // banned crate named in a test fixture is not an adoption of it, and
        // the rule cannot know which file a line belongs to without the
        // headers. The old fixture had none, so every hit was attributed to
        // the whole change.
        diff_content: concat!(
            "diff --git a/crates/api/src/lib.rs b/crates/api/src/lib.rs\n",
            "--- a/crates/api/src/lib.rs\n",
            "+++ b/crates/api/src/lib.rs\n",
            "@@ -1,1 +1,3 @@\n",
            " fn main() {}\n",
            "+use mongodb::Client;\n",
            "+use actix_web::App;\n",
        )
        .to_string(),
        changed_files: vec!["crates/api/src/lib.rs".to_string()],
        repo_working_dir: anvil::git_manager::SubjectRoot::asserted(
            std::path::PathBuf::from("/tmp"),
            anvil::git_manager::Uncloned::TestFixture,
        ),
        is_incremental: false,
        previous_head_sha: None,
    };

    let report = guard
        .evaluate_stack_whitelist(Path::new("/tmp"), &diff_ctx, false)
        .unwrap();
    assert!(!report.is_compliant);
    assert_eq!(report.violations.len(), 2);
    // The finding now names the file that carries it.
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.item == "crates/api/src/lib.rs:mongodb::")
    );
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.item == "crates/api/src/lib.rs:actix_web")
    );
}

#[test]
fn test_apex_adr_immutability_lock_allows_human_blocks_agent() {
    let guard = StackWhitelistGuard::new();

    let diff_ctx = PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 1201,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: "+ Modified apex doctrine".to_string(),
        changed_files: vec!["docs/decisions/ADR-0709-general-live-apex.md".to_string()],
        repo_working_dir: anvil::git_manager::SubjectRoot::asserted(
            std::path::PathBuf::from("/tmp"),
            anvil::git_manager::Uncloned::TestFixture,
        ),
        is_incremental: false,
        previous_head_sha: None,
    };

    // Agent PR fails
    let agent_report = guard
        .evaluate_stack_whitelist(Path::new("/tmp"), &diff_ctx, false)
        .unwrap();
    assert!(!agent_report.is_compliant);
    assert!(
        agent_report
            .violations
            .iter()
            .any(|v| v.category == "APEX_ADR_IMMUTABILITY_BREACH")
    );

    // Human PR passes
    let human_report = guard
        .evaluate_stack_whitelist(Path::new("/tmp"), &diff_ctx, true)
        .unwrap();
    assert!(human_report.is_compliant);
}

#[test]
fn test_asymmetric_dependency_ratchet_allows_removal_blocks_addition() {
    let guard = StackWhitelistGuard::new();

    // 1. Agent adding a dependency fails
    let add_diff = PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 1202,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        // A real `git diff --unified=3` fixture. The previous one was a bare
        // two-line fragment with no `diff --git` or `+++ b/` header, which no
        // git invocation produces; it passed only because the guard took the
        // path from `changed_files` and the text from the raw string.
        diff_content: "diff --git a/crates/api/Cargo.toml b/crates/api/Cargo.toml\n\
             --- a/crates/api/Cargo.toml\n+++ b/crates/api/Cargo.toml\n\
             @@ -5,0 +6,1 @@\n [dependencies]\n+serde_yaml = \"0.9\"\n"
            .to_string(),
        changed_files: vec!["crates/api/Cargo.toml".to_string()],
        repo_working_dir: anvil::git_manager::SubjectRoot::asserted(
            std::path::PathBuf::from("/tmp"),
            anvil::git_manager::Uncloned::TestFixture,
        ),
        is_incremental: false,
        previous_head_sha: None,
    };
    let add_report = guard
        .evaluate_stack_whitelist(Path::new("/tmp"), &add_diff, false)
        .unwrap();
    assert!(!add_report.is_compliant);
    assert!(
        add_report
            .violations
            .iter()
            .any(|v| v.category == "UNAUTHORIZED_DEPENDENCY_EXPANSION")
    );

    // 2. Agent removing an unused/misplaced dependency passes cleanly!
    let remove_diff = PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 1203,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: "diff --git a/crates/api/Cargo.toml b/crates/api/Cargo.toml\n\
             --- a/crates/api/Cargo.toml\n+++ b/crates/api/Cargo.toml\n\
             @@ -5,1 +5,0 @@\n [dependencies]\n-legacy_crate = \"0.1\"\n"
            .to_string(),
        changed_files: vec!["crates/api/Cargo.toml".to_string()],
        repo_working_dir: anvil::git_manager::SubjectRoot::asserted(
            std::path::PathBuf::from("/tmp"),
            anvil::git_manager::Uncloned::TestFixture,
        ),
        is_incremental: false,
        previous_head_sha: None,
    };
    let remove_report = guard
        .evaluate_stack_whitelist(Path::new("/tmp"), &remove_diff, false)
        .unwrap();
    assert!(remove_report.is_compliant);
}

/// The gate refused the pull request that wired it.
///
/// Measured, not supposed: the change that adds `stack_whitelist_guard` to the
/// certification corpus adds exactly one line matching a banned token --
/// `"+use mongodb::Client;\n"` inside `tests/stack_whitelist_guard_test.rs`,
/// the fixture directly above that proves the gate catches MongoDB. Rule 2
/// scanned every `+` line of the whole diff for a bare substring, so it read
/// its own fixture as an adoption of MongoDB and blocked the merge.
///
/// The same shape refuses any change to `BANNED_UNAPPROVED_STACK`, because
/// every entry in that table contains the token it bans. Both are checked
/// here, and so is the case that must still fail -- otherwise the fix is a
/// hole rather than a scope.
#[test]
fn the_gate_does_not_refuse_the_fixtures_and_the_table_that_define_it() {
    let guard = StackWhitelistGuard::new();

    let ctx = |diff: &str, files: Vec<&str>| PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: diff.to_string(),
        changed_files: files.into_iter().map(str::to_string).collect(),
        repo_working_dir: anvil::git_manager::SubjectRoot::asserted(
            std::path::PathBuf::from("/tmp"),
            anvil::git_manager::Uncloned::TestFixture,
        ),
        is_incremental: false,
        previous_head_sha: None,
    };

    // 1. The gate's own fixture, copied from the line this branch adds.
    let own_fixture = concat!(
        "diff --git a/tests/stack_whitelist_guard_test.rs b/tests/stack_whitelist_guard_test.rs\n",
        "--- a/tests/stack_whitelist_guard_test.rs\n",
        "+++ b/tests/stack_whitelist_guard_test.rs\n",
        "@@ -1,1 +1,2 @@\n",
        " fn t() {}\n",
        "+                +use mongodb::Client;\\n\";\n",
    );
    let report = guard
        .evaluate_stack_whitelist(Path::new("/tmp"), &ctx(own_fixture, vec![]), false)
        .unwrap();
    assert!(
        report.is_compliant,
        "the gate refuses the fixture that proves it works: {}",
        report.summary
    );

    // 2. The table itself. Every row names the crate it bans.
    let the_table = concat!(
        "diff --git a/src/stack_whitelist_guard/mod.rs b/src/stack_whitelist_guard/mod.rs\n",
        "--- a/src/stack_whitelist_guard/mod.rs\n",
        "+++ b/src/stack_whitelist_guard/mod.rs\n",
        "@@ -1,1 +1,2 @@\n",
        " const X: u8 = 0;\n",
        "+        (\"redis::\", \"Redis (Mandate: in-memory LRU/CAS per ADR-0703)\"),\n",
    );
    let report = guard
        .evaluate_stack_whitelist(Path::new("/tmp"), &ctx(the_table, vec![]), false)
        .unwrap();
    assert!(
        report.is_compliant,
        "the gate refuses every change to the list it enforces: {}",
        report.summary
    );

    // 3. And the case it exists for still fails, in a file that ships, as
    //    code rather than as a quoted name. A scope that also admits this is
    //    not a scope.
    let a_real_adoption = concat!(
        "diff --git a/src/store.rs b/src/store.rs\n",
        "--- a/src/store.rs\n",
        "+++ b/src/store.rs\n",
        "@@ -1,1 +1,2 @@\n",
        " fn t() {}\n",
        "+use redis::Client;\n",
    );
    let report = guard
        .evaluate_stack_whitelist(Path::new("/tmp"), &ctx(a_real_adoption, vec![]), false)
        .unwrap();
    assert!(
        !report.is_compliant,
        "an unapproved crate used in shipped code is what this gate is for"
    );
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.item == "src/store.rs:redis::"),
        "the finding does not name the file that carries it: {:?}",
        report.violations
    );
}

/// The gate, run against this repository's own change.
///
/// The hand-copied fixtures above did not catch either time this gate refused
/// the pull request that wires it. The first fix scanned every `+` line for a
/// bare substring and read `tests/stack_whitelist_guard_test.rs`'s own MongoDB
/// fixture as an adoption. The second stripped double-quoted spans and then
/// read its own explanatory comment -- ``a real `use redis::…` `` -- as an
/// adoption of Redis, because backticks are not quotes.
///
/// Both were invisible to fixtures because a fixture is a line someone chose.
/// This runs the real guard over the real diff, which is what "measured" has
/// to mean here: the gate's subject is the change it is part of.
#[test]
fn the_gate_does_not_refuse_this_repositorys_own_change() {
    let base = match std::process::Command::new("git")
        .args(["merge-base", "origin/dev", "HEAD"])
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => {
            eprintln!("skipped: no merge-base against origin/dev");
            return;
        }
    };

    let diff = match std::process::Command::new("git")
        .args(["diff", &format!("{base}..HEAD")])
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => {
            eprintln!("skipped: could not read the diff against the merge base");
            return;
        }
    };
    if diff.trim().is_empty() {
        eprintln!("skipped: nothing changed against the merge base");
        return;
    }

    let diff_ctx = PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 0,
        base_branch: "dev".to_string(),
        base_sha: base.clone(),
        head_sha: "HEAD".to_string(),
        diff_content: diff,
        changed_files: Vec::new(),
        repo_working_dir: anvil::git_manager::SubjectRoot::asserted(
            std::path::PathBuf::from("."),
            anvil::git_manager::Uncloned::TestFixture,
        ),
        is_incremental: false,
        previous_head_sha: None,
    };

    let report = StackWhitelistGuard::new()
        .evaluate_stack_whitelist(Path::new("."), &diff_ctx, true)
        .expect("the guard runs");

    let stack: Vec<&anvil::stack_whitelist_guard::StackWhitelistViolation> = report
        .violations
        .iter()
        .filter(|v| v.category == "UNAPPROVED_STACK_TECHNOLOGY")
        .collect();

    assert!(
        stack.is_empty(),
        "the gate accuses this repository's own change of adopting an \
         unapproved technology:\n{}\n\
         If one of these is a real adoption, an ADR is owed. If it is prose or \
         a fixture, the scan is reading something that is not code.",
        stack
            .iter()
            .map(|v| format!("  {} -- {}", v.item, v.description))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
