//! Managed clone identity, not authorization or remote-server attestation.

use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RepoIdentity {
    owner: String,
    name: String,
}

impl RepoIdentity {
    pub(super) fn parse(repo: &str) -> Result<Self> {
        let parts = repo.split('/').collect::<Vec<_>>();
        let valid = |part: &str| {
            !part.is_empty()
                && part.len() <= 100
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        };
        let [owner, name] = parts.as_slice() else {
            bail!("managed repository requires a valid owner/name identity");
        };
        if !valid(owner) || !valid(name) {
            bail!("managed repository requires a valid owner/name identity");
        }
        Ok(Self {
            owner: owner.to_ascii_lowercase(),
            name: name.to_ascii_lowercase(),
        })
    }

    pub(super) fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    pub(super) fn key(&self) -> String {
        format!("github!{}!{}!", self.owner, self.name)
    }

    pub(super) fn from_key(key: &str) -> Option<Self> {
        let framed = key.strip_prefix("github!")?.strip_suffix('!')?;
        let (owner, name) = framed.split_once('!')?;
        let identity = Self::parse(&format!("{owner}/{name}")).ok()?;
        (identity.key() == key).then_some(identity)
    }

    pub(super) fn path(&self, base: &Path) -> PathBuf {
        base.join(self.key())
    }

    pub(super) fn legacy_name_matches(&self, entry: &std::ffi::OsStr) -> bool {
        entry
            .to_str()
            .is_some_and(|name| name.eq_ignore_ascii_case(&self.name))
    }

    pub(super) fn clone_url(&self) -> String {
        format!("https://github.com/{}.git", self.slug())
    }

    pub(super) fn require_origin(&self, bytes: &[u8]) -> Result<()> {
        let url = one_record(bytes)?;
        if url.chars().any(char::is_whitespace) {
            bail!("managed origin is outside the supported URL profile");
        }
        let suffix = [
            "https://github.com/",
            "git@github.com:",
            "ssh://git@github.com/",
        ]
        .into_iter()
        .find_map(|prefix| url.strip_prefix(prefix));
        let Some(suffix) = suffix else {
            bail!("managed origin is outside the supported URL profile");
        };
        let slug = self.slug();
        // Otherwise one URL could admit both `name` and literal `name.git`.
        let matches = suffix.eq_ignore_ascii_case(&format!("{slug}.git"))
            || (!self.name.ends_with(".git") && suffix.eq_ignore_ascii_case(&slug));
        if !matches {
            bail!("managed origin does not match the requested repository");
        }
        Ok(())
    }
}

/// Exactly one bounded textual observation; never lossy decoding or trimming.
pub(super) fn one_record(bytes: &[u8]) -> Result<&str> {
    if bytes.len() > 4096 {
        bail!("managed repository observation exceeds the field bound");
    }
    let bytes = if let Some(line) = bytes.strip_suffix(b"\n") {
        line.strip_suffix(b"\r").unwrap_or(line)
    } else {
        bytes
    };
    let text = std::str::from_utf8(bytes)
        .map_err(|_| anyhow::anyhow!("managed repository observation is not UTF-8"))?;
    if text.is_empty() || text.chars().any(char::is_control) {
        bail!("managed repository observation is not one nonempty record");
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_manager::GitManager;

    fn identity() -> RepoIdentity {
        RepoIdentity::parse("first/shared").unwrap()
    }
    fn base() -> PathBuf {
        PathBuf::from("/ordinary/repos")
    }

    #[test]
    fn different_owners_do_not_share_a_managed_clone_path() {
        let manager = GitManager::new(base());
        assert_ne!(
            manager.get_repo_dir("first/shared").unwrap(),
            manager.get_repo_dir("second/shared").unwrap()
        );
    }

    #[test]
    fn invalid_names_are_errors_not_shared_sanitized_paths() {
        let manager = GitManager::new(base());
        for name in [
            "x/..",
            "../etc",
            "x/../..",
            "a/.",
            "owner/..%2f..",
            "",
            "a/b/c",
            "a/",
            "/b",
            "a/b\n",
        ] {
            assert!(manager.get_repo_dir(name).is_err(), "{name:?}");
        }
    }

    #[test]
    fn normal_names_have_canonical_owner_qualified_paths() {
        let manager = GitManager::new(base());
        assert_eq!(
            manager.get_repo_dir("oyatie/anvil").unwrap(),
            base().join("github!oyatie!anvil!")
        );
        assert_eq!(
            manager.get_repo_dir("FIRST/Shared").unwrap(),
            identity().path(&base())
        );
        assert_ne!(
            manager.get_repo_dir("a-b/c").unwrap(),
            manager.get_repo_dir("a/b-c").unwrap()
        );
        let longest =
            RepoIdentity::parse(&format!("{}/{}", "a".repeat(100), "b".repeat(100))).unwrap();
        assert_eq!(longest.key().len(), 209);
        assert!(RepoIdentity::parse(&format!("{}/b", "a".repeat(101))).is_err());
        assert_ne!(
            manager.get_repo_dir("a/b.").unwrap(),
            manager.get_repo_dir("a/b").unwrap()
        );
    }

    #[test]
    fn only_exact_new_keys_can_be_gc_candidates() {
        assert_eq!(RepoIdentity::from_key(&identity().key()), Some(identity()));
        for key in [
            "shared",
            ".worktrees",
            "github!FIRST!shared!",
            "github!first!shared",
            "github!a!b!c!",
            "github!a!..!",
        ] {
            assert!(RepoIdentity::from_key(key).is_none(), "{key}");
        }
        assert!(identity().legacy_name_matches(std::ffi::OsStr::new("SHARED")));
        assert!(!identity().legacy_name_matches(std::ffi::OsStr::new(&identity().key())));
    }

    #[test]
    fn finite_url_forms_bind_both_complete_segments() {
        for prefix in [
            "https://github.com/",
            "git@github.com:",
            "ssh://git@github.com/",
        ] {
            for suffix in ["first/shared", "FIRST/SHARED.git"] {
                identity()
                    .require_origin(format!("{prefix}{suffix}\n").as_bytes())
                    .unwrap();
            }
        }
        for url in [
            "https://github.com/other/shared.git",
            "https://github.com/first/other.git",
            "https://github.com.evil/first/shared.git",
            "https://user@github.com/first/shared.git",
            "https://github.com:443/first/shared.git",
            "ssh://other@github.com/first/shared.git",
            "file:///first/shared",
            "https://github.com/first/shared.git/",
            "https://github.com/first/shared?x",
            "https://github.com/first/shared#x",
            "https://github.com/first/%73hared",
            "https://github.com/first/shared/extra",
        ] {
            assert!(identity().require_origin(url.as_bytes()).is_err(), "{url}");
        }
    }

    #[test]
    fn literal_git_suffix_does_not_alias_the_other_repository() {
        let ordinary = RepoIdentity::parse("a/name").unwrap();
        let literal = RepoIdentity::parse("a/name.git").unwrap();
        ordinary
            .require_origin(b"https://github.com/a/name.git")
            .unwrap();
        assert!(
            literal
                .require_origin(b"https://github.com/a/name.git")
                .is_err()
        );
        literal
            .require_origin(b"https://github.com/a/name.git.git")
            .unwrap();
        assert!(
            ordinary
                .require_origin(b"https://github.com/a/name.git.git")
                .is_err()
        );
    }

    #[test]
    fn malformed_and_multiple_observations_never_collapse_to_one() {
        for bytes in [
            Vec::new(),
            vec![0xff],
            b"\n".to_vec(),
            b"https://github.com/first/shared\nhttps://github.com/first/shared\n".to_vec(),
            b"https://github.com/first/shared\t".to_vec(),
            b"https://github.com/first/shared\r".to_vec(),
            b" https://github.com/first/shared".to_vec(),
            vec![b'x'; 4097],
        ] {
            assert!(identity().require_origin(&bytes).is_err());
        }
        identity()
            .require_origin(b"https://github.com/first/shared.git\r\n")
            .unwrap();
    }
}
