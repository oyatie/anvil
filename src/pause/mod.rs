//! The gesture that stops Anvil, and the one place that answers whether it was
//! made.
//!
//! `FileLedger::kill_switch` has read `PAUSE` and `<repo>.PAUSE` since it was
//! written and has never had a production caller -- `FileLedger` itself is
//! never constructed anywhere in `src/` -- so touching those files stopped
//! nothing. This reads the same two filenames, from the CONFIGURED data
//! directory, on every iteration of the review pipeline.
//!
//! Not `<data_dir>/shape_delivery/`, where `FileLedger` documents its own: that
//! directory is created lazily by `FileLedger::save`, which never runs, so a
//! switch placed there would sit in a directory that does not exist and an
//! operator's `touch` would fail. A process-wide switch belongs at the root of
//! the data directory.

//!
//! # Why it is a file and not a flag or an API
//!
//! The pause has to be usable when Anvil is the thing going wrong. A flag needs
//! a restart, an endpoint needs the process to still be serving, and a database
//! row needs the database. `touch PAUSE` needs a shell.
//!
//! # Why an unreadable answer is a pause
//!
//! `Path::exists()` returns `false` for "the file is not there" and `false` for
//! "the directory could not be read" -- a permission change, a detached mount,
//! a full disk. For a kill switch those are opposite answers, and collapsing
//! them makes the failure mode of the check identical to not being paused.
//! [`Pause::engaged`] separates them and treats "could not tell" as engaged:
//! the whole point of this control is what it does when things are wrong.
//!
//! # What has to read it
//!
//! Every task `webhook_handlers` detaches, and every step inside a run that
//! takes authority. Those are different bounds and both are needed: a read at
//! task entry does not cover a certification that takes minutes, and a read
//! before one irreversible step does not cover its siblings.
//!
//! The doors, and what each does while paused if it does not read this:
//!
//! * the review door -- clones, runs a model turn, and can enlist;
//! * the fixer door -- clones, runs a model turn, pushes to the contributor;
//! * the trunk-CI triage door -- runs a model turn and files a PUBLIC issue
//!   with `gh issue create`, which is what an operator sees appearing after
//!   they thought they had stopped Anvil;
//! * the heal door -- pushes a heal commit and enlists, reaching
//!   `certify_for_enlistment` without passing through the review pipeline.
//!
//! Inside a run, the Enlist arm and the AutoFix arm both take authority, and
//! the AutoFix arm reaches the same fixer the door above guards.
//!
//! `every_autonomous_webhook_door_reads_the_pause` is keyed to `tokio::spawn`
//! rather than to that list, because a rule written as the doors it knows
//! about cannot see the next one.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Why Anvil is holding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Held {
    /// The fleet-wide `PAUSE` file is present.
    Fleet { path: PathBuf },
    /// `<owner>-<repo>.PAUSE` is present.
    Repository { repo: String, path: PathBuf },
    /// The pause could not be read, so nothing establishes that it is absent.
    Unreadable { path: PathBuf, why: String },
}

impl Held {
    /// What a log line and a pull-request comment should say.
    pub fn reason(&self) -> String {
        match self {
            Held::Fleet { path } => format!(
                "Anvil is paused: {} is present. Remove it to resume.",
                path.display()
            ),
            Held::Repository { repo, path } => format!(
                "Anvil is paused for {repo}: {} is present. Remove it to resume.",
                path.display()
            ),
            Held::Unreadable { path, why } => format!(
                "Anvil is holding because it could not read its pause file at {}: {why}. \
                 An unreadable pause is not an absent one.",
                path.display()
            ),
        }
    }
}

/// The pause, rooted at the directory that holds the files.
#[derive(Debug, Clone)]
pub struct Pause {
    dir: PathBuf,
}

impl Pause {
    /// Reads the pause from `dir`, the same directory `FileLedger` uses.
    pub fn in_dir(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The fleet-wide file. Presence pauses every repository.
    pub fn fleet_file(&self) -> PathBuf {
        self.dir.join("PAUSE")
    }

    /// The per-repository file, named as `FileLedger` names it.
    pub fn repository_file(&self, repo: &str) -> PathBuf {
        self.dir.join(format!("{}.PAUSE", repo.replace('/', "-")))
    }

    /// Whether Anvil is holding, logging the reason if it is.
    ///
    /// The shape a caller wants: the decision and its explanation are one
    /// thing, so a site cannot take the decision and drop the reason.
    /// `what` names the step being withheld.
    pub fn holds(&self, repo: &str, pr_number: u64, what: &str) -> bool {
        match self.engaged(repo) {
            Some(held) => {
                tracing::warn!("{}#{}: not {}. {}", repo, pr_number, what, held.reason());
                true
            }
            None => false,
        }
    }

    /// Whether Anvil is holding, and why. `None` means it is free to proceed,
    /// and is returned only when both files were read and found absent.
    pub fn engaged(&self, repo: &str) -> Option<Held> {
        let fleet = self.fleet_file();
        match present(&fleet) {
            Presence::Yes => return Some(Held::Fleet { path: fleet }),
            Presence::CouldNotTell(why) => {
                return Some(Held::Unreadable { path: fleet, why });
            }
            Presence::No => {}
        }

        let per_repo = self.repository_file(repo);
        match present(&per_repo) {
            Presence::Yes => Some(Held::Repository {
                repo: repo.to_string(),
                path: per_repo,
            }),
            Presence::CouldNotTell(why) => Some(Held::Unreadable {
                path: per_repo,
                why,
            }),
            Presence::No => None,
        }
    }
}

/// Three answers, where `Path::exists` gives two.
enum Presence {
    Yes,
    No,
    CouldNotTell(String),
}

fn present(path: &Path) -> Presence {
    match std::fs::metadata(path) {
        Ok(_) => Presence::Yes,
        // The one error that genuinely means absent. Every other error means
        // the question was not answered.
        Err(e) if e.kind() == ErrorKind::NotFound => Presence::No,
        Err(e) => Presence::CouldNotTell(e.to_string()),
    }
}
