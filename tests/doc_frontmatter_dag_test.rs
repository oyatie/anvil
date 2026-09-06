use anvil::doc_guard::frontmatter::FrontmatterValidator;
use std::path::Path;

#[test]
fn test_frontmatter_supersession_dag_validation() {
    let content = r#"---
schema: hyperscaler.doc.v1
title: Legacy Storage Driver
status: superseded
superseded_by: ["ADR-0709"]
owner: "@team/storage"
---
# Historical doc
"#;

    let res = FrontmatterValidator::validate_doc_frontmatter(
        "docs/adr/ADR-0196.md",
        content,
        Path::new("."),
    );
    assert!(res.is_ok());
}
