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

    /// Whether `dir` is the top of a checkout of its own.
    ///
    /// A separate method rather than a `kind` probe because `kind` may not be
    /// asked about paths that do not exist: the in-memory access used by the
    /// control tests panics on an unexpected lookup, deliberately, so that a
    /// walk reaching somewhere it should not is a failure and not a shrug.
    fn is_separate_checkout(&mut self, dir: &Path) -> bool;
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

    /// The one predicate, held in the library so all three walkers share it.
    ///
    /// It was spelled inline in three places for one revision of this change,
    /// and a review found the shape of that: the seam existed and two of the
    /// three call sites bypassed it, so each copy was independently regressible
    /// and only one of them had a test. `anvil::source_scan` carries the rule
    /// and `tests/source_scan_test.rs` pins it against a real filesystem in
    /// both `.git` forms.
    fn is_separate_checkout(&mut self, dir: &Path) -> bool {
        anvil::source_scan::is_separate_checkout(dir)
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
            // #218. A directory holding its own `.git` is a separate checkout,
            // and its sources are not this repository's sources. Anvil keeps
            // agent worktrees under `.claude/worktrees/` and a `devtree` beside
            // them, and each contributed a full copy of every real site to
            // censuses that call themselves closed.
            //
            // Tested at the point of DESCENT, so the root is never asked and
            // needs no exemption -- the repository under test has a `.git` too.
            if access.is_separate_checkout(&path) {
                continue;
            }
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
