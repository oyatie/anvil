use super::*;

fn rejection(command: Command) -> String {
    NonModelCommand::checked(command)
        .err()
        .expect("command must be rejected")
        .to_string()
}

#[test]
fn variable_absolute_env_and_shell_provider_aliases_are_rejected() {
    let variable = String::from("agy");
    assert!(rejection(Command::new(variable)).contains("typed AgentCommand"));
    assert!(rejection(Command::new("/usr/local/bin/claude")).contains("typed AgentCommand"));
    assert!(rejection(Command::new("env")).contains("finite non-model"));
    assert!(rejection(Command::new("/bin/sh")).contains("finite non-model"));
}

#[cfg(unix)]
#[test]
fn an_allowed_name_symlinked_to_a_provider_is_rejected() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let scratch = tempfile::tempdir().expect("scratch directory");
    let provider = scratch.path().join("agy");
    std::fs::write(&provider, "fixture").expect("provider fixture");
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755))
        .expect("executable provider fixture");
    let alias = scratch.path().join("git");
    symlink(&provider, &alias).expect("provider alias");

    let error = rejection(Command::new(alias));
    assert!(
        error.contains("alias resolves to a model provider"),
        "{error}"
    );
}

#[cfg(unix)]
#[test]
fn path_resolution_skips_nonexecutables_and_rejects_the_runnable_provider_alias() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let scratch = tempfile::tempdir().expect("scratch directory");
    let first = scratch.path().join("first");
    let second = scratch.path().join("second");
    std::fs::create_dir_all(&first).expect("first PATH entry");
    std::fs::create_dir_all(&second).expect("second PATH entry");
    let decoy = first.join("git");
    std::fs::write(&decoy, "not executable").expect("PATH decoy");
    std::fs::set_permissions(&decoy, std::fs::Permissions::from_mode(0o644))
        .expect("non-executable decoy");
    let provider = second.join("agy");
    std::fs::write(&provider, "provider fixture").expect("provider fixture");
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755))
        .expect("executable provider fixture");
    symlink(&provider, second.join("git")).expect("provider alias");

    let mut command = Command::new("git");
    command.env(
        "PATH",
        std::env::join_paths([&first, &second]).expect("hostile PATH"),
    );
    let error = rejection(command);
    assert!(
        error.contains("alias resolves to a model provider"),
        "{error}"
    );
}

#[cfg(unix)]
#[test]
fn checked_command_locks_path_to_the_runnable_entry_it_validated() {
    use std::os::unix::fs::PermissionsExt;

    let scratch = tempfile::tempdir().expect("scratch directory");
    let first = scratch.path().join("first");
    let second = scratch.path().join("second");
    std::fs::create_dir_all(&first).expect("first PATH entry");
    std::fs::create_dir_all(&second).expect("second PATH entry");
    std::fs::write(first.join("git"), "decoy").expect("PATH decoy");
    let selected = second.join("git");
    std::fs::write(&selected, "#!/bin/sh\nexit 0\n").expect("selected executable");
    std::fs::set_permissions(&selected, std::fs::Permissions::from_mode(0o755))
        .expect("selected executable permissions");

    let mut command = Command::new("git");
    command.env(
        "PATH",
        std::env::join_paths([&first, &second]).expect("ordered PATH"),
    );
    let NonModelCommand(command) =
        NonModelCommand::checked(command).expect("safe runnable selected");
    let path = command
        .as_std()
        .get_envs()
        .find_map(|(name, value)| (name == OsStr::new("PATH")).then_some(value).flatten())
        .expect("checked PATH");
    let locked_first = std::env::split_paths(path)
        .next()
        .expect("first PATH entry");
    assert_eq!(
        locked_first,
        std::fs::canonicalize(&second).expect("canonical selected directory")
    );
}

#[test]
fn even_prompt_free_provider_probes_cannot_enter_a_raw_runner() {
    let mut probe = Command::new("agy");
    probe.arg("--help");
    assert!(rejection(probe).contains("typed AgentCommand"));

    let mut turn = Command::new("agy");
    turn.args(["--print", "review body"]);
    assert!(rejection(turn).contains("typed AgentCommand"));
}
