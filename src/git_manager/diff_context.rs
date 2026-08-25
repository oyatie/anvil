use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
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
