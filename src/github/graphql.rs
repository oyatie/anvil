use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::info;

use crate::exec::{ExecClass, run_bounded};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewThreadNode {
    pub id: String,
    pub is_resolved: bool,
    pub path: Option<String>,
    pub line: Option<u64>,
    pub comments: Vec<ThreadComment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadComment {
    pub id: String,
    pub body: String,
    pub author_login: String,
}

pub struct GitHubGraphQLClient;

impl GitHubGraphQLClient {
    /// Constructs GraphQL mutation string to resolve a PR review thread
    pub fn build_resolve_thread_mutation(thread_id: &str) -> String {
        format!(
            r#"mutation {{ resolveReviewThread(input: {{ threadId: "{}" }}) {{ thread {{ isResolved }} }} }}"#,
            thread_id
        )
    }

    /// Resolves an open review thread via GitHub GraphQL API
    pub async fn resolve_review_thread(thread_id: &str) -> Result<()> {
        info!(
            "Resolving review thread {} via GitHub GraphQL API...",
            thread_id
        );
        let mutation = Self::build_resolve_thread_mutation(thread_id);

        let mut cmd = Command::new("gh");
        cmd.args(["api", "graphql", "-f", &format!("query={}", mutation)]);
        let output = run_bounded(cmd, ExecClass::Api, "gh api graphql resolveReviewThread")
            .await
            .context("Failed to execute gh api graphql")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("GitHub GraphQL resolveReviewThread error: {}", stderr);
        }

        Ok(())
    }

    /// Fetches all review threads for a pull request using GitHub GraphQL API
    pub async fn fetch_review_threads(repo: &str, pr_number: u64) -> Result<Vec<ReviewThreadNode>> {
        let parts: Vec<&str> = repo.split('/').collect();
        if parts.len() != 2 {
            bail!("Invalid repo slug: {}", repo);
        }
        let owner = parts[0];
        let name = parts[1];

        let query = format!(
            r#"query {{
                repository(owner: "{owner}", name: "{name}") {{
                    pullRequest(number: {pr_number}) {{
                        reviewThreads(first: 50) {{
                            nodes {{
                                id
                                isResolved
                                path
                                line
                                comments(first: 10) {{
                                    nodes {{
                                        id
                                        body
                                        author {{ login }}
                                    }}
                                }}
                            }}
                        }}
                    }}
                }}
            }}"#
        );

        let mut cmd = Command::new("gh");
        cmd.args(["api", "graphql", "-f", &format!("query={}", query)]);
        let output = run_bounded(cmd, ExecClass::Api, "gh api graphql reviewThreads")
            .await
            .context("Failed to query review threads")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("GitHub GraphQL query reviewThreads error: {}", stderr);
        }

        let json_val: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let mut threads = Vec::new();

        if let Some(nodes) = json_val
            .pointer("/data/repository/pullRequest/reviewThreads/nodes")
            .and_then(|n| n.as_array())
        {
            for node in nodes {
                let id = node
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let is_resolved = node
                    .get("isResolved")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let path = node
                    .get("path")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let line = node.get("line").and_then(|v| v.as_u64());

                let mut comments = Vec::new();
                if let Some(comment_nodes) =
                    node.pointer("/comments/nodes").and_then(|c| c.as_array())
                {
                    for c in comment_nodes {
                        let cid = c
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let body = c
                            .get("body")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let author = c
                            .pointer("/author/login")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        comments.push(ThreadComment {
                            id: cid,
                            body,
                            author_login: author,
                        });
                    }
                }

                threads.push(ReviewThreadNode {
                    id,
                    is_resolved,
                    path,
                    line,
                    comments,
                });
            }
        }

        Ok(threads)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_resolve_thread_mutation() {
        let mutation = GitHubGraphQLClient::build_resolve_thread_mutation("PRRT_kwDOABC123");
        assert!(mutation.contains("resolveReviewThread"));
        assert!(mutation.contains("PRRT_kwDOABC123"));
    }
}
