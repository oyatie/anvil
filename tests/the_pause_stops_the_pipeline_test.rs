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

/// Every door Anvil goes through on its own must read the pause, and stop.
///
/// Keyed to `tokio::spawn`, with no list of verbs. The first version of this
/// test asked whether a spawn body called one of
/// `["resolve_and_fix", "heal_ejected_pr", "execute_pr_review"]` -- the three
/// that existed when it was written -- and classified everything else as
/// "only reads or reports". The trunk-CI triage door, which runs a model turn
/// and files a public issue with `gh issue create`, was silently in that
/// "everything else". So was a door that enlisted into the merge queue. A rule
/// written as the instances it knew about cannot see the next one, and the
/// next one is the whole point.
///
/// Read through `code_only`, not `without_commentary`. That one keeps string
/// literals by documented design, so `warn!("resuming after pause")` satisfied
/// a scan for the word. The comment half was closed in the first version and
/// the literal half was left open.
///
/// A read that does not stop the door is not a guard, so the `return` is part
/// of the assertion. `if paused { warn!(...) }` and then pushing anyway passed
/// before.
#[test]
fn every_autonomous_webhook_door_reads_the_pause() {
    let raw = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/webhook/webhook_handlers.rs"),
    )
    .expect("the webhook handlers exist");
    let src = anvil::source_scan::code_only(&raw);

    let doors: Vec<usize> = src.match_indices("tokio::spawn(").map(|(i, _)| i).collect();
    assert!(
        !doors.is_empty(),
        "no detached tasks found in the webhook handlers. If they moved, this \
         test must follow them -- a scan that stops finding its subject is not \
         a fix."
    );

    let mut open = Vec::new();
    for at in doors {
        let line = src[..at].matches('\n').count() + 1;
        let body = match spawn_body(&src, at) {
            Some(b) => b,
            None => panic!(
                "could not find the end of the task spawned at \
                 src/webhook/webhook_handlers.rs:{line}. `code_only` does not \
                 model raw strings, and a scan that was fooled must not guess."
            ),
        };
        let read = body.find(".holds(");
        let stops = read.is_some_and(|r| body[r..].contains("return"));
        if !(body.contains("pause") && stops) {
            open.push(format!(
                "src/webhook/webhook_handlers.rs:{line}{}",
                if read.is_some() {
                    " (reads the pause and carries on)"
                } else {
                    ""
                }
            ));
        }
    }

    assert!(
        open.is_empty(),
        "{} detached task(s) act with no human in the loop and are not \
         stoppable: {}.\n\
         Anything this handler spawns runs unattended, so it must read the \
         pause and return. If a new door genuinely takes no authority -- it \
         only reads, or only answers -- it still needs a read here, or an \
         escape hatch naming the reason, which is what this tree does with \
         `SubjectRoot::asserted`.",
        open.len(),
        open.join(", ")
    );
}

/// The body of the task spawned at `at`, by brace depth over code-only text.
///
/// Refuses rather than guesses when the depth does not return to zero, because
/// `code_only` does not model raw strings and a scan that was fooled must not
/// answer. That is the third cut this tree has taken at this shape; the first
/// two counted braces over text that kept literal bodies and ran to end of
/// file on one unbalanced brace.
fn spawn_body(src: &str, at: usize) -> Option<&str> {
    let open = at + src[at..].find('{')?;
    let mut depth = 0i32;
    for (offset, b) in src[open..].bytes().enumerate() {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[open..open + offset + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

/// The switch must read the directory the daemon was configured with.
///
/// `Pause::in_dir("data")` compiles, passes every fixture in this file, and is
/// silently inert under `DATA_DIR=/var/lib/anvil`: `present()` maps `NotFound`
/// to "not paused", so a wrong directory is indistinguishable from no pause at
/// all, and the `Unreadable` fail-closed path does not fire. Every other
/// fixture here builds its own `Pause`, so none of them can see how the live
/// one is wired.
#[test]
fn the_daemon_points_the_pause_at_its_configured_data_directory() {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs"),
    )
    .expect("the daemon's composition root exists");

    let at = src.find("Pause::in_dir(").unwrap_or_else(|| {
        panic!(
            "the daemon no longer constructs a Pause. If that moved, this test \
             must follow it -- a scan that stops finding its subject is not a fix."
        )
    });
    let call: String = src[at..].chars().take(120).collect();
    assert!(
        call.contains("config.data_dir"),
        "the pause is wired to a literal rather than to the configured data \
         directory, so an operator who set DATA_DIR touches a file nothing \
         reads:\n  {}",
        call.lines().next().unwrap_or_default()
    );
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
