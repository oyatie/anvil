use anvil::task_orchestrator::{
    fan_out_after_implement, interview, run_layer_parallel, transitive_deps, DeliveryBoard,
    DeliveryRole, HandoffAgent, IntakeVerdict, InterviewDraft, ReadyHop, ScopedTaskDefinition,
    TaskExecutionReport,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

fn board_with_n_implement_ready(n: usize) -> DeliveryBoard {
    let mut board = DeliveryBoard::new();
    let mut done = transitive_deps(DeliveryRole::Implement);
    done.insert(DeliveryRole::Experiment);
    for i in 0..n {
        let path = format!("storage/core/item{i}.rs");
        board
            .admit_slice(
                format!("s{i}"),
                vec![path],
                HandoffAgent::Program,
                "example/monorepo",
                done.clone(),
            )
            .unwrap();
    }
    board
}

fn hop_task(hop: &ReadyHop) -> ScopedTaskDefinition {
    ScopedTaskDefinition {
        task_id: hop.slice_id.clone(),
        source_doc_path: format!("docs/{}.md", hop.slice_id),
        title: hop.slice_id.clone(),
        domain: "test".into(),
        priority: 1,
        target_files: hop.paths.clone(),
        dependencies: vec![],
        required_invariants: vec![],
        is_verified_ssot: true,
    }
}

fn git(dir: &Path, args: &[&str]) -> Output {
    let out = std::process::Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_TEMPLATE_DIR", "")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_MERGE_AUTOEDIT", "no")
        .env("GIT_AUTHOR_NAME", "anvil-test")
        .env("GIT_AUTHOR_EMAIL", "anvil-test@example.invalid")
        .env("GIT_COMMITTER_NAME", "anvil-test")
        .env("GIT_COMMITTER_EMAIL", "anvil-test@example.invalid")
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to exec git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} in {} failed: {}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn git_out(dir: &Path, args: &[&str]) -> String {
    String::from_utf8_lossy(&git(dir, args).stdout)
        .trim()
        .to_string()
}

fn worktree_count(repo: &Path) -> usize {
    git_out(repo, &["worktree", "list", "--porcelain"])
        .lines()
        .filter(|l| l.starts_with("worktree "))
        .count()
}

fn seed_repo(root: &Path) -> PathBuf {
    let repo = root.join("repo");
    fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "--quiet", "-b", "seed"]);
    git(
        &repo,
        &["config", "user.email", "anvil-test@example.invalid"],
    );
    git(&repo, &["config", "user.name", "anvil-test"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    fs::write(repo.join("README"), "seed\n").unwrap();
    git(&repo, &["add", "README"]);
    git(&repo, &["commit", "--quiet", "-m", "seed"]);
    repo
}

fn ok_report(task_id: String, branch_name: String) -> TaskExecutionReport {
    TaskExecutionReport {
        task_id,
        repo: "example/monorepo".into(),
        branch_name,
        pr_number: None,
        attempts: 1,
        tokens_consumed: 0,
        status: "ok".into(),
        summary: String::new(),
    }
}

#[test]
fn ten_disjoint_implement_hops_are_ten_not_one() {
    let board = board_with_n_implement_ready(10);
    let ready = board.lane_ready(DeliveryRole::Implement);
    assert_eq!(ready.len(), 10, "folding 10 slices onto 1 hop");
    DeliveryBoard::assert_lane_not_folded(DeliveryRole::Implement, 10, 1).unwrap_err();
    DeliveryBoard::assert_lane_not_folded(DeliveryRole::Implement, 10, 10).unwrap();
}

#[test]
fn overlapping_paths_admit_one_schedulable_implement() {
    let mut board = DeliveryBoard::new();
    let mut done = transitive_deps(DeliveryRole::Implement);
    done.insert(DeliveryRole::Experiment);
    for id in ["a", "b", "c"] {
        board
            .admit_slice(
                id,
                vec!["storage/core/same.rs".into()],
                HandoffAgent::Program,
                "example/monorepo",
                done.clone(),
            )
            .unwrap();
    }
    let ready = board.lane_ready(DeliveryRole::Implement);
    assert_eq!(ready.len(), 1);
}

#[test]
fn implement_complete_fans_out_and_frees_the_role() {
    let mut board = board_with_n_implement_ready(2);
    let hop_a = board
        .claim("s0", DeliveryRole::Implement, "agent-a")
        .unwrap();
    assert_eq!(board.lane_ready(DeliveryRole::Implement).len(), 1);
    board.complete(hop_a.hop_id).unwrap();
    let s0_ready: BTreeSet<_> = board
        .ready_hops()
        .into_iter()
        .filter(|h| h.slice_id == "s0")
        .map(|h| h.role)
        .collect();
    for role in fan_out_after_implement() {
        assert!(s0_ready.contains(role), "missing fan-out {role:?}");
    }
    assert!(s0_ready.len() > 1, "must not be a single sequential hop");
    assert_eq!(board.lane_ready(DeliveryRole::Implement).len(), 1);
}

#[test]
fn fresh_agent_cannot_be_reused_on_the_next_slice() {
    let mut board = board_with_n_implement_ready(2);
    board
        .claim("s0", DeliveryRole::Implement, "agent-1")
        .unwrap();
    let err = board
        .claim("s1", DeliveryRole::Implement, "agent-1")
        .unwrap_err()
        .to_string();
    assert!(err.contains("reused"));
}

#[test]
fn intake_rejects_tbd_and_dump_roots_and_routes_product_xor_program() {
    let tmp = tempdir().unwrap();
    fs::write(tmp.path().join("README.md"), "doc").unwrap();
    let vague = InterviewDraft {
        idea: "TBD".into(),
        research_citations: vec![],
        constraints: vec![],
        acceptance: vec![],
        target_paths: vec![],
    };
    match interview(&vague, tmp.path()) {
        IntakeVerdict::NeedClarification { questions } => assert!(!questions.is_empty()),
        other => panic!("{other:?}"),
    }

    fs::create_dir_all(tmp.path().join("docs")).unwrap();
    fs::write(tmp.path().join("docs/adr.md"), "x").unwrap();
    let dump = InterviewDraft {
        idea: "put a plan folder back".into(),
        research_citations: vec!["docs/adr.md".into()],
        constraints: vec!["none".into()],
        acceptance: vec!["plan/ exists".into()],
        target_paths: vec!["plan/foo.md".into()],
    };
    match interview(&dump, tmp.path()) {
        IntakeVerdict::Rejected { reason } => {
            assert!(reason.contains("dump") || reason.contains("plan"))
        }
        other => panic!("{other:?}"),
    }

    let product = InterviewDraft {
        idea: "Foundry blob picker".into(),
        research_citations: vec!["docs/adr.md".into()],
        constraints: vec!["tenant isolation".into()],
        acceptance: vec!["picker writes through the blob port".into()],
        target_paths: vec!["app/foundry/adapters/blob.rs".into()],
    };
    match interview(&product, tmp.path()) {
        IntakeVerdict::Packaged(p) => assert_eq!(p.handoff, HandoffAgent::Product),
        other => panic!("{other:?}"),
    }

    let mixed = InterviewDraft {
        idea: "wire foundry to storage core".into(),
        research_citations: vec!["docs/adr.md".into()],
        constraints: vec!["one".into()],
        acceptance: vec!["works".into()],
        target_paths: vec!["app/foundry/core/x.rs".into(), "storage/core/y.rs".into()],
    };
    match interview(&mixed, tmp.path()) {
        IntakeVerdict::NeedClarification { questions } => {
            assert!(questions.iter().any(|q| q.contains("mixed")));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn consumer_cannot_write_owner_paths_draft_stays_on_consumer() {
    DeliveryBoard::reject_write_to_foreign_paths(
        &["app/foundry/adapters/draft/blob.rs".into()],
        "storage/ports/blob.rs",
    )
    .unwrap_err();
    DeliveryBoard::file_draft_port(
        &["app/foundry/adapters/draft/blob.rs".into()],
        "app/foundry/adapters/draft/blob.rs",
    )
    .unwrap();
    DeliveryBoard::file_draft_port(&["app/foundry/core/x.rs".into()], "storage/ports/blob.rs")
        .unwrap_err();
}

/// N isolated git worktrees: one hop, one worktree, unique path, concurrent
/// commit. Merge onto the seed branch has zero conflicts. Peak in-flight
/// worktrees is N. A shared-tree checkout/switch is not this proof.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn n_isolated_worktrees_commit_disjoint_paths_and_merge_without_conflict() {
    let tmp = tempdir().unwrap();
    let repo = Arc::new(seed_repo(tmp.path()));
    let lanes = Arc::new(tmp.path().join("lanes"));
    fs::create_dir_all(lanes.as_path()).unwrap();

    let n = 8;
    let board = board_with_n_implement_ready(n);
    let ready = board.lane_ready(DeliveryRole::Implement);
    assert_eq!(ready.len(), n, "folding {n} slices onto 1 hop");
    DeliveryBoard::assert_lane_not_folded(DeliveryRole::Implement, n, n).unwrap();

    let tasks: Vec<_> = ready.iter().map(hop_task).collect();
    anvil::task_orchestrator::assert_layer_paths_disjoint(&tasks).unwrap();

    let seed_head = git_out(repo.as_path(), &["rev-parse", "HEAD"]);
    assert_eq!(worktree_count(repo.as_path()), 1, "fixture is seed only");
    assert!(repo.join(".git").is_dir());

    let live = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let peak_worktrees = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(tokio::sync::Barrier::new(n));
    let listed = Arc::new(tokio::sync::Barrier::new(n));
    let add_lock = Arc::new(tokio::sync::Mutex::new(()));

    let reports = tokio::time::timeout(
        Duration::from_secs(30),
        run_layer_parallel(tasks, {
            let live = Arc::clone(&live);
            let peak = Arc::clone(&peak);
            let peak_worktrees = Arc::clone(&peak_worktrees);
            let barrier = Arc::clone(&barrier);
            let listed = Arc::clone(&listed);
            let repo = Arc::clone(&repo);
            let lanes = Arc::clone(&lanes);
            let add_lock = Arc::clone(&add_lock);
            move |t| {
                let live = Arc::clone(&live);
                let peak = Arc::clone(&peak);
                let peak_worktrees = Arc::clone(&peak_worktrees);
                let barrier = Arc::clone(&barrier);
                let listed = Arc::clone(&listed);
                let repo = Arc::clone(&repo);
                let lanes = Arc::clone(&lanes);
                let add_lock = Arc::clone(&add_lock);
                async move {
                    let rel = t.target_files[0].clone();
                    let branch = format!("lane-{}", t.task_id);
                    let wt = lanes.join(&t.task_id);

                    {
                        let _gate = add_lock.lock().await;
                        let repo = PathBuf::from(repo.as_path());
                        let wt = wt.clone();
                        let branch = branch.clone();
                        tokio::task::spawn_blocking(move || {
                            git(
                                &repo,
                                &[
                                    "worktree",
                                    "add",
                                    "--quiet",
                                    "-b",
                                    &branch,
                                    wt.to_str().expect("utf8 worktree path"),
                                ],
                            );
                        })
                        .await
                        .unwrap();
                    }

                    assert!(wt.is_dir(), "lane worktree missing at {}", wt.display());
                    assert!(
                        wt.join(".git").is_file(),
                        "lane {} must be a linked worktree, not the seed checkout",
                        t.task_id
                    );

                    let now = live.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    barrier.wait().await;
                    if t.task_id == "s0" {
                        peak_worktrees.store(
                            worktree_count(repo.as_path()).saturating_sub(1),
                            Ordering::SeqCst,
                        );
                    }
                    listed.wait().await;

                    let body = format!("pub fn {}() {{}}\n", t.task_id);
                    let wt_commit = wt.clone();
                    let rel_commit = rel.clone();
                    let msg = t.task_id.clone();
                    tokio::task::spawn_blocking(move || {
                        if let Some(parent) = wt_commit.join(&rel_commit).parent() {
                            fs::create_dir_all(parent).unwrap();
                        }
                        fs::write(wt_commit.join(&rel_commit), body).unwrap();
                        git(&wt_commit, &["add", &rel_commit]);
                        git(&wt_commit, &["commit", "--quiet", "-m", &msg]);
                    })
                    .await
                    .unwrap();

                    live.fetch_sub(1, Ordering::SeqCst);
                    Ok(ok_report(t.task_id, branch))
                }
            }
        }),
    )
    .await
    .expect("a folded worker deadlocks a barrier of N hops")
    .expect("disjoint layer");

    assert_eq!(reports.len(), n);
    assert_eq!(
        peak.load(Ordering::SeqCst),
        n,
        "peak in-flight hops was {}, not {n}",
        peak.load(Ordering::SeqCst)
    );
    assert_eq!(
        peak_worktrees.load(Ordering::SeqCst),
        n,
        "peak in-flight worktrees was {}, not {n}",
        peak_worktrees.load(Ordering::SeqCst)
    );
    assert_eq!(
        worktree_count(repo.as_path()),
        n + 1,
        "lane worktrees were torn down before merge"
    );
    assert_eq!(
        git_out(repo.as_path(), &["rev-parse", "HEAD"]),
        seed_head,
        "seed HEAD moved; checkout/switch in a shared tree is not N-parallel"
    );
    assert!(
        git_out(repo.as_path(), &["status", "--porcelain"]).is_empty(),
        "seed tree was dirtied during the parallel hops"
    );
    for i in 0..n {
        assert!(
            !repo.join(format!("storage/core/item{i}.rs")).exists(),
            "item{i}.rs landed on the seed tree before merge"
        );
        let wt = lanes.join(format!("s{i}"));
        let body = fs::read_to_string(wt.join(format!("storage/core/item{i}.rs"))).unwrap();
        assert!(body.contains(&format!("s{i}")));
    }

    for i in 0..n {
        git(
            repo.as_path(),
            &[
                "merge",
                "--quiet",
                "--no-edit",
                "--no-ff",
                &format!("lane-s{i}"),
            ],
        );
    }
    for i in 0..n {
        let body = fs::read_to_string(repo.join(format!("storage/core/item{i}.rs"))).unwrap();
        assert!(body.contains(&format!("s{i}")));
    }
}

/// Overlap is a launcher bug: the layer must refuse before any hop closure
/// runs, so no worktree is added.
#[tokio::test]
async fn overlapping_paths_do_not_spawn_worktrees() {
    let tmp = tempdir().unwrap();
    let repo = Arc::new(seed_repo(tmp.path()));
    let spawned = Arc::new(AtomicUsize::new(0));
    let tasks = vec![
        hop_task(&ReadyHop {
            slice_id: "a".into(),
            role: DeliveryRole::Implement,
            paths: vec!["storage/core/same.rs".into()],
            handoff: HandoffAgent::Program,
        }),
        hop_task(&ReadyHop {
            slice_id: "b".into(),
            role: DeliveryRole::Implement,
            paths: vec!["storage/core/same.rs".into()],
            handoff: HandoffAgent::Program,
        }),
    ];

    let err = run_layer_parallel(tasks, {
        let spawned = Arc::clone(&spawned);
        let repo = Arc::clone(&repo);
        move |t| {
            spawned.fetch_add(1, Ordering::SeqCst);
            let repo = Arc::clone(&repo);
            let id = t.task_id.clone();
            async move {
                let wt = repo.parent().unwrap().join("lanes").join(&id);
                git(
                    repo.as_path(),
                    &[
                        "worktree",
                        "add",
                        "--quiet",
                        "-b",
                        &format!("lane-{id}"),
                        wt.to_str().unwrap(),
                    ],
                );
                Ok(ok_report(id, String::new()))
            }
        }
    })
    .await
    .unwrap_err()
    .to_string();

    assert!(err.contains("path occupancy overlap"));
    assert_eq!(spawned.load(Ordering::SeqCst), 0, "overlap spawned a hop");
    assert_eq!(
        worktree_count(repo.as_path()),
        1,
        "overlap added a worktree"
    );
}

/// Sequential merge commute of already-disjoint commits. The commits are
/// produced in isolated worktrees. Checkout-switching a shared tree is not
/// parallelism and is not this test.
#[test]
fn git_disjoint_commits_commute() {
    let tmp = tempdir().unwrap();
    let repo = seed_repo(tmp.path());
    let seed = git_out(&repo, &["rev-parse", "HEAD"]);

    for (name, file, body) in [
        ("a", "storage/core/a.rs", "pub fn a() {}\n"),
        ("b", "storage/core/b.rs", "pub fn b() {}\n"),
    ] {
        let wt = tmp.path().join(name);
        git(
            &repo,
            &[
                "worktree",
                "add",
                "--quiet",
                "-b",
                name,
                wt.to_str().unwrap(),
            ],
        );
        fs::create_dir_all(wt.join("storage/core")).unwrap();
        fs::write(wt.join(file), body).unwrap();
        git(&wt, &["add", file]);
        git(&wt, &["commit", "--quiet", "-m", name]);
    }

    let ab = tmp.path().join("merge-ab");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "merge-ab",
            ab.to_str().unwrap(),
            &seed,
        ],
    );
    git(&ab, &["merge", "--quiet", "--no-edit", "a"]);
    git(&ab, &["merge", "--quiet", "--no-edit", "b"]);
    let tree_ab = git_out(&ab, &["rev-parse", "HEAD^{tree}"]);

    let ba = tmp.path().join("merge-ba");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "merge-ba",
            ba.to_str().unwrap(),
            &seed,
        ],
    );
    git(&ba, &["merge", "--quiet", "--no-edit", "b"]);
    git(&ba, &["merge", "--quiet", "--no-edit", "a"]);
    let tree_ba = git_out(&ba, &["rev-parse", "HEAD^{tree}"]);

    assert_eq!(tree_ab, tree_ba, "disjoint path commits must commute");
    assert_eq!(
        git_out(&repo, &["rev-parse", "HEAD"]),
        seed,
        "seed checkout was switched; this test only merges already-disjoint commits"
    );
    assert!(ab.join("storage/core/a.rs").exists());
    assert!(ab.join("storage/core/b.rs").exists());
    assert!(ba.join("storage/core/a.rs").exists());
    assert!(ba.join("storage/core/b.rs").exists());
}
