//! The daemon must never treat its own source tree as a managed clone.
//!
//! Every Anvil write path — fixer, queue healer, and now change delivery —
//! mutates `repos_dir/<name>` and pushes it. Today `repos/` is gitignored and
//! each clone has its own `.git`, so the daemon's checkout and the clone of
//! `oyatie/anvil` differ by accident of layout. One `REPOS_DIR=..` in an env
//! file would make Anvil edit and push its running source from under itself.
//! The instruction "do not modify the running tree" cannot prevent that; a
//! boot invariant can.

use anvil::config::managed_clone_overlaps_daemon_tree;
use std::path::Path;

#[test]
fn a_clone_that_is_the_daemon_tree_is_refused() {
    let daemon = Path::new("/srv/anvil");
    let err = managed_clone_overlaps_daemon_tree(daemon, Some(daemon), daemon)
        .expect_err("the daemon's own tree must not be a managed clone");
    assert!(err.contains("daemon's own source tree"), "{err}");
}

#[test]
fn a_clone_that_contains_the_daemon_tree_is_refused() {
    // REPOS_DIR=.. with the daemon checked out as <parent>/anvil: the "clone"
    // named `anvil` resolves to the parent directory of the running tree.
    let clone = Path::new("/srv");
    let daemon = Path::new("/srv/anvil");
    let err = managed_clone_overlaps_daemon_tree(clone, None, daemon)
        .expect_err("a clone that contains the daemon tree must be refused");
    assert!(err.contains("runs inside managed clone"), "{err}");
}

#[test]
fn a_subdirectory_of_the_daemon_repository_is_refused() {
    // repos/anvil exists but is not its own repository: `git rev-parse
    // --show-toplevel` inside it answers with the daemon's toplevel. Writing
    // there writes into the running tree.
    let clone = Path::new("/srv/anvil/repos/anvil");
    let daemon = Path::new("/srv/anvil");
    let err = managed_clone_overlaps_daemon_tree(clone, Some(daemon), daemon)
        .expect_err("a clone inside the daemon's own git repository must be refused");
    assert!(err.contains("daemon's own git repository"), "{err}");
}

#[test]
fn a_separate_clone_under_repos_is_accepted() {
    // The real layout: repos/ under the daemon's cwd, gitignored, each clone
    // with its own toplevel.
    let clone = Path::new("/srv/anvil/repos/anvil");
    let daemon = Path::new("/srv/anvil");
    managed_clone_overlaps_daemon_tree(clone, Some(clone), daemon)
        .expect("a clone with its own git toplevel under repos/ is the intended layout");
}

#[test]
fn a_not_yet_cloned_repository_is_checked_by_path_only() {
    let clone = Path::new("/srv/anvil/repos/console");
    let daemon = Path::new("/srv/anvil");
    managed_clone_overlaps_daemon_tree(clone, None, daemon)
        .expect("an absent clone cannot overlap the daemon tree");
}

#[test]
fn the_real_layout_passes_the_boot_invariant() {
    // Runs the async invariant against this checkout's actual configuration:
    // cwd = the package root, REPOS_DIR unset -> <cwd>/repos.
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    rt.block_on(async {
        let config = anvil::config::Config::from_env();
        config
            .assert_managed_clones_are_not_this_tree()
            .await
            .expect("the checked-in layout must satisfy the invariant");
    });
}
