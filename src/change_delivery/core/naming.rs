//! Names derived only from the plan and the spec (I4): nothing a pull
//! request title, body, comment, webhook field or CI log contains ever
//! reaches a branch name, a label or a marker.

use super::model::{Move, ShardKey};
use sha2::{Digest, Sha256};

pub const LABEL_SHAPE_MOVE: &str = "anvil/shape-move";
pub const LABEL_STRUCTURE_ONLY: &str = "anvil/structure-only";

/// Stable identity of a shard: repo, rule, unit, the sorted (from, to)
/// pairs, and the spec version. Sixteen hex chars of SHA-256.
pub fn shard_key(
    repo: &str,
    rule_id: &str,
    unit: &str,
    moves: &[Move],
    spec_version: &str,
) -> ShardKey {
    let mut pairs: Vec<(&str, &str)> = moves
        .iter()
        .map(|m| (m.from.as_str(), m.to.as_str()))
        .collect();
    pairs.sort();
    let mut h = Sha256::new();
    h.update(repo.as_bytes());
    h.update([0]);
    h.update(rule_id.as_bytes());
    h.update([0]);
    h.update(unit.as_bytes());
    h.update([0]);
    for (from, to) in pairs {
        h.update(from.as_bytes());
        h.update([1]);
        h.update(to.as_bytes());
        h.update([0]);
    }
    h.update(spec_version.as_bytes());
    let hex = format!("{:x}", h.finalize());
    ShardKey(hex[..16].to_string())
}

fn slug(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

/// `anvil/shape/<rule>/<unit-slug>/<key8>/g<generation>`
pub fn branch_name(rule_id: &str, unit: &str, key: &ShardKey, generation: u32) -> String {
    format!(
        "anvil/shape/{}/{}/{}/g{generation}",
        slug(rule_id),
        slug(unit),
        &key.0[..8.min(key.0.len())]
    )
}

/// The first line of every shape PR body; the webhook pipeline recognises it
/// to review-but-never-fix, and the sweep recognises its own PRs by it.
pub fn pr_marker(
    repo: &str,
    rule_id: &str,
    unit: &str,
    key: &ShardKey,
    spec_version: &str,
    generation: u32,
) -> String {
    format!(
        "<!-- anvil:shape-move v1 repo={repo} rule={rule_id} unit={unit} shard={key} spec={spec_version} gen={generation} -->"
    )
}

/// Parses a marker back; `None` for anything that is not exactly ours.
pub fn parse_marker(line: &str) -> Option<ShardKey> {
    let inner = line
        .trim()
        .strip_prefix("<!-- anvil:shape-move v1 ")?
        .strip_suffix(" -->")?;
    inner
        .split(' ')
        .find_map(|kv| kv.strip_prefix("shard=").map(|k| ShardKey(k.to_string())))
}
