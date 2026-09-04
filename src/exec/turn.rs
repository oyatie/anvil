//! One model turn: the prompt on STDIN, the answer out of the stream.
//!
//! Five sites passed the prompt as an argv value to `--print`. argv is world-
//! readable through `ps` and is recorded by process accounting, and the prompts
//! carry the diff, review-comment bodies, conflict text and PR titles -- text
//! an outsider wrote. Putting it on STDIN costs nothing and removes the
//! disclosure.
//!
//! agy has no plain "read the prompt from stdin" spelling: `--print` is a flag
//! that takes a value, and `--print ""` with the default text input answers
//! `Error: empty prompt` -- measured against the installed CLI. The one input
//! mode that reads stdin is `--input-format stream-json`, which requires
//! `--output-format stream-json`, so the answer arrives as a `result` event
//! rather than as plain stdout. [`crate::exec::agy_agent`] builds that argv and [`run`]
//! delivers the prompt and unwraps the stream, so a call site sees the same
//! `String` it saw before.

use anyhow::{Result, bail};
use std::process::ExitStatus;
use std::time::Duration;

use crate::exec::AgentCommand;
use crate::model_prompt::ModelPrompt;

/// Pulls the final response out of agy's NDJSON event stream.
///
/// `--input-format stream-json` requires `--output-format stream-json`, so the
/// answer arrives as a `result` event rather than as plain stdout.
///
/// A stream with no `result` event, or one whose result is not `SUCCESS`, is an
/// error -- never an empty successful review. An empty string here would reach
/// `reviewer::parse_review_response` as unparseable output, and absent evidence
/// must not be mistaken for a measurement (invariant I1).
pub fn agy_stream_response(stdout: &str) -> Result<String> {
    let mut failure: Option<String> = None;

    for line in stdout.lines() {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line.trim()) else {
            continue;
        };
        let Some(result) = event.get("result") else {
            continue;
        };

        if result.get("status").and_then(|s| s.as_str()) == Some("SUCCESS") {
            return Ok(result
                .get("response")
                .and_then(|r| r.as_str())
                .unwrap_or_default()
                .to_string());
        }
        failure = Some(
            result
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("no error was reported")
                .to_string(),
        );
    }

    match failure {
        Some(error) => bail!("agy reported a failed turn: {}", error),
        None => bail!("agy emitted no result event, so no response was obtained"),
    }
}

/// What one turn produced.
///
/// Carries the status and stderr as well as the response, because a caller that
/// classifies its own outcome -- the doc-parity probe does -- needs to tell "ran
/// and said nothing parseable" from "did not run".
pub struct Turn {
    pub status: ExitStatus,
    pub response: String,
    pub stderr: String,
}

impl Turn {
    /// The response, or an error, under the rule for a turn that edits the
    /// workspace: any non-zero exit is a failed turn. Partial output from a
    /// process that died mid-edit is not a partial repair.
    pub fn into_result(self) -> Result<String> {
        super::interpret_agy_outcome(self.status.success(), &self.response, &self.stderr)
    }
}

/// Runs `cmd`, delivering `prompt` on STDIN, and unwraps the stream.
///
/// The stream is unwrapped only for a turn that succeeded: a non-zero exit is
/// reported with agy's own stderr rather than with whatever half of the stream
/// arrived before it died.
pub async fn run(
    cmd: AgentCommand,
    prompt: &ModelPrompt,
    budget: Duration,
    what: &str,
) -> Result<Turn> {
    let output = super::agent::deliver(cmd, prompt, budget, what).await?;
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let response = if output.status.success() {
        agy_stream_response(&String::from_utf8_lossy(&output.stdout))
            .map_err(|e| anyhow::anyhow!("{}; agy stderr: {}", e, stderr.trim()))?
    } else {
        String::new()
    };
    Ok(Turn {
        status: output.status,
        response,
        stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_successful_result_event_yields_the_response() {
        let stream = "{\"event\":\"start\"}\n                      {\"result\":{\"status\":\"SUCCESS\",\"response\":\"the answer\"}}\n";
        assert_eq!(agy_stream_response(stream).expect("parses"), "the answer");
    }

    /// A stream with no result is not an empty successful turn. An empty string
    /// reaches a parser as unparseable output, and absent evidence must not be
    /// mistaken for a measurement.
    #[test]
    fn a_stream_with_no_result_is_an_error() {
        agy_stream_response("{\"event\":\"start\"}\n").expect_err("no result event");
        agy_stream_response("").expect_err("no stream at all");
    }

    #[test]
    fn a_failed_result_carries_its_reason() {
        let stream = "{\"result\":{\"status\":\"ERROR\",\"error\":\"quota exhausted\"}}\n";
        let err = agy_stream_response(stream).expect_err("a failed turn is an error");
        assert!(err.to_string().contains("quota exhausted"), "{err}");
    }

    /// The argv must carry no prompt: that is the whole point of this module.
    #[test]
    fn the_argv_carries_an_empty_print_and_the_stream_formats() {
        let cmd = crate::exec::agy_agent(
            &crate::exec::Posture::in_workspace(std::env::temp_dir()),
            "low",
            Duration::from_secs(600),
            None,
        )
        .expect("valid fixed options");
        let args: Vec<String> = cmd
            .as_std()
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        let print = args.iter().position(|a| a == "--print").expect("--print");
        assert_eq!(
            args[print + 1],
            "",
            "the prompt must not be an argv value: {args:?}"
        );
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"570s".to_string()), "{args:?}");
    }
}
