use anvil::task_orchestrator::{ScopedTaskDefinition, SourceDocVerifier, TaskDagSequencer};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_source_doc_verifier_validates_truth_and_catches_contradictions() {
    let tmp = tempdir().expect("tempdir");
    let repo_root = tmp.path();

    // Create valid source file and directory
    let src_dir = repo_root.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("lib.rs"), "// lib").unwrap();

    // Create an ADR document
    let adr_dir = repo_root.join("docs/adr");
    std::fs::create_dir_all(&adr_dir).unwrap();
    let valid_adr = adr_dir.join("ADR-001-active-active.md");
    std::fs::write(
        &valid_adr,
        "# ADR-001: Active-Active Multi-Region\nStatus: Accepted\nPriority: P0\n",
    )
    .unwrap();

    let verifier = SourceDocVerifier::new();

    // 1. Nominal task test
    let task_valid = ScopedTaskDefinition {
        task_id: "ADR-001".to_string(),
        source_doc_path: "docs/adr/ADR-001-active-active.md".to_string(),
        title: "Active-Active Multi-Region".to_string(),
        domain: "replication".to_string(),
        priority: 0,
        target_files: vec!["src/lib.rs".to_string()],
        dependencies: vec![],
        required_invariants: vec!["vector_clock".to_string()],
        is_verified_ssot: true,
    };

    let res = verifier.verify_scoped_task(&task_valid, repo_root).unwrap();
    assert!(res.is_valid);
    assert!(res.contradiction_reason.is_none());

    // 2. Contradictory / Superseded ADR test
    let superseded_adr = adr_dir.join("ADR-002-old-monolith.md");
    std::fs::write(
        &superseded_adr,
        "# ADR-002: Monolith Architecture\nStatus: Superseded\nPriority: P2\n",
    )
    .unwrap();

    let task_deprecated = ScopedTaskDefinition {
        task_id: "ADR-002".to_string(),
        source_doc_path: "docs/adr/ADR-002-old-monolith.md".to_string(),
        title: "Monolith Architecture".to_string(),
        domain: "architecture".to_string(),
        priority: 2,
        target_files: vec!["src/lib.rs".to_string()],
        dependencies: vec![],
        required_invariants: vec![],
        is_verified_ssot: false,
    };

    let res_deprecated = verifier
        .verify_scoped_task(&task_deprecated, repo_root)
        .unwrap();
    assert!(!res_deprecated.is_valid);
    assert!(res_deprecated.contradiction_reason.is_some());
    assert!(res_deprecated
        .contradiction_reason
        .unwrap()
        .contains("SUPERSEDED"));
}

#[test]
fn test_task_dag_sequencer_orders_dependencies_topologically() {
    let sequencer = TaskDagSequencer::new();

    // Create tasks with dependencies:
    // P0: Task A (Foundation: Ingress HMAC) -> No deps
    // P1: Task B (Middleware: Semaphore Pool) -> Depends on Task A
    // P2: Task C (Feature: Auto Task Dispatch) -> Depends on Task B
    // P1: Task D (Parallel Root: WAL Persistence) -> No deps

    let task_a = ScopedTaskDefinition {
        task_id: "TASK-A".to_string(),
        source_doc_path: "docs/adr/ADR-A.md".to_string(),
        title: "Ingress HMAC".to_string(),
        domain: "security".to_string(),
        priority: 0,
        target_files: vec!["src/webhook/mod.rs".to_string()],
        dependencies: vec![],
        required_invariants: vec![],
        is_verified_ssot: true,
    };

    let task_b = ScopedTaskDefinition {
        task_id: "TASK-B".to_string(),
        source_doc_path: "docs/adr/ADR-B.md".to_string(),
        title: "Semaphore Pool".to_string(),
        domain: "concurrency".to_string(),
        priority: 1,
        target_files: vec!["src/webhook/mod.rs".to_string()],
        dependencies: vec!["TASK-A".to_string()],
        required_invariants: vec![],
        is_verified_ssot: true,
    };

    let task_c = ScopedTaskDefinition {
        task_id: "TASK-C".to_string(),
        source_doc_path: "docs/adr/ADR-C.md".to_string(),
        title: "Auto Task Dispatch".to_string(),
        domain: "pipeline".to_string(),
        priority: 2,
        target_files: vec!["src/task_orchestrator/mod.rs".to_string()],
        dependencies: vec!["TASK-B".to_string()],
        required_invariants: vec![],
        is_verified_ssot: true,
    };

    let task_d = ScopedTaskDefinition {
        task_id: "TASK-D".to_string(),
        source_doc_path: "docs/adr/ADR-D.md".to_string(),
        title: "WAL Persistence".to_string(),
        domain: "state".to_string(),
        priority: 1,
        target_files: vec!["src/state.rs".to_string()],
        dependencies: vec![],
        required_invariants: vec![],
        is_verified_ssot: true,
    };

    let tasks = vec![task_c, task_b, task_a, task_d];
    let stages = sequencer.sequence_tasks(tasks).unwrap();

    assert_eq!(stages.len(), 3);

    // Stage 0 must contain roots TASK-A (P0) and TASK-D (P1)
    let stage0_ids: Vec<String> = stages[0].tasks.iter().map(|t| t.task_id.clone()).collect();
    assert_eq!(stage0_ids, vec!["TASK-A", "TASK-D"]);

    // Stage 1 must contain TASK-B
    let stage1_ids: Vec<String> = stages[1].tasks.iter().map(|t| t.task_id.clone()).collect();
    assert_eq!(stage1_ids, vec!["TASK-B"]);

    // Stage 2 must contain TASK-C
    let stage2_ids: Vec<String> = stages[2].tasks.iter().map(|t| t.task_id.clone()).collect();
    assert_eq!(stage2_ids, vec!["TASK-C"]);
}

fn task(id: &str, files: &[&str]) -> ScopedTaskDefinition {
    ScopedTaskDefinition {
        task_id: id.to_string(),
        source_doc_path: format!("docs/{id}.md"),
        title: id.to_string(),
        domain: "test".to_string(),
        priority: 1,
        target_files: files.iter().map(|s| s.to_string()).collect(),
        dependencies: vec![],
        required_invariants: vec![],
        is_verified_ssot: true,
    }
}

#[test]
fn default_layout_rejects_dump_roots_and_accepts_faces() {
    use anvil::task_orchestrator::{layout_violations, CAP_CHILDREN, FACES, FORBIDDEN_NAMES};

    let bad = layout_violations(&[
        "plan/foo.md".into(),
        "libs/x.rs".into(),
        "storage/src/lib.rs".into(),
        "storage/plan/x.md".into(),
        "app/foundry/tasks/x.md".into(),
        "storage/core/journal.rs".into(),
        "app/foundry/ports/blob.rs".into(),
        "docs/decisions/ADR.md".into(),
    ]);
    assert!(bad.iter().any(|s| s.contains("plan/foo")));
    assert!(bad.iter().any(|s| s.contains("libs")));
    assert!(bad.iter().any(|s| s.contains("storage/src")));
    assert!(bad.iter().any(|s| s.contains("storage/plan")));
    assert!(bad.iter().any(|s| s.contains("foundry/tasks")));
    assert!(!bad.iter().any(|s| s.contains("storage/core")));
    assert!(!bad.iter().any(|s| s.contains("foundry/ports")));
    assert!(!bad.iter().any(|s| s.contains("docs/decisions")));
    assert!(FORBIDDEN_NAMES.contains(&"tasks"));
    assert!(CAP_CHILDREN.contains(&"ports"));
    assert_eq!(FACES, &["core", "ports", "adapters", "facade"]);
}

#[test]
fn path_occupancy_is_set_intersection_not_crate_lock() {
    use anvil::task_orchestrator::{occupy_move, path_sets_disjoint};

    assert!(path_sets_disjoint(
        &["storage/core/a.rs".into()],
        &["storage/core/b.rs".into()]
    ));
    assert!(!path_sets_disjoint(
        &["storage/core/a.rs".into()],
        &["storage/core/a.rs".into()]
    ));
    let mv: Vec<String> = occupy_move("storage/core/old.rs", "storage/core/new.rs")
        .into_iter()
        .collect();
    assert!(mv.contains(&"storage/core/old.rs".into()));
    assert!(mv.contains(&"storage/core/new.rs".into()));
    assert!(!path_sets_disjoint(&mv, &["storage/core/old.rs".into()]));
    assert!(!path_sets_disjoint(&mv, &["storage/core/new.rs".into()]));
    assert!(path_sets_disjoint(&mv, &["storage/core/other.rs".into()]));
}

#[tokio::test]
async fn overlapping_layer_fails_closed_before_spawn() {
    use anvil::task_orchestrator::{
        assert_layer_paths_disjoint, run_layer_parallel, TaskExecutionReport,
    };

    let tasks = vec![task("A", &["src/lib.rs"]), task("B", &["src/lib.rs"])];
    let err = assert_layer_paths_disjoint(&tasks).unwrap_err().to_string();
    assert!(err.contains("path occupancy overlap"));

    let spawned = Arc::new(AtomicUsize::new(0));
    let spawned_c = Arc::clone(&spawned);
    let err = run_layer_parallel(tasks, move |t| {
        spawned_c.fetch_add(1, Ordering::SeqCst);
        async move {
            Ok(TaskExecutionReport {
                task_id: t.task_id,
                repo: "lab".into(),
                branch_name: "feat/x".into(),
                pr_number: None,
                attempts: 1,
                tokens_consumed: 0,
                status: "ok".into(),
                summary: String::new(),
            })
        }
    })
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("path occupancy overlap"));
    assert_eq!(spawned.load(Ordering::SeqCst), 0);
}

#[test]
fn git_mv_overlap_on_either_end_fails_closed_before_spawn() {
    use anvil::task_orchestrator::{assert_layer_paths_disjoint, occupy_move};

    let occupied_paths: Vec<String> = occupy_move("storage/core/old.rs", "storage/core/new.rs")
        .into_iter()
        .collect();
    let occupied: Vec<&str> = occupied_paths.iter().map(String::as_str).collect();
    assert!(assert_layer_paths_disjoint(&[
        task("mv", &occupied),
        task("edit-new", &["storage/core/new.rs"]),
    ])
    .unwrap_err()
    .to_string()
    .contains("path occupancy overlap"));
    assert!(assert_layer_paths_disjoint(&[
        task("mv", &occupied),
        task("edit-old", &["storage/core/old.rs"]),
    ])
    .unwrap_err()
    .to_string()
    .contains("path occupancy overlap"));
    assert_layer_paths_disjoint(&[
        task("mv", &occupied),
        task("other", &["storage/core/other.rs"]),
    ])
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn disjoint_layer_peak_concurrency_is_n() {
    use anvil::task_orchestrator::{run_layer_parallel, TaskExecutionReport};

    let live = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let n = 3;
    let barrier = Arc::new(tokio::sync::Barrier::new(n));
    let tasks = vec![
        task("A", &["storage/core/a.rs"]),
        task("B", &["storage/core/b.rs"]),
        task("C", &["storage/core/c.rs"]),
    ];
    let live_c = Arc::clone(&live);
    let peak_c = Arc::clone(&peak);
    let barrier_c = Arc::clone(&barrier);
    let reports = tokio::time::timeout(
        Duration::from_secs(5),
        run_layer_parallel(tasks, move |t| {
            let live = Arc::clone(&live_c);
            let peak = Arc::clone(&peak_c);
            let barrier = Arc::clone(&barrier_c);
            async move {
                let now = live.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                barrier.wait().await;
                live.fetch_sub(1, Ordering::SeqCst);
                Ok(TaskExecutionReport {
                    task_id: t.task_id,
                    repo: "lab".into(),
                    branch_name: "feat/x".into(),
                    pr_number: None,
                    attempts: 1,
                    tokens_consumed: 0,
                    status: "ok".into(),
                    summary: String::new(),
                })
            }
        }),
    )
    .await
    .expect("serial for-await deadlocks a barrier of N")
    .expect("layer");

    assert_eq!(reports.len(), n);
    assert_eq!(
        peak.load(Ordering::SeqCst),
        n,
        "peak in-flight hops was {}, not {n}",
        peak.load(Ordering::SeqCst)
    );
}
