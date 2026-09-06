use super::*;

mod runtime;

fn rejection(command: Command) -> String {
    NonModelCommand::checked(command)
        .err()
        .expect("command must be rejected")
        .to_string()
}

#[test]
fn generic_raw_runner_cannot_mint_the_private_curl_capability() {
    assert!(rejection(Command::new("curl")).contains("finite non-model"));
}

#[cfg(unix)]
#[test]
fn osv_environment_clear_survives_canonical_rebinding_and_launch() {
    use std::path::Path;

    let mut request = Command::new("curl");
    super::net::apply_from(
        &mut request,
        [
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
            ("HOME".to_owned(), "/tmp/hostile-curlrc-home".to_owned()),
            ("OPENAI_API_KEY".to_owned(), "provider-secret".to_owned()),
        ],
    );

    // Use `env` as a harmless stand-in for the already-resolved curl binary:
    // this is the exact rebinder used immediately before transport launch.
    let mut rebound = bind_std_program(request.as_std(), Path::new("/usr/bin/env"), "env");
    let output = rebound.output().expect("launch rebound environment probe");
    assert!(output.status.success());
    let child_environment = String::from_utf8(output.stdout).expect("UTF-8 environment");
    assert!(child_environment.contains("PATH=/usr/bin:/bin"));
    assert!(!child_environment.contains("HOME="));
    assert!(!child_environment.contains("OPENAI_API_KEY="));
    assert!(!child_environment.contains(CLEARED_ENV_MARKER));
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
fn arbitrary_multicall_and_versioned_name_lookalikes_are_rejected() {
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
        let error = rejection(Command::new(alias));
        assert!(error.contains("trusted installed executable"), "{error}");
    }
}

#[cfg(unix)]
#[test]
fn an_arbitrary_regular_executable_cannot_gain_git_authority_by_name() {
    use std::os::unix::fs::PermissionsExt;

    let scratch = tempfile::tempdir().expect("scratch directory");
    let impostor = scratch.path().join("git");
    std::fs::write(&impostor, "#!/bin/sh\nexit 0\n").expect("impostor body");
    std::fs::set_permissions(&impostor, std::fs::Permissions::from_mode(0o755))
        .expect("executable impostor");

    let error = rejection(Command::new(impostor));
    assert!(error.contains("trusted installed executable"), "{error}");
}

#[cfg(unix)]
#[test]
fn unix_multicall_aliases_require_arg0_restoration() {
    assert!(canonical_name_is_admitted("cargo", "rustup"));
    assert!(canonical_name_is_admitted("npm", "npm-cli.js"));
    assert!(!canonical_name_is_admitted("npm", "npm.cmd"));
}

#[cfg(windows)]
#[test]
fn windows_npm_shim_is_the_only_admitted_launcher_alias() {
    assert!(canonical_name_is_admitted("npm", "npm.cmd"));
    assert!(!canonical_name_is_admitted("cargo", "rustup"));
    assert!(!canonical_name_is_admitted("npm", "npm-cli.js"));
    assert!(!canonical_name_is_admitted("git", "git.cmd"));
    assert!(!canonical_name_is_admitted("git", "cmd"));
}

#[test]
fn versioned_python_alias_policy_is_finite() {
    assert!(!canonical_name_is_admitted("python3", "python3.latest"));
    assert!(!canonical_name_is_admitted("python3", "python3.14."));
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
