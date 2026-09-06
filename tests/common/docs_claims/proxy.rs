//! A bounded lexical proxy over regular helper sources and one consumer.
//! External module mappings, includes, generated source and semantic Rust
//! reachability are not resolved. This inventory is not execution authority.

use std::path::{Path, PathBuf};

/// The needles, assembled from fragments so the list cannot match itself.
pub fn forbidden_needles() -> Vec<String> {
    vec![
        format!("{}::{}", "Command", "new"),
        format!("{}::{}", "process", "Command"),
        format!("{}::{}", "std", "process"),
        format!("{}::{}", "libc", "system"),
        format!("{}::{}", "std", "net"),
        format!("{}::{}", "fs", "write"),
        format!("{}::{}", "File", "create"),
        // `File::options()` returns an OpenOptions without ever naming the type,
        // so the `OpenOptions` needle below does not see it.
        format!("{}::{}", "File", "options"),
        format!("{}{}", "Open", "Options"),
        format!("{}::{}", "fs", "remove"),
        format!("{}::{}", "fs", "rename"),
        format!("{}::{}", "fs", "copy"),
        format!("{}::{}", "fs", "create_dir"),
        format!("{}::{}", "fs", "hard_link"),
        format!("{} \"{}\"", "extern", "C"),
        format!("{}!", "include"),
        format!("#[{}", "path"),
    ]
}

fn normalize(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '{' && *c != '}')
        .collect::<String>()
        .replace("r#", "")
}

/// Compare equally normalized source and needles; return original rule names.
pub fn forbidden_hits(source: &str) -> Vec<String> {
    let body = normalize(source);
    forbidden_needles()
        .into_iter()
        .filter(|needle| body.contains(&normalize(needle)))
        .collect()
}

/// Sorted, unique regular `.rs` files under the helper tree plus its consumer.
/// Reject source symlinks and unsupported matched kinds rather than certify an
/// incomplete inventory. Read/enumeration errors propagate to the assertions.
pub fn proxy_sources(root: &Path) -> Result<Vec<PathBuf>, String> {
    let tests = root.join("tests");
    require_directory(&tests)?;
    let consumer = tests.join("published_commands_reproduce_test.rs");
    if !metadata(&consumer)?.is_file() {
        return Err(format!(
            "{}: expected a regular source file",
            consumer.display()
        ));
    }
    let mut out = vec![consumer];
    let mut stack = vec![tests.join("common")];
    while let Some(dir) = stack.pop() {
        require_directory(&dir)?;
        let entries = std::fs::read_dir(&dir)
            .map_err(|e| format!("{}: cannot be listed ({e})", dir.display()))?;
        for entry in entries {
            let path = entry
                .map_err(|e| format!("{}: cannot be listed ({e})", dir.display()))?
                .path();
            let meta = metadata(&path)?;
            if meta.file_type().is_symlink() {
                return Err(format!(
                    "{}: source symlinks are unsupported",
                    path.display()
                ));
            }
            if meta.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                if !meta.is_file() {
                    return Err(format!(
                        "{}: expected a regular source file",
                        path.display()
                    ));
                }
                out.push(path);
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn metadata(path: &Path) -> Result<std::fs::Metadata, String> {
    std::fs::symlink_metadata(path)
        .map_err(|e| format!("{}: cannot inspect source entry ({e})", path.display()))
}

fn require_directory(path: &Path) -> Result<(), String> {
    if metadata(path)?.is_dir() {
        Ok(())
    } else {
        Err(format!(
            "{}: expected a source directory, not a link or other kind",
            path.display()
        ))
    }
}
