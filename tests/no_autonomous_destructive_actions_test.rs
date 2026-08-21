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

/// Strips `//` and `//!` lines so a comment explaining the ban does not trip it.
fn code_only(text: &str) -> String {
    text.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn nothing_closes_a_github_issue_autonomously() {
    let offenders: Vec<String> = sources_under("src")
        .into_iter()
        .filter(|(_, t)| {
            let c = code_only(t);
            c.contains(r#""issue","#) && c.contains(r#""close","#)
        })
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
    let auditor = fs::read_to_string("src/issue_reconciler/issue_auditor.rs")
        .expect("issue_auditor.rs must exist");
    let code = code_only(&auditor);

    if code.contains("Trunk CI is green") {
        let queries_ci =
            code.contains("gh run") || code.contains("check_runs") || code.contains("workflow_run");
        assert!(
            queries_ci,
            "issue_auditor publishes \"Trunk CI is green and passing all gates\" but \
             never queries CI -- the verdict comes from a title substring. Either query \
             the real signal or report NotMeasured and leave the issue open."
        );
    }
}

/// Change delivery builds branches for review; it must be incapable of the
/// destructive verbs. No force, no hook-skipping, and — in this build — no
/// push at all: pushing is the landing step, which arrives with its own
/// review and its own scan entry here.
#[test]
fn change_delivery_cannot_force_push_or_skip_hooks() {
    let forbidden = [
        "--force",
        "--force-with-lease",
        "\"-f\"",
        "--no-verify",
        "\"push\"",
    ];
    let mut hits = Vec::new();
    for (path, text) in sources_under("src/change_delivery") {
        let code = code_only(&text);
        for f in forbidden {
            if code.contains(f) {
                hits.push(format!("{path}: contains {f}"));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "change_delivery grew a destructive verb; landing changes must arrive \
         with their own review and their own entry in this scan: {hits:?}"
    );
}
