use anvil::corpus_auditor::{ContinuousHygieneEngine, CorpusAuditor};
use tempfile::tempdir;

#[test]
fn test_corpus_auditor_and_hygiene_engine() {
    let dir = tempdir().unwrap();
    let tenancy = dir.path().join("tenancy");
    std::fs::create_dir_all(&tenancy).unwrap();
    std::fs::write(
        tenancy.join("policy.md"),
        "---\ncanonical_authority: true\n---\n# Tenancy Policy",
    )
    .unwrap();

    let audit_report = CorpusAuditor::audit_repository(dir.path(), 180).unwrap();
    assert_eq!(audit_report.unauthorized_ssot_claims.len(), 1);

    let batch = ContinuousHygieneEngine::generate_maintenance_batch(dir.path(), 5, false).unwrap();
    assert_eq!(batch.files_modified.len(), 0); // policy.md had no last_verified_at, but we can verify batch runs cleanly
}

/// The tree both SSOT walks are asserted against.
///
/// Every `canonical_authority: true` sits outside `docs/` and `contracts/`, so
/// each is an unauthorized claim if the walk reaches it. Only `.github/` should
/// be reached.
fn ssot_fixture(root: &std::path::Path) {
    const CLAIM: &str = "---\ncanonical_authority: true\n---\n# Rule\n";

    // Outside `docs/` and `contracts/`, so each is an unauthorized SSOT claim
    // -- if the walk reaches it.
    std::fs::create_dir_all(root.join(".github")).unwrap();
    std::fs::write(root.join(".github/CONTRIBUTING.md"), CLAIM).unwrap();

    // A real `.git` directory: still skipped, so a pass cannot come from the
    // rule having been dropped altogether.
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join(".git/config.md"), CLAIM).unwrap();

    // Another repository's tree, marked the way `git worktree add` marks one.
    std::fs::create_dir_all(root.join("vendored")).unwrap();
    std::fs::write(root.join("vendored/.git"), "gitdir: /elsewhere\n").unwrap();
    std::fs::write(root.join("vendored/theirs.md"), CLAIM).unwrap();

    // Vendored dependencies, which the sweeper omitted from its own copy of the
    // skip list until this change, and build output. Both names, because
    // pinning only one makes "dropping them fails" true as a CONJUNCTION: with
    // `node_modules` alone in a fixture, dropping `buck-out` by itself stays
    // green. My commit message claimed both were covered when one was.
    std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
    std::fs::write(root.join("node_modules/pkg/readme.md"), CLAIM).unwrap();
    std::fs::create_dir_all(root.join("buck-out/gen")).unwrap();
    std::fs::write(root.join("buck-out/gen/generated.md"), CLAIM).unwrap();
}

/// The auditor's own walk, and the sweeper's, by IDENTITY rather than by count.
///
/// `audit_repository` runs two walks: it delegates freshness to
/// `FreshnessLedger::scan_repository`, and runs its own for SSOT claims and
/// frontmatter. Only the first had a walker-level test, so review seeded the
/// original skip block into this second walk and into the sweeper and the suite
/// stayed at 27/27 -- the defect could be reintroduced silently in two of the
/// three places this change fixes it.
///
/// The sweeper is the sharper of the two: it is the only walker that WRITES in
/// non-dry-run, rewriting on SSOT demotion and stubbing on archival. A checkout
/// regression there means writing into another repository's tree inside a
/// contributor's working copy.
///
/// Both report `Vec<String>` of paths, so this asserts what was found rather
/// than how much -- which is what a count cannot do.
///
/// It also pins `node_modules`, which nothing pinned: `grep -rn
/// "node_modules\|buck-out" tests/` returned zero before this. That name is
/// this change's own headline item, the drift where the sweeper had omitted it,
/// and it had no check behind it.
#[test]
fn the_ssot_walks_see_dot_github_and_not_a_checkout_or_vendored_deps() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    ssot_fixture(root);

    let report = CorpusAuditor::audit_repository(root, 180).expect("audit");
    let mut found = report.unauthorized_ssot_claims.clone();
    found.sort();
    assert_eq!(
        found,
        vec![".github/CONTRIBUTING.md".to_string()],
        "the auditor's own walk must see `.github/` and must not see `.git/`, a \
         nested checkout, or vendored dependencies"
    );
}

/// Same fixture, through the walker that WRITES.
///
/// Its own `#[test]`, and that is not tidiness: with both walkers asserted in
/// one test the first failure panics and the second assertion never runs, so a
/// seed in the auditor proved nothing about the sweeper. Review had to seed
/// twice to establish what one test appeared to cover.
///
/// `ssot_claims_demoted` is populated BEFORE the `dry_run` gate and the write
/// targets the same path, so this is the non-dry-run write set: a regression
/// here names the files that would be edited inside another repository's tree.
#[test]
fn the_sweeper_would_write_only_inside_the_repository_it_was_given() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    ssot_fixture(root);

    let sweep = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(anvil::doc_archival_sweeper::DocArchivalSweeper::sweep_repository(root, true))
        .expect("sweep");
    let mut demoted = sweep.ssot_claims_demoted.clone();
    demoted.sort();
    assert_eq!(
        demoted,
        vec![".github/CONTRIBUTING.md".to_string()],
        "the sweeper writes in non-dry-run, so a checkout regression here edits \
         another repository's files"
    );
}
