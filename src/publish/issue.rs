//! Issue bodies, on the same envelope as a review.

use super::{AnvilAction, Judged, body};

/// Same rules as the scorecard. Issue titles are a queue that engineers scan,
/// so the title is `[anvil] <subject> — <scope>`: prefix first so machine-filed
/// issues are filterable, subject before scope so the list is readable at a
/// glance without expanding.
pub struct Issue {
    pub title: String,
    pub body: String,
}

/// Builds an issue with a stable title shape and a signed body.
///
/// `evidence` is rendered verbatim and should already be terse -- a log tail, a
/// failing command, a diff excerpt. `next_step` is what the reader should do;
/// pass `None` rather than inventing one.
pub fn issue(
    action: AnvilAction,
    subject: &str,
    scope: &str,
    evidence: &str,
    next_step: Option<&str>,
    judged: Judged,
) -> Issue {
    let mut b = String::new();
    b.push_str(evidence.trim());
    if let Some(step) = next_step {
        b.push_str(&format!("\n\n**Next step:** {}", step.trim()));
    }
    Issue {
        title: format!("[anvil] {} — {}", subject.trim(), scope.trim()),
        body: body(action, &b, judged).to_string(),
    }
}
