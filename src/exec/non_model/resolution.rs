use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use super::ResolvedExecutable;

pub(super) fn resolve_executable(command: &std::process::Command) -> Option<ResolvedExecutable> {
    resolve_canonical_executable(command).map(|canonical| ResolvedExecutable {
        canonical,
        requested_name: String::new(),
    })
}

/// Resolves one requested executable to the regular executable the operating
/// system would launch. This exposes no execution capability; the provider
/// seam uses it to bind a finite provider name before applying child-owned
/// environment values.
pub(in crate::exec) fn resolve_canonical_executable(
    command: &std::process::Command,
) -> Option<PathBuf> {
    let program = Path::new(command.get_program());
    if program.components().count() > 1 {
        let candidate = if program.is_absolute() {
            program.to_path_buf()
        } else {
            command
                .get_current_dir()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(program)
        };
        return runnable_canonical(&candidate);
    }

    let explicit_path = command.get_envs().find_map(|(name, value)| {
        (name == OsStr::new("PATH")).then(|| value.map(OsStr::to_os_string))?
    });
    let search_path: OsString = explicit_path.or_else(|| std::env::var_os("PATH"))?;
    for dir in std::env::split_paths(&search_path) {
        let effective_dir = if dir.is_absolute() {
            dir
        } else {
            command
                .get_current_dir()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(dir)
        };
        let Some(canonical) = runnable_in(&effective_dir, program) else {
            continue;
        };
        return Some(canonical);
    }
    None
}

fn runnable_in(directory: &Path, program: &Path) -> Option<PathBuf> {
    let candidate = directory.join(program);
    if let Some(canonical) = runnable_canonical(&candidate) {
        return Some(canonical);
    }
    #[cfg(windows)]
    if program.extension().is_none() {
        for extension in ["exe", "cmd"] {
            let candidate = directory.join(program).with_extension(extension);
            if let Some(canonical) = runnable_canonical(&candidate) {
                return Some(canonical);
            }
        }
    }
    None
}

fn runnable_canonical(candidate: &Path) -> Option<PathBuf> {
    let canonical = std::fs::canonicalize(candidate).ok()?;
    let metadata = std::fs::metadata(&canonical).ok()?;
    if !metadata.is_file() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }
    Some(canonical)
}
