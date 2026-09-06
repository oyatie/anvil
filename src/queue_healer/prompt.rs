use anyhow::Result;

use crate::model_prompt::{HarnessText, ModelPrompt};
use crate::reviewer::untrusted::{Untrusted, UntrustedLabel};

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
