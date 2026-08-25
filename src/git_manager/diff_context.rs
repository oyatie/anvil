use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PrDiffContext {
    pub repo: String,
    pub pr_number: u64,
    pub base_branch: String,
    pub base_sha: String,
    pub head_sha: String,
    pub is_incremental: bool,
    pub previous_head_sha: Option<String>,
    pub diff_content: String,
    pub changed_files: Vec<String>,
    pub repo_working_dir: PathBuf,
}

/// One file's portion of a unified diff, split by what the change does.
///
/// Three gates each carried their own copy of this parsing, and all three
/// copies were wrong in the same two ways, because they were the same lines
/// pasted three times.
pub struct FileDiff {
    /// Repo-relative path, taken from the `+++ b/` header.
    pub path: String,
    /// Only the lines this change ADDS, without their `+`.
    pub added: String,
    /// Every line of this file's hunk, additions and context alike, without
    /// the leading marker.
    ///
    /// Separate from `added` because the two answer different questions. "Does
    /// this change introduce a mutating route" is about `added`; "does the file
    /// reference an Idempotency-Key" is about `all`, since a key already
    /// present is context the diff never adds.
    pub all: String,
}

/// Split a unified diff into its files, attributing each line to the path the
/// diff names for it.
///
/// The one place this parsing lives. What it replaces, verbatim from three
/// gates:
///
/// ```text
/// let mut current_file = "unknown.rs".to_string();
/// if let Some(first_line) = lines.first()
///     && let Some(path) = first_line.split_whitespace().last()
/// { current_file = path.trim_start_matches("b/").to_string(); }
/// ```
///
/// That block guesses a path from the last whitespace-delimited token of a
/// chunk's first line. Measured, on a chunk whose first line is ordinary code,
/// it reported the file as `registry.rs_lookup("a.rs");` -- a finding filed
/// against a path invented out of a fragment of the code it was reading. When
/// the first line yields nothing it reported `unknown.rs`, a file that does not
/// exist in any repository.
///
/// Here the path comes from the `+++ b/` header, which is the only thing in a
/// diff that states it. A hunk with no header is attributed to nothing and
/// returned to no caller, because a finding that cannot name its file is not a
/// finding an author can act on.
pub fn diffs_by_path(diff: &str) -> Vec<FileDiff> {
    let mut out: Vec<FileDiff> = Vec::new();
    let mut current: Option<usize> = None;

    for line in diff.lines() {
        // Both spellings state the path; neither guesses it. `diff --git` is
        // taken from the ` b/` side rather than by splitting on whitespace,
        // so a path containing a space survives. `+++ b/` wins where both are
        // present, because it is the one git writes per hunk.
        let header = line.strip_prefix("+++ b/").map(str::to_string).or_else(|| {
            line.strip_prefix("diff --git ")
                .and_then(|rest| rest.split_once(" b/"))
                .map(|(_, b)| b.to_string())
        });
        if let Some(path) = header {
            let path = path.trim().to_string();
            if path.is_empty() {
                continue;
            }
            current = Some(match out.iter().position(|f| f.path == path) {
                Some(i) => i,
                None => {
                    out.push(FileDiff {
                        path,
                        added: String::new(),
                        all: String::new(),
                    });
                    out.len() - 1
                }
            });
            continue;
        }
        // Headers are not content. `---` and `+++` are skipped before the
        // add/remove test, or `+++ b/x` would read as an added line beginning
        // with `++`.
        if line.starts_with("--- ") || line.starts_with("+++ ") {
            continue;
        }
        let Some(i) = current else {
            continue;
        };
        if line.starts_with("@@") {
            continue;
        }
        if let Some(body) = line.strip_prefix('+') {
            out[i].added.push_str(body);
            out[i].added.push('\n');
            out[i].all.push_str(body);
            out[i].all.push('\n');
        } else if let Some(body) = line.strip_prefix(' ') {
            // Context: present in the file, not introduced by this change.
            out[i].all.push_str(body);
            out[i].all.push('\n');
        }
        // A `-` line is what the change REMOVES. It belongs to neither: a gate
        // that reads it accuses the pull request of the thing it deletes.
    }
    out
}
