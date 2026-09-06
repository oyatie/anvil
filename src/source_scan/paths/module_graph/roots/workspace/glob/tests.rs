use super::{expand_parts, path_matches, segment_matches};
use std::collections::BTreeSet;

#[test]
fn literal_brackets_and_escaped_metacharacters_match_workspace_names() {
    let root = tempfile::tempdir().expect("workspace fixture");
    for name in ["[", "]", "plain"] {
        let member = root.path().join(name);
        std::fs::create_dir(&member).expect("member directory");
        std::fs::write(member.join("Cargo.toml"), "[package]\nname='member'\n").expect("manifest");
    }
    for (pattern, name) in [("[[]", "["), (r"\[", "["), (r"\]", "]")] {
        assert_eq!(
            super::expand_member_pattern(root.path(), pattern).expect("valid literal glob"),
            vec![root.path().join(name)],
            "pattern {pattern}"
        );
        assert!(super::excluded(name, &[pattern.to_owned()]).expect("valid exclusion"));
        assert!(!super::excluded("plain", &[pattern.to_owned()]).expect("valid exclusion"));
    }
    // A literal '*' is not a valid Windows filename, so exercise its
    // escaping without creating a platform-specific directory.
    assert!(segment_matches(r"\*", "*"));
    assert!(!segment_matches(r"\*", "plain"));
}

#[test]
fn repeated_star_matching_has_a_bounded_state_space() {
    let pattern = format!("{}b", "*a".repeat(15));
    let value = "a".repeat(30);
    let started = std::time::Instant::now();
    assert!(!segment_matches(&pattern, &value));
    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "segment matching revisited an exponential number of states"
    );
}

#[test]
fn repeated_globstars_have_a_bounded_path_and_expansion_state_space() {
    let pattern = vec!["**"; 24];
    let value = vec!["a"; 24];
    assert!(path_matches(&pattern, &value));

    let root = tempfile::tempdir().expect("globstar fixture");
    let mut directory = root.path().to_path_buf();
    for name in ["a", "b", "c", "d", "e", "f"] {
        directory.push(name);
        std::fs::create_dir(&directory).expect("nested directory");
    }
    let mut states = BTreeSet::new();
    let mut found = Vec::new();
    expand_parts(root.path(), &pattern, &mut states, &mut found).expect("expand globstars");
    assert!(states.len() <= 7 * (pattern.len() + 1));
}

#[test]
fn cargo_character_classes_allow_a_leading_literal_close_bracket() {
    assert!(segment_matches("[!]]", "x"));
    assert!(!segment_matches("[!]]", "]"));
    assert!(segment_matches("[]]", "]"));
    assert!(segment_matches("[^x]", "^"));
    assert!(segment_matches("[^x]", "x"));
    assert!(!segment_matches("[^x]", "y"));
}
