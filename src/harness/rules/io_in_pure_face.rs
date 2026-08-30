use super::super::corpus::Corpus;
use super::super::{Evaluated, Finding, Fixture, Requires, Rule};

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
