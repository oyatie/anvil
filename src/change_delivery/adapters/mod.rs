//! Concrete lane machinery: git worktrees with leases, the mechanical
//! rewrite engine, the local gate runner, the file-backed ledger, and the
//! guard that keeps every lane away from the daemon's own source tree.

pub mod gate_runner;
pub mod git_vcs;
pub mod ledger_file;
pub mod rewrite_mechanical;
pub mod self_source_guard;

pub use gate_runner::CargoGate;
pub use git_vcs::GitLaneVcs;
pub use ledger_file::FileLedger;
pub use rewrite_mechanical::MechanicalRewrite;
