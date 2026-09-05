//! Finite construction seam for commands used in a `ModelPrompt` OS-STDIN handoff.
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

pub(super) fn agy_help_probe() -> Result<ProviderProbeCommand> {
    let mut command = super::trusted_provider_command("agy")?;
    let workspace = std::env::current_dir()
        .map_err(|error| anyhow::anyhow!("resolve agy probe working directory: {error}"))?;
    Posture::in_workspace(workspace).apply(&mut command);
    command.args(agy_help_args());
    Ok(ProviderProbeCommand(command))
}

pub fn claude_agent(posture: &Posture, model: &str) -> Result<AgentCommand> {
    let args = claude_args(model)?;
    let mut cmd = super::command("claude", posture, Framing::Plain)?;
    cmd.args(args);
    Ok(cmd)
}

pub fn codex_agent(posture: &Posture, model: &str) -> Result<AgentCommand> {
    let args = codex_args(model)?;
    let mut cmd = super::command("codex", posture, Framing::Plain)?;
    cmd.args(args);
    Ok(cmd)
}

pub fn cursor_agent(posture: &Posture, model: Option<&str>) -> Result<AgentCommand> {
    let args = cursor_args(model)?;
    let mut cmd = super::command("cursor", posture, Framing::Plain)?;
    cmd.args(args);
    Ok(cmd)
}

pub fn grok_agent(posture: &Posture, model: &str) -> Result<AgentCommand> {
    let args = grok_args(model)?;
    let mut cmd = super::command("grok", posture, Framing::Plain)?;
    cmd.args(args);
    Ok(cmd)
}

pub fn agy_agent(
    posture: &Posture,
    effort: &str,
    budget: Duration,
    model: Option<&str>,
) -> Result<AgentCommand> {
    let args = agy_args(effort, budget, model)?;
    let mut cmd = super::command("agy", posture, Framing::AgyStreamJson)?;
    cmd.args(args);
    Ok(cmd)
}

fn agy_help_args() -> [&'static str; 1] {
    ["--help"]
}

fn claude_args(model: &str) -> Result<Vec<String>> {
    validate_model_selector(model)?;
    Ok(vec!["-p".into(), "--model".into(), model.into()])
}

fn codex_args(model: &str) -> Result<Vec<String>> {
    validate_model_selector(model)?;
    Ok(vec![
        "exec".into(),
        "-".into(),
        "--model".into(),
        model.into(),
    ])
}

fn cursor_args(model: Option<&str>) -> Result<Vec<String>> {
    let mut args = vec!["agent".into(), "--print".into()];
    if let Some(model) = model {
        validate_model_selector(model)?;
        args.extend(["--model".into(), model.into()]);
    }
    Ok(args)
}

fn grok_args(model: &str) -> Result<Vec<String>> {
    validate_model_selector(model)?;
    Ok(vec![
        "--prompt-file".into(),
        "/dev/stdin".into(),
        "--model".into(),
        model.into(),
    ])
}

fn agy_args(effort: &str, budget: Duration, model: Option<&str>) -> Result<Vec<String>> {
    validate_effort(effort)?;
    if let Some(model) = model {
        validate_model_selector(model)?;
    }
    let timeout = crate::exec::agy_print_timeout_arg(budget);
    let mut args = vec![
        "--print".into(),
        "".into(),
        "--input-format".into(),
        "stream-json".into(),
        "--output-format".into(),
        "stream-json".into(),
        "--effort".into(),
        effort.into(),
        "--print-timeout".into(),
        timeout,
        "--dangerously-skip-permissions".into(),
    ];
    if let Some(model) = model {
        args.extend(["--model".into(), model.into()]);
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agy_argv_is_complete_and_prompt_free() {
        let args = agy_args("high", Duration::from_secs(600), Some("gpt-5.6-sol"))
            .expect("valid selectors");
        let print = args.iter().position(|arg| arg == "--print").unwrap();
        assert_eq!(args[print + 1], "");
        assert!(args.windows(2).any(|w| w == ["--print-timeout", "570s"]));
        assert!(args.windows(2).any(|w| w == ["--model", "gpt-5.6-sol"]));
    }

    #[test]
    fn every_provider_argv_keeps_prompt_on_stdin_and_metadata_in_its_exact_slot() {
        let model = "sentinel-model";

        assert_eq!(
            claude_args(model).expect("valid selector"),
            ["-p", "--model", model]
        );
        assert_eq!(
            codex_args(model).expect("valid selector"),
            ["exec", "-", "--model", model]
        );
        assert_eq!(
            cursor_args(None).expect("optional selector"),
            ["agent", "--print"]
        );
        assert_eq!(
            cursor_args(Some(model)).expect("valid selector"),
            ["agent", "--print", "--model", model]
        );
        assert_eq!(
            grok_args(model).expect("valid selector"),
            ["--prompt-file", "/dev/stdin", "--model", model]
        );
        assert_eq!(
            agy_args("high", Duration::from_secs(600), Some(model)).expect("valid selectors"),
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
        for invalid in ["--model", "safe\n--prompt=attack"] {
            assert!(claude_args(invalid).is_err());
            assert!(codex_args(invalid).is_err());
            assert!(cursor_args(Some(invalid)).is_err());
            assert!(grok_args(invalid).is_err());
            assert!(agy_args("high", Duration::from_secs(600), Some(invalid)).is_err());
        }
        assert!(agy_args("high\n--model", Duration::from_secs(600), None).is_err());
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
        assert_eq!(agy_help_args(), ["--help"]);
    }
}
