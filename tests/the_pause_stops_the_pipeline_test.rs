//! Touching a file stops Anvil, and an unreadable file stops it too.
//!
//! `FileLedger::kill_switch` has read `PAUSE` and `<repo>.PAUSE` since it was
//! written and has never had a production caller. The gesture existed, the
//! filenames were documented, and touching them stopped nothing — which is
//! worse than having no switch, because an operator believes in it.
//!
//! These fixtures are the two halves: the switch answers correctly, and the
//! review pipeline reads it on every iteration rather than once at start-up.

use anvil::pause::{Held, Pause};

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("anvil-pause-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[test]
fn nothing_present_is_not_a_pause() {
    let dir = scratch("clear");
    assert_eq!(
        Pause::in_dir(&dir).engaged("oyatie/anvil"),
        None,
        "an empty directory is the running state; a switch that holds by \
         default is a switch nobody can leave on"
    );
}

#[test]
fn the_fleet_file_holds_every_repository() {
    let dir = scratch("fleet");
    std::fs::write(dir.join("PAUSE"), "").expect("touch");
    let pause = Pause::in_dir(&dir);
    for repo in ["oyatie/anvil", "oyatie/console", "oyatie/oyatie"] {
        let held = pause.engaged(repo).expect("the fleet file holds");
        assert!(matches!(held, Held::Fleet { .. }), "{repo}: {held:?}");
        assert!(
            held.reason().contains("Remove it to resume"),
            "the reason must say how to undo it: {}",
            held.reason()
        );
    }
}

#[test]
fn the_per_repository_file_holds_only_its_own() {
    let dir = scratch("one-repo");
    std::fs::write(dir.join("oyatie-console.PAUSE"), "").expect("touch");
    let pause = Pause::in_dir(&dir);

    let held = pause
        .engaged("oyatie/console")
        .expect("its own repo is held");
    assert!(matches!(held, Held::Repository { .. }), "{held:?}");
    assert!(held.reason().contains("oyatie/console"));

    assert_eq!(
        pause.engaged("oyatie/anvil"),
        None,
        "pausing one repository must not pause the fleet"
    );
}

/// The filename must be the one `FileLedger::kill_switch` already documents, or
/// the operator's gesture and the code's expectation are two different things.
#[test]
fn the_per_repository_filename_is_the_one_the_ledger_reads() {
    let pause = Pause::in_dir("data");
    assert_eq!(
        pause.repository_file("oyatie/console"),
        std::path::Path::new("data").join("oyatie-console.PAUSE")
    );
    assert_eq!(
        pause.fleet_file(),
        std::path::Path::new("data").join("PAUSE")
    );
}

/// The case `Path::exists()` cannot express. A directory that cannot be read
/// answers `false` to "is the pause file there", which is the same answer as
/// "Anvil is free to merge".
#[cfg(unix)]
#[test]
fn a_pause_that_cannot_be_read_is_a_pause() {
    use std::os::unix::fs::PermissionsExt;

    let dir = scratch("unreadable");
    let inner = dir.join("locked");
    std::fs::create_dir_all(&inner).expect("inner");
    std::fs::write(inner.join("PAUSE"), "").expect("touch");
    // Remove the execute bit, so the file cannot be stat-ed through the
    // directory. `Path::exists()` reports `false` here — indistinguishable
    // from an empty directory.
    std::fs::set_permissions(&inner, std::fs::Permissions::from_mode(0o000)).expect("chmod");

    let observed = Pause::in_dir(&inner).engaged("oyatie/anvil");

    // Restore before asserting, so a failure does not leave an unremovable
    // directory behind.
    let _ = std::fs::set_permissions(&inner, std::fs::Permissions::from_mode(0o755));
    let _ = std::fs::remove_dir_all(&dir);

    // Running as root defeats the permission bits entirely; the fixture cannot
    // arrange the condition, so it withholds rather than passing on a
    // measurement it did not take.
    if inner.join("PAUSE").exists()
        && observed
            == Some(Held::Fleet {
                path: inner.join("PAUSE"),
            })
    {
        eprintln!("skipped: the permission bits did not deny this process");
        return;
    }

    match observed {
        Some(Held::Unreadable { .. }) => {}
        other => panic!(
            "an unreadable pause must hold, because `Path::exists()` answers \
             `false` for both 'absent' and 'could not tell', and for a kill \
             switch those are opposite answers. Got: {other:?}"
        ),
    }
}

/// The switch is worth what the pipeline does with it, so the wiring is
/// asserted too — keyed to the call rather than to a line.
#[test]
fn the_review_pipeline_reads_the_pause_before_it_enlists() {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/webhook/pipelines/review.rs"),
    )
    .expect("the review pipeline exists");

    // Keyed to the field, not to a method name: `holds` and `engaged` are two
    // spellings of one consultation, and a scan tied to either reports the
    // pipeline as unguarded the moment the other is used.
    let reads: Vec<usize> = src.match_indices("state.pause.").map(|(i, _)| i).collect();
    assert!(
        reads.len() >= 2,
        "the review pipeline reads the pause {} time(s). Once is not each \
         iteration: certification takes minutes, and a pause touched during \
         them is a pause touched to stop exactly this enlistment.",
        reads.len()
    );

    let enlist = src
        .find(".enlist_into_merge_queue(")
        .expect("the pipeline still enlists; if that moved, this test must follow it");
    assert!(
        reads.iter().any(|at| *at < enlist),
        "every read of the pause happens after the enlistment. A switch \
         consulted only afterwards records the merge rather than preventing it."
    );

    // And the read nearest the enlistment must actually withhold it.
    let last_before = reads
        .iter()
        .filter(|at| **at < enlist)
        .max()
        .expect("checked above");
    let between = &src[*last_before..enlist];
    assert!(
        between.contains("return Ok(())"),
        "the pause is read before the enlistment and does not stop it:\n{}",
        between.lines().take(12).collect::<Vec<_>>().join("\n")
    );
}
