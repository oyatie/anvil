//! What Anvil stages in somebody else's checkout.

use tokio::process::Command;

/// Paths Anvil writes into somebody else's checkout. A commit Anvil pushes
/// carries what the change produced, never Anvil's own bookkeeping
/// (`.cursor/receipts` is the legacy location, still present in older
/// checkouts).
const ANVIL_OWNED_PATHS: &[&str] = &[
    crate::attestation_guard::ANVIL_RECEIPTS_DIR,
    ".cursor/receipts",
];

/// The `git add` that every Anvil staging site runs: stage the whole tree
/// except Anvil's own bookkeeping.
///
/// Returns the built `Command` rather than its arguments. Four sites each
/// spelled their own `["add", "-A"]` -- `certify.rs` did it sixteen lines under
/// a comment saying it must never do that -- and only `QueueHealer` carried the
/// exclusion, so three of them committed Anvil's receipt onto the pull request
/// it had just written the receipt into. Handing back a `Vec` was not enough:
/// a caller can take the arguments and pass `&args[..2]`, which is the bug
/// again with the shared function's name on it.
pub fn stage_excluding_receipts(repo_dir: &std::path::Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_dir).args(["add", "-A", "--", "."]);
    for p in ANVIL_OWNED_PATHS {
        cmd.arg(format!(":(exclude){p}"));
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// The exclusion has to name every path Anvil owns, not just the current
    /// one: a checkout carried over from before the move still has the legacy
    /// directory, and staging that is the same defect.
    #[test]
    fn the_staging_command_excludes_every_path_anvil_owns() {
        let cmd = stage_excluding_receipts(Path::new("/tmp"));
        let args: Vec<String> = cmd
            .as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(&args[..4], &["add", "-A", "--", "."]);
        for p in ANVIL_OWNED_PATHS {
            assert!(
                args.contains(&format!(":(exclude){p}")),
                "{p} would be staged into somebody else's commit: {args:?}"
            );
        }
    }
}
