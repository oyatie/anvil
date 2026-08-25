//! The registered rules. Adding one is a `register` call and a fixture.

use super::corpus::Corpus;
use super::judgement::{self, ConventionalHeader};
use super::{Evaluated, Finding, Fix, Fixture, Harness, Requires, Rule, Withheld};
use crate::pre_merge_guard::report::GateStatus;

/// A face that must hold no I/O may not depend on an I/O crate.
///
/// Measured on a 438-crate reference tree before being written: `ports` 1/56
/// and `core` 5/224 violate this, so the boundary is real rather than aspired
/// to. A rule the reference tree fails in bulk would be a wrong rule.
pub struct IoInPureFace;

const IO_CRATES: &[&str] = &["tokio", "reqwest", "sqlx", "hyper", "axum", "tonic"];
const PURE_FACES: &[&str] = &["ports", "core"];

impl Rule for IoInPureFace {
    fn id(&self) -> &'static str {
        "io_in_pure_face"
    }

    fn requires(&self) -> Requires {
        Requires::FileContents
    }

    fn examine(&self, corpus: &Corpus) -> Evaluated {
        let mut findings = Vec::new();
        let mut seen = 0usize;
        for subject in &corpus.subjects {
            let Some(face) = subject.face.as_deref() else {
                continue;
            };
            if !PURE_FACES.contains(&face) {
                continue;
            }
            let Some(body) = corpus.contents.get(&subject.path) else {
                continue;
            };
            seen += 1;
            let hits: Vec<&str> = IO_CRATES
                .iter()
                .copied()
                .filter(|c| {
                    body.contains(&format!("{c} =")) || body.contains(&format!("{c}.workspace"))
                })
                .collect();
            if !hits.is_empty() {
                findings.push(Finding {
                    rule: self.id(),
                    // Keyed on the unit and face, not the path: a key derived
                    // from a path is invalidated by this engine's own MovePath.
                    key: format!("{}/{face}", subject.owner.as_deref().unwrap_or("?")),
                    subject: subject.path.clone(),
                    detail: format!(
                        "`{face}` is a pure boundary and depends on {}",
                        hits.join(", ")
                    ),
                    fix: None,
                });
            }
        }
        Evaluated::measured(seen, findings)
    }

    fn fixture(&self) -> Fixture {
        Fixture {
            must_flag: Corpus::default().with_contents(
                "iam/ports/policy-api/Cargo.toml",
                "[dependencies]\ntokio = \"1\"\n",
            ),
            must_pass: Corpus::default().with_contents(
                "audit/ports/emission-kernel/Cargo.toml",
                "[dependencies]\nserde = \"1\"\n",
            ),
        }
    }
}

/// A crate's package name must be derivable from its path.
///
/// The reference tree carries 47/438 violations, all of them a vendor or
/// grouping prefix that survived a capability rename -- `messaging-file-adapter`
/// living under `bus/`. Each carries a `RenameSymbol` fix, so the repair is a
/// codemod rather than 47 hand edits.
pub struct PackageNameNotCanonical;

impl Rule for PackageNameNotCanonical {
    fn id(&self) -> &'static str {
        "package_name_not_canonical"
    }

    fn requires(&self) -> Requires {
        Requires::FileContents
    }

    fn examine(&self, corpus: &Corpus) -> Evaluated {
        let mut findings = Vec::new();
        let mut seen = 0usize;
        for subject in &corpus.subjects {
            if !subject.path.ends_with("/Cargo.toml") {
                continue;
            }
            let Some(body) = corpus.contents.get(&subject.path) else {
                continue;
            };
            let parts: Vec<&str> = subject.path.split('/').collect();
            let (Some(owner), Some(leaf)) = (
                subject.owner.as_deref(),
                parts.get(parts.len().wrapping_sub(2)).copied(),
            ) else {
                continue;
            };
            let Some(name) = body
                .lines()
                .find_map(|l| l.trim().strip_prefix("name = "))
                .map(|v| v.trim_matches('"'))
            else {
                continue;
            };
            seen += 1;
            if name != leaf && name != format!("{owner}-{leaf}") {
                findings.push(Finding {
                    rule: self.id(),
                    key: format!("{owner}/{leaf}"),
                    subject: subject.path.clone(),
                    detail: format!("package `{name}` is neither `{leaf}` nor `{owner}-{leaf}`"),
                    fix: Some(Fix::Rename {
                        from: name.to_string(),
                        to: format!("{owner}-{leaf}"),
                    }),
                });
            }
        }
        Evaluated::measured(seen, findings)
    }

    fn fixture(&self) -> Fixture {
        Fixture {
            must_flag: Corpus::default().with_contents(
                "bus/adapters/file/Cargo.toml",
                "[package]\nname = \"messaging-file-adapter\"\n",
            ),
            must_pass: Corpus::default().with_contents(
                "bus/adapters/file/Cargo.toml",
                "[package]\nname = \"bus-file\"\n",
            ),
        }
    }
}

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
        let added = change
            .diff_content
            .lines()
            .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
            .count();
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

/// The one registration point.
pub fn registered() -> Harness {
    let mut h = Harness::default();
    h.register(Box::new(IoInPureFace))
        .register(Box::new(PackageNameNotCanonical))
        .register(Box::new(ConventionalCommitSubject))
        .register(Box::new(SecretOnAddedLine));
    h
}
