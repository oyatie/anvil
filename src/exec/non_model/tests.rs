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
fn allowed_name_symlinked_to_an_unadmitted_launcher_is_rejected() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let scratch = tempfile::tempdir().expect("scratch directory");
    let launcher = scratch.path().join("env");
    std::fs::write(&launcher, "fixture").expect("launcher fixture");
    std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755))
        .expect("executable launcher fixture");
    let alias = scratch.path().join("git");
    symlink(&launcher, &alias).expect("launcher alias");

    let error = rejection(Command::new(alias));
    assert!(
        error.contains("outside the finite non-model tool seam"),
        "{error}"
    );
}

#[cfg(unix)]
#[test]
fn finite_legitimate_multicall_and_versioned_aliases_are_admitted() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let scratch = tempfile::tempdir().expect("scratch directory");
    for (requested, canonical) in [
        ("cargo", "rustup"),
        ("npm", "npm-cli.js"),
        ("python3", "python3.14"),
    ] {
        let target = scratch.path().join(canonical);
        std::fs::write(&target, "fixture").expect("canonical fixture");
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))
            .expect("executable canonical fixture");
        let alias = scratch.path().join(requested);
        symlink(&target, &alias).expect("legitimate alias");
        NonModelCommand::checked(Command::new(alias)).expect("finite alias is admitted");
    }
}

#[test]
fn windows_npm_shim_is_the_only_admitted_command_script_alias() {
    assert!(canonical_name_is_admitted("npm", "npm.cmd"));
    assert!(!canonical_name_is_admitted("git", "git.cmd"));
    assert!(!canonical_name_is_admitted("git", "cmd"));
    assert!(!canonical_name_is_admitted("python3", "python3.latest"));
    assert!(!canonical_name_is_admitted("python3", "python3.14."));
}

#[tokio::test]
async fn real_cargo_alias_executes_with_cargo_multicall_semantics() {
    let mut command = Command::new("cargo");
    command.arg("--version");
    let checked = NonModelCommand::checked(command).expect("host cargo is admitted");
    let output = transport::run_for(
        checked,
        Duration::from_secs(10),
        "cargo alias smoke test",
        None,
    )
    .await
    .expect("host cargo executes");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("cargo "));
}

#[tokio::test]
async fn installed_npm_and_python_aliases_retain_execution_semantics() {
    for (program, expected) in [("npm", "npm"), ("python3", "Python")] {
        let mut command = Command::new(program);
        command.arg("--version");
        if resolve_executable(command.as_std()).is_none() {
            continue;
        }
        let checked = NonModelCommand::checked(command)
            .unwrap_or_else(|error| panic!("installed {program} is admitted: {error}"));
        let output = transport::run_for(
            checked,
            Duration::from_secs(30),
            "installed alias semantics smoke test",
            None,
        )
        .await
        .unwrap_or_else(|error| panic!("installed {program} executes: {error}"));
        assert!(output.status.success(), "{program} --version failed");
        let diagnostic = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if program == "npm" {
            assert!(
                diagnostic
                    .trim()
                    .starts_with(|character: char| character.is_ascii_digit()),
                "unexpected npm version: {diagnostic:?}"
            );
        } else {
            assert!(
                diagnostic.contains(expected),
                "unexpected {program} version: {diagnostic:?}"
            );
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn checked_command_rejects_path_fallback_if_the_validated_entry_disappears() {
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
    assert_eq!(
        command.as_std().get_program(),
        std::fs::canonicalize(&selected)
            .expect("canonical selected executable")
            .as_os_str()
    );
    std::fs::remove_file(&selected).expect("remove validated executable");
    let fallback = first.join("git");
    std::fs::write(&fallback, "#!/bin/sh\nexit 0\n").expect("fallback executable");
    std::fs::set_permissions(&fallback, std::fs::Permissions::from_mode(0o755))
        .expect("fallback executable permissions");
    let error = transport::run_for(
        NonModelCommand(command),
        Duration::from_secs(10),
        "removed admitted executable",
        None,
    )
    .await
    .expect_err("the retained PATH must not select a fallback after admission");
    assert!(error.to_string().contains("failed to run"), "{error}");
}

#[cfg(unix)]
#[tokio::test]
async fn checked_command_preserves_path_for_admitted_tool_descendants() {
    use std::os::unix::fs::symlink;

    let scratch = tempfile::tempdir().expect("scratch directory");
    let descendant_dir = scratch.path().join("descendant");
    std::fs::create_dir_all(&descendant_dir).expect("descendant directory");
    let git = resolve_executable(Command::new("git").as_std())
        .expect("installed git")
        .canonical;
    let echo = resolve_executable(Command::new("echo").as_std())
        .expect("installed echo")
        .canonical;
    symlink(echo, descendant_dir.join("git-anvil-descendant"))
        .expect("descendant executable symlink");

    let path = std::env::join_paths([&descendant_dir]).expect("fixture PATH");
    let mut command = Command::new(git);
    command.args(["anvil-descendant", "descendant-ok"]);
    command.env("PATH", &path);
    let checked = NonModelCommand::checked(command).expect("safe runnable selected");
    let output = transport::run_for(
        checked,
        Duration::from_secs(10),
        "descendant PATH smoke test",
        None,
    )
    .await
    .expect("admitted tool can find its descendant");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "descendant-ok"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn status_transport_never_hands_the_callers_stdin_to_a_forwarder() {
    let scratch = tempfile::tempdir().expect("scratch directory");
    let input = scratch.path().join("operator-input");
    std::fs::write(&input, "must-not-reach-forwarder\n").expect("operator input fixture");

    // Wrap the already-installed `grep` directly so this unit test exercises
    // the transport's stdin normalization rather than admission's rebinding.
    // `grep` exits 0 if it receives the fixture and 1 on the transport's null
    // stdin, without creating a new executable that macOS may provenance-scan.
    let mut command = Command::new("grep");
    command.args(["-q", "."]);
    command.stdin(std::fs::File::open(input).expect("operator input handle"));
    let status = transport::run_status(NonModelCommand(command), "forwarder stdin isolation")
        .await
        .expect("status fixture executes");
    assert_eq!(status.code(), Some(1), "forwarder consumed caller stdin");
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
