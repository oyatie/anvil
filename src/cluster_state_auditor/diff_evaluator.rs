//! Structural comparison of two manifests, over whatever the caller supplies.
//!
//! # What was here
//!
//! `compare_cluster_state` reported drift on exactly one pair of literal
//! substrings, and constructed the finding from four more literals. Supplied
//! anything else — the same field drifting by different amounts, or a different
//! field drifting at all — it returned no findings. That is the sixth fabricated
//! constant of this lane, sitting one file away from the caller: a comparison
//! that recognises only its own operands is a constant, not a measurement (I2).
//!
//! # What is here now
//!
//! A pure structural diff over the two texts it is given. Each line that carries
//! a `key: value` pair is addressed by its indentation plus its key, so repeated
//! keys at different nesting depths do not collide, and every differing,
//! added or removed value is reported. It does not know what a replica is, and
//! that is the point: it discriminates on the operands, so a real cluster
//! readback plugged into it will produce real findings.
//!
//! This is deliberately a comparator, not a YAML semantic differ. It compares
//! text as written, so a reordered mapping or a value spelled differently but
//! meaning the same thing reads as drift. Stated rather than implied — the
//! failure being corrected here is a claim outrunning its implementation.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterDriftFinding {
    pub resource_name: String,
    pub resource_namespace: String,
    pub live_field: String,
    pub git_field: String,
    pub diff_description: String,
}

/// A `key: value` line, addressed by nesting depth so that keys repeated at
/// different levels are distinct entries.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ManifestField {
    /// Indentation width plus the key text: the address of this field.
    address: String,
    /// The key as written, without list-item punctuation.
    key: String,
    value: String,
}

/// Splits a manifest into its addressable scalar fields, in file order.
///
/// Lines with no value (`spec:`) are structure, not data, and are skipped: they
/// carry nothing that can drift on its own.
fn fields(manifest: &str) -> Vec<ManifestField> {
    let mut out = Vec::new();
    for line in manifest.lines() {
        let indent = line.len() - line.trim_start().len();
        let mut rest = line.trim_start();
        // A list item may carry a key on the same line: `- image: console:1`.
        let mut is_item = false;
        if let Some(stripped) = rest.strip_prefix("- ") {
            rest = stripped.trim_start();
            is_item = true;
        }
        if rest.is_empty() || rest.starts_with('#') {
            continue;
        }
        let Some((key, value)) = rest.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        out.push(ManifestField {
            address: format!("{}|{}{}", indent, if is_item { "-" } else { "" }, key),
            key: key.to_string(),
            value: value.to_string(),
        });
    }
    out
}

/// The value of a field by key, wherever it appears. Used only to name the
/// resource a finding belongs to.
fn lookup<'a>(fields: &'a [ManifestField], key: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|f| f.key == key)
        .map(|f| f.value.as_str())
}

const UNNAMED: &str = "(no name declared in manifest)";
const NO_NAMESPACE: &str = "(no namespace declared in manifest)";

pub struct ClusterDiffEvaluator;

impl Default for ClusterDiffEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl ClusterDiffEvaluator {
    pub fn new() -> Self {
        Self
    }

    /// Reports every field on which the live readback and the declared manifest
    /// disagree, including fields present in only one of the two.
    ///
    /// Both arguments come from the caller. This function invents neither, and
    /// returns an empty vector when they agree — a synchronised cluster is not
    /// accused.
    pub fn compare_cluster_state(
        &self,
        live_manifest: &str,
        git_manifest: &str,
    ) -> Vec<ClusterDriftFinding> {
        let live = fields(live_manifest);
        let git = fields(git_manifest);

        let name = lookup(&git, "name")
            .or_else(|| lookup(&live, "name"))
            .unwrap_or(UNNAMED)
            .to_string();
        let namespace = lookup(&git, "namespace")
            .or_else(|| lookup(&live, "namespace"))
            .unwrap_or(NO_NAMESPACE)
            .to_string();

        let mut findings = Vec::new();

        for lf in &live {
            match git.iter().find(|gf| gf.address == lf.address) {
                Some(gf) if gf.value == lf.value => {}
                Some(gf) => findings.push(ClusterDriftFinding {
                    resource_name: name.clone(),
                    resource_namespace: namespace.clone(),
                    live_field: format!("{}: {}", lf.key, lf.value),
                    git_field: format!("{}: {}", gf.key, gf.value),
                    diff_description: format!(
                        "Out-of-band mutation: `{}` is `{}` in the live cluster and `{}` in Git.",
                        lf.key, lf.value, gf.value
                    ),
                }),
                None => findings.push(ClusterDriftFinding {
                    resource_name: name.clone(),
                    resource_namespace: namespace.clone(),
                    live_field: format!("{}: {}", lf.key, lf.value),
                    git_field: String::new(),
                    diff_description: format!(
                        "Out-of-band addition: `{}` is set to `{}` in the live cluster and is not \
                         declared in Git.",
                        lf.key, lf.value
                    ),
                }),
            }
        }

        for gf in &git {
            if !live.iter().any(|lf| lf.address == gf.address) {
                findings.push(ClusterDriftFinding {
                    resource_name: name.clone(),
                    resource_namespace: namespace.clone(),
                    live_field: String::new(),
                    git_field: format!("{}: {}", gf.key, gf.value),
                    diff_description: format!(
                        "Declared but absent: Git sets `{}` to `{}`; the live cluster does not \
                         report it.",
                        gf.key, gf.value
                    ),
                });
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deployment(replicas: &str, image: &str) -> String {
        format!(
            "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: console\nspec:\n  replicas: {replicas}\n  template:\n    spec:\n      containers:\n        - image: {image}\n"
        )
    }

    #[test]
    fn detects_drift_in_values_it_was_not_written_against() {
        let eval = ClusterDiffEvaluator::new();
        for (live, git) in [("10", "3"), ("5", "2"), ("1", "9")] {
            let f = eval.compare_cluster_state(
                &deployment(live, "console:1.4.0"),
                &deployment(git, "console:1.4.0"),
            );
            assert_eq!(f.len(), 1, "live={live} git={git}: {f:?}");
        }
    }

    #[test]
    fn detects_drift_in_a_field_that_is_not_a_replica_count() {
        let eval = ClusterDiffEvaluator::new();
        let f = eval.compare_cluster_state(
            &deployment("3", "console:1.4.1-hotfix"),
            &deployment("3", "console:1.4.0"),
        );
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].diff_description.contains("image"), "{:?}", f[0]);
    }

    #[test]
    fn a_synchronised_cluster_is_not_accused() {
        let eval = ClusterDiffEvaluator::new();
        for m in [
            deployment("3", "console:1.4.0"),
            deployment("5", "console:1.4.1-hotfix"),
        ] {
            assert!(eval.compare_cluster_state(&m, &m).is_empty());
        }
    }

    #[test]
    fn a_field_present_on_only_one_side_is_drift_in_both_directions() {
        let eval = ClusterDiffEvaluator::new();
        let git = "kind: Deployment\nmetadata:\n  name: console\n";
        let live = "kind: Deployment\nmetadata:\n  name: console\n  namespace: prod\n";
        assert_eq!(eval.compare_cluster_state(live, git).len(), 1);
        assert_eq!(eval.compare_cluster_state(git, live).len(), 1);
    }

    #[test]
    fn the_same_key_at_two_depths_does_not_collide() {
        let eval = ClusterDiffEvaluator::new();
        // `spec:` appears twice in a Deployment; only the nested one differs.
        let live = "spec:\n  paused: true\n  template:\n    spec:\n      paused: false\n";
        let git = "spec:\n  paused: true\n  template:\n    spec:\n      paused: true\n";
        let f = eval.compare_cluster_state(live, git);
        assert_eq!(f.len(), 1, "{f:?}");
    }
}
