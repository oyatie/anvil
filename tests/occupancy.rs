//! Occupancy on this tree: hubs are N=1; `tests/*.rs` is N-wide.
//! The git worktree proof uses a throwaway repo so this hop does not
//! edit `src/main.rs` or `docs/doctrine.md`.

use anvil::change_delivery::core::shard::{
    SpawnKind, SpawnRefused, admit_spawn, anvil_hubs, is_open_test_crate, occupy_move,
    path_sets_disjoint,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use tempfile::tempdir;

fn set(paths: &[&str]) -> BTreeSet<String> {
    paths.iter().map(|s| s.to_string()).collect()
}

fn run(dir: &Path, args: &[&str]) {
    let st = Command::new(args[0])
        .args(&args[1..])
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(st.success(), "{args:?} in {}", dir.display());
}

#[test]
fn tests_star_rs_is_open_lib_rs_is_hub() {
    let hubs = anvil_hubs();
    assert!(hubs.contains("src/lib.rs"));
    assert!(hubs.contains("src/main.rs"));
    assert!(hubs.contains("docs/doctrine.md"));
    assert!(is_open_test_crate("tests/occupancy.rs"));
    assert!(is_open_test_crate("tests/nparallel_lane_0.rs"));
    assert!(!is_open_test_crate("tests/nested/foo.rs"));
    assert!(!is_open_test_crate("src/change_delivery/core/occupancy.rs"));
}

#[test]
fn disjoint_open_paths_are_parallel() {
    let hubs = anvil_hubs();
    let a = set(&["tests/nparallel_lane_0.rs"]);
    let b = set(&["tests/nparallel_lane_1.rs"]);
    assert!(path_sets_disjoint(&a, &b));
    assert_eq!(
        admit_spawn(&a, &hubs, std::slice::from_ref(&b), true).unwrap(),
        SpawnKind::Parallel
    );
    assert_eq!(
        admit_spawn(&b, &hubs, &[a], true).unwrap(),
        SpawnKind::Parallel
    );
}

#[test]
fn overlap_refuses_before_spawn() {
    let hubs = anvil_hubs();
    let err = admit_spawn(
        &set(&["tests/occupancy.rs"]),
        &hubs,
        &[set(&["tests/occupancy.rs"])],
        true,
    )
    .unwrap_err();
    assert_eq!(
        err,
        SpawnRefused::Overlap {
            path: "tests/occupancy.rs".into()
        }
    );
}

#[test]
fn hub_on_stale_base_is_refused() {
    let hubs = anvil_hubs();
    let err = admit_spawn(&set(&["src/main.rs"]), &hubs, &[], false).unwrap_err();
    assert_eq!(err, SpawnRefused::HubOnStaleBase);
}

#[test]
fn second_hub_hop_is_refused() {
    let hubs = anvil_hubs();
    let err = admit_spawn(
        &set(&["docs/doctrine.md"]),
        &hubs,
        &[set(&["src/lib.rs"])],
        true,
    )
    .unwrap_err();
    assert_eq!(err, SpawnRefused::HubAlreadyInFlight);
}

#[test]
fn git_mv_occupies_both_ends() {
    let hubs = anvil_hubs();
    let mv = occupy_move("tests/old.rs", "tests/new.rs");
    let err = admit_spawn(&set(&["tests/old.rs"]), &hubs, &[mv], true).unwrap_err();
    assert!(matches!(err, SpawnRefused::Overlap { .. }));
}

#[test]
fn n_worktrees_on_open_test_paths_merge() {
    let n = 8;
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    run(root, &["git", "init", "-q"]);
    run(root, &["git", "config", "user.email", "n@example.com"]);
    run(root, &["git", "config", "user.name", "n"]);
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(root.join("README"), "seed\n").unwrap();
    run(root, &["git", "add", "README"]);
    run(root, &["git", "commit", "-qm", "seed"]);
    let seed = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .unwrap();
    let seed = String::from_utf8(seed.stdout).unwrap();
    let seed = seed.trim().to_string();

    // Lane setup happens here, serially, before any thread starts.
    //
    // It used to run inside each thread. Two consequences, both bad. `git
    // worktree add` mutates the shared `.git/worktrees` directory and is not
    // safe to run concurrently against one repository -- eight simultaneous
    // adds can race and fail with "failed to read .git/worktrees/wtN/commondir".
    // And because the add sat before `barrier.wait()`, a thread that panicked
    // there never reached the barrier, so the other seven blocked forever: the
    // test did not fail, it HUNG. A hanging test is worse than a failing one,
    // because it is indistinguishable from a slow one and CI reports nothing.
    //
    // Creating a lane is not the property under test. Holding N lanes live at
    // once is, and that is still measured by `peak` across the concurrent
    // section below.
    for i in 0..n {
        let wt = root.join(format!("wt{i}"));
        run(
            root,
            &[
                "git",
                "worktree",
                "add",
                "-q",
                "-b",
                &format!("lane-{i}"),
                wt.to_str().unwrap(),
            ],
        );
    }

    let barrier = Arc::new(Barrier::new(n));
    let peak = Arc::new(AtomicUsize::new(0));
    let live = Arc::new(AtomicUsize::new(0));
    let mut joins = Vec::new();
    for i in 0..n {
        let root = root.to_path_buf();
        let barrier = Arc::clone(&barrier);
        let peak = Arc::clone(&peak);
        let live = Arc::clone(&live);
        joins.push(std::thread::spawn(move || {
            let wt = root.join(format!("wt{i}"));
            let now = live.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now, Ordering::SeqCst);
            barrier.wait();
            fs::create_dir_all(wt.join("tests")).unwrap();
            let rel = format!("tests/nparallel_lane_{i}.rs");
            fs::write(
                wt.join(&rel),
                format!("#[test] fn lane_{i}() {{ assert_eq!({i}, {i}); }}\n"),
            )
            .unwrap();
            run(&wt, &["git", "add", &rel]);
            run(&wt, &["git", "commit", "-qm", &format!("lane {i}")]);
            live.fetch_sub(1, Ordering::SeqCst);
            i
        }));
    }
    for j in joins {
        j.join().unwrap();
    }
    assert_eq!(peak.load(Ordering::SeqCst), n);

    let after = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(after.stdout).unwrap().trim(),
        seed.as_str(),
        "seed HEAD must not move until merge"
    );

    for i in 0..n {
        let st = Command::new("git")
            .args(["merge", "-q", "--no-edit", &format!("lane-{i}")])
            .current_dir(root)
            .status()
            .unwrap();
        assert!(st.success(), "lane-{i} conflicted");
        assert!(root.join(format!("tests/nparallel_lane_{i}.rs")).is_file());
    }
}
