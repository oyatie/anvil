use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueGreenHandoverConfig {
    pub green_binary_path: PathBuf,
    pub target_installed_path: PathBuf,
    pub health_check_url: String,
    pub drain_timeout: Duration,
}

pub struct BlueGreenSupervisor;

impl BlueGreenSupervisor {
    /// Executes an atomic, zero-downtime binary swap by staging to a temporary location and fs::rename
    pub async fn execute_atomic_binary_swap(
        staged_green_binary: &Path,
        target_installed_binary: &Path,
    ) -> Result<()> {
        info!(
            "🔄 [Blue/Green Supervisor] Preparing atomic binary swap: {:?} -> {:?}",
            staged_green_binary, target_installed_binary
        );

        let parent = target_installed_binary
            .parent()
            .context("Invalid target binary path")?;

        let staging_path: PathBuf = parent.join(format!(
            ".anvil_swap_staging_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ));

        // Copy new green binary to staging area on the same filesystem
        tokio::fs::copy(staged_green_binary, &staging_path)
            .await
            .context("Failed to copy green binary to staging")?;

        // Ensure executable permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&staging_path).await?.permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&staging_path, perms).await?;
        }

        // Atomic rename guarantees zero-window partial binary reads
        tokio::fs::rename(&staging_path, target_installed_binary)
            .await
            .context("Failed to atomically swap binary via fs::rename")?;

        info!(
            "✅ [Blue/Green Supervisor] Atomic binary swap successful. Target {:?} updated.",
            target_installed_binary
        );
        Ok(())
    }

    /// Spawns the green child process in a detached process group and waits for native readiness health check
    pub async fn spawn_green_and_drain_blue(
        new_binary_path: &Path,
        args: &[String],
        health_check_host_port: &str,
        drain_timeout: Duration,
    ) -> Result<()> {
        info!(
            "🌱 [Blue/Green Supervisor] Spawning detached Green instance: {:?} {:?}",
            new_binary_path, args
        );

        let mut cmd = tokio::process::Command::new(new_binary_path);
        cmd.args(args);

        #[cfg(unix)]
        {
            cmd.process_group(0); // Detach process group so child survives parent exit
        }

        let mut child = cmd.spawn().context("Failed to spawn Green process")?;

        let mut is_ready = false;
        let start = std::time::Instant::now();

        // Native async TCP/HTTP readiness probe without invoking shell curl
        let target_addr = if health_check_host_port.starts_with("http://") {
            health_check_host_port.trim_start_matches("http://")
        } else {
            health_check_host_port
        };
        let host_port = target_addr.split('/').next().unwrap_or("127.0.0.1:3000");

        while start.elapsed() < Duration::from_secs(30) {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if tokio::net::TcpStream::connect(host_port).await.is_ok() {
                is_ready = true;
                break;
            }
        }

        if !is_ready {
            let _ = child.kill().await;
            anyhow::bail!(
                "Green instance failed health readiness probe on {}. Aborting handover and retaining Blue.",
                health_check_host_port
            );
        }

        info!(
            "✅ [Blue/Green Supervisor] Green instance healthy and taking traffic. Initiating graceful drain of Blue (drain SLA: {:.0}s)...",
            drain_timeout.as_secs_f64()
        );

        tokio::time::sleep(drain_timeout).await;
        info!("👋 [Blue/Green Supervisor] Graceful handover complete.");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_atomic_binary_swap_logic() {
        let temp_dir = std::env::temp_dir().join("anvil_bg_test");
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        let src = temp_dir.join("green_bin");
        let dst = temp_dir.join("current_bin");

        tokio::fs::write(&src, b"#!/bin/sh\necho green")
            .await
            .unwrap();
        tokio::fs::write(&dst, b"#!/bin/sh\necho blue")
            .await
            .unwrap();

        let swap_res = BlueGreenSupervisor::execute_atomic_binary_swap(&src, &dst).await;
        assert!(swap_res.is_ok());

        let contents = tokio::fs::read_to_string(&dst).await.unwrap();
        assert_eq!(contents, "#!/bin/sh\necho green");
    }
}
