use anvil::corpus_auditor::FreshnessLedger;
use tempfile::tempdir;

#[test]
fn test_freshness_ledger_metrics() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("active.rs"), "fn a() {}").unwrap();
    std::fs::write(dir.path().join("active.md"), "# Active").unwrap();

    let report = FreshnessLedger::scan_repository(dir.path(), 180);
    assert_eq!(report.total_files, 2);
    assert_eq!(report.dormant_files_count, 0);
    assert_eq!(report.freshness_ratio, 1.0);
}

/// The walk itself must see `.github/`, and must not see another checkout.
///
/// Asserted through `scan_repository`, not through the predicate it calls.
/// Review measured why that matters: seeding the original skip block back into
/// `corpus_auditor::auditor` left the suite green, because every test drove the
/// predicate directly and none drove a walker past a `.github/` file.
///
/// # The count alone is not the assertion, and this is why
///
/// The first version of this test asserted `total_files == 3` against a
/// fixture where the buggy walk counted `own.md`, `vendored/.git` and
/// `vendored/theirs.md` -- also three. `vendored/.git` is a FILE, the form
/// `git worktree add` writes, so the broken walk counted the very marker that
/// should have stopped it. Two different sets of three, one assertion, green
/// under both. That is the proxy failure this test exists to close, committed
/// inside the test that closes it.
///
/// So the fixture is built so the two readings cannot coincide -- the checkout
/// holds more files than `.github/` does -- and the dormant record, which
/// carries PATHS rather than a count, is what the identities are read from.
#[test]
fn the_walk_counts_dot_github_and_skips_a_nested_checkout() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(root.join("own.md"), "# own").unwrap();

    // `.github/` is not `.git/`. A string prefix could not tell them apart, and
    // these are the files that were lost: measured in anvil's own tree, EIGHT
    // `.md`/`.yaml`/`.yml` files live under `.github/` and every one was
    // invisible to this ledger. Not latent -- live, in this repository.
    std::fs::create_dir_all(root.join(".github/workflows")).unwrap();
    std::fs::write(root.join(".github/workflows/ci.yml"), "on: push\n").unwrap();
    std::fs::write(root.join(".github/CONTRIBUTING.md"), "# contributing").unwrap();
    std::fs::write(root.join(".gitignore"), "target\n").unwrap();

    // A real `.git` directory is still skipped, so a pass here cannot come from
    // the rule having been dropped altogether.
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();

    // Another repository's tree, marked the way `git worktree add` marks one,
    // and deliberately larger than the `.github/` set so no count can satisfy
    // both readings.
    let vendored = root.join("vendored");
    std::fs::create_dir_all(&vendored).unwrap();
    std::fs::write(vendored.join(".git"), "gitdir: /elsewhere\n").unwrap();
    for n in ["a.md", "b.md", "c.md", "d.md"] {
        std::fs::write(vendored.join(n), "# theirs").unwrap();
    }

    let report = FreshnessLedger::scan_repository(root, 180);

    // Correct: own.md, .github/workflows/ci.yml, .github/CONTRIBUTING.md,
    // .gitignore. The broken walk instead counts own.md plus the checkout's
    // five files -- six, and never four.
    assert_eq!(
        report.total_files, 4,
        "expected own.md, both files under .github/ and .gitignore; \
         `.git/HEAD` and the nested checkout must contribute nothing"
    );
}
