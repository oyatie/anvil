//! Finite construction seam for commands that may receive a `ModelPrompt`.
//!
//! This is a child of `exec::agent`, so it alone can reach the private command
//! constructor and argv mutator. Adding a provider or flag requires an explicit
//! edit to this module; contributor text cannot be appended at a call site.

use anyhow::{Result, bail};
use std::ffi::OsStr;
use std::path::Path;
use std::time::Duration;

use super::{AgentCommand, Framing, Posture, ProviderProbeCommand};

const MAX_MODEL_SELECTOR_BYTES: usize = 128;
const PROVIDER_PROGRAMS: &[&str] = &[
    "agy",
    "claude",
    "codex",
    "cursor",
    "cursor-agent",
    "gemini",
    "grok",
];

pub(in crate::exec) fn is_provider_program(program: &OsStr) -> bool {
    let name = Path::new(program)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .trim_end_matches(".exe");
    PROVIDER_PROGRAMS.contains(&name)
}

fn validate_model_selector(value: &str) -> Result<()> {
    let allowed = |byte: u8| {
        byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'.' | b'_' | b'/' | b':' | b'@' | b'+')
    };
    if value.is_empty()
        || value.len() > MAX_MODEL_SELECTOR_BYTES
        || value.starts_with('-')
        || !value.bytes().all(allowed)
    {
        bail!("invalid provider model selector: {value:?}");
    }
    Ok(())
}

fn validate_effort(value: &str) -> Result<()> {
    if !matches!(value, "low" | "medium" | "high" | "xhigh" | "max" | "ultra") {
        bail!("invalid agy reasoning effort: {value:?}");
    }
    Ok(())
}

pub(super) fn agy_help_probe() -> ProviderProbeCommand {
    let mut command = tokio::process::Command::new("agy");
    command.arg("--help");
    ProviderProbeCommand(command)
}

pub fn claude_agent(posture: &Posture, model: &str) -> Result<AgentCommand> {
    validate_model_selector(model)?;
    let mut cmd = super::command("claude", posture, Framing::Plain);
    cmd.args(["-p", "--model", model]);
    Ok(cmd)
}

pub fn codex_agent(posture: &Posture, model: &str) -> Result<AgentCommand> {
    validate_model_selector(model)?;
    let mut cmd = super::command("codex", posture, Framing::Plain);
    cmd.args(["exec", "-", "--model", model]);
    Ok(cmd)
}

pub fn cursor_agent(posture: &Posture, model: Option<&str>) -> Result<AgentCommand> {
    let mut cmd = super::command("cursor", posture, Framing::Plain);
    cmd.args(["agent", "--print"]);
    if let Some(model) = model {
        validate_model_selector(model)?;
        cmd.args(["--model", model]);
    }
    Ok(cmd)
}

pub fn grok_agent(posture: &Posture, model: &str) -> Result<AgentCommand> {
    validate_model_selector(model)?;
    let mut cmd = super::command("grok", posture, Framing::Plain);
    cmd.args(["--prompt-file", "/dev/stdin", "--model", model]);
    Ok(cmd)
}

pub fn agy_agent(
    posture: &Posture,
    effort: &str,
    budget: Duration,
    model: Option<&str>,
) -> Result<AgentCommand> {
    validate_effort(effort)?;
    if let Some(model) = model {
        validate_model_selector(model)?;
    }
    let timeout = crate::exec::agy_print_timeout_arg(budget);
    let mut cmd = super::command("agy", posture, Framing::AgyStreamJson);
    cmd.args([
        "--print",
        "",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--effort",
        effort,
        "--print-timeout",
        &timeout,
        "--dangerously-skip-permissions",
    ]);
    if let Some(model) = model {
        cmd.args(["--model", model]);
    }
    Ok(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(cmd: &AgentCommand) -> Vec<String> {
        cmd.as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn agy_argv_is_complete_and_prompt_free() {
        let cmd = agy_agent(
            &Posture::in_workspace("."),
            "high",
            Duration::from_secs(600),
            Some("gpt-5.6-sol"),
        )
        .expect("valid selectors");
        let args = args(&cmd);
        let print = args.iter().position(|arg| arg == "--print").unwrap();
        assert_eq!(args[print + 1], "");
        assert!(args.windows(2).any(|w| w == ["--print-timeout", "570s"]));
        assert!(args.windows(2).any(|w| w == ["--model", "gpt-5.6-sol"]));
    }

    #[test]
    fn every_provider_argv_keeps_prompt_on_stdin_and_metadata_in_its_exact_slot() {
        let posture = Posture::in_workspace(".");
        let model = "sentinel-model";

        assert_eq!(
            args(&claude_agent(&posture, model).expect("valid selector")),
            ["-p", "--model", model]
        );
        assert_eq!(
            args(&codex_agent(&posture, model).expect("valid selector")),
            ["exec", "-", "--model", model]
        );
        assert_eq!(
            args(&cursor_agent(&posture, None).expect("optional selector")),
            ["agent", "--print"]
        );
        assert_eq!(
            args(&cursor_agent(&posture, Some(model)).expect("valid selector")),
            ["agent", "--print", "--model", model]
        );
        assert_eq!(
            args(&grok_agent(&posture, model).expect("valid selector")),
            ["--prompt-file", "/dev/stdin", "--model", model]
        );
        assert_eq!(
            args(
                &agy_agent(&posture, "high", Duration::from_secs(600), Some(model),)
                    .expect("valid selectors"),
            ),
            [
                "--print",
                "",
                "--input-format",
                "stream-json",
                "--output-format",
                "stream-json",
                "--effort",
                "high",
                "--print-timeout",
                "570s",
                "--dangerously-skip-permissions",
                "--model",
                model,
            ]
        );
    }

    #[test]
    fn dynamic_provider_options_reject_argv_syntax() {
        let posture = Posture::in_workspace(".");
        for invalid in ["--model", "safe\n--prompt=attack"] {
            assert!(claude_agent(&posture, invalid).is_err());
            assert!(codex_agent(&posture, invalid).is_err());
            assert!(cursor_agent(&posture, Some(invalid)).is_err());
            assert!(grok_agent(&posture, invalid).is_err());
            assert!(agy_agent(&posture, "high", Duration::from_secs(600), Some(invalid)).is_err());
        }
        assert!(agy_agent(&posture, "high\n--model", Duration::from_secs(600), None).is_err());
    }

    #[test]
    fn registry_recognises_variables_alias_paths_and_legacy_provider_names() {
        let variable = String::from("agy");
        assert!(is_provider_program(OsStr::new(&variable)));
        assert!(is_provider_program(OsStr::new(
            "/usr/local/bin/cursor-agent"
        )));
        assert!(is_provider_program(OsStr::new("gemini")));
        assert!(!is_provider_program(OsStr::new("cargo")));
    }

    #[test]
    fn provider_presence_probe_has_one_finite_prompt_free_argument() {
        let probe = agy_help_probe();
        let args: Vec<_> = probe.0.as_std().get_args().collect();
        assert_eq!(args, ["--help"]);
    }
}
