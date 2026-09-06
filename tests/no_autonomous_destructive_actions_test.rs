//! Anvil proposes; a human disposes.
//!
//! `issue_auditor` reached `ResolvedByCommit` -- publishing "Trunk CI is green
//! and passing all gates on the latest commit" -- from a title substring match
//! alone, never querying CI. That verdict then drove `gh issue close`, so a
//! claim that was never evaluated closed another team's issue.
//!
//! Prompting does not prevent this recurring; a test does. These scan source
//! for the destructive verb rather than trusting review to catch it.

use std::fs;
use std::path::Path;

fn sources_under(dir: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![Path::new(dir).to_path_buf()];
    while let Some(p) = stack.pop() {
        let Ok(entries) = fs::read_dir(&p) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|x| x == "rs")
                && let Ok(text) = fs::read_to_string(&path)
            {
                out.push((path.display().to_string(), text));
            }
        }
    }
    out
}

use anvil::source_scan::without_commentary;

// These are spelling/co-location checks, not resolved command or dataflow proof.
fn has_issue_close(source: &str) -> bool {
    let code = without_commentary(source);
    code.contains(r#""issue","#) && code.contains(r#""close","#)
}

fn has_unqueried_ci_claim(source: &str) -> bool {
    let code = without_commentary(source);
    code.contains("Trunk CI is green")
        && !(code.contains("gh run")
            || code.contains("check_runs")
            || code.contains("workflow_run"))
}

fn destructive_spellings(source: &str) -> Vec<&'static str> {
    let code = without_commentary(source);
    [
        "--force",
        "--force-with-lease",
        "\"-f\"",
        "--no-verify",
        "\"push\"",
    ]
    .into_iter()
    .filter(|spelling| code.contains(spelling))
    .collect()
}

#[test]
fn nothing_closes_a_github_issue_autonomously() {
    let offenders: Vec<String> = sources_under("src")
        .into_iter()
        .filter(|(_, t)| has_issue_close(t))
        .map(|(f, _)| f)
        .collect();

    assert!(
        offenders.is_empty(),
        "these invoke `gh issue close` autonomously: {:?}\n\
         Anvil publishes a proposal and a human closes. A verdict derived from a \
         substring match must never close another team's issue -- the reader sees a \
         confident reason with no way to know it was never verified.",
        offenders
    );
}

#[test]
fn a_published_claim_about_ci_is_not_made_without_querying_ci() {
    let auditor = anvil::source_scan::paths::module_source(
        "src/issue_reconciler/issue_auditor",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    assert!(
        !has_unqueried_ci_claim(&auditor),
        "issue_auditor publishes \"Trunk CI is green and passing all gates\" but \
             never queries CI -- the verdict comes from a title substring. Either query \
             the real signal or report NotMeasured and leave the issue open."
    );
}

/// Change delivery builds branches for review; it must be incapable of the
/// destructive verbs. No force, no hook-skipping, and — in this build — no
/// push at all: pushing is the landing step, which arrives with its own
/// review and its own scan entry here.
#[test]
fn change_delivery_cannot_force_push_or_skip_hooks() {
    let mut hits = Vec::new();
    for (path, text) in sources_under("src/change_delivery") {
        for f in destructive_spellings(&text) {
            hits.push(format!("{path}: contains {f}"));
        }
    }
    assert!(
        hits.is_empty(),
        "change_delivery grew a destructive verb; landing changes must arrive \
         with their own review and their own entry in this scan: {hits:?}"
    );
}

#[test]
fn issue_close_predicate_preserves_literals_but_excludes_commentary() {
    let close = r#"command.args(["issue", "close", "42"]);"#;
    assert!(has_issue_close(close));
    assert!(!has_issue_close(&format!("// {close}\n")));
    assert!(!has_issue_close(&format!("/* {close} */")));
    assert!(!has_issue_close(
        r#"command.args(["issue", "view", "42"]);"#
    ));
}

#[test]
fn ci_claim_predicate_requires_a_visible_query_spelling() {
    let claim = r#"publish("Trunk CI is green");"#;
    assert!(has_unqueried_ci_claim(claim));
    assert!(!has_unqueried_ci_claim("publish(\"NotMeasured\");"));
    assert!(!has_unqueried_ci_claim(&format!("// {claim}\n")));
    assert!(!has_unqueried_ci_claim(&format!("/* {claim} */")));
    for query in [r#"command("gh run");"#, "check_runs();", "workflow_run();"] {
        assert!(!has_unqueried_ci_claim(&format!("{claim}\n{query}")));
        assert!(has_unqueried_ci_claim(&format!("{claim}\n// {query}")));
        assert!(has_unqueried_ci_claim(&format!("{claim}\n/* {query} */")));
    }
}

#[test]
fn destructive_predicate_checks_each_flag_and_ignores_commentary() {
    for spelling in [
        "--force",
        "--force-with-lease",
        "\"-f\"",
        "--no-verify",
        "\"push\"",
    ] {
        let literal = if spelling.starts_with('"') {
            spelling.to_owned()
        } else {
            format!("\"{spelling}\"")
        };
        let source = format!("command.arg({literal});");
        assert!(destructive_spellings(&source).contains(&spelling));
        assert!(destructive_spellings(&format!("// {source}\n")).is_empty());
        assert!(destructive_spellings(&format!("/* {source} */")).is_empty());
    }
    assert!(destructive_spellings(r#"command.args(["status", "--short"]);"#).is_empty());
}
