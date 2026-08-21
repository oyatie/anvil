//! The local gate a lane must pass before anything leaves it. Gate kinds are
//! an allowlist; a repository this build has no gate for is Unavailable,
//! which is not a pass (I1).

use crate::change_delivery::ports::{GateResult, LaneWorktree, LocalGate};
use crate::exec::{ExecClass, run_bounded};
use async_trait::async_trait;
use tokio::process::Command;

pub struct CargoGate;

#[async_trait]
impl LocalGate for CargoGate {
    async fn run(&self, lane: &LaneWorktree) -> GateResult {
        let manifest = crate::shape::ports::LanguageProfile::RustCargo.unit_marker();
        if !lane.path.join(manifest).exists() {
            return GateResult::Unavailable {
                reason: format!("no root {manifest}; no gate this build can run here"),
            };
        }
        let mut cmd = Command::new("cargo");
        cmd.current_dir(&lane.path).args(["check", "--quiet"]);
        match run_bounded(cmd, ExecClass::Build, "cargo check (lane gate)").await {
            Ok(out) if out.status.success() => GateResult::Passed {
                label: "cargo check".into(),
            },
            Ok(out) => GateResult::Failed {
                label: "cargo check".into(),
                why: String::from_utf8_lossy(&out.stderr)
                    .lines()
                    .take(10)
                    .collect::<Vec<_>>()
                    .join("\n"),
            },
            Err(e) => GateResult::Failed {
                label: "cargo check".into(),
                why: e.to_string(),
            },
        }
    }
}
