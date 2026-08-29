//! The fleet sweep task: measure in `shape`, plan in `change_delivery`.
//!
//! The composition root for those two units. Neither imports the other —
//! doing the planning inside the sweep made `shape` depend on
//! `change_delivery` while `change_delivery` depends on `shape`, closing a
//! cycle whose every edge is individually legal and which therefore no
//! per-edge rule can see.
//!
//! Report-only (I25): blocks nobody, mutates nothing in any repository.

use tracing::info;

/// Start the hourly sweep over every watched repository.
pub fn spawn(state: &crate::webhook::AppState) {
    let deps = crate::shape::facade::sweep::SweepDeps {
        git_mgr: state.git_mgr.clone(),
        telemetry: state.telemetry_store.clone(),
        data_dir: state.config.data_dir.clone(),
    };
    let repos = state.config.watched_repos.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            for repo in &repos {
                // Measure in `shape`, plan in `change_delivery`, compose
                // here. The sweep used to do both, which made shape import
                // change_delivery while change_delivery imports shape --
                // a cycle no per-edge rule can see. The composition root
                // is the one place allowed to know both units.
                match crate::shape::facade::sweep::sweep_repo(&deps, repo).await {
                    Ok(crate::shape::facade::sweep::Swept::Skipped(why)) => {
                        info!("[Shape Sweep] {why}")
                    }
                    Ok(crate::shape::facade::sweep::Swept::Measured { report, summary }) => {
                        match crate::change_delivery::facade::plan::write_move_plan(
                            &deps.data_dir,
                            repo,
                            &report,
                        )
                        .await
                        {
                            Ok(path) => info!("[Shape Sweep] {summary} -> {}", path.display()),
                            Err(e) => {
                                tracing::warn!("[Shape Sweep] {summary}; plan not written: {e}")
                            }
                        }
                    }
                    Err(e) => tracing::warn!("[Shape Sweep] {repo} noticed: {e}"),
                }
            }
        }
    });
}
