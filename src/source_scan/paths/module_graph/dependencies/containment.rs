use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn contained_source(path: &Path, repo_root: &Path) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("cannot resolve included source {}: {error}", path.display()))?;
    if !canonical.starts_with(repo_root) {
        return Err(format!(
            "included source {} escapes repository {}",
            path.display(),
            repo_root.display()
        ));
    }
    let metadata = fs::metadata(&canonical)
        .map_err(|error| format!("cannot inspect source {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("included source {} is not a file", path.display()));
    }
    Ok(canonical)
}
