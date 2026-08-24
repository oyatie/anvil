//! Removed required fields in a changed wire contract.
//!
//! # What replaced what
//!
//! The predicate was a path containing `api/` or `proto/` and the diff text
//! containing `-   required:` -- a minus sign and exactly three spaces. Every
//! `required:` in this repository's own `openapi/openapi.yaml` sits at eight or
//! fourteen spaces, and no line anywhere in the tree is indented by three, so
//! the check could not fire on the one contract Anvil ships. On a hit it
//! emitted `service_name: "oyatie-backend"` and `impacted_consumer:
//! "oyatie-console"`: two literals, identical on every pull request in every
//! repository, and the only thing in the finding that spoke to blast radius.
//!
//! `buf breaking` compiles both sides into a `FileDescriptorSet` and compares
//! descriptors; `oasdiff` parses both specs into an OpenAPI model. Formatting is
//! invisible to either. What is compared here is the *set of names* a
//! `required:` key carries, read out of the removed and the added lines
//! separately and compared as sets, so re-indenting a block is not a wire
//! break and `required: [repo, pr_number]` losing a member is.
//!
//! # What is not claimed
//!
//! The consumer set. Pact knows consumers because they published contracts to a
//! broker; Confluent Schema Registry checks a subject against its own
//! registered version history; buf compares against a stored image or a BSR
//! module. Every one of them learns the downstream set from a registration, and
//! none infers it. Anvil has no broker, no registry and no module graph, so the
//! finding names the contract and the field and says so, which is the
//! report-the-change-and-delegate behaviour `oasdiff` has and the guess none of
//! them makes.
//!
//! Direction is not distinguished either. Removing a member of a *response*
//! schema's `required:` breaks readers; removing one from a *request* schema
//! relaxes the contract. Telling those apart needs the resolved document, not
//! the hunks, so both are reported under Confluent's BACKWARD reading -- a
//! required field disappeared from a registered schema -- and the fidelity
//! registry records the over-report.

use serde::{Deserialize, Serialize};

/// The sentence attached to every finding, and the reason it is a sentence
/// rather than a service name.
pub const NO_CONSUMER_REGISTRY: &str = "no consumer registry is configured (no Pact broker, schema registry or module graph), \
     so the impacted consumer set is not derived and is not claimed";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossServiceFinding {
    /// The contract file the removal was read out of.
    pub contract_file: String,
    /// The name that a `required:` key carried before this change and does not
    /// carry after it.
    pub removed_required_field: String,
    pub contract_type: String,
    pub breaking_change_reason: String,
}

/// Whether a changed path is a wire contract this gate can *read*.
///
/// YAML only, and deliberately. `required_names` below parses the two YAML
/// spellings and nothing else: JSON Schema writes `"required": ["repo"]` with a
/// quote in front of the key, and proto2 writes `required string name = 1;`
/// with no key at all, so admitting `.json` and `.proto` produced findings of
/// `[]` under a summary saying the file had been read. A scope wider than the
/// parser is the same over-claim this gate was repaired for, one layer down.
/// Teaching the parser those spellings widens this predicate again; until then
/// it admits what it can open.
pub fn is_wire_contract(path: &str) -> bool {
    let structured = path.ends_with(".yaml") || path.ends_with(".yml");
    let lower = path.to_ascii_lowercase();
    structured
        && (lower.contains("openapi")
            || lower.contains("swagger")
            || lower
                .split('/')
                .any(|seg| seg == "api" || seg == "proto" || seg == "contracts"))
}

/// Every name carried by a `required:` key anywhere in `lines`.
///
/// Indentation-insensitive by construction: nothing here reads a column. The
/// two YAML spellings both appear in this repository's own contract --
/// `required: [repo, pr_number]` and a `required:` key followed by `- name`
/// items -- and a block ends at the first line that is not an item.
fn required_names(lines: &[&str]) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_block = false;
    for line in lines {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("required:") {
            let rest = rest.trim();
            in_block = rest.is_empty();
            if let Some(inner) = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                names.extend(
                    inner
                        .split(',')
                        .map(|n| n.trim().trim_matches(['"', '\'']).to_string())
                        .filter(|n| !n.is_empty()),
                );
            }
            continue;
        }
        if in_block {
            match t.strip_prefix("- ") {
                Some(item) => names.push(item.trim().trim_matches(['"', '\'']).to_string()),
                None => in_block = false,
            }
        }
    }
    names
}

/// The `+`/`-` marker stripped from one side of a diff hunk. Context lines
/// belong to both sides.
fn side(diff: &str, marker: char) -> Vec<&str> {
    diff.lines()
        .filter(|l| !l.starts_with("+++") && !l.starts_with("---") && !l.starts_with("@@"))
        .filter_map(|l| match l.chars().next() {
            Some(c) if c == marker => Some(&l[1..]),
            Some('+') | Some('-') => None,
            _ => Some(l),
        })
        .collect()
}

/// Names a changed contract file's `required:` keys lost.
pub fn removed_required_fields(file_path: &str, diff_content: &str) -> Vec<CrossServiceFinding> {
    if !is_wire_contract(file_path) {
        return Vec::new();
    }

    let before = required_names(&side(diff_content, '-'));
    let after = required_names(&side(diff_content, '+'));

    before
        .iter()
        .filter(|n| !after.contains(n))
        .map(|name| CrossServiceFinding {
            contract_file: file_path.to_string(),
            removed_required_field: name.clone(),
            contract_type: "OpenAPI wire schema".to_string(),
            breaking_change_reason: format!(
                "`{name}` was a required field and is not one after this change. Under \
                 Confluent Schema Registry's BACKWARD reading that breaks readers of the \
                 registered schema; {NO_CONSUMER_REGISTRY}."
            ),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_yaml_spellings_yield_the_same_names() {
        assert_eq!(
            required_names(&["  required: [repo, pr_number]"]),
            vec!["repo", "pr_number"]
        );
        assert_eq!(
            required_names(&[
                "        required:",
                "          - repo",
                "          - pr_number"
            ]),
            vec!["repo", "pr_number"]
        );
    }

    #[test]
    fn a_block_ends_at_the_first_line_that_is_not_an_item() {
        assert_eq!(
            required_names(&["required:", "  - repo", "properties:", "  - not_required"]),
            vec!["repo"]
        );
    }

    #[test]
    fn only_contract_files_this_parser_can_read_are_admitted() {
        assert!(is_wire_contract("openapi/openapi.yaml"));
        assert!(is_wire_contract("api/service.yml"));
        assert!(!is_wire_contract("src/api_contract_guard.rs"));
        assert!(!is_wire_contract("docs/api-notes.md"));
        // Admitted before this gate could read either spelling, which made the
        // summary claim a file had been read that the parser skipped.
        assert!(!is_wire_contract("services/api/schema.json"));
        assert!(!is_wire_contract("proto/user.proto"));
    }
}
