//! Physical Rust sources for repository self-conformance tests.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct SourceFile {
    pub path: PathBuf,
    pub text: String,
}

#[derive(Clone, Copy)]
pub enum EntryKind {
    Directory,
    RegularFile,
    Other,
}

pub trait SourceAccess {
    fn read_dir(&mut self, dir: &Path) -> io::Result<Vec<io::Result<PathBuf>>>;
    fn kind(&mut self, path: &Path) -> io::Result<EntryKind>;
    fn read_text(&mut self, path: &Path) -> io::Result<String>;
}

struct Filesystem;

impl SourceAccess for Filesystem {
    fn read_dir(&mut self, dir: &Path) -> io::Result<Vec<io::Result<PathBuf>>> {
        Ok(fs::read_dir(dir)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect())
    }

    fn kind(&mut self, path: &Path) -> io::Result<EntryKind> {
        let metadata = fs::metadata(path)?;
        Ok(if metadata.is_dir() {
            EntryKind::Directory
        } else if metadata.is_file() {
            EntryKind::RegularFile
        } else {
            EntryKind::Other
        })
    }

    fn read_text(&mut self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }
}

pub fn rust_sources(root: &Path) -> Result<Vec<SourceFile>, String> {
    rust_sources_with(root, &mut Filesystem)
}

pub fn rust_sources_with(
    root: &Path,
    access: &mut impl SourceAccess,
) -> Result<Vec<SourceFile>, String> {
    if !matches!(
        access
            .kind(root)
            .map_err(|error| format!("cannot inspect {}: {error}", root.display()))?,
        EntryKind::Directory
    ) {
        return Err(format!("{}: expected a source directory", root.display()));
    }
    let mut paths = Vec::new();
    discover(root, access, &mut paths)?;
    paths.sort();
    if paths.is_empty() {
        return Err(format!("{}: no Rust source files found", root.display()));
    }
    paths
        .into_iter()
        .map(|path| {
            let text = access
                .read_text(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            Ok(SourceFile { path, text })
        })
        .collect()
}

fn discover(
    dir: &Path,
    access: &mut impl SourceAccess,
    out: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let entries = access
        .read_dir(dir)
        .map_err(|error| format!("cannot list {}: {error}", dir.display()))?;
    for entry in entries {
        let path =
            entry.map_err(|error| format!("cannot list entry in {}: {error}", dir.display()))?;
        let kind = access
            .kind(&path)
            .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
        if matches!(kind, EntryKind::Directory) {
            discover(&path, access, out)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            if !matches!(kind, EntryKind::RegularFile) {
                return Err(format!(
                    "{}: expected a regular Rust source file",
                    path.display()
                ));
            }
            out.push(path);
        }
    }
    Ok(())
}
