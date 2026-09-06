//! Byte-preserving masks and nested comments through the actual shared lexer.

use anvil::source_scan::{code_only, without_commentary};

fn assert_positions(source: &str, output: &str, code: &str) {
    assert_eq!(output.len(), source.len(), "UTF-8 byte length shifted");
    let newlines = |text: &str| {
        text.bytes()
            .enumerate()
            .filter_map(|(at, byte)| (byte == b'\n').then_some(at))
            .collect::<Vec<_>>()
    };
    assert_eq!(newlines(output), newlines(source));
    let at = source.find(code).expect("control code exists in input");
    assert_eq!(&output[at..at + code.len()], code);
}

#[test]
fn unicode_comment_masks_preserve_byte_positions_in_both_projections() {
    let source = "let α = 1; // é한🦀\n/* ñ\n界 */ let after = α;\n";
    for output in [code_only(source), without_commentary(source)] {
        assert_positions(source, &output, "let after = α;");
        assert!(output.starts_with("let α = 1;"));
        for commentary in ["é", "한", "🦀", "ñ", "界"] {
            assert!(!output.contains(commentary));
        }
    }
}

#[test]
fn unicode_literal_mask_preserves_bytes_while_other_projection_keeps_literal() {
    let source = "let s = \"é한🦀\n界\"; let after = 1;";
    let masked = code_only(source);
    assert_positions(source, &masked, "let after = 1;");
    for literal in ["é", "한", "🦀", "界"] {
        assert!(!masked.contains(literal));
    }
    assert_eq!(masked.matches('"').count(), 2);
    assert_eq!(without_commentary(source), source);
}

#[test]
fn nested_comments_do_not_expose_the_outer_remainder() {
    let source = "before(); /* outer /* inner */ fake_call(); */ after();";
    for output in [code_only(source), without_commentary(source)] {
        assert_positions(source, &output, "after();");
        assert!(output.starts_with("before();"));
        for commentary in ["outer", "inner", "fake_call", "/*", "*/"] {
            assert!(!output.contains(commentary), "{output}");
        }
    }
}

#[test]
fn nested_comments_preserve_newlines_and_depth_across_lines() {
    let source = "/* é\n/* 한 /* 🦀 */ still_inner */\nstill_outer */\nafter();\n";
    for output in [code_only(source), without_commentary(source)] {
        assert_positions(source, &output, "after();");
        assert_eq!(output.trim(), "after();");
    }
}

#[test]
fn an_unterminated_outer_comment_stays_masked_after_inner_close() {
    let source = "before(); /* outer /* inner */ still_comment\n界";
    for output in [code_only(source), without_commentary(source)] {
        assert_positions(source, &output, "before();");
        assert_eq!(output.trim(), "before();");
    }
}

#[test]
fn quoted_delimiters_and_escaped_quotes_do_not_change_comment_depth() {
    let source = r#"let s = "é \" /* not_comment */ // literal"; after(); /* real */"#;
    let kept = without_commentary(source);
    assert_positions(source, &kept, "after();");
    assert!(kept.contains(r#""é \" /* not_comment */ // literal""#));
    assert!(!kept.contains("real"));
    let masked = code_only(source);
    assert_positions(source, &masked, "after();");
    for literal in ["é", "not_comment", "literal", "real"] {
        assert!(!masked.contains(literal));
    }
}

#[test]
fn ordinary_code_and_adjacent_comments_keep_existing_projection_contracts() {
    let code = "let α = (1 / 2) * 3;\n";
    assert_eq!(code_only(code), code);
    assert_eq!(without_commentary(code), code);
    let source = "a();/**//* ordinary */// line\nb();";
    for output in [code_only(source), without_commentary(source)] {
        assert_positions(source, &output, "b();");
        assert!(output.starts_with("a();"));
        assert!(!output.contains("ordinary"));
        assert!(!output.contains("line"));
    }
}
