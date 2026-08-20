use anvil::task_orchestrator::{ScopedTaskDefinition, SourceDocVerifier, TaskDagSequencer};
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
