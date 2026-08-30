//! `anvil swap` installs the new build over the running one, in that direction.
//!
//! It did the opposite. `execute_atomic_binary_swap(staged_green, target)` was
//! called as `execute_atomic_binary_swap(&current_exe, &green_binary)`, so the
//! RUNNING binary was copied over the NEW build: the upgrade destroyed the
//! artifact it was installing, left the old one running, and printed
//! "🎉 Blue/Green Self-Replacement Successful!".
//!
//! The unit test beside the function could not see it. It calls the function
//! directly, with the arguments in the right order, so it proves the copy works
//! and says nothing about who calls it. Two `&Path` parameters of the same type
//! are transposable in silence, and only the caller was wrong.
//!
//! `BlueGreenSupervisor::plan` is the fix: the two ends arrive named, and this
//! file checks the naming rather than the copying.

use anvil::recovery::BlueGreenSupervisor;
use std::path::PathBuf;

/// The new build is the source; the running binary is what gets written over.
#[test]
fn the_new_build_is_the_source_and_the_running_binary_is_the_target() {
    let new_build = PathBuf::from("/build/target/release/anvil");
    let running = PathBuf::from("/usr/local/bin/anvil");

    let swap = BlueGreenSupervisor::plan(new_build.clone(), running.clone());

    assert_eq!(
        swap.green, new_build,
        "the swap reads from something other than the new build, so the upgrade \
         installs the wrong binary"
    );
    assert_eq!(
        swap.installed, running,
        "the swap writes over something other than the binary in place"
    );
    assert_ne!(
        swap.green, swap.installed,
        "source and target are the same path"
    );
}

/// And the command wires them that way round.
///
/// Keyed to the call, over `code_only`, so the prose beside it -- which names
/// both ends while explaining the defect -- cannot satisfy the scan.
#[test]
fn the_swap_command_names_the_new_build_as_the_source() {
    let src = anvil::source_scan::code_only(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cli/handlers.rs"),
        )
        .expect("the cli handlers exist"),
    );

    let at = src
        .find("execute_atomic_binary_swap(")
        .expect("the swap command is gone; if it moved, this test must follow it");
    let args: String = src[at..].chars().take(160).collect();

    assert!(
        args.contains("swap.green") && args.contains("swap.installed"),
        "the swap command passes two bare paths rather than the named ends of a \
         `BinarySwap`. That is how the running binary came to be copied over the \
         new build:\n{}",
        args.lines().take(6).collect::<Vec<_>>().join("\n")
    );

    let green_at = args.find("swap.green").expect("checked above");
    let installed_at = args.find("swap.installed").expect("checked above");
    assert!(
        green_at < installed_at,
        "the new build is not passed first, so it is the target rather than the \
         source and the upgrade overwrites it"
    );
}
