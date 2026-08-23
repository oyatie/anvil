//! Wire-compatibility checks, scoped to files that carry a wire schema.
//!
//! # What the real tools do, and which half of it is here
//!
//! `buf breaking` parses both revisions into `FileDescriptorSet`s and compares
//! them against an `--against` baseline (a BSR module, a git ref, or a prebuilt
//! image); Confluent Schema Registry compares a candidate against the registered
//! versions of a subject under BACKWARD / FORWARD / FULL; `oasdiff` compares two
//! resolved OpenAPI documents across 219 breaking checks. All three parse the
//! schema, all three hold a stored baseline, and all three are only ever pointed
//! at schema files.
//!
//! No protobuf compiler, schema registry or resolved-document differ is
//! available here, so the parser and the baseline are out of reach. What is in
//! reach is the scope, and the scope is the half that was manufacturing
//! accusations: this module previously matched any removed line whose lowercase
//! form contained `=` and one of four type words, in any file of any kind, which
//! made `to_string()` in a Rust source file a breaking wire schema change.
//!
//! Everything below is a text heuristic over one pull request's diff. It reads
//! removed and added lines, not a parsed schema, so it sees a hunk rather than a
//! type: a field moved between messages reads as deleted-and-added, and a change
//! outside a hunk is invisible. It is not `buf breaking`, and the fidelity
//! registry records the distance.

/// Which wire schema language a changed file is written in, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaKind {
    /// A `.proto` definition.
    Protobuf,
    /// An OpenAPI / Swagger description.
    OpenApi,
}

/// Marks a finding as field-number reuse, so `SchemaEvolutionReport` can report
/// tag renumbering without re-deciding what the checker already decided.
pub const RENUMBERING_MARKER: &str = "reuses field number";

/// The wire schema language of `path`, or `None` if it carries no wire schema.
///
/// This is the whole fix for the false-positive class: a path that is not a
/// schema is never scanned, so no Rust, Markdown, TOML or CI YAML line can be
/// reported as a wire break. It is a filename match, which means a `.proto`
/// checked in under another extension is missed, and it is deliberately narrow
/// for OpenAPI -- an arbitrary `.yaml` is a workflow or a manifest far more
/// often than an API description, and pulling all of them into scope would
/// recreate the defect with a different predicate.
pub fn classify(path: &str) -> Option<SchemaKind> {
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.ends_with(".proto") {
        return Some(SchemaKind::Protobuf);
    }
    let is_yaml = name.ends_with(".yaml") || name.ends_with(".yml");
    if is_yaml && (name.starts_with("openapi") || name.starts_with("swagger")) {
        return Some(SchemaKind::OpenApi);
    }
    None
}

/// A line of schema content the diff removed or added, with its diff marker
/// stripped. `None` for the diff's own `---`/`+++` header lines, for context
/// lines, and for comments.
fn schema_line(line: &str, marker: char) -> Option<&str> {
    let doubled = if marker == '-' { "---" } else { "+++" };
    if !line.starts_with(marker) || line.starts_with(doubled) {
        return None;
    }
    let content = line[1..].trim();
    if content.is_empty() || content.starts_with("//") || content.starts_with('#') {
        return None;
    }
    Some(content)
}

/// A protobuf field declaration: `optional string customer_id = 3;` parses to
/// `("customer_id", 3)`. Anything whose right-hand side is not a bare number is
/// not a field declaration -- which is what keeps a Rust `let x = y;` out even
/// when one slips into scope.
fn field_declaration(content: &str) -> Option<(&str, u32)> {
    let (lhs, rhs) = content.split_once('=')?;
    let number: u32 = rhs
        .trim()
        .trim_end_matches(';')
        .trim_end_matches(']')
        .split('[')
        .next()?
        .trim()
        .parse()
        .ok()?;
    let name = lhs.split_whitespace().last()?;
    Some((name, number))
}

/// The field numbers and names an added `reserved` statement withdraws:
/// `reserved 2, 15;` and `reserved "customer_id";` are how protobuf says a
/// deletion is deliberate and the number will never be handed out again.
fn reserved_tokens(content: &str) -> Option<(Vec<u32>, Vec<String>)> {
    let rest = content.strip_prefix("reserved ")?;
    let mut numbers = Vec::new();
    let mut names = Vec::new();
    for token in rest.trim_end_matches(';').split(',') {
        let token = token.trim();
        // `reserved 2 to 15;` -- take both ends and everything between.
        if let Some((lo, hi)) = token.split_once(" to ")
            && let (Ok(lo), Ok(hi)) = (lo.trim().parse::<u32>(), hi.trim().parse::<u32>())
        {
            numbers.extend(lo..=hi.min(lo.saturating_add(512)));
            continue;
        }
        if let Ok(n) = token.parse::<u32>() {
            numbers.push(n);
        } else if let Some(name) = token.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
            names.push(name.to_string());
        }
    }
    Some((numbers, names))
}

#[derive(Clone, Debug, Default)]
pub struct CompatibilityChecker;

impl CompatibilityChecker {
    pub fn new() -> Self {
        Self
    }

    /// Checks one file's section of a unified diff, dispatching on the schema
    /// language of `path`. A path carrying no wire schema yields no findings
    /// because it is never read.
    pub fn check_file_diff(&self, path: &str, file_diff: &str) -> Vec<String> {
        match classify(path) {
            Some(SchemaKind::Protobuf) => self.check_proto(path, file_diff),
            Some(SchemaKind::OpenApi) => self.check_openapi(path, file_diff),
            None => Vec::new(),
        }
    }

    /// `buf breaking`'s two WIRE-category rules this text can speak to:
    /// `FIELD_NO_DELETE_UNLESS_NUMBER_RESERVED` -- a deleted field must reserve
    /// its number, or a later field reuses it and old readers decode the new
    /// field into the old one -- and `MESSAGE_SAME_REQUIRED_FIELDS`, which
    /// forbids adding a proto2 `required` field because an old writer emits no
    /// such field and the new reader rejects the message.
    fn check_proto(&self, path: &str, file_diff: &str) -> Vec<String> {
        let mut removed: Vec<(&str, u32)> = Vec::new();
        let mut added: Vec<(&str, u32)> = Vec::new();
        let mut reserved_numbers: Vec<u32> = Vec::new();
        let mut reserved_names: Vec<String> = Vec::new();
        let mut violations = Vec::new();

        for line in file_diff.lines() {
            if let Some(content) = schema_line(line, '-')
                && let Some(field) = field_declaration(content)
            {
                removed.push(field);
            }
            let Some(content) = schema_line(line, '+') else {
                continue;
            };
            if let Some((numbers, names)) = reserved_tokens(content) {
                reserved_numbers.extend(numbers);
                reserved_names.extend(names);
            }
            if let Some(field) = field_declaration(content) {
                added.push(field);
                if content.split_whitespace().next() == Some("required") {
                    violations.push(format!(
                        "{path}: required field `{}` added; an old writer emits no such field and \
                         the new reader rejects the message (buf MESSAGE_SAME_REQUIRED_FIELDS)",
                        field.0
                    ));
                }
            }
        }

        for (name, number) in removed {
            if added.iter().any(|(n, num)| *n == name && *num == number) {
                continue; // moved within the hunk, not withdrawn
            }
            if let Some((reused_by, _)) = added.iter().find(|(_, num)| *num == number) {
                violations.push(format!(
                    "{path}: field `{reused_by}` {RENUMBERING_MARKER} {number}, previously \
                     `{name}`; an old reader decodes the new field into the old one"
                ));
            } else if !reserved_numbers.contains(&number)
                && !reserved_names.iter().any(|n| n == name)
            {
                violations.push(format!(
                    "{path}: field `{name}` (number {number}) deleted without `reserved {number};` \
                     (buf FIELD_NO_DELETE_UNLESS_NUMBER_RESERVED)"
                ));
            }
        }

        violations
    }

    /// `oasdiff`'s `api-path-removed`: a consumer calling a withdrawn endpoint
    /// gets a 404 under the new contract.
    ///
    /// One check, not `oasdiff`'s 219. It reads path keys off the diff rather
    /// than two resolved documents, so a removed *operation* under a surviving
    /// path, a narrowed response type and a newly required request property are
    /// all invisible here.
    fn check_openapi(&self, path: &str, file_diff: &str) -> Vec<String> {
        let endpoint = |content: &str| -> Option<String> {
            let key = content.strip_suffix(':')?;
            key.starts_with('/').then(|| key.to_string())
        };

        let kept: Vec<String> = file_diff
            .lines()
            .filter_map(|l| schema_line(l, '+').and_then(endpoint))
            .collect();

        file_diff
            .lines()
            .filter_map(|l| schema_line(l, '-').and_then(endpoint))
            .filter(|removed| !kept.contains(removed))
            .map(|removed| {
                format!(
                    "{path}: endpoint `{removed}` removed; a consumer that follows the published \
                     contract gets a 404 (oasdiff api-path-removed)"
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_schema_paths_are_in_scope() {
        assert_eq!(classify("proto/order.proto"), Some(SchemaKind::Protobuf));
        assert_eq!(classify("openapi/openapi.yaml"), Some(SchemaKind::OpenApi));
        assert_eq!(classify("api/swagger.yml"), Some(SchemaKind::OpenApi));
        assert_eq!(classify("src/schema_evolution/mod.rs"), None);
        assert_eq!(classify(".github/workflows/ci.yaml"), None);
        assert_eq!(classify("openapi.md"), None);
    }

    #[test]
    fn a_field_declaration_needs_a_numeric_tag() {
        assert_eq!(
            field_declaration("optional string customer_id = 3;"),
            Some(("customer_id", 3))
        );
        assert_eq!(
            field_declaration("string name = 4 [deprecated = true];"),
            Some(("name", 4))
        );
        assert_eq!(
            field_declaration(r#"let f = "migration.sql".to_string();"#),
            None
        );
        // Nothing to the left of the `=`, so there is no field name to report.
        assert_eq!(field_declaration("  = 3;"), None);
    }

    #[test]
    fn a_blank_or_header_line_carries_no_schema_content() {
        assert_eq!(schema_line("-", '-'), None);
        assert_eq!(schema_line("--- a/proto/order.proto", '-'), None);
        assert_eq!(schema_line("+++ b/proto/order.proto", '+'), None);
        assert_eq!(schema_line("   string x = 1;", '-'), None);
        assert_eq!(schema_line("-  string x = 1;", '-'), Some("string x = 1;"));
    }

    #[test]
    fn reserved_covers_numbers_ranges_and_names() {
        assert_eq!(
            reserved_tokens("reserved 2, 15;"),
            Some((vec![2, 15], vec![]))
        );
        assert_eq!(
            reserved_tokens("reserved 9 to 11;"),
            Some((vec![9, 10, 11], vec![]))
        );
        assert_eq!(
            reserved_tokens(r#"reserved "customer_id";"#),
            Some((vec![], vec!["customer_id".to_string()]))
        );
        assert_eq!(reserved_tokens("string x = 1;"), None);

        // `reserved 1 to max;` spans half a billion numbers. Materialising the
        // range would allocate 2GB from one line of someone else's diff, so it
        // is capped -- a deletion above the cap reports as unreserved, which is
        // the safe direction for a gate that blocks.
        let (numbers, _) = reserved_tokens("reserved 1 to 536870911;").expect("a reserved range");
        assert_eq!(numbers.len(), 513);
    }

    #[test]
    fn a_deleted_proto_field_is_a_finding_and_a_reserved_one_is_not() {
        let checker = CompatibilityChecker::new();
        assert_eq!(
            checker
                .check_file_diff("proto/order.proto", "-  string user_id = 1;")
                .len(),
            1
        );
        assert!(
            checker
                .check_file_diff(
                    "proto/order.proto",
                    "-  string user_id = 1;\n+  reserved 1;"
                )
                .is_empty()
        );
    }

    #[test]
    fn an_identical_line_removed_from_rust_is_not_read_at_all() {
        let checker = CompatibilityChecker::new();
        assert!(
            checker
                .check_file_diff("src/lib.rs", "-  string user_id = 1;")
                .is_empty()
        );
    }
}
