//! Taking an arming away from a head that did not certify.
//!
//! Its own file because `merge_enlister` is 451 lines past the file budget and
//! the oversized-file ratchet says a file may be split, moved or shrunk but not
//! fattened. The arming lives next door in `enlist_into_merge_queue`; this is
//! the other direction, and the two are worth reading together.

use tracing::{info, warn};

/// What disarming established.
///
/// Command acceptance is not proof of previous arming. A failed command does
/// not establish absence, even when its process completed normally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disarmed {
    /// The forge accepted the disable command; previous arming is unmeasured.
    DisableAccepted,
    /// The disable was not established. Whether anything is armed is unknown.
    Unknown { detail: String },
}

impl Disarmed {
    fn from_completion(completion: anyhow::Result<std::process::Output>) -> Self {
        match completion {
            Ok(out) if out.status.success() => Self::DisableAccepted,
            Ok(out) => Self::Unknown {
                detail: format!("disable command returned {}", out.status),
            },
            Err(_) => Self::Unknown {
                detail: "disable command did not complete successfully".to_string(),
            },
        }
    }

    pub fn report(&self, repo: &str, pr_number: u64) {
        match self {
            Self::DisableAccepted => info!("{repo}#{pr_number}: auto-merge disable accepted"),
            Self::Unknown { detail } => warn!(
                "{repo}#{pr_number}: auto-merge disarm is UNKNOWN; arming may remain: {detail}"
            ),
        }
    }
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
    /// Continue rejection after attempting disarm, but report Unknown when
    /// the command fails. A nonzero exit is not proof that nothing was armed.
    /// Returning [`Disarmed`] prevents `?` from abandoning the rejection.
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
        let completion = crate::exec::run_bounded(
            cmd,
            crate::exec::ExecClass::Api,
            "gh pr merge --disable-auto",
        )
        .await;
        Disarmed::from_completion(completion)
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

#[cfg(all(test, unix))]
mod tests {
    use super::Disarmed;
    use std::{
        os::unix::process::ExitStatusExt,
        process::{ExitStatus, Output},
    };

    fn output(status: i32) -> Output {
        Output {
            status: ExitStatus::from_raw(status),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    }

    #[test]
    fn successful_completion_establishes_only_acceptance() {
        assert_eq!(
            Disarmed::from_completion(Ok(output(0))),
            Disarmed::DisableAccepted
        );
    }

    #[test]
    fn completed_nonzero_and_runner_failure_are_unknown() {
        assert!(matches!(
            Disarmed::from_completion(Ok(output(256))),
            Disarmed::Unknown { .. }
        ));
        assert!(matches!(
            Disarmed::from_completion(Err(anyhow::anyhow!("synthetic failure"))),
            Disarmed::Unknown { .. }
        ));
    }
}
