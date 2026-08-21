//! Repository name validation for the manual `/api/*` control surface.
//!
//! The `/webhook` path already allowlists against `WATCHED_REPOS`
//! (`webhook_handlers.rs`), but the manual handlers did not. `POST /api/review`
//! with `{"repo": "attacker/evil"}` would clone that repository and run an AI
//! agent with `--dangerously-skip-permissions` inside it.
//!
//! Two independent checks, both required (invariant I4):
//!   1. syntactic — the name must be `owner/repo` with a conservative charset,
//!      which also blocks the `"x/.."` traversal that `get_repo_dir` accepted;
//!   2. authorization — the name must appear in the configured allowlist.

use crate::config::Config;

/// Rejects anything that is not a plain `owner/repo` pair.
///
/// Deliberately conservative: no path separators beyond the single `/`, no
/// `..`, no whitespace, no shell metacharacters.
pub fn is_syntactically_valid(repo: &str) -> bool {
    let mut parts = repo.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let ok = |seg: &str| {
        !seg.is_empty()
            && seg.len() <= 100
            && seg != "."
            && seg != ".."
            && seg
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    };
    ok(owner) && ok(name)
}

/// Validates a caller-supplied repository against syntax and the allowlist.
///
/// Returns the reason for rejection, suitable for a 400 response body.
pub fn validate(config: &Config, repo: &str) -> Result<(), String> {
    if !is_syntactically_valid(repo) {
        return Err(format!(
            "Repository '{}' is not a valid owner/repo name",
            repo
        ));
    }
    if !config
        .watched_repos
        .iter()
        .any(|w| w.eq_ignore_ascii_case(repo))
    {
        return Err(format!("Repository '{}' is not in WATCHED_REPOS", repo));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal() {
        // `get_repo_dir` takes the segment after the last '/', so "x/.." became
        // repos_base_dir.join("..") — writing executable hooks outside repos/.
        for bad in ["x/..", "../etc", "x/../..", "..", "a/b/c", "/etc/passwd"] {
            assert!(!is_syntactically_valid(bad), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn rejects_injection_and_malformed_shapes() {
        for bad in [
            "owner/repo;rm -rf /",
            "owner /repo",
            "owner/",
            "/repo",
            "",
            "ownerrepo",
            "owner/re po",
            "owner/repo\n",
        ] {
            assert!(!is_syntactically_valid(bad), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn accepts_real_repository_names() {
        for good in ["oyatie/anvil", "oyatie/console", "a-b_c.d/e-f_g.h", "o/r"] {
            assert!(is_syntactically_valid(good), "{good:?} must be accepted");
        }
    }
}
