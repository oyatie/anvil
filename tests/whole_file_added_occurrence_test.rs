//! Actual parser-to-whole-file checks in ordinary private files, never executed Rust.
use anvil::git_manager::diff_context::diffs_by_path;
use anvil::monorepo_guard::MonorepoViolation;
use anvil::monorepo_guard::whole_file_expansion::{FileChange, WholeFileExpansion};

fn evaluate(source: &str, body: &str, core: bool) -> anyhow::Result<Vec<MonorepoViolation>> {
    let dir = tempfile::tempdir()?;
    let path = if core { "src/core/x.rs" } else { "src/x.rs" };
    std::fs::create_dir_all(dir.path().join("src/core"))?;
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname='occurrences'\nversion='0.0.0'\nedition='2024'\n",
    )?;
    std::fs::write(
        dir.path().join("src/lib.rs"),
        if core {
            "#[path=\"core/x.rs\"] mod x;\n"
        } else {
            "mod x;\n"
        },
    )?;
    std::fs::write(dir.path().join(path), source)?;
    let files = diffs_by_path(&format!(
        "diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n{body}"
    ));
    WholeFileExpansion::evaluate_whole_file(dir.path(), path, &FileChange::from_diff(&files[0]))
}

fn unwraps(source: &str, body: &str) -> Vec<MonorepoViolation> {
    evaluate(source, body, false)
        .unwrap()
        .into_iter()
        .filter(|finding| finding.category == "PRODUCTION_UNWRAP_DETECTED")
        .collect()
}

#[test]
fn added_unwrap_survives_neighboring_literal_and_comment_masking() {
    for line in [
        "fn f() { let _s = \"é\"; value().unwrap(); }",
        "fn f() { value().unwrap(); } // explanation",
    ] {
        let found = unwraps(&format!("{line}\n"), &format!("@@ -0,0 +1 @@\n+{line}\n"));
        assert_eq!(found.len(), 1);
        assert!(found[0].description.contains("at line 1."));
    }
}

#[test]
fn equal_lines_charge_only_the_added_physical_occurrence() {
    let line = "    value().unwrap();";
    let source = format!("fn f() {{\n{line}\n{line}\n}}\n");
    let found = unwraps(&source, &format!("@@ -2 +2,2 @@\n {line}\n+{line}\n"));
    assert_eq!(found.len(), 1);
    assert!(found[0].description.contains("at line 3."));
}

#[test]
fn test_module_masking_and_multiline_comments_preserve_positions() {
    let source = "#[cfg(test)] mod tests {\nfn f() { value().unwrap(); }\n}\n/* first\n   second */\nfn f() { value().unwrap(); }\n";
    let body = "@@ -1,2 +1,3 @@\n #[cfg(test)] mod tests {\n+fn f() { value().unwrap(); }\n }\n@@ -4,0 +6 @@\n+fn f() { value().unwrap(); }\n";
    let found = unwraps(source, body);
    assert_eq!(found.len(), 1);
    assert!(found[0].description.contains("at line 6."));
}

#[test]
fn untouched_removed_and_quoted_unwraps_are_not_new_findings() {
    let source = "fn f() { value().unwrap(); }\nfn g() {}\n";
    assert!(
        unwraps(
            source,
            "@@ -1 +1,2 @@\n fn f() { value().unwrap(); }\n+fn g() {}\n"
        )
        .is_empty()
    );
    assert!(
        unwraps(
            "fn g() {}\n",
            "@@ -1,2 +1 @@\n-fn f() { value().unwrap(); }\n fn g() {}\n"
        )
        .is_empty()
    );
    for line in [
        "// value().unwrap()",
        "const S: &str = \"value().unwrap()\";",
    ] {
        assert!(unwraps(&format!("{line}\n"), &format!("@@ -0,0 +1 @@\n+{line}\n")).is_empty());
    }
}

#[test]
fn unavailable_or_mismatched_coordinates_are_errors_not_clean_results() {
    for body in [
        "+fn f() {}\n",
        "@@ -0,0 +1,2 @@\n+fn f() {}\n",
        "@@ -0,0 +1 @@\n+fn other() {}\n",
        "@@ -1,0 +2 @@\n+fn f() {}\n",
    ] {
        assert!(evaluate("fn f() {}\n", body, false).is_err(), "{body}");
    }
}

#[test]
fn core_io_uses_occurrences_but_retains_its_lexical_policy() {
    let found = evaluate(
        "use sqlx::Pool;\nuse sqlx::Pool;\n",
        "@@ -1 +1,2 @@\n use sqlx::Pool;\n+use sqlx::Pool;\n",
        true,
    )
    .unwrap();
    let io: Vec<_> = found
        .iter()
        .filter(|finding| finding.category == "CLEAN_ARCHITECTURE_CORE_IO_VIOLATION")
        .collect();
    assert_eq!(io.len(), 1);
    assert!(io[0].description.contains("at line 2."));
    // This separate inherited false-positive is deliberately NOT repaired here.
    let quoted = evaluate("// sqlx::Pool\n", "@@ -0,0 +1 @@\n+// sqlx::Pool\n", true).unwrap();
    assert!(
        quoted
            .iter()
            .any(|finding| finding.category == "CLEAN_ARCHITECTURE_CORE_IO_VIOLATION")
    );
}
