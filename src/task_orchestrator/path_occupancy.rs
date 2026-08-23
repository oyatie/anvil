//! Mechanical occupancy: git paths, not crate locks or task boards.
//!
//! Two live tasks commute iff their path sets are disjoint. A `git mv`
//! occupies both ends. Overlap is a launcher bug; this module fails closed.

use anyhow::{bail, Result};
use std::collections::BTreeSet;

use super::source_doc_verifier::ScopedTaskDefinition;

/// Occupancy of a rename `old -> new` is `{old, new}`.
pub fn occupy_move(old: &str, new: &str) -> BTreeSet<String> {
    [old, new].into_iter().map(str::to_string).collect()
}

pub fn path_sets_disjoint(a: &[String], b: &[String]) -> bool {
    let a: BTreeSet<&str> = a.iter().map(String::as_str).collect();
    b.iter().all(|p| !a.contains(p.as_str()))
}

/// Fail if any two tasks in a parallel layer share a path.
pub fn assert_layer_paths_disjoint(tasks: &[ScopedTaskDefinition]) -> Result<()> {
    for (i, left) in tasks.iter().enumerate() {
        for right in tasks.iter().skip(i + 1) {
            if !path_sets_disjoint(&left.target_files, &right.target_files) {
                bail!(
                    "path occupancy overlap: {} {:?} ∩ {} {:?}",
                    left.task_id,
                    left.target_files,
                    right.task_id,
                    right.target_files
                );
            }
        }
    }
    Ok(())
}
