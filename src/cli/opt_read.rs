//! Reading a file the caller may not have named.

/// The bytes at `path`, or `None` because no path was given.
///
/// The `Result` sits outside the `Option`: an unreadable file is not an absent
/// one, and a caller that collapses them accepts the first as the second.
pub async fn read_opt(path: Option<&std::path::PathBuf>) -> std::io::Result<Option<Vec<u8>>> {
    match path {
        Some(p) => tokio::fs::read(p).await.map(Some),
        None => Ok(None),
    }
}
