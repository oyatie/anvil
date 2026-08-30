//! Taking an arming away from a head that did not certify.
//!
//! Its own file because `merge_enlister` is 451 lines past the file budget and
//! the oversized-file ratchet says a file may be split, moved or shrunk but not
//! fattened. The arming lives next door in `enlist_into_merge_queue`; this is
//! the other direction, and the two are worth reading together.

use tracing::{info, warn};

/// What disarming established.
///
/// Three answers, not two. "Nothing was armed" is a measurement; "the forge
/// could not be reached" is not, and collapsing them would let an unreachable
/// forge read as a pull request that was safe all along.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disarmed {
    /// Auto-merge was on and is now off.
    WasArmed,
    /// The forge answered, and there was nothing to disable.
    NothingArmed { detail: String },
    /// The call did not complete. Whether anything is still armed is unknown.
    Unknown { detail: String },
}

impl super::MergeEnlister {
    /// Takes auto-merge off a pull request that this run did not certify.
    ///
    /// # The window this closes
    ///
    /// `--match-head-commit` binds arming to the head a report measured, and
    /// GitHub validates it once, at the moment auto-merge is enabled. The merge
    /// happens later, whenever the required checks go green. A contributor with
    /// write access who pushes after that point moves the head and GitHub does
    /// NOT disable auto-merge for it, so the commit that eventually merges can
    /// be one no report ever measured.
    ///
    /// The review pipeline already re-certifies each head it sees. What it did
    /// with an inadmissible one was `warn!`. This is the other half: a head
    /// that does not certify takes the arming away with it.
    ///
    /// # Why every failure here is survivable
    ///
    /// Disarming can only prevent a merge, never cause one, so the conservative
    /// direction is to attempt it and continue. `gh` exits non-zero when there
    /// is no auto-merge to disable, which is the common case -- most pull
    /// requests were never armed -- and treating that as an error would fill
    /// the log with failures for the normal path.
    ///
    /// That asymmetry is the whole reason this returns [`Disarmed`] rather than
    /// `Result`: a caller must not be able to write `?` here and abandon the
    /// rest of a rejection because a pull request had nothing armed.
    pub async fn disarm_auto_merge(&self, repo: &str, pr_number: u64) -> Disarmed {
        let mut cmd = crate::exec::gh();
        cmd.args([
            "pr",
            "merge",
            &pr_number.to_string(),
            "--repo",
            repo,
            "--disable-auto",
        ]);
        match crate::exec::run_bounded(
            cmd,
            crate::exec::ExecClass::Api,
            "gh pr merge --disable-auto",
        )
        .await
        {
            Ok(out) if out.status.success() => {
                info!("{repo}#{pr_number}: auto-merge disarmed; this head did not certify.");
                Disarmed::WasArmed
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                Disarmed::NothingArmed { detail: stderr }
            }
            // Not silently swallowed: the call did not complete, so whether
            // anything is still armed is unknown, and a caller reporting this
            // as "nothing was armed" would be stating a fact nobody measured.
            Err(e) => {
                warn!("{repo}#{pr_number}: could not reach the forge to disarm auto-merge: {e}");
                Disarmed::Unknown {
                    detail: e.to_string(),
                }
            }
        }
    }
}

/// Disarms unless this run is enlisting.
///
/// The rule lives here rather than at the call site. Written as the NEGATION of
/// `Enlist` on purpose: a per-arm call is what rots, because the next
/// `NextPhase` variant gets an arm and nobody remembers the disarm. Expressed
/// this way, a new variant disarms by default and has to be argued out.
pub async fn unless_enlisting(
    enlister: &super::MergeEnlister,
    phase: &crate::webhook::next_phase::NextPhase,
    repo: &str,
    pr_number: u64,
) -> Option<Disarmed> {
    if matches!(phase, crate::webhook::next_phase::NextPhase::Enlist) {
        return None;
    }
    Some(enlister.disarm_auto_merge(repo, pr_number).await)
}
