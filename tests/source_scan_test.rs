//! `code_only` is now production code, so it is tested as production code.
//!
//! It was a private helper in one test file while seven other spellings of the
//! same idea lived elsewhere. Extracting it made it shared; these are the
//! properties a caller is entitled to rely on.

use anvil::source_scan::{code_only, without_commentary};

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
    // Nine implementations of this idea existed under four behaviours and one
    // name -- the generating class of this session, found in the test
    // scaffolding rather than the product. All nine now forward to
    // `source_scan`, so the behaviour has one definition and the count is zero.
    //
    // Migrating them was not a rename. Two scans BROKE on the shared
    // `code_only` and were right to: the diff-parsing ratchet's count fell from
    // nineteen sites to two, because every marker it matches is spelled as a
    // string literal and `code_only` strips literal bodies. That is what
    // produced `without_commentary`: a scan whose subject IS a literal needs
    // the literals kept, and one whose subject is a construct needs them gone.
    // Two questions, two names, rather than one name with four behaviours.
    const REMAINING: usize = 0;

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
                    let defines = body.contains("fn code_only(")
                        || body.contains("fn code_before_comment(")
                        || body.contains("fn code_only_body(");
                    // A thin wrapper that DELEGATES is not another
                    // implementation. Three remain by name, because their call
                    // sites and the fidelity registry's citations refer to
                    // them, but each one forwards to the shared scanner -- so
                    // the behaviour has exactly one definition, which is the
                    // property this counts.
                    let delegates = body.contains("source_scan::");
                    // Similar name, different logic. Not everything that looks
                    // like duplication is: `code_before_comment` reads CONFIG
                    // -- YAML, shell, SQL -- so it strips `#` and `--` as well,
                    // and treats `//` as code when preceded by `:` or
                    // `http://example.com` loses its host. Migrating it to the
                    // Rust scanner was tried and reverted: the URL guard went
                    // with it and a cleartext-endpoint test found zero where it
                    // expects one. Rule of Three applies to the same LOGIC.
                    let different_language = p.ends_with("harness/cleartext_scan.rs");
                    if defines && !delegates && !different_language {
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

// ---------------------------------------------------------------------------
// without_commentary: the other question
// ---------------------------------------------------------------------------

#[test]
fn without_commentary_keeps_the_literal_a_scan_is_looking_for() {
    // The distinction that forced two functions. A scan whose SUBJECT is a
    // literal -- the `"+++ b/"` a diff parser keys on -- must still see it.
    let src = r#"if let Some(p) = line.strip_prefix("+++ b/") { } // a comment"#;
    let kept = without_commentary(src);
    assert!(kept.contains("+++ b/"), "the marker was stripped: {kept}");
    assert!(!kept.contains("a comment"));

    // And the other function must NOT, which is why swapping them silently
    // took a ratchet from nineteen sites to two.
    assert!(!code_only(src).contains("+++ b/"));
}

#[test]
fn without_commentary_still_removes_every_kind_of_comment() {
    let out = without_commentary(concat!(
        "let a = 1; // line\n",
        "/* block */\n",
        "let b = \"kept\";\n",
    ));
    assert!(!out.contains("line"));
    assert!(!out.contains("block"));
    assert!(out.contains("let a = 1;"));
    assert!(out.contains("kept"), "the literal body survives");
}

#[test]
fn without_commentary_does_not_end_a_literal_on_an_escaped_quote() {
    // Same escape handling as `code_only`: falling out of a literal early
    // makes the scanner read the rest of the line as code.
    let out = without_commentary(r#"let s = "a \" // not a comment"; let after = 1;"#);
    assert!(
        out.contains("not a comment"),
        "left the literal early: {out}"
    );
    assert!(out.contains("let after = 1;"));
}

#[test]
fn without_commentary_preserves_byte_offsets() {
    let src = "let a = 1; // comment
let b = 2;
";
    let out = without_commentary(src);
    assert_eq!(out.len(), src.len());
    let at = src.find("let b").expect("present");
    assert_eq!(&out[at..at + 5], "let b");
}

/// The governance scans must actually run before a push.
///
/// A remedy the ledger records as live at `PrePush` and which the hook does not
/// invoke is the same defect as a gate that cannot fire: it reads as covered
/// and refuses nothing. Asserted against the hook's text rather than its
/// behaviour, because running a push from a test is not something a test may do.
#[test]
fn the_pre_push_hook_runs_the_source_only_scans() {
    let hook = std::fs::read_to_string("src/git_manager/hooks/pre-push")
        .expect("the pre-push hook is in the tree");

    for scan in [
        "diff_parsing_ratchet_test",
        "prose_counts_test",
        "source_scan_test",
        "removals_are_not_reachable_by_accident_test",
        "postmortem_ledger_test",
        "gate_proof_test",
    ] {
        assert!(
            hook.contains(scan),
            "pre-push does not run {scan}, so the class it refuses is only caught in CI -- \
             which is after the fact, and every CI catch is another wave of fixes"
        );
    }

    // And it must still refuse on failure rather than reporting and continuing.
    assert!(
        hook.contains("REFUSED: a governance scan failed"),
        "the hook runs the scans and does not act on the result"
    );
}
