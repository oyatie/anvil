//! CODEOWNERS pattern semantics, pinned before the matcher is redesigned.
//!
//! `codeowners_matches` routes ownership and had no test of any kind. These
//! characterise what it does today so a rewrite can be shown to preserve it —
//! a redesign with nothing to compare against is a rewrite done blind.

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
