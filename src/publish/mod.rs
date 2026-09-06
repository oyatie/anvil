//! Standardized formatting for everything Anvil publishes to GitHub.
//!
//! - One signature form: `*🤖 [Action] by Oyatie Anvil*`, always closed.
//! - Every artifact carries a hidden idempotency marker, so it is amended in
//!   place rather than re-posted.
//! - The action is a typed enum, so a call site cannot invent a variant.

pub mod issue;
pub mod proof_line;
pub mod scorecard;
pub use issue::{Issue, issue};

use serde::{Deserialize, Serialize};

/// What Anvil did. One variant per published artifact type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnvilAction {
    Reviewed,
    Certified,
    Triaged,
    Fixed,
    Healed,
    Reconciled,
    Enlisted,
    Evaluated,
    Attested,
    Blocked,
}

impl AnvilAction {
    /// The bracketed action name used in the signature.
    pub const fn label(self) -> &'static str {
        match self {
            AnvilAction::Reviewed => "Reviewed",
            AnvilAction::Certified => "Certified",
            AnvilAction::Triaged => "Triaged",
            AnvilAction::Fixed => "Fixed",
            AnvilAction::Healed => "Healed",
            AnvilAction::Reconciled => "Reconciled",
            AnvilAction::Enlisted => "Enlisted",
            AnvilAction::Evaluated => "Evaluated",
            AnvilAction::Attested => "Attested",
            AnvilAction::Blocked => "Blocked",
        }
    }

    /// Hidden HTML marker identifying this artifact for in-place amendment.
    ///
    /// Kept stable: changing one orphans every comment previously posted under
    /// it, which then gets duplicated rather than updated.
    pub const fn marker(self) -> &'static str {
        match self {
            AnvilAction::Reviewed => "<!-- ANVIL:REVIEW -->",
            AnvilAction::Certified | AnvilAction::Blocked => "<!-- ANVIL_SCORECARD_RECEIPT -->",
            AnvilAction::Triaged => "<!-- ANVIL:TRIAGE -->",
            AnvilAction::Fixed => "<!-- ANVIL:FIX -->",
            AnvilAction::Healed => "<!-- ANVIL:HEAL -->",
            AnvilAction::Reconciled => "<!-- ANVIL:RECONCILE -->",
            AnvilAction::Enlisted => "<!-- ANVIL:ENLIST -->",
            AnvilAction::Evaluated => "<!-- ANVIL:EVALUATE -->",
            AnvilAction::Attested => "<!-- ANVIL:ATTEST -->",
        }
    }
}

/// The mandatory signature line.
///
/// `Certified` and `Blocked` deliberately share the scorecard marker: they are
/// the two outcomes of the same artifact, so a PR that goes from blocked to
/// certified updates one comment rather than accumulating a history of both.
pub fn signature(action: AnvilAction) -> String {
    format!("*🤖 [{}] by Oyatie Anvil*", action.label())
}

/// What revision an artifact judged.
///
/// Not an `Option<&str>`. A published comment with no revision anchor is
/// ambiguous the moment anyone force-pushes: a reader cannot tell whether the
/// findings describe the head they are looking at or one that no longer
/// exists. Making the choice a type means a call site must DECIDE rather than
/// default, and `NotRevisionScoped` has to be written down and justified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Judged {
    /// The commit this artifact was computed against.
    Rev(String),
    /// The artifact is about a repository or a queue rather than a commit --
    /// a stale-environment sweep, a trunk-health issue. Deliberate, not a
    /// fallback for "the sha was inconvenient to thread through".
    NotRevisionScoped,
}

impl Judged {
    /// The anchor as it renders: twelve characters, which is what GitHub shows
    /// and enough to be unambiguous in any repository this will ever see.
    fn anchor(&self) -> String {
        match self {
            Judged::Rev(sha) => {
                let short: String = sha.trim().chars().take(12).collect();
                format!(" · `{short}`")
            }
            Judged::NotRevisionScoped => String::new(),
        }
    }
}

/// Assembles a complete published body: marker, content, separator, signature.
///
/// The marker leads so it is present even if the body is later truncated by the
/// GitHub comment size limit. The revision closes it, because that is the fact
/// a reader needs to know whether the content above still applies.
///
/// This is the deterministic half of everything Anvil publishes: the marker,
/// the action, the separator and the revision are mechanical, and only
/// `content` is written by a model.
/// A body that has been through the envelope.
///
/// The field is private and this module is the only place that can build one,
/// so a transport taking a `Published` cannot be handed a raw `String`. That
/// moves "everything we publish is signed and anchored" from a rule checked at
/// a few call sites to one the compiler enforces at all of them -- including
/// call sites nobody has written yet, which is the half a standing test cannot
/// reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published(String);

impl Published {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Adopt a body that already carries the envelope.
    ///
    /// The one legitimate way in from outside: a comment being amended in
    /// place was signed when it was first posted, and re-enveloping it would
    /// double the signature. Refuses anything unsigned, so this is not a hole.
    pub fn already_enveloped(content: &str) -> Option<Self> {
        is_signed(content).then(|| Published(content.trim_end().to_string()))
    }
}

impl std::fmt::Display for Published {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub fn body(action: AnvilAction, content: &str, judged: Judged) -> Published {
    Published(format!(
        "{}\n{}\n\n---\n{}{}",
        action.marker(),
        content.trim_end(),
        signature(action),
        judged.anchor()
    ))
}

/// A uniformly styled issue: deterministic title, findings-only body.
///
/// Whether a body carries the mandatory signature.
///
/// Used by the GitHub transport to catch an unsigned publication at its source
/// rather than discovering it on the pull request.
pub fn is_signed(body: &str) -> bool {
    body.contains("] by Oyatie Anvil*")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: &[AnvilAction] = &[
        AnvilAction::Reviewed,
        AnvilAction::Certified,
        AnvilAction::Triaged,
        AnvilAction::Fixed,
        AnvilAction::Healed,
        AnvilAction::Reconciled,
        AnvilAction::Enlisted,
        AnvilAction::Evaluated,
        AnvilAction::Attested,
        AnvilAction::Blocked,
    ];

    #[test]
    fn every_signature_has_the_mandatory_shape() {
        for a in ALL {
            let s = signature(*a);
            assert!(s.starts_with("*🤖 ["), "{s}");
            assert!(s.ends_with("] by Oyatie Anvil*"), "{s}");
            // The historical defect: an unclosed italic marker rendering a
            // literal asterisk in some comments and italics in others.
            assert_eq!(s.matches('*').count(), 2, "italics must be closed: {s}");
        }
    }

    #[test]
    fn every_published_body_carries_a_marker_and_the_signature() {
        for a in ALL {
            let b = body(*a, "some findings", Judged::NotRevisionScoped);
            assert!(b.as_str().starts_with(a.marker()), "marker must lead: {b}");
            assert!(b.as_str().ends_with(&signature(*a)));
            assert!(b.as_str().contains("\n---\n"), "separator missing");
        }
    }

    #[test]
    fn markers_are_stable_and_scorecard_outcomes_share_one() {
        // Certified and Blocked are two outcomes of the same artifact, so the
        // scorecard is amended in place rather than duplicated.
        assert_eq!(
            AnvilAction::Certified.marker(),
            AnvilAction::Blocked.marker()
        );
        // The pre-existing scorecard marker is preserved; changing it would
        // orphan every comment already posted under it.
        assert_eq!(
            AnvilAction::Certified.marker(),
            "<!-- ANVIL_SCORECARD_RECEIPT -->"
        );
    }

    #[test]
    fn distinct_actions_do_not_collide_on_a_marker() {
        let mut seen = std::collections::HashMap::new();
        for a in ALL {
            seen.entry(a.marker()).or_insert_with(Vec::new).push(*a);
        }
        for (marker, actions) in seen {
            assert!(
                actions.len() == 1
                    || actions
                        .iter()
                        .all(|x| matches!(x, AnvilAction::Certified | AnvilAction::Blocked)),
                "{marker} is shared by unrelated actions: {actions:?}"
            );
        }
    }

    #[test]
    fn issues_have_a_filterable_deterministic_title() {
        let i = issue(
            AnvilAction::Triaged,
            "Trunk CI failure",
            "oyatie/console@main",
            "cargo test failed: 3 tests",
            Some("re-run locally with `cargo test --all-targets`"),
            Judged::NotRevisionScoped,
        );
        assert!(
            i.title.starts_with("[anvil] "),
            "must be filterable: {}",
            i.title
        );
        assert_eq!(i.title, "[anvil] Trunk CI failure — oyatie/console@main");
        assert!(i.body.contains("**Next step:**"));
        assert!(is_signed(&i.body));
        assert!(i.body.starts_with(AnvilAction::Triaged.marker()));
    }

    #[test]
    fn an_issue_without_a_known_next_step_states_none() {
        let i = issue(
            AnvilAction::Triaged,
            "s",
            "c",
            "evidence",
            None,
            Judged::NotRevisionScoped,
        );
        assert!(!i.body.contains("Next step"), "must not invent a next step");
    }

    #[test]
    fn is_signed_detects_the_canonical_signature_only() {
        assert!(is_signed(
            body(AnvilAction::Fixed, "x", Judged::NotRevisionScoped).as_str()
        ));
        assert!(!is_signed("a comment with no signature"));
        // The historical unbracketed variant must NOT satisfy the check.
        assert!(!is_signed("*🤖 Fixed by Oyatie Anvil*"));
    }

    #[test]
    fn body_does_not_double_space_when_content_is_already_padded() {
        let b = body(AnvilAction::Fixed, "done\n\n\n", Judged::NotRevisionScoped);
        assert!(
            !b.as_str().contains("\n\n\n"),
            "trailing whitespace must be trimmed"
        );
    }
}
