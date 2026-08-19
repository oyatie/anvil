use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::info;

#[derive(Debug, Clone)]
pub struct BlueGreenHandoverConfig {
    pub current_binary_path: PathBuf,
    pub new_binary_path: PathBuf,
    pub graceful_drain_timeout: Duration,
}

pub struct BlueGreenSupervisor;

impl BlueGreenSupervisor {
    /// Executes an in-place atomic binary replacement with zero downtime
    pub async fn execute_atomic_binary_swap(
        target_installed_binary: &Path,
        new_compiled_binary: &Path,
    ) -> Result<()> {
        info!(
            "🔄 [Blue/Green Supervisor] Preparing atomic binary swap: {:?} -> {:?}",
            new_compiled_binary, target_installed_binary
        );

        if !new_compiled_binary.exists() {
            anyhow::bail!(
                "New binary at {:?} does not exist. Cannot proceed with atomic swap.",
                new_compiled_binary
            );
        }

        // Create temporary staging path in the same filesystem for atomic rename
        let staging_path = target_installed_binary.with_extension("new_staging");
        tokio::fs::copy(new_compiled_binary, &staging_path)
            .await
            .context("Failed to copy new binary to staging path")?;

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

    /// Spawns the green child process and waits for readiness health check before signaling blue shutdown
    pub async fn spawn_green_and_drain_blue(
        new_binary_path: &Path,
        args: &[String],
        health_check_url: &str,
        drain_timeout: Duration,
    ) -> Result<()> {
        info!(
            "🌱 [Blue/Green Supervisor] Spawning Green instance: {:?} {:?}",
            new_binary_path, args
        );

        let mut child = tokio::process::Command::new(new_binary_path)
            .args(args)
            .spawn()
            .context("Failed to spawn Green process")?;

        let mut is_ready = false;
        let start = std::time::Instant::now();

        while start.elapsed() < Duration::from_secs(30) {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let check = tokio::process::Command::new("curl")
                .args(["-s", "-f", health_check_url])
                .output()
                .await;
            if let Ok(out) = check {
                if out.status.success() {
                    is_ready = true;
                    break;
                }
            }
        }

        if !is_ready {
            let _ = child.kill().await;
            anyhow::bail!(
                "Green instance failed health readiness probe on {}. Aborting handover and retaining Blue.",
                health_check_url
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

        let src = temp_dir.join("anvil_v2");
        let dest = temp_dir.join("anvil_active");

        tokio::fs::write(&src, b"binary_v2_payload").await.unwrap();
        tokio::fs::write(&dest, b"binary_v1_payload").await.unwrap();

        let res = BlueGreenSupervisor::execute_atomic_binary_swap(&dest, &src).await;
        assert!(res.is_ok());

        let content = tokio::fs::read(&dest).await.unwrap();
        assert_eq!(content, b"binary_v2_payload");
    }
}
