use anyhow::Result;
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
                  reviewThreads(first: 50) {{
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
            pr = pr_number
        );

        let output = tokio::process::Command::new("gh")
            .args(["api", "graphql", "-f", &format!("query={}", query)])
            .output()
            .await;

        let mut unresolved = Vec::new();

        if let Ok(out) = output {
            if out.status.success() {
                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                    if let Some(nodes) = val["data"]["repository"]["pullRequest"]["reviewThreads"]
                        ["nodes"]
                        .as_array()
                    {
                        for thread in nodes {
                            let is_resolved = thread["isResolved"].as_bool().unwrap_or(true);
                            if !is_resolved {
                                if let Some(comment) = thread["comments"]["nodes"]
                                    .as_array()
                                    .and_then(|a| a.first())
                                {
                                    unresolved.push(UnresolvedReviewThread {
                                        thread_id: thread["id"].as_str().unwrap_or("").to_string(),
                                        path: comment["path"].as_str().unwrap_or("").to_string(),
                                        line: comment["line"].as_u64(),
                                        comment_body: comment["body"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_string(),
                                        author: comment["author"]["login"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

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
