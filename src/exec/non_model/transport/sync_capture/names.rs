// Names are internally unique, not secret and not an authorization mechanism.

use std::io;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PIPE: AtomicU64 = AtomicU64::new(0);

pub(super) enum Stream {
    Stdout,
    Stderr,
}

pub(super) fn next_name(stream: Stream) -> io::Result<String> {
    let serial = next_serial(&NEXT_PIPE)?;
    let role = match stream {
        Stream::Stdout => "stdout",
        Stream::Stderr => "stderr",
    };
    Ok(format!(
        r"\\.\pipe\anvil-sync-{}-{serial}-{role}",
        std::process::id()
    ))
}

fn next_serial(counter: &AtomicU64) -> io::Result<u64> {
    // `try_update`, not `fetch_update`: std renamed it. Nightly warns and CI's
    // `clippy -D warnings` makes that fatal, which is the pin doing its job --
    // one line now instead of a red trunk the day stable ships the rename.
    counter
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| io::Error::other("synchronous capture pipe names exhausted"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_exhaustion_fails_without_wrapping_or_reusing_a_serial() {
        let counter = AtomicU64::new(u64::MAX - 1);
        assert_eq!(
            next_serial(&counter).expect("last available serial"),
            u64::MAX - 1
        );
        assert!(next_serial(&counter).is_err());
        assert!(next_serial(&counter).is_err());
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn internal_stream_names_are_distinct_and_keep_their_fixed_namespace() {
        let stdout = next_name(Stream::Stdout).expect("stdout name");
        let stderr = next_name(Stream::Stderr).expect("stderr name");
        assert_ne!(stdout, stderr);
        for name in [&stdout, &stderr] {
            assert!(name.starts_with(r"\\.\pipe\anvil-sync-"));
            assert!(name.contains(&format!("-{}-", std::process::id())));
        }
        assert!(stdout.ends_with("-stdout"));
        assert!(stderr.ends_with("-stderr"));
    }
}
