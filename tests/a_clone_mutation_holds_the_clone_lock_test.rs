//! A site that mutates the shared clone must hold that clone's lock.
//!
//! `one_clone_one_writer_test` exercises the LOCK: it proves a mutex handed
//! out per repository excludes two holders. That is correct by construction --
//! it is an `Arc<Mutex<()>>` -- and it says nothing about whether the fixer or
//! the reconciler ever takes it. Deleting both call sites left the whole suite
//! green at 2467 passed, which is how a fix ends up shipping with nothing
//! measuring it.
//!
//! So this measures the USE: it refuses any production function that reaches a
//! clone with `ensure_repo_cloned` and then mutates its working tree without
//! the guard.
//!
//! What it does NOT check, because a text scan cannot: how long the guard is
//! held. A caller that takes `locked_clone`, clones the path out of it and
//! lets the guard drop still spells `locked_clone`, so this passes it. That
//! extent is enforced instead by `LockedClone::run_git`, which borrows the
//! guard across the await -- the evasion does not compile.
//!
//! KNOWN UNCOVERED, so that a green run here is not read as "every site". Two
//! production writers of the shared clone are invisible to this scan because
//! the acquisition and the mutating verbs sit in different function bodies:
//! `PrSelfHealer::auto_heal_pr_branch` and `DocArchivalSweeper`, both reached
//! from `cli/handlers.rs` where the clone is acquired and the verbs are not.
//! Neither is fixed by taking this lock: they run as CLI subcommands, a
//! different PROCESS, and an in-process mutex excludes nothing across
//! processes. The upgrade is an advisory lock on the clone directory, named in
//! `git_manager/clone_lock.rs`.

use std::path::{Path, PathBuf};

/// Git subcommands that write to a working tree. Reading (`rev-parse`,
/// `status`, `diff`, `log`) is not mutation and is deliberately absent.
const MUTATING_GIT_VERBS: &[&str] = &["\"checkout\"", "\"commit\"", "\"push\"", "\"add\""];

/// Acquiring the path without the guard.
const UNLOCKED_ACQUISITION: &str = "ensure_repo_cloned";

/// The spelling that carries both.
const LOCKED_ACQUISITION: &str = "locked_clone";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn production_sources(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            production_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs")
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            out.push((path, text));
        }
    }
}

/// Bodies of `fn` items, matched by brace depth from the opening `{`.
///
/// Crude, and not sound in the direction first claimed here. A `}` inside a
/// string ends a body early; a `{` inside one does the opposite -- depth never
/// returns to zero, the body runs on, and it can absorb a later function whose
/// `locked_clone` then exculpates a real offender. Measured today: 0 offenders
/// and 0 exculpated across `src/`, but this is a heuristic that can miss, not
/// one that cannot. The guard's EXTENT is not checked here at all -- that is
/// `LockedClone::run_git`'s borrow, which the compiler enforces.
fn function_bodies(source: &str) -> Vec<String> {
    let mut bodies = Vec::new();
    let bytes = source.as_bytes();
    let mut cursor = 0;
    while let Some(found) = source[cursor..].find("fn ") {
        let start = cursor + found;
        let Some(open) = source[start..].find('{').map(|o| start + o) else {
            break;
        };
        let mut depth = 0i32;
        let mut end = open;
        for (offset, byte) in bytes[open..].iter().enumerate() {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + offset;
                        break;
                    }
                }
                _ => {}
            }
        }
        if end > open {
            bodies.push(source[open..=end].to_string());
        }
        cursor = start + 3;
    }
    bodies
}

fn offenders() -> Vec<String> {
    let mut sources = Vec::new();
    production_sources(&repo_root().join("src"), &mut sources);
    assert!(
        sources.len() > 50,
        "the src scan found almost nothing, so a pass here would mean nothing"
    );

    let mut found = Vec::new();
    for (path, text) in &sources {
        let rel = path
            .strip_prefix(repo_root())
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        // The lock's own module defines both spellings; it is not a caller.
        if rel == "src/git_manager/clone_lock.rs" || rel == "src/git_manager/mod.rs" {
            continue;
        }
        for body in function_bodies(text) {
            if !body.contains(UNLOCKED_ACQUISITION) {
                continue;
            }
            let verbs: Vec<&str> = MUTATING_GIT_VERBS
                .iter()
                .filter(|v| body.contains(**v))
                .copied()
                .collect();
            if verbs.is_empty() || body.contains(LOCKED_ACQUISITION) {
                continue;
            }
            found.push(format!(
                "{rel}: mutates with {} and no guard",
                verbs.join(", ")
            ));
        }
    }
    found
}

/// The property: no production function takes a clone unlocked and then writes
/// to it.
#[test]
fn no_production_site_mutates_a_clone_it_did_not_lock() {
    let offenders = offenders();
    assert!(
        offenders.is_empty(),
        "a site reaches the shared clone with `{UNLOCKED_ACQUISITION}` and mutates its \
         working tree without holding that clone's lock, so a second pull request's \
         checkout can land mid-write and its tree be pushed onto this one's branch. \
         Use `GitManager::{LOCKED_ACQUISITION}`, which hands back the path and the \
         guard together: {offenders:#?}"
    );
}

/// The scan has to be able to see the defect it refuses.
///
/// Without this the check passes whether or not it can match anything, which
/// is the failure it was written against.
#[test]
fn the_scan_finds_an_unlocked_mutation_when_one_is_present() {
    let seeded = r#"
        async fn resolve_and_fix(&self, repo: &str) -> Result<()> {
            let repo_dir = self.git_mgr.ensure_repo_cloned(repo).await?;
            let mut cmd = Command::new("git");
            cmd.current_dir(&repo_dir).args(["checkout", "-B", "pr-1"]);
            Ok(())
        }
    "#;
    let bodies = function_bodies(seeded);
    assert!(
        bodies.iter().any(|b| b.contains(UNLOCKED_ACQUISITION)
            && b.contains("\"checkout\"")
            && !b.contains(LOCKED_ACQUISITION)),
        "the scan could not see an unlocked mutation written directly in front of it"
    );

    let fixed = seeded.replace(
        "self.git_mgr.ensure_repo_cloned(repo).await?",
        "self.git_mgr.locked_clone(repo).await?",
    );
    assert!(
        !function_bodies(&fixed)
            .iter()
            .any(|b| b.contains(UNLOCKED_ACQUISITION)),
        "the scan still accuses a site that took the locked spelling"
    );
}
