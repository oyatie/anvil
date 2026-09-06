//! Merge admission may only believe GitHub about thread resolution.
//!
//! Two controls named in doctrine §2 were satisfiable without resolving
//! anything:
//!
//!   1. `merge_enlister` decided a review thread was resolved when its body
//!      contained `"Fixed:"`, `"Resolved:"` or `"✅"`. Anvil's own fixer replies
//!      begin with `✅` (`src/fixer/mod.rs`), so Anvil resolved its own threads,
//!      and any human resolved theirs with an emoji.
//!   2. `UnresolvedReviewGuard` asked GitHub the right question and then
//!      reported "zero unresolved threads" for every way the answer could fail
//!      to arrive: a non-zero `gh` exit, unparseable JSON, a GraphQL error
//!      payload, a thread missing `isResolved`, a thread whose comments were not
//!      returned, and a PR with more than the fifty threads requested.
//!
//! Each case below is the seeded defect for one of those paths. They are written
//! against `parse_review_threads`, which is the whole decision: everything above
//! it is the `gh` spawn and everything below it is `is_empty()`.

use anvil::unresolved_review_guard::parse_review_threads;

/// The shape GitHub returns, with `nodes` filled in by the caller.
fn payload(nodes: &str, has_next_page: bool) -> String {
    format!(
        r#"{{"data":{{"repository":{{"pullRequest":{{"reviewThreads":{{
             "pageInfo":{{"hasNextPage":{has_next_page}}},
             "nodes":[{nodes}]}}}}}}}}}}"#
    )
}

fn thread(id: &str, resolved: &str, comments: &str) -> String {
    format!(r#"{{"id":"{id}","isResolved":{resolved},"comments":{{"nodes":[{comments}]}}}}"#)
}

const COMMENT: &str =
    r#"{"body":"please fix","path":"src/main.rs","line":42,"author":{"login":"reviewer"}}"#;

#[test]
fn an_unresolved_thread_is_reported() {
    let out = payload(&thread("T_1", "false", COMMENT), false);
    let threads = parse_review_threads(true, out.as_bytes(), "").expect("well-formed answer");
    assert_eq!(threads.len(), 1, "one unresolved thread must be reported");
    assert_eq!(threads[0].thread_id, "T_1");
    assert_eq!(threads[0].path, "src/main.rs");
    assert_eq!(threads[0].line, Some(42));
}

#[test]
fn a_resolved_thread_is_spared() {
    let out = payload(&thread("T_1", "true", COMMENT), false);
    let threads = parse_review_threads(true, out.as_bytes(), "").expect("well-formed answer");
    assert!(
        threads.is_empty(),
        "a thread GitHub calls resolved must not block: {threads:?}"
    );
}

/// A comment body that says the magic words resolves nothing. These three
/// phrases are the corpus a substring resolver accepts; only `isResolved`
/// decides.
#[test]
fn a_thread_is_open_until_github_says_otherwise() {
    for body in ["Fixed: done", "Resolved: see above", "✅ addressed"] {
        let comment = format!(
            r#"{{"body":{},"path":"src/main.rs","line":1,"author":{{"login":"anvil"}}}}"#,
            serde_json::to_string(body).expect("json string")
        );
        let out = payload(&thread("T_1", "false", &comment), false);
        let threads = parse_review_threads(true, out.as_bytes(), "").expect("well-formed answer");
        assert_eq!(
            threads.len(),
            1,
            "{body:?} is a comment, not a resolution; the thread is still open"
        );
    }
}

#[test]
fn a_failed_query_is_an_error_not_an_empty_list() {
    let err = parse_review_threads(false, b"", "gh: HTTP 502")
        .expect_err("a query that did not succeed establishes nothing");
    assert!(
        err.to_string().contains("502"),
        "the refusal must carry why: {err}"
    );
}

#[test]
fn an_unparseable_answer_is_an_error_not_an_empty_list() {
    parse_review_threads(true, b"<html>rate limited</html>", "")
        .expect_err("an answer that did not parse establishes nothing");
}

#[test]
fn a_graphql_error_payload_is_an_error_not_an_empty_list() {
    let out = br#"{"data":null,"errors":[{"message":"Could not resolve to a Repository"}]}"#;
    parse_review_threads(true, out, "").expect_err("a GraphQL error establishes nothing");
}

/// `isResolved` absent used to mean resolved. Absent evidence of resolution is
/// not evidence of resolution.
#[test]
fn a_thread_that_does_not_say_whether_it_is_resolved_blocks() {
    let out = payload(
        &format!(r#"{{"id":"T_1","comments":{{"nodes":[{COMMENT}]}}}}"#),
        false,
    );
    let threads = parse_review_threads(true, out.as_bytes(), "").expect("well-formed answer");
    assert_eq!(
        threads.len(),
        1,
        "a thread with no isResolved field is not a resolved thread"
    );
}

/// An unresolved thread whose comments were not returned used to be dropped on
/// the floor by `if let Some(comment)`.
#[test]
fn an_unresolved_thread_with_no_comments_still_blocks() {
    let out = payload(
        r#"{"id":"T_1","isResolved":false,"comments":{"nodes":[]}}"#,
        false,
    );
    let threads = parse_review_threads(true, out.as_bytes(), "").expect("well-formed answer");
    assert_eq!(
        threads.len(),
        1,
        "a thread is unresolved whether or not its first comment came back"
    );
}

/// The query asks for fifty threads. Thread fifty-one used to be invisible, so a
/// busy PR passed by being busy.
#[test]
fn a_truncated_thread_list_is_an_error_not_a_clean_bill() {
    let out = payload(&thread("T_1", "true", COMMENT), true);
    let err = parse_review_threads(true, out.as_bytes(), "")
        .expect_err("a truncated page establishes nothing about the threads not on it");
    assert!(
        err.to_string().to_lowercase().contains("more"),
        "the refusal must say the list was incomplete: {err}"
    );
}

/// The refusal must survive the pipeline glue between the guard and the gate.
///
/// `parse_review_threads` refusing is worth nothing if the pipeline turns that
/// refusal back into a value. `certify_pull_request` publishes a report of
/// seventy-two gate statuses; a caller that recovered from this error would have
/// to invent an `UnresolvedReviewReport` to fill the slot, and the only honest
/// invention is "clean" — which is the fail-open this test exists to prevent.
///
/// Keyed to the call, not to a line: the scan finds the call site wherever it
/// moves, and fails loudly if it can no longer find it at all.
#[test]
fn the_certification_pipeline_does_not_recover_from_an_unreadable_answer() {
    const CALL: &str = ".evaluate_unresolved_reviews(";
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sites = Vec::new();

    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs")
                && let Ok(body) = std::fs::read_to_string(&p)
            {
                for (n, _) in body.match_indices(CALL) {
                    // The tail of the expression, up to the end of the
                    // statement. Long enough to hold `.await?` and any
                    // recovery combinator, short enough not to run into the
                    // next one.
                    let tail: String = body[n..].chars().take(200).collect();
                    let stmt = tail.split(';').next().unwrap_or(&tail).to_string();
                    sites.push((p.display().to_string(), stmt));
                }
            }
        }
    }

    assert!(
        !sites.is_empty(),
        "nothing under src/ calls `{CALL}` any more. The gate is fed by this \
         call; if it moved, this test must follow it — a scan that stops finding \
         its subject is not a fix."
    );

    for (file, stmt) in &sites {
        let flat = stmt.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            stmt.contains('?'),
            "{file} calls `{CALL}` and does not propagate its error:\n  {flat}\n\
             An unreadable thread list must not become a gate status."
        );
        for recovery in [
            "unwrap_or",
            "unwrap_or_default",
            "unwrap_or_else",
            // `.or_else(` was missing, and it is the one combinator that can
            // fabricate a clean report AND keep the `?` -- so the call still
            // looks fallible while an unreadable answer becomes an empty list
            // of unresolved threads. That is precisely the shape this file
            // exists to refuse.
            ".or_else(",
            ".or(",
            ".ok()",
            "if let Ok",
        ] {
            assert!(
                !stmt.contains(recovery),
                "{file} recovers from `{CALL}` with `{recovery}`:\n  {flat}\n\
                 There is no honest value to recover with: the slot this fills \
                 is a gate status, and the only one a recovery can supply is \
                 'clean'."
            );
        }
    }
}
