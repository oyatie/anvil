//! A site that mutates the shared clone must hold that clone's lock.
//!
//! `one_clone_one_writer_test` exercises the LOCK: it proves a mutex handed
//! out per repository excludes two holders. That is correct by construction --
//! it is an `Arc<Mutex<()>>` -- and it says nothing about whether the fixer or
//! the reconciler ever takes it. Deleting both call sites left the whole suite
//! green at 2467 passed, which is how a fix ends up shipping with nothing
//! measuring it.
//!
//! So this measures the USE. `GitManager::locked_clone` hands back the path
//! and the guard together, and the two-step spelling -- `ensure_repo_cloned`
//! for the path, `lock_clone` for the lock -- lets a caller take the first and
//! forget the second, invisibly. This refuses any production function that
//! reaches a clone and then mutates its working tree without the guard.

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
/// Crude on purpose: a brace inside a string literal would end a body early,
/// which can only make this scan look at LESS text and so can only produce a
/// false pass on a defect, never a false accusation against clean code.
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
