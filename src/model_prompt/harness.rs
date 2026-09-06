//! Closed vocabulary for harness-authored model-prompt text.

/// The complete vocabulary of harness-authored text that may enter a model
/// prompt without an `Untrusted` wrapper.
///
/// This is an enum rather than an `&'static str` wrapper on purpose. Safe Rust
/// cannot forge a new variant containing runtime text, including text promoted
/// to `'static` with `Box::leak`. Adding trusted prose therefore requires an
/// explicit, reviewable change at this boundary.
pub(crate) enum HarnessText {
    ReviewerPreambleAndRepository,
    ReviewerPrNumber,
    ReviewerMode,
    ReviewerIncrementalMode,
    ReviewerUnknownPreviousSha,
    ReviewerToCurrentHead,
    ReviewerFullMode,
    ReviewerBaseBranch,
    ReviewerRubricHeading,
    ReviewerAspect(usize),
    ReviewerRubricLensHeading,
    ReviewerStance(usize),
    ReviewerRubricEnd,
    ReviewerResponseFormat,
    DocGuardPreambleAndRepository,
    DocGuardPrNumber,
    DocGuardMetadataEnd,
    DocGuardResponseContract,
    FixApplyPreambleAndRepository,
    FixApplyRepositoryEnd,
    FixApplyItemStart,
    FixApplyItemHeaderEnd,
    FixApplyItemEnd,
    FixApplyMissingPath,
    FixApplyMissingProposal,
    FixApplyTask,
    FixSelfCorrectionPreamble,
    FixSelfCorrectionTask,
    EvaluatorPreambleAndRepository,
    EvaluatorRepositoryEnd,
    EvaluatorItemStart,
    EvaluatorItemEnd,
    EvaluatorGeneralPath,
    EvaluatorLine,
    EvaluatorNotApplicable,
    EvaluatorFieldEnd,
    EvaluatorItemBoundary,
    EvaluatorResponseContract,
    CiPreambleAndRepository,
    CiRunId,
    CiCommitSha,
    CiUnknownCommitSha,
    CiMetadataEnd,
    CiResponseContract,
    QueuePreamble,
    QueueRepositoryStart,
    QueueContextAndBaseBranch,
    QueueHeadBranch,
    QueueConflictPresent,
    QueueNoTextConflict,
    QueueRepairTask,
    QueueRetryTask,
    SubscriptionProbeTask,
}

impl HarnessText {
    pub(super) fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::ReviewerResponseFormat
                | Self::DocGuardResponseContract
                | Self::FixApplyTask
                | Self::FixSelfCorrectionTask
                | Self::EvaluatorResponseContract
                | Self::CiResponseContract
                | Self::QueueRepairTask
                | Self::QueueRetryTask
                | Self::SubscriptionProbeTask
        )
    }

    pub(super) fn append_to(self, rendered: &mut String) {
        match self {
            Self::ReviewerAspect(index) => {
                let (aspect, question) = crate::reviewer::rubric::REVIEW_ASPECTS
                    .get(index)
                    .expect("review aspect token index is in range");
                rendered.push('A');
                rendered.push_str(&(index + 1).to_string());
                rendered.push_str(". ");
                rendered.push_str(aspect);
                rendered.push_str(": ");
                rendered.push_str(question);
                rendered.push('\n');
            }
            Self::ReviewerStance(index) => {
                let stance = crate::reviewer::rubric::REVIEW_STANCES
                    .get(index)
                    .expect("review stance token index is in range");
                rendered.push_str(&(index + 1).to_string());
                rendered.push_str(". ");
                rendered.push_str(stance);
                rendered.push('\n');
            }
            Self::ReviewerRubricLensHeading => {
                rendered.push_str("\n## Canonical ");
                rendered.push_str(&crate::reviewer::rubric::REVIEW_STANCES.len().to_string());
                rendered.push_str("-Lens Adversarial Review Rubric — how to look:\n");
            }
            fixed => rendered.push_str(fixed.fixed_text()),
        }
    }

    fn fixed_text(&self) -> &'static str {
        match self {
            Self::ReviewerPreambleAndRepository => {
                "You are Anvil, the Autonomous Code Review & Adversarial Quality Sentinel for Oyatie and Console.\n\
                 You evaluate Pull Requests using a 16-Lens Canonical Reasoning Framework and emit structured JSON reviews.\n\
                 \n## PR Metadata:\n- **Repository**: "
            }
            Self::ReviewerPrNumber => "\n- **PR Number**: #",
            Self::ReviewerMode => "\n- **Mode**: ",
            Self::ReviewerIncrementalMode => {
                "INCREMENTAL REVIEW (Delta commits since previous review SHA "
            }
            Self::ReviewerUnknownPreviousSha => "unknown",
            Self::ReviewerToCurrentHead => " to current HEAD ",
            Self::ReviewerFullMode => "FULL PR REVIEW (Head: ",
            Self::ReviewerBaseBranch => ")\n**Base Branch:**\n",
            Self::ReviewerRubricHeading => {
                "## What you MUST examine (report on every one; say so if an aspect does not apply):\n"
            }
            Self::ReviewerRubricEnd => "\n",
            Self::ReviewerResponseFormat => {
                "## Response Format Instructions:\n\
                 You MUST respond with a single valid JSON object enclosed in a ```json codeblock.\n\
                 Schema:\n\
                 {\n  \"summary\": \"Markdown summary with 16-lens table, executive overview, and critical risks\",\n  \"verdict\": \"APPROVE | COMMENT | REQUEST_CHANGES\",\n  \"comments\": [{\"path\": \"file.ext\", \"line\": 42, \"side\": \"RIGHT\", \"body\": \"Finding description\"}]\n}\n"
            }
            Self::DocGuardPreambleAndRepository => {
                "You are Oyatie's Principal Documentation Architect. Evaluate whether this Pull Request has sufficient documentation parity or if documentation must be updated.\n\n## Pull Request Information:\n- **Repository**: "
            }
            Self::DocGuardPrNumber => "\n- **PR Number**: #",
            Self::DocGuardMetadataEnd => "\n",
            Self::DocGuardResponseContract => {
                r####"## Evaluation Criteria:
1. **API / Public Interface Changes**: Are new public functions, types, routes, or CLI flags introduced without docstrings or `docs/reference/` updates?
2. **Architectural / Doctrine Shifts**: Does this change introduce a new architectural decision, storage pattern, cell boundary, or platform contract that requires an ADR (in `docs/adr/` or `docs/decisions/`)?
3. **User-Facing / Config Changes**: Does `README.md`, `CLAUDE.md`, `AGENTS.md`, or runbooks need updating?
4. **Changelog**: Does `CHANGELOG.md` need a release note entry?

## Output Format:
Output strictly valid JSON matching this schema:
```json
{
  "is_doc_sufficient": false,
  "missing_doc_summary": "Explanation of what documentation or ADR is missing",
  "doc_files_to_update": ["docs/reference/feature.md", "CHANGELOG.md"],
  "suggested_adr_title": null
}
```

Note: If documentation is already sufficient, set `is_doc_sufficient: true`, `missing_doc_summary: null`, `doc_files_to_update: []`.
"####
            }
            Self::FixApplyPreambleAndRepository => {
                "You are Oyatie's Principal Engineer. Directly implement code fixes in this workspace for `"
            }
            Self::FixApplyRepositoryEnd => "` to resolve the following valid review findings:\n\n",
            Self::FixApplyItemStart => "### BEGIN VALID REVIEW ITEM [",
            Self::FixApplyItemHeaderEnd => "]\n",
            Self::FixApplyItemEnd => "\n--- END VALID REVIEW ITEM ---\n",
            Self::FixApplyMissingPath => "## File Path\nN/A\n",
            Self::FixApplyMissingProposal => "## Proposed Fix\nFix as required\n",
            Self::FixApplyTask => {
                "Inspect the workspace files, make all necessary edits cleanly, ensure types and tests are preserved or updated, and complete the implementation."
            }
            Self::FixSelfCorrectionPreamble => {
                "The previous code edits caused build or test failures. Inspect the repository, check the current diff, diagnose the root cause, and fix the errors so that the test suite passes cleanly.\n\n"
            }
            Self::FixSelfCorrectionTask => {
                "Use the workspace and test failures as the authority. Make only the repair requested above, then leave the corrected files in the working tree."
            }
            Self::EvaluatorPreambleAndRepository => {
                "You are Oyatie's Senior Principal Engineer. Evaluate the following code review feedback items for repository `"
            }
            Self::EvaluatorRepositoryEnd => {
                "` to determine if each item is a **Valid Issue** or a **False Signal**.\n\n## Review Feedback Items:\n"
            }
            Self::EvaluatorItemStart => "### Item [",
            Self::EvaluatorItemEnd => "]\n",
            Self::EvaluatorGeneralPath => "## File Path\nGeneral PR\n",
            Self::EvaluatorLine => "- **Line**: ",
            Self::EvaluatorNotApplicable => "N/A",
            Self::EvaluatorFieldEnd => "\n",
            Self::EvaluatorItemBoundary => "\n--- END REVIEW FEEDBACK ITEM ---\n",
            Self::EvaluatorResponseContract => {
                r####"## Evaluation Instructions:
1. Cross-reference each comment with the actual codebase in this workspace.
2. Determine:
   - `is_valid`: `true` if this is a legitimate bug, missing type validation, concurrency issue, security risk, or performance regression requiring code changes.
   - `is_valid`: `false` if this is a false positive, misunderstood intent, already handled by another layer, or invalid suggestion.
3. Provide a clear technical `rationale` for each decision.

## Output Format:
Return strictly valid JSON matching this schema:
```json
{
  "evaluations": [
    {
      "item_index": 0,
      "is_valid": true,
      "rationale": "Clear technical explanation of why valid or why false signal",
      "files_to_edit": ["path/to/file.ext"],
      "proposed_fix": "Description of exact change needed"
    }
  ]
}
```
"####
            }
            Self::CiPreambleAndRepository => {
                "You are Antigravity's Principal Infrastructure & Trunk Reliability Engineer. Conduct an automated Root Cause Diagnosis for a failed CI workflow.\n\n## Failure Context:\n- **Repository**: "
            }
            Self::CiRunId => "\n- **Workflow Run ID**: #",
            Self::CiCommitSha => "\n- **Commit SHA**: ",
            Self::CiUnknownCommitSha => "unknown",
            Self::CiMetadataEnd => "\n",
            Self::CiResponseContract => {
                r####"## Instructions:
1. Identify the exact error: compilation error, panicking test assertion, timing/flake hazard, network timeout, or infrastructure crash.
2. Pinpoint the culprit file and line number if visible in the stack trace.
3. Formulate clear, actionable remediation steps.

## Output Format:
Output strictly valid JSON matching this schema:
```json
{
  "failure_category": "COMPILATION | TEST_PANIC | TIMING_FLAKE | INFRASTRUCTURE | LINT_TYPE",
  "root_cause": "Concise 1-2 sentence explanation of the failure mechanism",
  "culprit_file_and_line": "path/to/file.rs:42",
  "actionable_remediation": "Clear instructions for fixing the problem",
  "formatted_markdown": "Markdown diagnostic headed with the workflow and branch values from Failure Context, followed by an attribute table, diagnostic breakdown, and recommended remediation"
}
```
"####
            }
            Self::QueuePreamble => {
                "You are Oyatie's Principal Merge Train Resilience Engineer. Pull Request #"
            }
            Self::QueueRepositoryStart => " on repository `",
            Self::QueueContextAndBaseBranch => {
                "` failed or was ejected from the GitHub Merge Queue due to train divergence or semantic conflict against trunk.\n\n**Context:**\n**Base Branch:**\n"
            }
            Self::QueueHeadBranch => "**PR Head Branch:**\n",
            Self::QueueConflictPresent => "**Merge Conflict Status:** Merge conflicts present.\n",
            Self::QueueNoTextConflict => {
                "**Merge Conflict Status:** No textual conflict; semantic or test divergence.\n"
            }
            Self::QueueRepairTask => {
                r####"**Task:**
1. Inspect the workspace, resolve any git merge conflict markers (`<<<<<<<`), and fix any broken type definitions or API calls caused by upstream trunk changes.
2. Ensure the codebase compiles and passes all tests.
3. Do NOT commit; leave your changes in the working tree.
"####
            }
            Self::QueueRetryTask => {
                "Tests failed after merging trunk. Inspect test output, fix the errors, and ensure all tests pass. Do NOT commit."
            }
            Self::SubscriptionProbeTask => {
                "\nUse the classified probe data above only to verify that the selected subscription provider can complete a model turn."
            }
            Self::ReviewerAspect(_) | Self::ReviewerRubricLensHeading | Self::ReviewerStance(_) => {
                unreachable!("dynamic fixed-table fragments are appended separately")
            }
        }
    }
}
