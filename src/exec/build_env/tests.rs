//! Configuration only: no ambient environment reads or process launch.
use super::*;
use std::collections::BTreeMap;
use std::env::VarError;
use std::ffi::OsString;
use tokio::process::Command;

fn entries(cmd: &Command) -> BTreeMap<OsString, Option<OsString>> {
    cmd.as_std()
        .get_envs()
        .map(|(name, value)| (name.to_owned(), value.map(OsString::from)))
        .collect()
}

#[test]
fn only_allowed_names_are_observed_and_retained() {
    let mut cmd = Command::new("cargo");
    let mut observed = Vec::new();
    apply_from(&mut cmd, |name| {
        observed.push(name.to_owned());
        Ok(format!("synthetic-{name}"))
    });
    assert_eq!(observed, BUILD_INHERITED);
    let configured = entries(&cmd);
    for name in BUILD_INHERITED {
        assert_eq!(
            configured.get(&OsString::from(name)),
            Some(&Some(OsString::from(format!("synthetic-{name}"))))
        );
    }
    for name in NEVER_HANDED_OVER.iter().copied().chain(["UNLISTED_INPUT"]) {
        assert!(!configured.contains_key(&OsString::from(name)));
    }
}

#[test]
fn existing_overrides_are_cleared_before_allowed_values_are_set() {
    let mut cmd = Command::new("cargo");
    cmd.env("UNLISTED_INPUT", "synthetic-old")
        .env("GITHUB_WEBHOOK_SECRET", "synthetic-forbidden")
        .env("PATH", "synthetic-old-path");
    apply_from(&mut cmd, |name| match name {
        "PATH" => Ok("synthetic-new-path".to_owned()),
        _ => Err(VarError::NotPresent),
    });
    let configured = entries(&cmd);
    assert!(!configured.contains_key(&OsString::from("UNLISTED_INPUT")));
    assert!(!configured.contains_key(&OsString::from("GITHUB_WEBHOOK_SECRET")));
    assert_eq!(
        configured.get(&OsString::from("PATH")),
        Some(&Some(OsString::from("synthetic-new-path")))
    );
}

#[test]
fn observation_errors_are_omitted_but_empty_success_is_retained() {
    let mut cmd = Command::new("cargo");
    apply_from(&mut cmd, |name| match name {
        "PATH" => Err(VarError::NotPresent),
        // Supplied enum outcome, not a platform-specific encoding fixture.
        "HOME" => Err(VarError::NotUnicode(OsString::from("synthetic-error"))),
        "LANG" => Ok(String::new()),
        "TZ" => Ok("synthetic-zone".to_owned()),
        _ => Err(VarError::NotPresent),
    });
    let configured = entries(&cmd);
    for absent in ["PATH", "HOME"] {
        assert!(!configured.contains_key(&OsString::from(absent)));
    }
    assert_eq!(
        configured.get(&OsString::from("LANG")),
        Some(&Some(OsString::new()))
    );
    assert_eq!(
        configured.get(&OsString::from("TZ")),
        Some(&Some(OsString::from("synthetic-zone")))
    );
}

#[test]
fn absent_observations_still_clear_existing_overrides() {
    let mut cmd = Command::new("cargo");
    cmd.env("PATH", "synthetic-path")
        .env("UNLISTED_INPUT", "synthetic-unlisted")
        .env("ANTHROPIC_API_KEY", "synthetic-forbidden");
    apply_from(&mut cmd, |_| Err(VarError::NotPresent));
    let configured = entries(&cmd);
    for name in ["PATH", "UNLISTED_INPUT", "ANTHROPIC_API_KEY"] {
        assert!(!configured.contains_key(&OsString::from(name)));
    }
    // get_envs does not expose inheritance; source bindings pin env_clear.
}
