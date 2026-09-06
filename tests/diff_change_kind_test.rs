//! Shared parser evidence using inert strings only.

use anvil::git_manager::diff_context::{BothSides, FileChangeKind as Kind, diffs_by_path};

fn normal(path: &str, before: &str, after: &str, metadata: &str, body: &str) -> String {
    format!(
        "diff --git a/{path} b/{path}\n{metadata}--- {before}\n+++ {after}\n@@ -1 +1 @@\n{body}"
    )
}

#[test]
fn complete_add_delete_modify_endpoints_are_distinct() {
    for (before, after, marker, expected) in [
        ("/dev/null", "b/x.rs", "new file mode 100644\n", Kind::Added),
        (
            "a/x.rs",
            "/dev/null",
            "deleted file mode 100644\n",
            Kind::Deleted,
        ),
        ("a/x.rs", "b/x.rs", "", Kind::Modified),
    ] {
        let files = diffs_by_path(&normal("x.rs", before, after, marker, "+new\n-old\n"));
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "x.rs");
        assert_eq!(files[0].change_kind(), Some(expected));
    }
}

#[test]
fn empty_binary_and_mode_only_observations_need_no_hunk() {
    for (marker, expected) in [
        ("new file mode 100644", Kind::Added),
        ("deleted file mode 100644", Kind::Deleted),
        ("old mode 100644\nnew mode 100755", Kind::Modified),
    ] {
        for tail in ["", "Binary files differ\n"] {
            let files = diffs_by_path(&format!("diff --git a/x b/x\n{marker}\n{tail}"));
            assert_eq!(files[0].change_kind(), Some(expected));
            assert!(files[0].added().is_empty());
            assert!(files[0].after_change().is_empty());
            assert_eq!(files[0].net_lines(), 0);
        }
    }
}

#[test]
fn rename_and_copy_preserve_both_identities_even_without_a_hunk() {
    for (verb, expected) in [("rename", Kind::Renamed), ("copy", Kind::Copied)] {
        for tail in [
            "",
            "--- a/old name.rs\n+++ b/new name.rs\n@@ -1 +1 @@\n-old\n+new\n",
            "old mode 100644\nnew mode 100755\n",
        ] {
            let diff = format!(
                "diff --git a/old name.rs b/new name.rs\nsimilarity index 100%\n{verb} from old name.rs\n{verb} to new name.rs\n{tail}"
            );
            let files = diffs_by_path(&diff);
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].path, "new name.rs");
            assert_eq!(files[0].previous_path(), Some("old name.rs"));
            assert_eq!(files[0].change_kind(), Some(expected));
        }
    }
}

#[test]
fn content_projections_and_multiple_hunks_remain_separate() {
    let diff = normal(
        "x.rs",
        "a/x.rs",
        "b/x.rs",
        "index abc..def 100644\n",
        " context\n-old\n+new\n@@ -4 +4 @@\n+more\n",
    );
    let files = diffs_by_path(&diff);
    let file = &files[0];
    assert_eq!(file.added(), "new\nmore\n");
    assert_eq!(file.after_change(), "context\nnew\nmore\n");
    assert_eq!(file.net_lines(), 1);
    let raw = file.both_sides(BothSides::ContractComparesRemovedFields);
    assert!(raw.contains("-old\n"));
    assert!(!raw.contains("index abc"));
    assert!(!file.added().contains("old"));
    assert!(!file.after_change().contains("old"));
}

#[test]
fn header_only_attribution_does_not_invent_change_kind() {
    assert!(diffs_by_path("+orphan\n").is_empty());
    let files = diffs_by_path(
        "+++ b/src/my file.rs\n+one\n+++ b/other.rs\n+two\n+++ b/src/my file.rs\n+three\n",
    );
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].path, "src/my file.rs");
    assert_eq!(files[0].added(), "one\nthree\n");
    assert_eq!(files[0].change_kind(), None);
    let header = diffs_by_path("diff --git a/x b/x\n+one\n");
    assert_eq!(header[0].path, "x");
    assert_eq!(header[0].change_kind(), None);
}

#[test]
fn contradictory_or_partial_evidence_is_unknown() {
    for diff in [
        normal(
            "x",
            "/dev/null",
            "b/x",
            "new file mode 100644\ndeleted file mode 100644\n",
            "",
        ),
        normal("x", "/dev/null", "/dev/null", "", ""),
        normal("x", "a/other", "b/x", "", ""),
        "diff --git a/x b/y\nrename from x\ncopy to y\n".into(),
        "diff --git a/x b/y\nrename from x\n".into(),
        "diff --git a/x b/x\nold mode 100644\n".into(),
        "diff --git a/x b/x\n--- a/x\n+++ b/x\n+++ b/y\n".into(),
    ] {
        assert!(
            diffs_by_path(&diff)
                .iter()
                .all(|file| file.change_kind().is_none()),
            "{diff}"
        );
    }
}

#[test]
fn repeated_sections_cannot_erase_a_contradiction() {
    let add = normal("x", "/dev/null", "b/x", "new file mode 100644\n", "+one\n");
    let delete = normal(
        "x",
        "a/x",
        "/dev/null",
        "deleted file mode 100644\n",
        "-one\n",
    );
    let files = diffs_by_path(&format!("{add}{delete}{add}"));
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].change_kind(), None);
    assert_eq!(files[0].net_lines(), 1);
}

#[test]
fn unsupported_sections_reset_all_previous_metadata_and_content() {
    let diff = "diff --git a/x b/x\nnew file mode 100644\n+one\ndiff --cc unknown\n+not-x\ndiff --git a/y b/y\n+two\n";
    let files = diffs_by_path(diff);
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].change_kind(), Some(Kind::Added));
    assert_eq!(files[0].added(), "one\n");
    assert_eq!(files[1].change_kind(), None);
    assert_eq!(files[1].added(), "two\n");
}

#[test]
fn hunk_text_is_not_file_metadata() {
    let diff = normal(
        "x",
        "a/x",
        "b/x",
        "",
        "+new file mode 100644\n+rename from other\n+++ b/ordinary-content\n--- a/removed-content\n",
    );
    let files = diffs_by_path(&diff);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].change_kind(), Some(Kind::Modified));
    assert!(files[0].added().contains("++ b/ordinary-content"));
    assert_eq!(files[0].net_lines(), 2);
}
