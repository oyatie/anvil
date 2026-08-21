use anvil::github::GitHubGraphQLClient;

#[test]
fn test_build_resolve_thread_mutation_structure() {
    let mutation = GitHubGraphQLClient::build_resolve_thread_mutation("PRRT_kwDOABC12345");
    assert!(mutation.contains("mutation {"));
    assert!(mutation.contains("resolveReviewThread(input: { threadId: \"PRRT_kwDOABC12345\" })"));
    assert!(mutation.contains("thread { isResolved }"));
}
