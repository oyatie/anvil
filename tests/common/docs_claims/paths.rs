//! Working-tree path admission and finite final-component glob expansion.

use std::path::{Path, PathBuf};

pub fn repo_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .map_err(|e| format!("the repository root is unreadable ({e})"))
}

/// Check the final component, canonicalize, then enforce resolved containment.
/// These metadata operations precede containment. Intermediate links may be
/// followed; this is neither an atomic admission/read nor a race-free boundary.
pub fn gate(root: &Path, p: &Path, shown: &str) -> Result<(), String> {
    let refuse = || {
        Err(format!(
            "`{shown}` is not a readable path inside the repository"
        ))
    };
    // Inspect the final component only; retain one generic refusal message.
    match std::fs::symlink_metadata(p) {
        Ok(meta) if meta.file_type().is_symlink() => return refuse(),
        Ok(_) => {}
        // Absent, unreadable and outside-root paths share this diagnostic.
        Err(_) => return refuse(),
    }
    let Ok(real) = p.canonicalize() else {
        return refuse();
    };
    if !real.starts_with(root) {
        return refuse();
    }
    // Apply the dot-component rule to the resolved path as well as the glob.
    if real
        .strip_prefix(root)
        .unwrap_or(&real)
        .components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
    {
        return Err(format!(
            "`{shown}` resolves through a path component beginning with `.`; the \
             corpus excludes dot-component paths in the live working tree"
        ));
    }
    Ok(())
}

/// Regular Markdown documents under `dir` in the live working tree.
/// Includes admitted untracked files; no index-membership query is performed.
/// Directory recursion and every entry retain the existing gate.
pub fn md_files_under(root: &Path, dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        // Gate each directory before listing its entries.
        let shown_dir = d.strip_prefix(root).unwrap_or(&d).display().to_string();
        gate(root, &d, &shown_dir)?;
        let entries = std::fs::read_dir(&d)
            .map_err(|e| format!("`{}` cannot be listed ({e})", d.display()))?;
        for entry in entries {
            let p = entry
                .map_err(|e| format!("`{}` cannot be listed ({e})", d.display()))?
                .path();
            let shown = p.strip_prefix(root).unwrap_or(&p).display().to_string();
            gate(root, &p, &shown)?;
            let meta = std::fs::symlink_metadata(&p)
                .map_err(|_| format!("`{shown}` is not a readable path inside the repository"))?;
            if meta.is_dir() {
                stack.push(p);
            } else if markdown_file_admission(
                meta.is_file(),
                p.extension().is_some_and(|e| e.eq_ignore_ascii_case("md")),
            )
            .map_err(|why| format!("`{shown}`: {why}"))?
            {
                out.push(p);
            }
        }
    }
    out.sort();
    Ok(out)
}

pub fn plan_docs() -> Result<Vec<PathBuf>, String> {
    let root = repo_root()?;
    let dir = root.join("docs/plan");
    md_files_under(&root, &dir)
}

/// Expand a trailing-component `*` by reading the directory. Bounded, and it
/// cannot execute anything; an unmatched pattern yields nothing, which the
/// caller reports rather than treating as an empty-and-green zero.
pub fn expand(root: &Path, glob: &str) -> Result<Vec<PathBuf>, String> {
    // Literal dot components, including parent traversal, are outside the corpus.
    if glob.split('/').any(|c| c.starts_with('.')) {
        return Err(format!(
            "`{glob}` has a path component beginning with `.`; the corpus is the \
             live working tree with dot-component paths excluded"
        ));
    }
    let (dir, file) = match glob.rfind('/') {
        Some(slash) => (&glob[..slash], &glob[slash + 1..]),
        None => (".", glob),
    };
    // The existing gate precedes regular-file admission and content reads.
    let Some((prefix, suffix)) = file.split_once('*') else {
        // This additional regular-file probe follows the existing gate.
        let p = root.join(glob);
        gate(root, &p, glob)?;
        if !p.is_file() {
            return Err(format!(
                "`{glob}` is not a readable path inside the repository"
            ));
        }
        return Ok(vec![p]);
    };
    let base = root.join(dir);
    gate(root, &base, glob)?;
    let entries = std::fs::read_dir(&base)
        .map_err(|e| format!("`{}` cannot be listed ({e})", base.display()))?;
    let mut hits = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("`{}` cannot be listed ({e})", base.display()))?;
        let n = entry.file_name().to_string_lossy().to_string();
        // A shell `*` does not match a leading dot; matching one here would
        // silently disagree with the command this replaces.
        if n.starts_with('.') || !n.starts_with(prefix) || !n.ends_with(suffix) {
            continue;
        }
        if n.len() < prefix.len() + suffix.len() {
            continue;
        }
        // Refused rather than skipped: the walk errors on an out-of-repo
        // entry, and a glob silently excluding one had the two paths answering
        // the same question differently.
        let p = entry.path();
        gate(root, &p, glob)?;
        if p.is_file() {
            hits.push(p);
        }
    }
    hits.sort();
    Ok(hits)
}

/// Nondirectory entry decision, isolated from filesystem operations.
pub fn markdown_file_admission(
    is_regular_file: bool,
    has_markdown_extension: bool,
) -> Result<bool, &'static str> {
    if !has_markdown_extension {
        Ok(false)
    } else if is_regular_file {
        Ok(true)
    } else {
        Err("unsupported Markdown document kind: expected a regular file")
    }
}
