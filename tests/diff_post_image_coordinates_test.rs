//! Inert strings through the actual shared parser; no Git or process fixtures.
use anvil::git_manager::diff_context::{BothSides, diffs_by_path};

fn section(body: &str) -> String {
    format!("diff --git a/x.rs b/x.rs\n--- a/x.rs\n+++ b/x.rs\n{body}")
}

fn positions(body: &str) -> Result<Vec<(usize, String)>, String> {
    let files = diffs_by_path(&section(body));
    files[0]
        .added_post_image_lines()
        .map(|lines| {
            lines
                .iter()
                .map(|line| (line.line(), line.text().to_owned()))
                .collect()
        })
        .map_err(str::to_owned)
}

#[test]
fn coordinates_follow_each_side_and_keep_hunk_gaps() {
    let body = "@@ -2,3 +2,3 @@ label\n context\n-old\n+équal\n tail\n@@ -8 +8,2 @@\n-old\n+équal\n+last\n\\ No newline at end of file\n";
    assert_eq!(
        positions(body).unwrap(),
        [(3, "équal".into()), (8, "équal".into()), (9, "last".into())]
    );
    let files = diffs_by_path(&section(body));
    assert_eq!(files[0].added(), "équal\néqual\nlast\n");
    assert_eq!(files[0].net_lines(), 1);
    assert_eq!(
        files[0].after_change(),
        "context\néqual\ntail\néqual\nlast\n"
    );
    assert!(
        files[0]
            .both_sides(BothSides::ContractComparesRemovedFields)
            .contains("-old\n")
    );
}

#[test]
fn zero_width_ranges_and_crlf_have_defined_coordinates() {
    assert_eq!(
        positions("@@ -0,0 +1 @@\n+one\n").unwrap(),
        [(1, "one".into())]
    );
    assert_eq!(positions("@@ -1 +0,0 @@\n-old\n").unwrap(), []);
    assert_eq!(
        positions("@@ -1 +1 @@\r\n-old\r\n+é\r\n").unwrap(),
        [(1, "é".into())]
    );
    assert_eq!(
        positions("@@ -1 +1 @@\n-a\n+b\n@@ -1,0 +2 @@\n+c\n").unwrap(),
        [(1, "b".into()), (2, "c".into())]
    );
}

#[test]
fn unavailable_coordinates_do_not_erase_legacy_added_text() {
    for body in [
        "+x\n",
        "@@ nonsense\n+x\n",
        "@@@ -1 +1 @@@\n+x\n",
        "@@ -0,0 +1,2 @@\n+x\n",
        "@@ -0,0 +1 @@\n+x\n+y\n",
        "@@ -1 +1 @@\nx\n",
        "@@ -0,0 +0 @@\n+x\n",
        "@@ -0,0 +999999999999999999999999999999 @@\n+x\n",
        "@@ -0,0 +1 @@\n+x\n@@ -0,0 +1 @@\n+x\n",
        "@@ -4 +4 @@\n-a\n+x\n@@ -2 +2 @@\n-a\n+x\n",
    ] {
        assert!(positions(body).is_err(), "{body}");
        let files = diffs_by_path(&section(body));
        if body.contains("+x\n") {
            assert!(files[0].added().contains("x\n"));
        }
    }
}

#[test]
fn identity_and_section_boundaries_cannot_launder_uncertainty() {
    let valid = section("@@ -0,0 +1 @@\n+x\n");
    let repeated = diffs_by_path(&format!("{valid}{valid}"));
    assert!(repeated[0].added_post_image_lines().is_err());
    for tail in ["Binary files differ\n", "GIT binary patch\n", "+x\n"] {
        let files = diffs_by_path(&section(tail));
        assert!(files[0].added_post_image_lines().is_err());
    }
    let mode = diffs_by_path("diff --git a/x.rs b/x.rs\nold mode 100644\nnew mode 100755\n");
    assert!(mode[0].added_post_image_lines().unwrap().is_empty());
    let unknown = diffs_by_path("+++ b/x.rs\n@@ -0,0 +1 @@\n+x\n");
    assert!(unknown[0].added_post_image_lines().is_err());
    let rename = diffs_by_path(
        "diff --git a/old.rs b/new.rs\nrename from old.rs\nrename to new.rs\n--- a/old.rs\n+++ b/new.rs\n@@ -1 +1 @@\n-old\n+new\n",
    );
    assert_eq!(rename[0].path, "new.rs");
    assert_eq!(rename[0].added_post_image_lines().unwrap()[0].line(), 1);
    let reset = diffs_by_path(&format!(
        "{valid}diff --cc skipped\n+not-x\n{}",
        section("@@ -0,0 +1 @@\n+y\n").replace("x.rs", "y.rs")
    ));
    assert_eq!(reset.len(), 2);
    assert_eq!(reset[1].added_post_image_lines().unwrap()[0].text(), "y");
    // Dropped unsupported sections remain the separate whole-diff completeness gap.
}
