use super::super::corpus::Corpus;
use super::super::{Evaluated, Finding, Fixture, Requires, Rule, Withheld};

/// No line a change adds may introduce a cleartext endpoint or an explicit
/// insecure-transport opt-in (CWE-319).
///
/// The gate this replaces computed `passed = findings == 0` over the diff text.
/// An empty diff therefore published `Passed`: examined nothing, found nothing,
/// reported clean. `Evaluated::measured` refuses that by construction, so the
/// same absence now withholds.
///
/// Coverage is added lines rather than changed files, for the reason
/// [`super::secret_on_added_line`] gives: a change of pure deletions has files
/// and no line this rule can judge.
///
/// It is a text lint and is not promoted by moving here. It cannot observe that
/// a workload presented a SPIFFE SVID or that a mesh enforces mTLS, and the
/// fidelity registry says so.
pub struct CleartextTransport;

impl Rule for CleartextTransport {
    fn id(&self) -> &'static str {
        "cleartext_transport_status"
    }

    fn requires(&self) -> Requires {
        Requires::Changeset
    }

    fn examine(&self, corpus: &Corpus) -> Evaluated {
        let Some(change) = corpus.changeset.as_ref() else {
            return Evaluated::Withheld(Withheld::InputsAbsent {
                needed: Requires::Changeset,
            });
        };
        let seen = super::added_line_count(&change.diff_content);
        let auditor = crate::harness::cleartext_scan::IdentityAuditor::new();
        let findings = auditor
            .audit_cleartext_transport(&change.diff_content)
            .into_iter()
            .map(|violation| Finding {
                rule: self.id(),
                key: format!("{}::{violation}", change.head_sha),
                subject: change.changed_files.join(", "),
                detail: violation,
                fix: None,
            })
            .collect();
        Evaluated::measured(seen, findings)
    }

    /// The scheme is assembled, never contiguous in committed source.
    ///
    /// A fixture for a cleartext lint necessarily contains a cleartext URL, and
    /// this repository runs this lint over its own commits -- so writing one
    /// whole makes this file a finding against itself. `SecretOnAddedLine`
    /// carries the same note for the same reason.
    fn fixture(&self) -> Fixture {
        let insecure = format!("ht{}", "tp");
        let secure = format!("{insecure}s");
        Fixture {
            must_flag: Corpus::of_diff(
                &["src/client.rs"],
                &format!(
                    "--- a/src/client.rs\n+++ b/src/client.rs\n\
                     +const UPSTREAM: &str = \"{insecure}://payments.internal/charge\";\n"
                ),
            ),
            // The same endpoint over TLS. A rule that flags this refuses the
            // change that fixes the defect.
            must_pass: Corpus::of_diff(
                &["src/client.rs"],
                &format!(
                    "--- a/src/client.rs\n+++ b/src/client.rs\n\
                     -const UPSTREAM: &str = \"{insecure}://payments.internal/charge\";\n\
                     +const UPSTREAM: &str = \"{secure}://payments.internal/charge\";\n"
                ),
            ),
        }
    }
}
