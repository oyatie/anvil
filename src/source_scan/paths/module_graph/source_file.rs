use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub(super) fn child_module_dir(parent: &Path) -> PathBuf {
    let directory = parent.parent().unwrap_or(Path::new("."));
    match parent.file_stem().and_then(|stem| stem.to_str()) {
        // Crate roots are handled explicitly by their callers. Only `mod.rs`
        // intrinsically stores child modules beside itself; an external module
        // redirected to a file named `lib.rs` or `main.rs` still owns a
        // same-stem child directory.
        Some("mod") => directory.to_path_buf(),
        Some(stem) => directory.join(stem),
        None => directory.to_path_buf(),
    }
}

pub(super) fn existing_file(path: &Path) -> Result<bool, String> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("cannot inspect source {}: {error}", path.display())),
    }
}

pub(super) fn read_parsed(path: &Path) -> Result<(String, syn::File), String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read production source {}: {error}", path.display()))?;
    let source = String::from_utf8(bytes)
        .map_err(|error| format!("production source {} is not UTF-8: {error}", path.display()))?;
    let parsed = syn::parse_file(&source)
        .map_err(|error| format!("cannot parse production source {}: {error}", path.display()))?;
    Ok((source, parsed))
}
