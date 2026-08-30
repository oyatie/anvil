//! A source scan keyed to a filename goes blind the day its subject is split.
//!
//! Splitting a file into a directory is routine here: the oversized-file
//! ratchet demands it, and this tree has done it to `gate_proof`,
//! `pre_merge_guard`, `harness::rules`, `occupancy` and `merge_enlister`. Every
//! scan that names `src/<thing>.rs` stops finding its subject that day --
//! blind, not failing, because a scan that reads nothing finds nothing wrong.
//!
//! `source_scan::module_source` reads whichever form exists, and refuses to
//! return an empty string for a module that is not there.

use anvil::source_scan::paths::module_source;
use std::path::Path;

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// A module that is a directory today.
#[test]
fn a_split_module_is_read_whole() {
    let src = module_source("src/merge_enlister", repo());
    assert!(
        src.contains("fn enlist_into_merge_queue"),
        "the module's own entry point is missing, so this read is not whole"
    );
    assert!(
        src.contains("fn disarm_auto_merge"),
        "a file under the directory was not read, so a scan over this module \
         would miss whatever lives in it"
    );
}

/// A module that is still a single file.
#[test]
fn an_unsplit_module_is_read_the_same_way() {
    let src = module_source("src/queue_healer", repo());
    assert!(
        !src.trim().is_empty(),
        "a module that is still one file must read the same way, or callers \
         have to know which form each subject takes"
    );
}

/// The empty answer has no spelling.
///
/// This is the whole point. Returning `String::new()` for a module that moved
/// is what makes the scan report a clean subject it never read -- absent
/// evidence read as a pass, which is I1.
#[test]
fn a_module_that_is_not_there_is_refused_rather_than_read_as_empty() {
    let missing = std::panic::catch_unwind(|| {
        module_source("src/a_module_this_repository_does_not_have", repo())
    });
    assert!(
        missing.is_err(),
        "a missing module returned source instead of refusing. A scan that \
         cannot find its subject must say so: reporting nothing wrong with a \
         module nobody read is the defect, not the fallback."
    );
}
