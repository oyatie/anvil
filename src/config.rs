use crate::ai_driver::{ModelExecutionConfig, ModelProvider};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub watched_repos: Vec<String>,
    pub repos_dir: PathBuf,
    pub data_dir: PathBuf,
    pub rules_path: Option<PathBuf>,
    pub agy_effort: String,
    pub auto_forward_webhooks: bool,
    pub ai_provider: ModelProvider,
    pub specific_model: Option<String>,
    pub webhook_secret: Option<String>,
    /// Previous webhook secret, honoured during a rotation window.
    ///
    /// GitHub signs a delivery with whichever secret the hook held when the
    /// delivery was created. During rotation, in-flight deliveries still carry
    /// the old signature, so verifying against only the new secret drops them.
    pub webhook_secret_previous: Option<String>,
    /// The repository that holds this daemon's own source (`SELF_REPO`).
    ///
    /// Anvil is a managed repository like any other, with one difference: a
    /// shape rule is enabled in block mode here before it is enabled anywhere
    /// else, and the daemon must never mutate the tree it is running from.
    pub self_repo: String,
}

/// Refuses a managed clone that is, or contains, or is the same git
/// repository as, the tree the daemon is running from.
///
/// Every write path (fixer, queue healer, change delivery) mutates
/// `repos_dir/<name>` and pushes it. If that path resolves to the daemon's own
/// checkout, Anvil edits its running source under itself. Today the two differ
/// only by accident of layout: `repos/` is gitignored and each clone carries
/// its own `.git`. This makes the separation a boot invariant.
///
/// `clone_toplevel` is `git rev-parse --show-toplevel` inside the clone
/// (`None` when the clone is not a git repository, e.g. not yet cloned).
/// `daemon_toplevel` is the same for the daemon's working directory. All paths
/// are expected canonical.
pub fn managed_clone_overlaps_daemon_tree(
    clone_path: &Path,
    clone_toplevel: Option<&Path>,
    daemon_toplevel: &Path,
) -> Result<(), String> {
    if clone_path == daemon_toplevel {
        return Err(format!(
            "managed clone {} is the daemon's own source tree",
            clone_path.display()
        ));
    }
    if daemon_toplevel.starts_with(clone_path) {
        return Err(format!(
            "the daemon runs inside managed clone {} (daemon tree {})",
            clone_path.display(),
            daemon_toplevel.display()
        ));
    }
    if let Some(top) = clone_toplevel
        && top == daemon_toplevel
    {
        return Err(format!(
            "managed clone {} belongs to the daemon's own git repository {}",
            clone_path.display(),
            daemon_toplevel.display()
        ));
    }
    Ok(())
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000);

        let watched_repos_str = std::env::var("WATCHED_REPOS")
            .unwrap_or_else(|_| "oyatie/oyatie,oyatie/console,oyatie/anvil".to_string());
        let watched_repos = watched_repos_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let repos_dir = std::env::var("REPOS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| current_dir.join("repos"));
        let data_dir = std::env::var("DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| current_dir.join("data"));

        let rules_path = std::env::var("RULES_PATH")
            .map(PathBuf::from)
            .ok()
            .or_else(|| {
                let default_rules = current_dir.join("rules.md");
                if default_rules.exists() {
                    Some(default_rules)
                } else {
                    None
                }
            });

        let agy_effort = std::env::var("AGY_EFFORT").unwrap_or_else(|_| "high".to_string());
        let auto_forward_webhooks = std::env::var("AUTO_FORWARD_WEBHOOKS")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(true);

        let ai_provider_str = std::env::var("AI_PROVIDER").unwrap_or_else(|_| "agy".to_string());
        let ai_provider = ModelProvider::from_str_name(&ai_provider_str);
        let specific_model = std::env::var("AI_MODEL").ok();
        let webhook_secret = std::env::var("GITHUB_WEBHOOK_SECRET")
            .ok()
            .filter(|s| !s.is_empty());
        let webhook_secret_previous = std::env::var("GITHUB_WEBHOOK_SECRET_PREVIOUS")
            .ok()
            .filter(|s| !s.is_empty());
        let self_repo = std::env::var("SELF_REPO").unwrap_or_else(|_| "oyatie/anvil".to_string());

        Self {
            host,
            port,
            watched_repos,
            repos_dir,
            data_dir,
            rules_path,
            agy_effort,
            auto_forward_webhooks,
            ai_provider,
            specific_model,
            webhook_secret,
            webhook_secret_previous,
            self_repo,
        }
    }

    /// Boot invariant: no managed clone may be the daemon's own source tree.
    ///
    /// Fails closed — a daemon that cannot establish the separation does not
    /// serve. Clones that do not exist yet are checked by path only.
    pub async fn assert_managed_clones_are_not_this_tree(&self) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let daemon_toplevel = git_toplevel(&cwd)
            .await
            .unwrap_or_else(|| cwd.canonicalize().unwrap_or(cwd.clone()));
        let git_mgr = crate::git_manager::GitManager::new(self.repos_dir.clone());
        for repo in &self.watched_repos {
            let clone = git_mgr.get_repo_dir(repo);
            let clone_canonical = clone.canonicalize().unwrap_or_else(|_| clone.clone());
            let clone_toplevel = if clone.is_dir() {
                git_toplevel(&clone).await
            } else {
                None
            };
            managed_clone_overlaps_daemon_tree(
                &clone_canonical,
                clone_toplevel.as_deref(),
                &daemon_toplevel,
            )
            .map_err(|why| {
                anyhow::anyhow!(
                    "refusing to start: {} (watched repo {}). Set REPOS_DIR to a directory \
                     outside this checkout.",
                    why,
                    repo
                )
            })?;
        }
        Ok(())
    }

    pub fn to_model_config(&self) -> ModelExecutionConfig {
        ModelExecutionConfig {
            provider: self.ai_provider.clone(),
            specific_model: self.specific_model.clone(),
            reasoning_effort: self.agy_effort.clone(),
            print_timeout_secs: 300,
        }
    }
}

/// `git rev-parse --show-toplevel` for `dir`, canonicalised; `None` when
/// `dir` is not inside a git work tree or git cannot be run.
async fn git_toplevel(dir: &Path) -> Option<PathBuf> {
    let mut cmd = tokio::process::Command::new("git");
    cmd.current_dir(dir).args(["rev-parse", "--show-toplevel"]);
    let out = crate::exec::run_bounded(
        cmd,
        crate::exec::ExecClass::Quick,
        "git rev-parse --show-toplevel",
    )
    .await
    .ok()?;
    if !out.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }
    let p = PathBuf::from(raw);
    Some(p.canonicalize().unwrap_or(p))
}
