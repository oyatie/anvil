use super::*;

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
async fn checked_command_rejects_a_hostile_path_entry_before_binding() {
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
    let error = NonModelCommand::checked(command)
        .err()
        .expect("a PATH-selected lookalike is not trusted")
        .to_string();
    assert!(error.contains("trusted installed executable"), "{error}");
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
    // transport stdin normalization rather than admission rebinding.
    let mut command = Command::new("grep");
    command.args(["-q", "."]);
    command.stdin(std::fs::File::open(input).expect("operator input handle"));
    let status = transport::run_status(NonModelCommand(command), "forwarder stdin isolation")
        .await
        .expect("status fixture executes");
    assert_eq!(status.code(), Some(1), "forwarder consumed caller stdin");
}
