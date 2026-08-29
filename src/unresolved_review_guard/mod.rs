use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::github::GitHubClient;

pub mod thread_scanner;
pub use thread_scanner::{ThreadScanner, UnresolvedReviewThread};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedReviewReport {
    pub is_clean: bool,
    pub unresolved_threads: Vec<UnresolvedReviewThread>,
    pub summary: String,
}

/// How many review threads one query asks GitHub for.
///
/// A page boundary is not a clean bill of health, so [`parse_review_threads`]
/// refuses a truncated answer rather than judging the PR on the threads that
/// happened to fit.
pub const THREAD_PAGE_SIZE: usize = 50;

/// The unresolved threads GitHub reports, or an error saying why it did not
/// report.
///
/// This function is the whole decision: above it is a `gh` spawn, below it is
/// `is_empty()`. An empty `Vec` is what a clean pull request looks like, so
/// every way of learning nothing must be an error and not a fallthrough --
/// otherwise a non-zero `gh` exit, an unparseable body, a GraphQL error
/// payload, a thread carrying no `isResolved` field, an unresolved thread
/// whose comments did not come back, or a fifty-first thread each publishes a
/// passing gate on no evidence at all.
///
/// # Errors
///
/// Every one of those six, each with the reason. Absent evidence of resolution
/// is not evidence of resolution.
pub fn parse_review_threads(
    status_success: bool,
    stdout: &[u8],
    stderr: &str,
) -> Result<Vec<UnresolvedReviewThread>> {
    if !status_success {
        let why = stderr.trim();
        bail!(
            "the review threads could not be read, so nothing establishes that none are \
             unresolved: gh exited non-zero{}",
            if why.is_empty() {
                String::new()
            } else {
                format!(": {why}")
            }
        );
    }

    let val: serde_json::Value = serde_json::from_slice(stdout).context(
        "the review threads could not be read, so nothing establishes that none are unresolved: \
         the answer did not parse as JSON",
    )?;

    // GraphQL reports failure in-band, with HTTP 200 and a null `data`.
    if let Some(errors) = val.get("errors").and_then(|e| e.as_array())
        && !errors.is_empty()
    {
        bail!(
            "the review threads could not be read, so nothing establishes that none are \
             unresolved: GraphQL returned {} error(s), first: {}",
            errors.len(),
            errors[0]["message"].as_str().unwrap_or("(no message)")
        );
    }

    let threads = &val["data"]["repository"]["pullRequest"]["reviewThreads"];
    let Some(nodes) = threads["nodes"].as_array() else {
        bail!(
            "the review threads could not be read, so nothing establishes that none are \
             unresolved: the answer carried no reviewThreads.nodes"
        );
    };

    // `first: THREAD_PAGE_SIZE` truncates silently, so a pull request that
    // outgrows the page would otherwise pass by being busy.
    if threads["pageInfo"]["hasNextPage"].as_bool().unwrap_or(true) {
        bail!(
            "the review threads could not be read in full, so nothing establishes that none are \
             unresolved: GitHub reports more than the {THREAD_PAGE_SIZE} threads requested"
        );
    }

    let mut unresolved = Vec::new();
    for thread in nodes {
        // Absent means unresolved: a thread that does not say it is resolved
        // has not been shown to be.
        if thread["isResolved"].as_bool().unwrap_or(false) {
            continue;
        }
        // An unresolved thread blocks whether or not its first comment came
        // back; the comment is how the refusal is described, not what makes it
        // one.
        let comment = thread["comments"]["nodes"]
            .as_array()
            .and_then(|a| a.first());
        unresolved.push(UnresolvedReviewThread {
            thread_id: thread["id"].as_str().unwrap_or("").to_string(),
            path: comment
                .and_then(|c| c["path"].as_str())
                .unwrap_or("")
                .to_string(),
            line: comment.and_then(|c| c["line"].as_u64()),
            comment_body: comment
                .and_then(|c| c["body"].as_str())
                .unwrap_or("(comment body not returned)")
                .to_string(),
            author: comment
                .and_then(|c| c["author"]["login"].as_str())
                .unwrap_or("")
                .to_string(),
        });
    }
    Ok(unresolved)
}

pub struct UnresolvedReviewGuard {
    #[allow(dead_code)]
    github_client: Arc<GitHubClient>,
    #[allow(dead_code)]
    scanner: ThreadScanner,
}

impl UnresolvedReviewGuard {
    pub fn new(github_client: Arc<GitHubClient>) -> Self {
        let scanner = ThreadScanner::new();
        Self {
            github_client,
            scanner,
        }
    }

    /// Evaluates PR review threads via GitHub GraphQL API, blocking merge queue admission if any thread is unresolved
    pub async fn evaluate_unresolved_reviews(
        &self,
        repo: &str,
        pr_number: u64,
    ) -> Result<UnresolvedReviewReport> {
        info!(
            "Running UnresolvedReviewGuard (Zero-Unresolved-Comments Merge Invariant) on {}#{}...",
            repo, pr_number
        );

        // Query review threads for the PR
        let query = format!(
            r#"
            query {{
              repository(owner: "{owner}", name: "{name}") {{
                pullRequest(number: {pr}) {{
                  reviewThreads(first: {page}) {{
                    pageInfo {{ hasNextPage }}
                    nodes {{
                      id
                      isResolved
                      comments(first: 1) {{
                        nodes {{
                          body
                          path
                          line
                          author {{
                            login
                          }}
                        }}
                      }}
                    }}
                  }}
                }}
              }}
            }}
            "#,
            owner = repo.split('/').next().unwrap_or(""),
            name = repo.split('/').nth(1).unwrap_or(""),
            pr = pr_number,
            page = THREAD_PAGE_SIZE
        );

        let mut gh_cmd = tokio::process::Command::new("gh");
        gh_cmd.args(["api", "graphql", "-f", &format!("query={}", query)]);
        // Fail closed: this guard blocks merge-queue admission, so a query that
        // never completed (spawn failure or the api-class timeout) must not fall
        // through to an empty thread list and report "zero unresolved comments".
        let output = crate::exec::run_bounded(
            gh_cmd,
            crate::exec::ExecClass::Api,
            "gh api graphql (unresolved review threads)",
        )
        .await
        .context("Failed to query PR review threads")?;

        // Propagated, not swallowed: an unreadable answer must reach the
        // caller as an error, because an empty `Vec` is what a clean pull
        // request looks like.
        let unresolved = parse_review_threads(
            output.status.success(),
            &output.stdout,
            &String::from_utf8_lossy(&output.stderr),
        )
        .with_context(|| format!("🚨 Merge queue admission blocked on {}#{}", repo, pr_number))?;

        let is_clean = unresolved.is_empty();
        let summary = if is_clean {
            "✅ PASSED (Zero unresolved review comments or threads on PR)".to_string()
        } else {
            format!(
                "❌ FAILED ({} unresolved review thread(s) must be addressed before merge queue admission)",
                unresolved.len()
            )
        };

        Ok(UnresolvedReviewReport {
            is_clean,
            unresolved_threads: unresolved,
            summary,
        })
    }
}
