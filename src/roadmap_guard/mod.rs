use std::path::Path;

/// Verifies whether an issue title/body matches the live masterplan work item space.
///
/// Returns `true` when there is no `specs/masterplan.json` to check against, and
/// `false` when the issue references a retired planning prefix (legacy omc/omx).
pub fn verify_issue_roadmap_alignment(
    repo_dir: &Path,
    issue_title: &str,
    issue_body: &str,
) -> bool {
    if !repo_dir.join("specs/masterplan.json").exists() {
        return true; // If no masterplan, allow
    }

    !(issue_title.contains(".omc/")
        || issue_body.contains(".omc/")
        || issue_title.contains(".omx/")
        || issue_body.contains(".omx/"))
}
