//! CODEOWNERS pattern semantics, pinned before the matcher is redesigned.
//!
//! `codeowners_matches` routes ownership and had no test of any kind. These
//! characterise what it does today so a rewrite can be shown to preserve it —
//! a redesign with nothing to compare against is a rewrite done blind.

use anvil::change_delivery::core::OwnerMap;
use anvil::change_delivery::core::owners::codeowners_matches as m;

#[test]
fn a_bare_star_matches_everything() {
    assert!(m("*", "src/lib.rs"));
    assert!(m("*", "a/b/c/d.rs"));
    assert!(m("*", "README.md"));
}

#[test]
fn a_pattern_with_no_slash_matches_a_basename_at_any_depth() {
    assert!(m("lib.rs", "src/lib.rs"));
    assert!(m("lib.rs", "a/b/lib.rs"));
    assert!(m("lib.rs", "lib.rs"));
    assert!(!m("lib.rs", "src/main.rs"));
}

#[test]
fn a_trailing_slash_matches_the_directorys_contents_not_the_directory() {
    assert!(m("docs/", "docs/adr.md"));
    assert!(m("docs/", "a/docs/adr.md"));
    // The directory entry itself has no slash after it, so it is not contents.
    assert!(!m("docs/", "docs"));
}

#[test]
fn a_leading_slash_anchors_to_the_repository_root() {
    assert!(m("/src/lib.rs", "src/lib.rs"));
    assert!(!m("/src/lib.rs", "vendor/src/lib.rs"));
}

#[test]
fn an_unanchored_path_pattern_matches_at_any_depth() {
    assert!(m("src/lib.rs", "src/lib.rs"));
    assert!(m("src/lib.rs", "crates/a/src/lib.rs"));
}

#[test]
fn a_single_star_spans_one_segment_and_double_star_spans_many() {
    assert!(m("/src/*.rs", "src/lib.rs"));
    assert!(!m("/src/*.rs", "src/a/lib.rs"));
    assert!(m("/src/**/*.rs", "src/a/b/lib.rs"));
}

#[test]
fn a_non_matching_pattern_is_refused() {
    assert!(!m("/docs/", "src/lib.rs"));
    assert!(!m("billing/", "src/lib.rs"));
    assert!(!m("*.md", "src/lib.rs"));
}

#[test]
fn an_extension_pattern_matches_by_basename_anywhere() {
    assert!(m("*.md", "README.md"));
    assert!(m("*.md", "docs/adr/0001.md"));
}

#[test]
fn an_anchored_basename_matches_only_the_root_file() {
    assert!(m("/README.md", "README.md"));
    assert!(!m("/README.md", "docs/README.md"));
    assert!(!m("/README.md", "a/b/README.md"));
    assert!(m("README.md", "a/b/README.md"));
}

#[test]
fn an_anchored_directory_requires_contents_at_the_root() {
    for path in ["docs/file.md", "docs/nested/file.md"] {
        assert!(m("/docs/", path), "{path}");
    }
    for path in ["docs", "x/docs", "nested/docs/file.md"] {
        assert!(!m("/docs/", path), "{path}");
    }
    assert!(m("docs/", "nested/docs/file.md"));
    assert!(m("/src/docs/", "src/docs/file.md"));
    assert!(!m("/src/docs/", "src/docs"));
    assert!(!m("/src/docs/", "nested/src/docs/file.md"));
}

#[test]
fn anchored_unicode_and_recursive_patterns_keep_segment_boundaries() {
    assert!(m("/文档/**/*.md", "文档/说明.md"));
    assert!(m("/文档/**/*.md", "文档/深/说明.md"));
    assert!(!m("/文档/**/*.md", "nested/文档/说明.md"));
    assert!(m("/文档/", "文档/说明.md"));
    assert!(!m("/文档/", "文档"));
    assert!(m("/文*档/", "文书档/说明.md"));
}

#[test]
fn the_last_specific_rule_wins_only_where_its_anchor_matches() {
    let owners = OwnerMap::from_codeowners("docs/ @docs\n/README.md @root\n");
    assert_eq!(owners.owners_of("README.md"), ["@root".into()].into());
    assert_eq!(owners.owners_of("docs/README.md"), ["@docs".into()].into());
    let unanchored = OwnerMap::from_codeowners("docs/ @docs\nREADME.md @readme\n");
    assert_eq!(
        unanchored.owners_of("docs/README.md"),
        ["@readme".into()].into()
    );
}
