use anvil::task_orchestrator::{
    fan_out_after_implement, interview, transitive_deps, DeliveryBoard, DeliveryRole, HandoffAgent,
    IntakeVerdict, InterviewDraft,
};
use std::collections::BTreeSet;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
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

#[tokio::test]
async fn real_n_parallel_writes_all_land() {
    let tmp = tempdir().unwrap();
    let n = 8;
    let live = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let mut futs = Vec::new();
    for i in 0..n {
        let dir = tmp.path().to_path_buf();
        let live = Arc::clone(&live);
        let peak = Arc::clone(&peak);
        futs.push(async move {
            let now = live.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now, Ordering::SeqCst);
            let path = dir.join(format!("storage-core-item{i}.rs"));
            let body = format!("pub fn item{i}() -> u32 {{ {i} }}\n");
            tokio::task::spawn_blocking(move || fs::write(path, body))
                .await
                .unwrap()
                .unwrap();
            live.fetch_sub(1, Ordering::SeqCst);
            i
        });
    }
    let got = futures::future::join_all(futs).await;
    assert_eq!(got.len(), n);
    assert_eq!(peak.load(Ordering::SeqCst), n, "writes did not overlap");
    for i in 0..n {
        let body = fs::read_to_string(tmp.path().join(format!("storage-core-item{i}.rs"))).unwrap();
        assert!(body.contains(&format!("item{i}")));
    }
}

#[test]
fn git_disjoint_commits_commute() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    run(root, &["git", "init", "-q"]);
    run(root, &["git", "config", "user.email", "a@example.com"]);
    run(root, &["git", "config", "user.name", "a"]);
    fs::write(root.join("README"), "x").unwrap();
    run(root, &["git", "add", "README"]);
    run(root, &["git", "commit", "-qm", "seed"]);
    for i in 0..4 {
        run(root, &["git", "checkout", "-q", "-b", &format!("b{i}")]);
        fs::create_dir_all(root.join("storage/core")).unwrap();
        fs::write(
            root.join(format!("storage/core/n{i}.rs")),
            format!("pub fn n{i}() {{}}\n"),
        )
        .unwrap();
        run(root, &["git", "add", &format!("storage/core/n{i}.rs")]);
        run(root, &["git", "commit", "-qm", &format!("n{i}")]);
        run(root, &["git", "checkout", "-q", "-"]);
    }
    for i in 0..4 {
        let status = std::process::Command::new("git")
            .args(["merge", "-q", "--no-edit", &format!("b{i}")])
            .current_dir(root)
            .status()
            .unwrap();
        assert!(status.success(), "disjoint merge b{i} conflicted");
    }
}

fn run(dir: &std::path::Path, args: &[&str]) {
    let st = std::process::Command::new(args[0])
        .args(&args[1..])
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(st.success(), "{args:?}");
}
