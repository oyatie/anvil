//! The ledger on disk: one JSON file per repository under
//! `data_dir/shape_delivery/`, written atomically. A cache — GitHub is the
//! source of truth — so a lost file costs a lookup, never a duplicate.

use crate::change_delivery::ports::DeliveryLedger;
use std::path::PathBuf;

pub struct FileLedger {
    dir: PathBuf,
}

impl FileLedger {
    pub fn new(data_dir: &std::path::Path) -> Self {
        FileLedger {
            dir: data_dir.join("shape_delivery"),
        }
    }

    fn path_for(&self, repo: &str) -> PathBuf {
        self.dir.join(format!("{}.json", repo.replace('/', "-")))
    }

    pub async fn load(&self, repo: &str) -> DeliveryLedger {
        match tokio::fs::read(self.path_for(repo)).await {
            Ok(bytes) => DeliveryLedger::parse(&bytes).unwrap_or_else(|_| DeliveryLedger {
                repo: repo.to_string(),
                ..Default::default()
            }),
            Err(_) => DeliveryLedger {
                repo: repo.to_string(),
                ..Default::default()
            },
        }
    }

    pub async fn save(&self, ledger: &DeliveryLedger) -> Result<(), String> {
        let _ = tokio::fs::create_dir_all(&self.dir).await;
        let path = self.path_for(&ledger.repo);
        let tmp = path.with_extension("json.tmp");
        tokio::fs::write(&tmp, ledger.to_json())
            .await
            .map_err(|e| e.to_string())?;
        tokio::fs::rename(&tmp, &path)
            .await
            .map_err(|e| e.to_string())
    }

    /// The fleet kill switch and the per-repo one: presence pauses.
    pub fn kill_switch(&self, repo: &str) -> bool {
        self.dir.join("PAUSE").exists()
            || self
                .dir
                .join(format!("{}.PAUSE", repo.replace('/', "-")))
                .exists()
    }
}
