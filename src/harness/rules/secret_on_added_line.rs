use super::super::corpus::Corpus;
use super::super::judgement;
use super::super::{Evaluated, Finding, Fixture, Requires, Rule, Withheld};
use crate::pre_merge_guard::report::GateStatus;

/// No credential may appear on a line the change ADDS.
///
/// The first rule at the [`Requires::Changeset`] rung. It exists to prove the
/// rung carries a real gate rather than a declared one: a working-tree corpus
/// cannot express this question at all, because "does this file contain a key"
/// and "does this change add a key" have different answers for the pull request
/// that DELETES a leaked one -- and reading the whole diff refused exactly that
/// pull request until [`judgement::scan_for_secrets`] was fixed to read
/// `+` lines only.
///
/// That scanner is the judgement, called not copied.
pub struct SecretOnAddedLine;

impl Rule for SecretOnAddedLine {
    fn id(&self) -> &'static str {
        "secret_on_added_line"
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
        let added = super::added_line_count(&change.diff_content);
        let findings = match judgement::scan_for_secrets(&change.diff_content) {
            GateStatus::Failed(why) => vec![Finding {
                rule: self.id(),
                key: change.head_sha.clone(),
                subject: change.changed_files.join(", "),
                detail: format!("{why} (on a line this change adds)."),
                fix: None,
            }],
            _ => Vec::new(),
        };
        // Coverage is added lines, not changed files: a diff of pure deletions
        // has files and zero lines this rule can judge, and reporting it as
        // measured would be the same claim the deleted-key bug made.
        Evaluated::measured(added, findings)
    }

    fn fixture(&self) -> Fixture {
        // Split so the key is never a contiguous literal in committed source.
        // A seeded defect for a credential scanner necessarily contains
        // something credential-shaped, and writing it whole makes this file a
        // finding against itself -- which is how it was caught: the self-scan
        // over recent history failed on the commit that introduced it.
        let key = format!("AKIA{}", "IOSFODNN7EXAMPLE");
        Fixture {
            must_flag: Corpus::of_diff(
                &["src/config.rs"],
                &format!(
                    "--- a/src/config.rs\n+++ b/src/config.rs\n+const KEY: &str = \"{key}\";\n"
                ),
            ),
            // The same credential on a REMOVED line. A rule that flags this is
            // refusing the pull request that cleans up the leak.
            must_pass: Corpus::of_diff(
                &["src/config.rs"],
                &format!(
                    "--- a/src/config.rs\n+++ b/src/config.rs\n-const KEY: &str = \"{key}\";\n\
                     +const KEY: &str = env!(\"AWS_KEY\");\n"
                ),
            ),
        }
    }
}
