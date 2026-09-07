//! Admission-time custody under trusted, stable host/Git configuration.

use super::repository_identity::{RepoIdentity, one_record};
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};
use tokio::process::Command;

#[derive(Clone, Copy)]
enum Location {
    Existing,
    Vacant,
    Legacy,
}

struct Observation {
    base: PathBuf,
    target: PathBuf,
    top: PathBuf,
    common: PathBuf,
    primary: bool,
    fetch: Vec<u8>,
    push: Vec<u8>,
    helper_override: bool,
}

fn validate(identity: &RepoIdentity, observed: &Observation) -> Result<()> {
    if !observed.primary
        || observed.target != identity.path(&observed.base)
        || observed.top != observed.target
        || observed.common != observed.target.join(".git")
    {
        bail!("managed repository is not the expected standalone primary checkout");
    }
    if observed.helper_override {
        bail!("managed origin has an unsupported transport helper override");
    }
    identity.require_origin(&observed.fetch)?;
    identity.require_origin(&observed.push)?;
    Ok(())
}

#[async_trait::async_trait]
trait CheckoutIo: Sync {
    async fn location(&self, base: &Path, identity: &RepoIdentity) -> Result<Location>;
    async fn prepare(&self, base: &Path) -> Result<()>;
    async fn clone_repo(&self, identity: &RepoIdentity, path: &Path) -> Result<()>;
    async fn observe(&self, base: &Path, path: &Path) -> Result<Observation>;
    async fn refresh(&self, path: &Path);
    async fn hooks(&self, path: &Path);
}

async fn acquire_with(
    io: &impl CheckoutIo,
    base: &Path,
    identity: &RepoIdentity,
) -> Result<PathBuf> {
    let path = identity.path(base);
    match io.location(base, identity).await? {
        Location::Legacy => {
            bail!("MigrationRequired: legacy basename checkout requires an operator decision")
        }
        Location::Vacant => {
            io.prepare(base).await?;
            io.clone_repo(identity, &path).await?;
            validate(identity, &io.observe(base, &path).await?)?;
        }
        Location::Existing => {
            validate(identity, &io.observe(base, &path).await?)?;
            io.prepare(base).await?;
            io.refresh(&path).await;
        }
    }
    io.hooks(&path).await;
    Ok(path)
}

pub(super) async fn acquire(base: &Path, repo: &str) -> Result<PathBuf> {
    let identity = RepoIdentity::parse(repo)?;
    acquire_with(&Host, base, &identity)
        .await
        .map_err(|error| anyhow::anyhow!("managed repository {}: {error}", identity.slug()))
}

pub(super) async fn validate_existing(base: &Path, identity: &RepoIdentity) -> Result<()> {
    validate(identity, &Host.observe(base, &identity.path(base)).await?)
}

struct Host;

fn stage_error(stage: &'static str) -> anyhow::Error {
    anyhow::anyhow!("managed repository {stage} failed")
}

async fn git(path: &Path, args: &[&str], stage: &'static str) -> Result<std::process::Output> {
    let mut command = Command::new("git");
    command.current_dir(path).args(args);
    crate::exec::run_bounded(command, crate::exec::ExecClass::Quick, stage)
        .await
        .map_err(|_| stage_error(stage))
}

async fn successful_git(path: &Path, args: &[&str], stage: &'static str) -> Result<Vec<u8>> {
    let output = git(path, args, stage).await?;
    if !output.status.success() {
        return Err(stage_error(stage));
    }
    Ok(output.stdout)
}

async fn canonical(path: &Path) -> Result<PathBuf> {
    tokio::fs::canonicalize(path)
        .await
        .map_err(|_| stage_error("path observation"))
}

async fn observed_path(path: &Path, args: &[&str]) -> Result<PathBuf> {
    let bytes = successful_git(path, args, "primary checkout observation").await?;
    let value = PathBuf::from(one_record(&bytes)?);
    canonical(&if value.is_absolute() {
        value
    } else {
        path.join(value)
    })
    .await
}

#[async_trait::async_trait]
impl CheckoutIo for Host {
    async fn location(&self, base: &Path, identity: &RepoIdentity) -> Result<Location> {
        match tokio::fs::symlink_metadata(identity.path(base)).await {
            Ok(_) => return Ok(Location::Existing),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(stage_error("target observation")),
        }
        let mut entries = match tokio::fs::read_dir(base).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Location::Vacant);
            }
            Err(_) => return Err(stage_error("legacy directory observation")),
        };
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|_| stage_error("legacy entry observation"))?
        {
            if identity.legacy_name_matches(&entry.file_name()) {
                return Ok(Location::Legacy);
            }
        }
        Ok(Location::Vacant)
    }

    async fn prepare(&self, base: &Path) -> Result<()> {
        tokio::fs::create_dir_all(base.join(".worktrees"))
            .await
            .map_err(|_| stage_error("managed directory preparation"))
    }

    async fn clone_repo(&self, identity: &RepoIdentity, path: &Path) -> Result<()> {
        let mut command = Command::new("git");
        command.args(["clone", &identity.clone_url()]).arg(path);
        let output = crate::exec::run_bounded(command, crate::exec::ExecClass::Vcs, "git clone")
            .await
            .map_err(|_| stage_error("clone"))?;
        if !output.status.success() {
            return Err(stage_error("clone"));
        }
        Ok(())
    }

    async fn observe(&self, base: &Path, path: &Path) -> Result<Observation> {
        let target_kind = tokio::fs::symlink_metadata(path)
            .await
            .map_err(|_| stage_error("target kind"))?
            .file_type();
        let git_kind = tokio::fs::symlink_metadata(path.join(".git"))
            .await
            .map_err(|_| stage_error("git directory kind"))?
            .file_type();
        if !target_kind.is_dir() || !git_kind.is_dir() {
            return Err(stage_error("standalone primary directory"));
        }
        let base = canonical(base).await?;
        let target = canonical(path).await?;
        if target.parent() != Some(base.as_path()) {
            return Err(stage_error("target containment"));
        }
        let top = observed_path(&target, &["rev-parse", "--show-toplevel"]).await?;
        let common = observed_path(&target, &["rev-parse", "--git-common-dir"]).await?;
        let fetch = successful_git(
            &target,
            &["remote", "get-url", "--all", "origin"],
            "fetch origin observation",
        )
        .await?;
        let push = successful_git(
            &target,
            &["remote", "get-url", "--push", "--all", "origin"],
            "push origin observation",
        )
        .await?;
        let helper = git(
            &target,
            &["config", "--get-all", "remote.origin.vcs"],
            "origin helper observation",
        )
        .await?;
        let helper_override = match helper.status.code() {
            Some(0) => true,
            Some(1) if helper.stdout.is_empty() => false,
            _ => return Err(stage_error("origin helper observation")),
        };
        Ok(Observation {
            base,
            target,
            top,
            common,
            primary: true,
            fetch,
            push,
            helper_override,
        })
    }

    async fn refresh(&self, path: &Path) {
        let mut command = Command::new("git");
        command
            .current_dir(path)
            .args(["fetch", "origin", "--prune"]);
        // Identity admission is not evidence that this best-effort refresh succeeded.
        let _ = crate::exec::run_bounded(
            command,
            crate::exec::ExecClass::Vcs,
            "git fetch origin --prune",
        )
        .await;
    }

    async fn hooks(&self, path: &Path) {
        let _ = super::GitManager::install_repo_hooks(path).await;
    }
}

#[cfg(test)]
mod tests;
