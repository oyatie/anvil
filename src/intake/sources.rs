//! Helpers for the modules that raise work.
//!
//! Producers live in the module that OWNS the finding, not here. `intake` is
//! shared vocabulary and must stay a leaf: a module every producer imports,
//! importing every producer back, is the hub-and-spoke shape that put seventy
//! of this repository's modules into one dependency cycle. A first draft of
//! this file did exactly that with one producer, which is the point at which
//! it is cheap to correct.
//!
//! So `postmortem` depends on `intake` and declares its own work items;
//! `intake` knows about none of them.

use super::Subject;

/// The subject of a finding about a named thing in a repository.
pub fn subject(repo: &str, locus: &str) -> Subject {
    Subject {
        repo: repo.to_string(),
        locus: Some(locus.to_string()),
    }
}

/// The subject of a finding about the repository itself.
pub fn repo_subject(repo: &str) -> Subject {
    Subject {
        repo: repo.to_string(),
        locus: None,
    }
}
