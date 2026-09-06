use anyhow::Result;

use crate::model_prompt::{HarnessText, ModelPrompt};
use crate::reviewer::untrusted::{Untrusted, UntrustedLabel};

/// Preserve the captured status signal even when failure has no diagnostic text.
/// Keep stdout then stderr with one separator when both exist, without trimming
/// or an early cap. This preserves channel text, not chronological interleaving.
pub(super) fn merge_conflict_details(output: &std::process::Output) -> Option<String> {
    if output.status.success() {
        return None;
    }
    let mut details = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !details.is_empty() && !stderr.is_empty() {
        details.push('\n');
    }
    details.push_str(&stderr);
    Some(details)
}

/// Builds the exact write-capable queue-repair prompt. Branch names and merge
/// diagnostics remain typed data while their roles and the repair task remain
/// trusted harness text.
pub fn build_queue_repair_prompt(
    repo: &str,
    pr_number: u64,
    base_branch: &str,
    head_branch: &str,
    conflict_details: Option<&str>,
) -> Result<ModelPrompt> {
    let mut prompt = ModelPrompt::builder();
    prompt
        .push_harness(HarnessText::QueuePreamble)
        .push_u64(pr_number)
        .push_harness(HarnessText::QueueRepositoryStart);
    prompt.push_repository(repo)?;
    prompt
        .push_harness(HarnessText::QueueContextAndBaseBranch)
        .push_untrusted(Untrusted::new(UntrustedLabel::BranchName, base_branch))
        .push_harness(HarnessText::QueueHeadBranch)
        .push_untrusted(Untrusted::new(UntrustedLabel::BranchName, head_branch));
    if let Some(details) = conflict_details {
        prompt
            .push_harness(HarnessText::QueueConflictPresent)
            .push_untrusted(Untrusted::new(UntrustedLabel::MergeConflict, details));
    } else {
        prompt.push_harness(HarnessText::QueueNoTextConflict);
    }
    prompt.push_harness(HarnessText::QueueRepairTask);
    prompt.finish()
}

#[cfg(all(test, any(unix, windows)))]
mod tests {
    use super::merge_conflict_details;
    use std::process::{ExitStatus, Output};

    fn captured(success: bool, stdout: &[u8], stderr: &[u8]) -> Output {
        #[cfg(unix)]
        let status = {
            use std::os::unix::process::ExitStatusExt;
            ExitStatus::from_raw(if success { 0 } else { 1 << 8 })
        };
        #[cfg(windows)]
        let status = {
            use std::os::windows::process::ExitStatusExt;
            ExitStatus::from_raw(if success { 0 } else { 1 })
        };
        Output {
            status,
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
        }
    }

    #[test]
    fn failed_merge_details_retain_stdout_and_stderr() {
        for (stdout, stderr, expected) in [
            ("ordinary output", "", "ordinary output"),
            ("", "ordinary diagnostic", "ordinary diagnostic"),
            (
                "ordinary output",
                "ordinary diagnostic",
                "ordinary output\nordinary diagnostic",
            ),
            (
                "ordinary output\n",
                "ordinary diagnostic\n",
                "ordinary output\n\nordinary diagnostic\n",
            ),
        ] {
            let output = captured(false, stdout.as_bytes(), stderr.as_bytes());
            assert_eq!(merge_conflict_details(&output).as_deref(), Some(expected));
        }
    }

    #[test]
    fn failed_merge_without_text_remains_present() {
        assert_eq!(
            merge_conflict_details(&captured(false, b"", b"")).as_deref(),
            Some("")
        );
    }

    #[test]
    fn successful_merge_has_no_conflict_details() {
        for (stdout, stderr) in [("", ""), ("ordinary output", "ordinary diagnostic")] {
            assert!(
                merge_conflict_details(&captured(true, stdout.as_bytes(), stderr.as_bytes()))
                    .is_none()
            );
        }
    }

    #[test]
    fn merge_diagnostics_preserve_lossy_decoding_compatibility() {
        let output = captured(false, &[b'a', 0xff], &[0xfe, b'b']);
        assert_eq!(
            merge_conflict_details(&output).as_deref(),
            Some("a\u{fffd}\n\u{fffd}b")
        );
    }
}
