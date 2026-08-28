//! Reading a file the caller may not have named.

/// The bytes at `path`, or `None` because no path was given.
///
/// Two CLI arms take an optional path and want its contents or nothing.
/// `Option::map` cannot hold an `await`, so each had spelled the match by hand
/// -- and both had reached for `std::fs` inside an async fn, parking a worker
/// thread on every invocation that named a baseline or a policy.
///
/// The `Result` is outside the `Option` deliberately: "no path was given" and
/// "the path was given and could not be read" are different answers, and a
/// caller that collapses them silently accepts an unreadable file as an absent
/// one.
pub async fn read_opt(path: Option<&std::path::PathBuf>) -> std::io::Result<Option<Vec<u8>>> {
    match path {
        Some(p) => tokio::fs::read(p).await.map(Some),
        None => Ok(None),
    }
}
