//! The mechanical rewrite engine: renames only. A file or directory move is
//! `git mv`; anything that needs code edited — a crate rename, a skeleton
//! template, an import rewrite — is refused here and belongs to a tenant
//! codemod with an oracle, never to a broader tool by fallback.

use crate::change_delivery::ports::{
    LaneError, LaneWorktree, MoveKind, RewriteEngine, Shard, VcsPort,
};
use async_trait::async_trait;

pub struct MechanicalRewrite;

#[async_trait]
impl RewriteEngine for MechanicalRewrite {
    fn name(&self) -> &'static str {
        "mechanical (git mv only)"
    }

    async fn apply(
        &self,
        vcs: &dyn VcsPort,
        lane: &LaneWorktree,
        shard: &Shard,
    ) -> Result<(), LaneError> {
        for m in &shard.moves {
            match m.kind {
                MoveKind::MoveFile | MoveKind::MoveDir | MoveKind::SplitSatellite => {
                    vcs.apply_move(lane, &m.from, &m.to).await?;
                }
                MoveKind::RenameCrate | MoveKind::CreateSkeleton | MoveKind::AddManifest => {
                    return Err(LaneError::Refused(format!(
                        "{:?} is not mechanical; it needs a codemod with an oracle ({} -> {})",
                        m.kind, m.from, m.to
                    )));
                }
            }
        }
        Ok(())
    }
}
