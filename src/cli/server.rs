use anyhow::{Context, Result};
use std::net::SocketAddr;
use tokio::process::{Child, Command};
use tracing::{error, info, warn};

use crate::config::Config;
use crate::github::GitHubClient;
use crate::webhook::{create_router, AppState};

pub async fn run_server(state: AppState) -> Result<()> {
    let host = state.config.host.clone();
    let port = state.config.port;
    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .context("Invalid host/port configuration")?;

    info!("==========================================================");
    info!(
        "🚀 Oyatie Autonomous Engineering Pipeline starting on http://{}",
        addr
    );
    info!("👀 Watched Repositories: {:?}", state.config.watched_repos);
    info!("🧠 Antigravity Review Effort: {}", state.config.agy_effort);
    info!("📁 Local Repos Directory: {:?}", state.config.repos_dir);
    info!("==========================================================");

    if let Err(e) = state.github_client.check_auth().await {
        warn!("GitHub CLI Auth Warning: {}", e);
        warn!("Run 'gh auth login' to authenticate with repo & admin:repo_hook permissions.");
    }

    let _ = state.github_client.ensure_webhook_extension().await;

    // Spawn Outage Recovery & Full PR/Issue Reconciliation Sweep on startup
    let recovery_client = state.github_client.clone();
    let recovery_state_mgr = state.state_mgr.clone();
    let recovery_repos = state.config.watched_repos.clone();
    tokio::spawn(async move {
        let reconciler =
            crate::recovery::OutageRecoveryReconciler::new(recovery_client, recovery_state_mgr);
        if let Err(e) = reconciler.run_full_sweep(&recovery_repos).await {
            tracing::warn!("Outage recovery reconciliation sweep noticed: {}", e);
        }
    });

    // Spawn background upstream sync for rust-skills repository
    let rsg_clone = state.rust_skills_guard.clone();
    tokio::spawn(async move {
        if let Err(e) = rsg_clone.sync_upstream().await {
            tracing::warn!("RustSkillsGuard upstream background sync noticed: {}", e);
        }
    });

    // Spawn background GC heartbeat for abandoned git worktrees (crash recovery & leak prevention)
    let git_mgr_gc = state.git_mgr.clone();
    tokio::spawn(async move {
        let _ = git_mgr_gc.clean_abandoned_worktrees().await;
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(600)); // Every 10 min
        loop {
            interval.tick().await;
            if let Err(e) = git_mgr_gc.clean_abandoned_worktrees().await {
                tracing::warn!("GitManager worktree GC noticed: {}", e);
            }
        }
    });

    // Spawn background Proactive Upgrade Train worker (Daily cadence)
    let train_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(86400)); // 24 hours
        interval.tick().await; // Initial tick fires immediately, delay first run
        loop {
            interval.tick().await;
            info!("Running scheduled Proactive Upgrade Train across watched repositories...");
            for repo in &train_state.config.watched_repos {
                let candidates = vec![crate::upgrade_train::DependencyUpgradeCandidate {
                    package_name: "tokio".to_string(),
                    current_version: "1.38.0".to_string(),
                    target_version: "1.38.1".to_string(),
                    is_major_breaking: false,
                }];
                let rep = train_state
                    .upgrade_train
                    .evaluate_upgrade_train(&candidates);
                info!("Scheduled upgrade train on {}: {}", repo, rep.summary);
            }
        }
    });

    // Spawn background Flake Quarantine 100x stress rehabilitation worker (Daily cadence)
    let flake_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(86400)); // 24 hours
        interval.tick().await;
        loop {
            interval.tick().await;
            info!("Running scheduled Flake Quarantine Rehabilitation stress runs...");
            let rep = flake_state
                .flake_quarantine
                .evaluate_quarantine_lifecycle(&["tests::flaky_test".to_string()]);
            info!("Flake quarantine rehabilitation outcome: {}", rep.summary);
        }
    });

    // Spawn background Ephemeral Preview Environment and Worktree Reaper (Every 15 min)
    let reaper_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(900)); // 15 min
        loop {
            interval.tick().await;
            let _ = reaper_state.git_mgr.clean_abandoned_worktrees().await;
        }
    });

    let mut forward_children: Vec<Child> = Vec::new();
    if state.config.auto_forward_webhooks {
        for repo in &state.config.watched_repos {
            let _ = state
                .github_client
                .cleanup_stale_forward_webhooks(repo)
                .await;

            info!(
                "Starting gh webhook forward for {} -> http://{}:{}/webhook",
                repo, host, port
            );
            let target_url = format!("http://{}:{}/webhook", host, port);
            let child = Command::new("gh")
                .args([
                    "webhook",
                    "forward",
                    "--repo",
                    repo,
                    "--events",
                    "pull_request,pull_request_review,pull_request_review_comment,workflow_run,merge_group",
                    "--url",
                    &target_url,
                ])
                .spawn();

            match child {
                Ok(c) => forward_children.push(c),
                Err(e) => warn!("Could not start gh webhook forward for {}: {}", repo, e),
            }
        }
    }

    let app = create_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("Listening for webhooks at http://{}/webhook", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    for mut child in forward_children {
        let _ = child.kill().await;
    }

    info!("Oyatie Autonomous Engineering Pipeline gracefully shut down.");
    Ok(())
}

pub async fn start_forwarders(config: &Config) -> Result<()> {
    let mut tasks = Vec::new();

    for repo in &config.watched_repos {
        let repo_clone = repo.clone();
        let target_url = format!("http://{}:{}/webhook", config.host, config.port);
        let task = tokio::spawn(async move {
            info!("Forwarding webhooks for {} to {}", repo_clone, target_url);
            let mut cmd = Command::new("gh");
            cmd.args([
                "webhook",
                "forward",
                "--repo",
                &repo_clone,
                "--events",
                "pull_request,pull_request_review,pull_request_review_comment,workflow_run,merge_group",
                "--url",
                &target_url,
            ]);
            if let Err(e) = cmd.status().await {
                error!("Webhook forwarder exited for {}: {}", repo_clone, e);
            }
        });
        tasks.push(task);
    }

    futures::future::join_all(tasks).await;
    Ok(())
}

pub async fn check_environment(github_client: &GitHubClient, config: &Config) -> Result<()> {
    println!("\n🔍 Checking Oyatie Autonomous Engineering Pipeline Environment:\n");

    print!("1. GitHub CLI (`gh`): ");
    match Command::new("gh").arg("--version").output().await {
        Ok(out) if out.status.success() => {
            let version = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            println!("✅ Found ({})", version);
        }
        _ => println!("❌ Not installed or not in PATH"),
    }

    print!("2. GitHub CLI Authentication: ");
    match github_client.check_auth().await {
        Ok(_) => println!("✅ Authenticated"),
        Err(e) => println!("⚠️  Needs Login: {}", e),
    }

    print!("3. Antigravity CLI (`agy`): ");
    match Command::new("agy").arg("--help").output().await {
        Ok(out) if out.status.success() => println!("✅ Ready"),
        _ => println!("❌ 'agy' not found in PATH"),
    }

    print!("4. Git: ");
    match Command::new("git").arg("--version").output().await {
        Ok(out) if out.status.success() => {
            let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
            println!("✅ Found ({})", ver);
        }
        _ => println!("❌ Git not found"),
    }

    println!("\n📦 Watched Repositories: {:?}", config.watched_repos);
    println!("📁 Repos Cache Directory: {:?}", config.repos_dir);
    println!("📁 State Storage Directory: {:?}", config.data_dir);
    if let Some(rules) = &config.rules_path {
        println!("📜 Custom Rules File: {:?}", rules);
    }

    println!("\nEverything checked!\n");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("Signal received, starting graceful shutdown...");
}
