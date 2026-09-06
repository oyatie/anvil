//! Portable contracts: inert Markdown/source text and ordinary local files only.

use super::common::docs_claims::{
    Claim, check_claim, claims_from_text, claims_in, evaluate, forbidden_hits, forbidden_needles,
    markdown_file_admission, md_files_under, proxy_sources,
};
use std::path::Path;

fn parse(text: &str) -> Vec<Claim> {
    claims_from_text(Path::new("ordinary.md"), text)
}

#[test]
fn fence_closers_preserve_opening_character_length_and_suffix() {
    for (opening, candidate, closes) in [
        ("````", "```", false),
        ("````", "````", true),
        ("````", "`````", true),
        ("```", "~~~", false),
        ("~~~", "```", false),
        ("~~~", "~~~~ \t", true),
        ("~~~", "~~~ text", false),
        ("```", "```~", false),
        ("```", "    ```", false),
        ("```", "   ```", true),
    ] {
        let claims = parse(&format!(
            "{opening}\n{candidate}\ncount 'a' in ordinary.md #= 1\n"
        ));
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].fenced, !closes, "{opening:?}, {candidate:?}");
        assert_eq!(claims[0].line, 3);
    }
}

#[test]
fn opening_fences_have_finite_indentation_and_info_rules() {
    for (opening, opens) in [
        ("```", true),
        ("   ```text", true),
        ("    ```", false),
        ("\t```", false),
        ("``", false),
        ("```text`", false),
        ("~~~text`", true),
    ] {
        let claims = parse(&format!("{opening}\ncount 'a' in ordinary.md #= 1\n"));
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].fenced, opens, "{opening:?}");
    }
}

#[test]
fn opening_markers_are_retained_but_not_fenced_body() {
    let claims = parse("``` count 'a' in ordinary.md #= 1\ncount 'b' in ordinary.md #= 2\n```\n");
    assert_eq!(claims.len(), 2);
    assert!(!claims[0].fenced);
    assert!(claims[0].spec.starts_with("```"));
    assert!(claims[1].fenced);
    assert_eq!(claims[1].line, 2);
}

#[test]
fn invalid_fence_like_markers_keep_their_literal_specification() {
    let claims = parse("````\n``` count 'a' in ordinary.md #= 1\n````\n#=\n");
    assert_eq!(claims.len(), 2);
    assert!(claims[0].fenced);
    assert_eq!(claims[0].spec, "``` count 'a' in ordinary.md");
    assert!(!claims[1].fenced);
    assert!(claims[1].spec.is_empty());
    assert!(claims[1].expected.is_empty());
    assert_eq!(claims[1].line, 4);
}

#[test]
fn every_declared_proxy_needle_uses_the_source_normalization() {
    for needle in forbidden_needles() {
        assert!(forbidden_hits(&needle).contains(&needle), "{needle:?}");
        let spaced = needle.chars().map(|c| format!("{c} ")).collect::<String>();
        assert!(forbidden_hits(&spaced).contains(&needle), "{needle:?}");
        assert!(forbidden_hits(&format!("// {needle}\n")).is_empty());
    }
    assert!(forbidden_hits("use std::{r#process};").contains(&"std::process".to_string()));
    assert!(forbidden_hits("let ordinary = 1;").is_empty());
}

#[test]
fn complete_historical_platform_prerequisite_is_unix_gated() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(root.join("tests/docs_claims_legacy/mod.rs"))
        .expect("historical source");
    let syntax = syn::parse_file(&source).expect("parse source only");
    let function = syntax
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Fn(function)
                if function.sig.ident == "containment_is_exercised_rather_than_asserted" =>
            {
                Some(function)
            }
            _ => None,
        })
        .expect("historical function exists");
    assert!(
        function.attrs.iter().any(|attr| {
            attr.path().is_ident("cfg")
                && attr
                    .parse_args::<syn::Path>()
                    .is_ok_and(|path| path.is_ident("unix"))
        }),
        "the whole historical function, not only a nested block, must be Unix-gated"
    );
}

#[test]
fn markdown_admission_requires_regular_files_only_for_matched_entries() {
    assert_eq!(markdown_file_admission(true, true), Ok(true));
    assert_eq!(markdown_file_admission(true, false), Ok(false));
    assert_eq!(markdown_file_admission(false, false), Ok(false));
    assert_eq!(
        markdown_file_admission(false, true),
        Err("unsupported Markdown document kind: expected a regular file")
    );
}

#[test]
fn ordinary_working_tree_documents_and_directories_remain_distinct() {
    let tmp = tempfile::tempdir().expect("temporary directory");
    let root = tmp.path().canonicalize().expect("canonical root");
    let dir = root.join("docs/plan");
    std::fs::create_dir_all(dir.join("nested.md")).expect("ordinary directories");
    let first = dir.join("first.MD");
    let nested = dir.join("nested.md/second.md");
    std::fs::write(&first, "a a\nb\na\n").expect("ordinary file");
    std::fs::write(&nested, "```\ncount 'a' in docs/plan/first.MD #= 2\n```\n")
        .expect("ordinary document");
    std::fs::write(dir.join("ignored.txt"), "ordinary text").expect("ordinary file");
    assert_eq!(
        md_files_under(&root, &dir).expect("walk"),
        vec![first, nested.clone()]
    );
    assert_eq!(
        evaluate(&root, "count 'a' in docs/plan/first.MD").expect("count lines"),
        "2"
    );
    let claims = claims_in(&nested).expect("read wrapper");
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].file, nested);
    assert_eq!(claims[0].line, 2);
    assert!(claims[0].fenced);
}

#[test]
fn proxy_inventory_recurses_over_regular_sources_and_keeps_the_consumer() {
    let tmp = tempfile::tempdir().expect("temporary directory");
    let root = tmp.path().canonicalize().expect("canonical root");
    std::fs::create_dir_all(root.join("tests/common/nested/deeper")).expect("ordinary directories");
    let paths = [
        "tests/common/mod.rs",
        "tests/common/nested/deeper/leaf.rs",
        "tests/common/nested/mod.rs",
        "tests/published_commands_reproduce_test.rs",
    ];
    for path in paths {
        std::fs::write(root.join(path), "// harmless source text, never compiled\n")
            .expect("ordinary file");
    }
    std::fs::write(root.join("tests/common/ignored.md"), "ordinary text").expect("ordinary file");
    assert_eq!(
        proxy_sources(&root).expect("source inventory"),
        paths.map(|p| root.join(p)).to_vec()
    );
}

#[test]
fn every_marker_keeps_its_verdict_without_reading_a_claim_target() {
    let root = Path::new("unused");
    for (text, error) in [
        ("~~~ info #= 1\n", "outside a fenced block"),
        ("ordinary #= 1\n", "outside a fenced block"),
        ("```\nnot-a-count #= 1\n", "not a claim"),
        ("```\ncount 'a' in ordinary.md #=\n", "no expected value"),
    ] {
        let claims = parse(text);
        assert_eq!(claims.len(), 1);
        assert!(
            check_claim(root, &claims[0])
                .expect("explicit failure")
                .contains(error)
        );
    }
}

#[test]
fn proxy_inventory_requires_both_the_helper_tree_and_regular_consumer() {
    for consumer_kind in ["missing", "directory", "regular"] {
        let tmp = tempfile::tempdir().expect("temporary directory");
        let root = tmp.path().canonicalize().expect("canonical root");
        std::fs::create_dir(root.join("tests")).expect("ordinary directory");
        let consumer = root.join("tests/published_commands_reproduce_test.rs");
        match consumer_kind {
            "directory" => std::fs::create_dir(consumer).expect("ordinary directory"),
            "regular" => std::fs::write(consumer, "// ordinary source").expect("ordinary file"),
            _ => {}
        }
        assert!(
            proxy_sources(&root).is_err(),
            "missing helper tree: {consumer_kind}"
        );
    }
}

#[test]
fn repository_proxy_membership_includes_each_extracted_owner() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let paths = proxy_sources(root).expect("actual source inventory");
    for relative in [
        "tests/common/mod.rs",
        "tests/common/docs_claims.rs",
        "tests/common/docs_claims/markdown.rs",
        "tests/common/docs_claims/paths.rs",
        "tests/common/docs_claims/proxy.rs",
        "tests/published_commands_reproduce_test.rs",
    ] {
        assert!(paths.contains(&root.join(relative)), "missing {relative}");
    }
}
