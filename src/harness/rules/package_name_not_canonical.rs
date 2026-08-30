use super::super::corpus::Corpus;
use super::super::{Evaluated, Finding, Fix, Fixture, Requires, Rule};

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
