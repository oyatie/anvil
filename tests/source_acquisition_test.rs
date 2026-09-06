//! Inert controls for the shared source-acquisition algorithms.

#[path = "source_acquisition/merge_base.rs"]
pub mod merge_base_sources;
#[path = "source_acquisition/mod.rs"]
pub mod source_acquisition;

use anvil::ratchet::facade::derived::Derived;
use anvil::ratchet::ports::RefError;
use anvil::shape::adapters::InMemoryTree;
use anvil::shape::ports::{SourceError, TreeSource};
use source_acquisition::{EntryKind, SourceAccess, rust_sources_with};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

struct MemorySources {
    fault: &'static str,
    reads: Vec<PathBuf>,
}

impl MemorySources {
    fn new(fault: &'static str) -> Self {
        Self {
            fault,
            reads: Vec::new(),
        }
    }
}

impl SourceAccess for MemorySources {
    fn read_dir(&mut self, dir: &Path) -> io::Result<Vec<io::Result<PathBuf>>> {
        let name = dir.to_str().expect("in-memory path");
        if (name == "src" && self.fault == "root_list")
            || (name == "src/nested.rs" && self.fault == "nested_list")
        {
            return Err(io::Error::other("injected directory failure"));
        }
        match name {
            "src" if self.fault == "empty" => Ok(Vec::new()),
            "src" => {
                let mut entries = vec![Ok("src/z.rs".into())];
                if self.fault == "entry" {
                    entries.push(Err(io::Error::other("injected entry failure")));
                }
                entries
                    .extend(["src/nested.rs", "src/skip.txt", "src/opaque"].map(|p| Ok(p.into())));
                Ok(entries)
            }
            "src/nested.rs" => Ok(vec![Ok("src/nested.rs/a.rs".into())]),
            unexpected => panic!("unexpected enumeration: {unexpected}"),
        }
    }

    fn kind(&mut self, path: &Path) -> io::Result<EntryKind> {
        let name = path.to_str().expect("in-memory path");
        if (name == "src" && self.fault == "root_kind")
            || (name == "src/nested.rs" && self.fault == "entry_kind")
        {
            return Err(io::Error::other("injected metadata failure"));
        }
        match name {
            "src" if self.fault == "root_file" => Ok(EntryKind::RegularFile),
            "src/z.rs" if self.fault == "nonregular" => Ok(EntryKind::Other),
            "src" | "src/nested.rs" => Ok(EntryKind::Directory),
            "src/z.rs" | "src/nested.rs/a.rs" | "src/skip.txt" => Ok(EntryKind::RegularFile),
            "src/opaque" => Ok(EntryKind::Other),
            unexpected => panic!("unexpected kind lookup: {unexpected}"),
        }
    }

    fn read_text(&mut self, path: &Path) -> io::Result<String> {
        assert!(matches!(
            path.to_str(),
            Some("src/z.rs" | "src/nested.rs/a.rs")
        ));
        self.reads.push(path.to_path_buf());
        if path == Path::new("src/z.rs") {
            match self.fault {
                "read" => return Err(io::Error::other("injected read failure")),
                "utf8" => return Err(io::Error::from(io::ErrorKind::InvalidData)),
                _ => {}
            }
        }
        Ok("// preserved\nconst NAME: &str = \"literal\";\n#[cfg(test)] mod tests {}\n".into())
    }
}

#[test]
fn a_readable_file_does_not_hide_a_nested_directory_error() {
    let mut clean = MemorySources::new("");
    assert_eq!(
        rust_sources_with(Path::new("src"), &mut clean)
            .unwrap()
            .len(),
        2
    );
    assert_eq!(clean.reads.len(), 2, "independent readable-corpus control");
    let mut failed = MemorySources::new("nested_list");
    let error = rust_sources_with(Path::new("src"), &mut failed).expect_err("incomplete corpus");
    assert!(error.contains("src/nested.rs"), "{error}");
    assert!(error.contains("list"), "{error}");
}

#[test]
fn physical_scope_is_sorted_and_text_is_unchanged() {
    let mut access = MemorySources::new("");
    let sources = rust_sources_with(Path::new("src"), &mut access).unwrap();
    let paths: Vec<_> = sources.iter().map(|file| file.path.as_path()).collect();
    assert_eq!(
        paths,
        vec![Path::new("src/nested.rs/a.rs"), Path::new("src/z.rs")]
    );
    assert_eq!(access.reads.len(), 2, "non-Rust entries must not be opened");
    for file in sources {
        assert_eq!(
            file.text,
            "// preserved\nconst NAME: &str = \"literal\";\n#[cfg(test)] mod tests {}\n"
        );
    }
}

#[test]
fn enumeration_and_metadata_failures_are_not_empty_successes() {
    for (fault, path, operation) in [
        ("root_list", "src", "list"),
        ("entry", "src", "list entry"),
        ("root_kind", "src", "inspect"),
        ("entry_kind", "src/nested.rs", "inspect"),
        ("root_file", "src", "expected a source directory"),
        ("empty", "src", "no Rust source files"),
    ] {
        let error =
            rust_sources_with(Path::new("src"), &mut MemorySources::new(fault)).unwrap_err();
        assert!(
            error.contains(path) && error.contains(operation),
            "{fault}: {error}"
        );
    }
}

#[test]
fn selected_read_errors_fail_after_another_file_was_read() {
    for fault in ["read", "utf8"] {
        let mut access = MemorySources::new(fault);
        let error = rust_sources_with(Path::new("src"), &mut access).unwrap_err();
        assert_eq!(
            access.reads,
            vec![
                PathBuf::from("src/nested.rs/a.rs"),
                PathBuf::from("src/z.rs")
            ]
        );
        assert!(error.contains("cannot read src/z.rs"), "{fault}: {error}");
    }
}

#[test]
fn a_selected_nonregular_source_is_refused_without_reading_it() {
    let mut access = MemorySources::new("nonregular");
    let error = rust_sources_with(Path::new("src"), &mut access).unwrap_err();
    assert!(
        error.contains("src/z.rs") && error.contains("regular Rust source"),
        "{error}"
    );
    assert!(access.reads.is_empty());
}

#[test]
fn historical_sources_keep_file_boundaries_and_scope() {
    let tree = InMemoryTree::default()
        .with_file("src/z.rs", "// z\n")
        .with_file("src/nested/a.rs", "#[cfg(test)] mod tests {}\n")
        .with_file("tests/ignored.rs", "ignored")
        .with_file("src/ignored.txt", "ignored");
    assert_eq!(
        merge_base_sources::rust_sources(&tree).unwrap(),
        vec![
            (
                "src/nested/a.rs".into(),
                "#[cfg(test)] mod tests {}\n".into()
            ),
            ("src/z.rs".into(), "// z\n".into()),
        ]
    );
}

#[test]
fn historical_empty_unloaded_and_invalid_utf8_are_errors() {
    let empty = InMemoryTree::default().with_file("tests/only.rs", "not selected");
    assert!(
        merge_base_sources::rust_sources(&empty)
            .unwrap_err()
            .contains("no Rust source files")
    );
    let unloaded = InMemoryTree::from_paths("base", &["src/absent.rs"]);
    assert!(
        merge_base_sources::rust_sources(&unloaded)
            .unwrap_err()
            .contains("src/absent.rs")
    );
    let invalid = InMemoryTree::new(
        "base",
        vec!["src/invalid.rs".into()],
        BTreeMap::from([("src/invalid.rs".into(), vec![0xff])]),
    );
    let error = merge_base_sources::rust_sources(&invalid).unwrap_err();
    assert!(
        error.contains("src/invalid.rs") && error.contains("UTF-8"),
        "{error}"
    );
}

struct UnavailableContents {
    tree: InMemoryTree,
    missing: bool,
}

impl TreeSource for UnavailableContents {
    fn rev(&self) -> &str {
        self.tree.rev()
    }
    fn paths(&self) -> &[String] {
        self.tree.paths()
    }
    fn loaded(&self) -> &BTreeMap<String, Vec<u8>> {
        self.tree.loaded()
    }
    fn read(&self, _: &str) -> Result<Option<&[u8]>, SourceError> {
        if self.missing {
            Ok(None)
        } else {
            Err(SourceError::Unavailable("injected read failure".into()))
        }
    }
}

#[test]
fn listed_historical_sources_cannot_disappear() {
    for missing in [false, true] {
        let tree = UnavailableContents {
            tree: InMemoryTree::from_paths("base", &["src/listed.rs"]),
            missing,
        };
        let error = merge_base_sources::rust_sources(&tree).unwrap_err();
        assert!(error.contains("src/listed.rs"), "{error}");
        assert!(
            error.contains(if missing {
                "no contents"
            } else {
                "cannot read"
            }),
            "{error}"
        );
    }
}

#[test]
fn required_comparison_preserves_outer_and_inner_failures() {
    let outer = merge_base_sources::required::<usize>(Err(RefError::Unavailable(
        "injected absence".into(),
    )));
    assert!(outer.unwrap_err().contains("cannot acquire merge-base"));
    let inner = merge_base_sources::required::<usize>(Ok(Derived {
        at_merge_base: Err("incomplete corpus".into()),
        merge_base: "exact-base".into(),
    }));
    let error = inner.unwrap_err();
    assert!(
        error.contains("cannot measure merge-base exact-base")
            && error.contains("incomplete corpus")
    );
}

#[test]
fn required_comparison_keeps_successful_measurement_and_revision() {
    assert_eq!(
        merge_base_sources::required(Ok(Derived {
            at_merge_base: Ok(0usize),
            merge_base: "exact-base".into(),
        }))
        .unwrap(),
        Derived {
            at_merge_base: 0,
            merge_base: "exact-base".into()
        }
    );
}
