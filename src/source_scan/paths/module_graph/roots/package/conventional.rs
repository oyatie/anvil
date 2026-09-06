use std::fs;
use std::path::{Path, PathBuf};

use super::{contained_file, existing_file};

pub(super) fn all_roots(package_dir: &Path, repo_root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut roots = Vec::new();
    push_if_file(&mut roots, package_dir.join("src/lib.rs"), repo_root)?;
    push_if_file(&mut roots, package_dir.join("src/main.rs"), repo_root)?;
    roots.extend(binary_roots(package_dir, repo_root)?);
    Ok(roots)
}

pub(super) fn binary_roots(package_dir: &Path, repo_root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut roots = Vec::new();
    let bin_dir = package_dir.join("src/bin");
    let entries = match fs::read_dir(&bin_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(roots),
        Err(error) => {
            return Err(format!(
                "cannot read Cargo binary directory {}: {error}",
                bin_dir.display()
            ));
        }
    };
    for entry in entries {
        let entry = entry
            .map_err(|error| format!("cannot read an entry in {}: {error}", bin_dir.display()))?;
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "cannot inspect binary root {}: {error}",
                entry.path().display()
            )
        })?;
        let path = entry.path();
        if file_type.is_symlink() {
            return Err(format!(
                "Cargo binary root candidate {} is a symlink",
                path.display()
            ));
        }
        if file_type.is_file() && path.extension().is_some_and(|extension| extension == "rs") {
            roots.push(contained_file(&path, repo_root)?);
        } else if file_type.is_dir() {
            push_if_file(&mut roots, path.join("main.rs"), repo_root)?;
        }
    }
    Ok(roots)
}

pub(super) fn push_if_file(
    roots: &mut Vec<PathBuf>,
    path: PathBuf,
    repo_root: &Path,
) -> Result<(), String> {
    if existing_file(&path)? {
        roots.push(contained_file(&path, repo_root)?);
    }
    Ok(())
}
