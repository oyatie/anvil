//! Raising what the audits already found, so it re-enters the system.
//!
//! `intake::Queue` and all six `work_items()` producers had zero production
//! callers. Every audit pass printed what it found and queued none of it, which
//! is the arc `intake`'s own module doc names as its reason to exist: a finding
//! printed and not queued is a finding that will be found again.
//!
//! # Why the composition happens here
//!
//! `intake/sources.rs` states that intake is a leaf and must not import its
//! producers, so only a composition root may know all of them at once. The
//! hourly sweep is that root for repository-level work.
//!
//! # Four producers, not six, and the two absences are the point
//!
//! `incident_sentry::work_items` cannot return a non-empty vec: its
//! `live_golden_signals` returns `None` unconditionally, so the report is
//! `measured: false` and the producer early-returns. `review_memory::work_items`
//! likewise: its `is_aligned` is a hardcoded `true`. Wiring either would raise
//! a constant into the backlog and make the queue's depth a statement about
//! nothing — the exact defect `fidelity` exists to name, committed on the
//! intake side instead of the gate side.
//!
//! They are named here rather than silently skipped, so the gap is visible to a
//! reader and so wiring them later is one line each once they measure anything.

use std::collections::BTreeMap;

use crate::intake::{Queue, WorkItem};

/// Every producer this sweep can honestly raise from, and what it raised.
///
/// Returned rather than logged so a caller can assert on it: a sweep whose only
/// output is a log line cannot be tested for having run.
pub struct Raised {
    pub queue: Queue,
    /// Producer name to item count, including producers that raised nothing.
    /// A zero is evidence the producer ran; an absent key is evidence it did
    /// not, and those are different facts.
    pub by_producer: BTreeMap<&'static str, usize>,
}

impl Raised {
    /// Item counts by the source that raised them.
    pub fn by_source(&self) -> BTreeMap<String, usize> {
        let mut out: BTreeMap<String, usize> = BTreeMap::new();
        for item in self.queue.outstanding() {
            *out.entry(item.source.as_str().to_string()).or_default() += 1;
        }
        out
    }
}

/// Raise the postmortem ledger's unbuilt remedies and the corpus audit's
/// findings for one repository.
///
/// `corpus` is `None` when the audit could not run — an unreadable checkout, a
/// clone that did not complete. That is recorded as the producer being absent
/// rather than as it having found nothing, because "we did not look" and "we
/// looked and there was nothing" are the two answers this whole codebase exists
/// to keep apart.
pub fn raise_for_repo(
    repo: &str,
    corpus: Option<&crate::corpus_auditor::CorpusAuditReport>,
) -> Raised {
    let mut queue = Queue::new();
    let mut by_producer: BTreeMap<&'static str, usize> = BTreeMap::new();

    let mut raise_all = |name: &'static str, items: Vec<WorkItem>| {
        let mut raised = 0usize;
        for item in items {
            // `raise` is idempotent by derived identity, so an item the last
            // sweep already saw does not count as new work this sweep.
            if queue.raise(item) {
                raised += 1;
            }
        }
        by_producer.insert(name, raised);
    };

    raise_all("postmortem", crate::postmortem::work_items(repo));
    if let Some(corpus) = corpus {
        raise_all("corpus_auditor", corpus.work_items(repo));
    }

    Raised { queue, by_producer }
}
