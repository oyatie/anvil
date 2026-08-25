//! `code_only` is now production code, so it is tested as production code.
//!
//! It was a private helper in one test file while seven other spellings of the
//! same idea lived elsewhere. Extracting it made it shared; these are the
//! properties a caller is entitled to rely on.

use anvil::source_scan::code_only;

#[test]
fn a_line_comment_is_removed_and_the_code_before_it_survives() {
    let out = code_only("let x = 1; // set x to 1\nlet y = 2;\n");
    assert!(out.contains("let x = 1;"));
    assert!(!out.contains("set x to 1"));
    assert!(out.contains("let y = 2;"), "a comment ends at the newline");
}

#[test]
fn a_block_comment_is_removed_even_across_lines() {
    // The weakest of the eight spellings dropped whole `//` lines and left
    // block comments entirely, so a scanner reading one saw commentary as code.
    let out = code_only("let a = 1;\n/* explanation\n   continues here */\nlet b = 2;\n");
    assert!(out.contains("let a = 1;"));
    assert!(out.contains("let b = 2;"));
    assert!(!out.contains("explanation"));
    assert!(!out.contains("continues here"));
}

#[test]
fn a_string_body_is_removed_and_its_quotes_are_kept() {
    // Keeping the quotes is what lets a scanner still see that a literal was
    // there, without seeing what was in it. A gate looking for `unwrap()` must
    // not match a test fixture that merely quotes the word.
    let out = code_only(r#"let s = "call unwrap() here"; let t = 3;"#);
    assert!(!out.contains("unwrap()"));
    assert!(out.contains('"'));
    assert!(out.contains("let t = 3;"));
}

#[test]
fn a_string_spanning_lines_stays_a_string_the_whole_way() {
    // The defect that forced the extraction. A line-wise scanner sees the
    // second line with no opening quote and reads it as code, so prose inside
    // a multi-line literal was reported as a call site.
    let src = "let doc = \"first line\n\
               both_sides(BothSides::Whatever)\n\
               last line\";\nlet after = 1;\n";
    let out = code_only(src);
    assert!(
        !out.contains("both_sides("),
        "text inside a multi-line string was read as code: {out}"
    );
    assert!(out.contains("let after = 1;"));
}

#[test]
fn an_escaped_quote_does_not_end_the_string() {
    // Without this the scanner falls out of the literal early and reads the
    // rest of the file as code.
    let out = code_only(r#"let s = "a \" b unwrap()"; let after = 1;"#);
    assert!(!out.contains("unwrap()"));
    assert!(out.contains("let after = 1;"));
}

#[test]
fn byte_offsets_are_preserved() {
    // Everything removed becomes spaces rather than disappearing, so a scanner
    // can report a line and column from the stripped text without a second
    // pass over the original. Nothing else pinned this.
    let src = "let a = 1; // comment\nlet b = 2;\n";
    let out = code_only(src);
    assert_eq!(out.len(), src.len(), "offsets shifted");
    assert_eq!(
        out.lines().count(),
        src.lines().count(),
        "line numbering shifted"
    );
    let needle = src.find("let b").expect("present");
    assert_eq!(&out[needle..needle + 5], "let b", "column shifted");
}

#[test]
fn code_only_is_not_re_spelled_a_ninth_time() {
    // Eight implementations of this idea existed under four behaviours and one
    // name. That is the generating class of this session, found in the test
    // scaffolding rather than the product. The count may fall and must not
    // rise; each remaining one is a behaviour-preserving migration away.
    const REMAINING: usize = 8;

    let mut found: Vec<String> = Vec::new();
    for dir in ["src", "tests"] {
        let mut stack = vec![std::path::PathBuf::from(dir)];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    if p.ends_with("source_scan/mod.rs") {
                        continue;
                    }
                    // Scanned as CODE, using the function under test. Without
                    // it this scan matched its own string literals and counted
                    // itself -- which is precisely the defect the extraction
                    // exists to remove, demonstrated on the detector itself.
                    let body = code_only(&std::fs::read_to_string(&p).unwrap_or_default());
                    if body.contains("fn code_only(")
                        || body.contains("fn code_before_comment(")
                        || body.contains("fn code_only_body(")
                    {
                        found.push(p.display().to_string());
                    }
                }
            }
        }
    }
    found.sort();
    assert_eq!(
        found.len(),
        REMAINING,
        "{} local spelling(s) of `code_only` remain; the ledger records {REMAINING}.\n\
         If this ROSE, a ninth was written -- use `anvil::source_scan::code_only`.\n\
         If this FELL, lower REMAINING here in the same change.\n  {}",
        found.len(),
        found.join("\n  ")
    );
}
