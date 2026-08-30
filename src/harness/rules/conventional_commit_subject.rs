use super::super::corpus::Corpus;
use super::super::judgement::ConventionalHeader;
use super::super::{Evaluated, Finding, Fixture, Requires, Rule, Withheld};

/// Every commit subject the change adds must be a Conventional Commit header.
///
/// The first rule at the [`Requires::History`] rung, and the reason that rung
/// is a variant rather than a `Vec<String>` field nobody checks. Gate 38 took
/// `commit_subjects: &[String]` and could not distinguish a range that adds no
/// judgeable subject from a log that was never fetched; it published the second
/// as an accusation at every pull request whose commits never reached it.
///
/// Here the distinction is structural: `commit_subjects: None` fails
/// [`Corpus::satisfies`] before `examine` is ever called, so a missing log is
/// withheld by the harness rather than judged by the rule.
///
/// The judgement itself is [`ConventionalHeader::judge`], unchanged --
/// the regex, the type list and the generated-subject exemptions are already
/// shipped and tested, and a second spelling of them is the duplication this
/// harness exists to end.
pub struct ConventionalCommitSubject;

impl Rule for ConventionalCommitSubject {
    fn id(&self) -> &'static str {
        "conventional_commit_subject"
    }

    fn requires(&self) -> Requires {
        Requires::History
    }

    fn examine(&self, corpus: &Corpus) -> Evaluated {
        let Some(subjects) = corpus.commit_subjects.as_deref() else {
            return Evaluated::Withheld(Withheld::InputsAbsent {
                needed: Requires::History,
            });
        };
        let header = ConventionalHeader::new();
        let mut findings = Vec::new();
        let mut seen = 0usize;
        for subject in subjects {
            // `None` means the subject is generated and exempt -- not judged,
            // so it does not count toward coverage either. Counting it would
            // let a pull request of nothing but generated commits report as
            // measured over subjects the rule never actually judged.
            let Some(verdict) = header.judge(subject) else {
                continue;
            };
            seen += 1;
            if !verdict.valid {
                findings.push(Finding {
                    rule: self.id(),
                    key: subject.clone(),
                    subject: subject.clone(),
                    detail: verdict.message,
                    fix: None,
                });
            }
        }
        Evaluated::measured(seen, findings)
    }

    fn fixture(&self) -> Fixture {
        Fixture {
            must_flag: Corpus::of_paths(&["src/lib.rs"])
                .with_commits(vec!["made it work".to_string()]),
            must_pass: Corpus::of_paths(&["src/lib.rs"])
                .with_commits(vec!["fix(harness): withhold on an absent log".to_string()]),
        }
    }
}
