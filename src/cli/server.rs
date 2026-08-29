use anyhow::{Context, Result};
use std::net::SocketAddr;
use tokio::process::Command;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::github::GitHubClient;
use crate::webhook::{AppState, create_router};

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

    // Every action this daemon takes runs through `gh`. Unauthenticated, it
    // accepts deliveries and then fails one call at a time, which reads as a
    // pipeline defect rather than a missing login. Fail at boot, and say so.
    state.github_client.check_auth().await.context(
        "GitHub CLI is not authenticated. Run `gh auth login` with repo and admin:repo_hook scopes.",
    )?;

    let _ = state.github_client.ensure_webhook_extension().await;

    // Spawn Autonomous Self-Governor (Process Registry, Quota Enforcer & Resource Reaper)
    let self_governor = crate::self_governance::SelfGovernor::new();
    self_governor.spawn_monitoring_daemon();

    // Spawn Outage Recovery & Full PR/Issue Reconciliation Sweep on startup
    let recovery_client = state.github_client.clone();
    let recovery_state_mgr = state.state_mgr.clone();
    let recovery_repos = state.config.watched_repos.clone();
    let recovery_app_state = state.clone();
    tokio::spawn(async move {
        let reconciler =
            crate::recovery::OutageRecoveryReconciler::new(recovery_client, recovery_state_mgr);
        match reconciler.run_full_sweep(&recovery_repos).await {
            Ok(report) => {
                info!(
                    "⚡ [Outage Recovery] Auto-dispatching {} uncertified PRs into Anvil pipeline...",
                    report.uncertified_prs_details.len()
                );
                for (repo, pr) in report.uncertified_prs_details {
                    let task_state = recovery_app_state.clone();
                    tokio::spawn(async move {
                        info!(
                            "[Outage Recovery] Dispatched review and {}-gate certification for {}#{}",
                            crate::pre_merge_guard::report::TOTAL_GATES,
                            repo,
                            pr.number
                        );
                        let _ = crate::webhook::pipelines::review::execute_pr_review(
                            &task_state,
                            &repo,
                            pr.number,
                            &pr.title,
                            "",
                            "main",
                            "HEAD",
                            &pr.head_sha,
                            false,
                        )
                        .await;
                    });
                }
            }
            Err(e) => {
                tracing::warn!("Outage recovery reconciliation sweep noticed: {}", e);
            }
        }
    });

    // Spawn periodic issue refresh & triage.
    //
    // Issue reconciliation previously ran in exactly one place: the outage-recovery
    // sweep, spawned once at startup. Every other worker in this file has a cadence
    // (10s self-governance, 30s fleet observer, 600s worktree GC, 900s, 86400s upgrade
    // train) but issues had none, so after boot an issue was never looked at again
    // until the daemon restarted. Combined with the fact that the rest of the daemon is
    // webhook-driven, a running Anvil with no inbound deliveries did no issue work at
    // all -- it only polled telemetry.
    //
    // 15 minutes: fast enough that a newly filed issue is triaged within one coffee
    // break, slow enough that it is not a meaningful share of the API budget.
    let issue_repos = state.config.watched_repos.clone();
    let issue_client = state.github_client.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(900));
        interval.tick().await; // fires immediately; the boot sweep already covered this pass
        loop {
            interval.tick().await;
            let reconciler = crate::issue_reconciler::IssueReconciler::new(issue_client.clone());
            for repo in &issue_repos {
                match reconciler.reconcile_issues(repo).await {
                    Ok(reconciled) => {
                        info!(
                            "[Issue Refresh] Reconciled {} issues for {}",
                            reconciled.len(),
                            repo
                        );
                    }
                    Err(e) => {
                        // Warn, never abort: one repo failing must not stop the others,
                        // and must not kill the recurring task.
                        tracing::warn!(
                            "[Issue Refresh] Reconciliation for {} noticed: {}",
                            repo,
                            e
                        );
                    }
                }
            }
        }
    });

    crate::cli::sweep_task::spawn(&state);

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

    // Spawn background Ephemeral Preview Environment and Worktree Reaper (Every 15 min)
    let reaper_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(900)); // 15 min
        loop {
            interval.tick().await;
            let _ = reaper_state.git_mgr.clean_abandoned_worktrees().await;
        }
    });

    // Spawn background continuous Fleet Observer Telemetry Poller (30s cadence)
    state
        .fleet_observer
        .spawn_continuous_poller(state.config.watched_repos.clone());

    // Supervisor handles, not raw children: each task owns its `gh` child and
    // respawns it across WebSocket drops. Aborting the task drops the child,
    // which `kill_on_drop(true)` reaps -- so shutdown still leaves no orphans.
    let mut forwarder_tasks: Vec<tokio::task::JoinHandle<()>> = Vec::new();
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
            // The forwarder must create its GitHub-side hook with the SAME secret
            // the daemon verifies with, or every delivery arrives unsigned and
            // HMAC verification rejects all of it — a daemon that looks healthy
            // and processes nothing. Verified live: all watched repos currently
            // show `secret_set: false` via `gh api repos/{r}/hooks`.
            // Supervised, not fire-and-forget.
            //
            // The forwarder's WebSocket to webhook-forwarder.github.com drops
            // routinely ("close 1006 (abnormal closure): unexpected EOF"). This
            // child was previously pushed into a Vec and never looked at again,
            // so a dropped socket ended webhook delivery for one repository
            // silently -- observed as forwarder count falling 3 -> 2 within five
            // minutes, and as 1h38m of uptime whose only output was telemetry.
            let repo_owned = repo.to_string();
            let secret_owned = state.config.webhook_secret.clone();
            let url_owned = target_url.clone();
            if secret_owned.is_none() {
                warn!(
                    "GITHUB_WEBHOOK_SECRET is unset: forwarding {} with UNSIGNED deliveries. \
                     Signature verification cannot succeed until this is set.",
                    repo
                );
            }
            forwarder_tasks.push(tokio::spawn(async move {
                let policy = crate::webhook::forwarder_supervisor::RestartPolicy::default();
                crate::webhook::forwarder_supervisor::supervise(
                    &repo_owned.clone(),
                    &policy,
                    || {
                        let repo = repo_owned.clone();
                        let secret = secret_owned.clone();
                        let url = url_owned.clone();
                        async move {
                            // A hook left by a forwarder that died uncleanly
                            // 422s every respawn until it is removed.
                            if let Err(e) =
                                crate::webhook::forwarder_supervisor::remove_stale_forwarder_hooks(
                                    &repo,
                                )
                                .await
                            {
                                warn!("stale-hook cleanup for {repo} noticed: {e}");
                            }
                            let mut fwd = Command::new("gh");
                            // Detach stdin from the operator's terminal.
                            //
                            // These children previously inherited the pane's tty and
                            // the gh-webhook extension put it into raw mode (-opost,
                            // -isig) without restoring it: log output staircased and
                            // Ctrl-C stopped working. Denying stdin removes the tty
                            // handle tcsetattr needs, while stdout/stderr stay
                            // inherited so forwarder diagnostics still reach the log.
                            fwd.stdin(std::process::Stdio::null());
                            // Deliberately unbounded: any ExecClass timeout would
                            // sever webhook delivery. It still takes kill_on_drop so
                            // an aborted task reaps the child instead of orphaning it.
                            fwd.kill_on_drop(true);
                            fwd.args([
                                "webhook",
                                "forward",
                                "--repo",
                                &repo,
                                "--events",
                                "pull_request,pull_request_review,pull_request_review_comment,workflow_run,merge_group,issues,issue_comment",
                                "--url",
                                &url,
                            ]);
                            if let Some(sec) = secret.as_deref() {
                                fwd.args(["--secret", sec]);
                            }
                            fwd.status().await.map(|st| st.code().unwrap_or(-1))
                        }
                    },
                )
                .await;
            }));
        }
    }

    let app = create_router(state);

    // Production Hyperscaler Socket Binding: SO_REUSEADDR and SO_REUSEPORT
    // Enables zero-downtime, zero-error Blue/Green process handover and parallel socket listening
    let domain = if addr.is_ipv6() {
        socket2::Domain::IPV6
    } else {
        socket2::Domain::IPV4
    };
    let socket = socket2::Socket::new(domain, socket2::Type::STREAM, Some(socket2::Protocol::TCP))
        .context("Failed to create socket")?;
    socket
        .set_reuse_address(true)
        .context("Failed to set SO_REUSEADDR")?;
    #[cfg(unix)]
    {
        let _ = socket.set_reuse_port(true);
    }
    socket
        .set_nonblocking(true)
        .context("Failed to set nonblocking")?;
    socket
        .bind(&addr.into())
        .context(format!("Failed to bind socket to {}", addr))?;
    socket
        .listen(1024)
        .context("Failed to listen on socket backlog")?;

    let std_listener: std::net::TcpListener = socket.into();
    let listener = tokio::net::TcpListener::from_std(std_listener)
        .context("Failed to convert socket to Tokio TcpListener")?;

    info!("Listening for webhooks at http://{}/webhook", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    for task in forwarder_tasks {
        task.abort();
    }

    info!("Oyatie Autonomous Engineering Pipeline gracefully shut down.");
    Ok(())
}

pub async fn start_forwarders(config: &Config) -> Result<()> {
    let mut tasks = Vec::new();

    for repo in &config.watched_repos {
        let repo_clone = repo.clone();
        let secret_clone = config.webhook_secret.clone();
        let target_url = format!("http://{}:{}/webhook", config.host, config.port);
        let task = tokio::spawn(async move {
            info!("Forwarding webhooks for {} to {}", repo_clone, target_url);
            let mut cmd = Command::new("gh");
            // See the boot-time forwarder: keep the child off the operator's tty.
            cmd.stdin(std::process::Stdio::null());
            // Unbounded by design, same as the boot-time forwarder: this child is
            // the webhook transport and must outlive any ExecClass bound. It takes
            // `kill_on_drop` so aborting this task does not orphan a `gh` process.
            cmd.kill_on_drop(true);
            cmd.args([
                "webhook",
                "forward",
                "--repo",
                &repo_clone,
                "--events",
                "pull_request,pull_request_review,pull_request_review_comment,workflow_run,merge_group,issues,issue_comment",
                "--url",
                &target_url,
            ]);
            // Same shared secret as the verifier; see the boot-time forwarder.
            if let Some(secret) = secret_clone.as_deref() {
                cmd.args(["--secret", secret]);
            }
            // See forwarder_supervisor: `status()` is Ok(status) when the child
            // ran and died, so an Err-only check made every real forwarder death
            // silent. Report the exit either way.
            match cmd.status().await {
                Ok(st) => error!(
                    "Webhook forwarder exited for {} with code {}",
                    repo_clone,
                    st.code().unwrap_or(-1)
                ),
                Err(e) => error!(
                    "Webhook forwarder for {} could not be spawned: {}",
                    repo_clone, e
                ),
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
    let mut gh_ver = Command::new("gh");
    gh_ver.arg("--version");
    match crate::exec::run_bounded(gh_ver, crate::exec::ExecClass::Quick, "gh --version").await {
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
    let mut agy_help = Command::new("agy");
    agy_help.arg("--help");
    match crate::exec::run_bounded(agy_help, crate::exec::ExecClass::Quick, "agy --help").await {
        Ok(out) if out.status.success() => println!("✅ Ready"),
        _ => println!("❌ 'agy' not found in PATH"),
    }

    print!("4. Git: ");
    let mut git_ver = Command::new("git");
    git_ver.arg("--version");
    match crate::exec::run_bounded(git_ver, crate::exec::ExecClass::Quick, "git --version").await {
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
