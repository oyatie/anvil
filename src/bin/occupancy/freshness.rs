//! Private evidence boundary between the fail-closed workflow and occupancy.
//! These records are authenticated by their workflow producer, not by this
//! parser. No local receipt can establish GitHub provenance on its own.
use anvil::change_delivery::facade::occupancy::HubBaseFreshness;
use std::io::Read;

const MAX_RECORD_BYTES: usize = 32 * 1024;
const MAX_FIELD_BYTES: usize = 4096;

#[derive(Debug)]
pub(super) struct FreshnessProof {
    fields: [String; 12],
    kind: HubBaseFreshness,
}

impl FreshnessProof {
    pub(super) fn parse(record: &str) -> Result<Self, String> {
        if record.len() > MAX_RECORD_BYTES {
            return Err("freshness record exceeds 32768 bytes".to_owned());
        }
        let lines: Vec<&str> = record
            .strip_suffix('\n')
            .unwrap_or(record)
            .split('\n')
            .collect();
        if lines.len() != 12 {
            return Err("freshness record requires exactly twelve fields".to_owned());
        }
        for (index, field) in lines.iter().enumerate() {
            if field.chars().all(char::is_whitespace)
                || field.len() > MAX_FIELD_BYTES
                || field.chars().any(char::is_control)
            {
                return Err(format!(
                    "freshness field {index} is blank, oversized, or contains controls"
                ));
            }
        }
        if lines[0] != "occupancy-freshness-v1" {
            return Err("unsupported freshness record version".to_owned());
        }
        for repo in &lines[1..4] {
            if !repository_identity(repo) {
                return Err("freshness requires owner/repository identities".to_owned());
            }
        }
        if lines[1] != lines[2] {
            return Err("freshness base repository differs from expected repository".to_owned());
        }
        let width = lines[6].len();
        if !matches!(width, 40 | 64)
            || lines[6..].iter().any(|id| {
                id.len() != width
                    || !id
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        {
            return Err("freshness object IDs must be full, lowercase, and one format".to_owned());
        }
        if lines[6] != lines[7] {
            return Err("freshness resolved head differs from event head".to_owned());
        }
        let predecessor = match lines[4] {
            "dev" | "main" => None,
            "staging" => Some("dev"),
            "canary" => Some("staging"),
            "production" => Some("canary"),
            _ => return Err("unsupported freshness destination".to_owned()),
        };
        if let Some(head) = predecessor
            && (lines[3] != lines[2] || lines[5] != head)
        {
            return Err("freshness promotion is not the same-repository predecessor".to_owned());
        }
        let at_tip = lines[8] == lines[9];
        let same_tree = lines[10] == lines[11];
        if at_tip && !same_tree {
            return Err("freshness assigns contradictory trees to the same commit".to_owned());
        }
        let kind = if at_tip {
            HubBaseFreshness::AtDestinationTip
        } else if predecessor.is_some() && same_tree {
            HubBaseFreshness::EquivalentPromotionTree
        } else {
            HubBaseFreshness::Stale
        };
        let fields = lines
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "freshness field cardinality changed".to_owned())?;
        Ok(Self { fields, kind })
    }

    pub(super) fn kind(&self) -> HubBaseFreshness {
        self.kind
    }

    /// Compare complete identities above; truncate only the displayed names.
    pub(super) fn diagnostic(&self) -> String {
        let f = &self.fields;
        format!(
            "freshness {:?}: expected={} source={}:{} destination={}:{} event_head={} resolved_head={} destination_tip={} merge_base={} destination_tree={} merge_base_tree={}",
            self.kind,
            display(&f[1]),
            display(&f[3]),
            display(&f[5]),
            display(&f[2]),
            display(&f[4]),
            f[6],
            f[7],
            f[8],
            f[9],
            f[10],
            f[11]
        )
    }
}

fn repository_identity(raw: &str) -> bool {
    raw.split_once('/').is_some_and(|(owner, repo)| {
        !owner.is_empty()
            && !repo.is_empty()
            && !repo.contains('/')
            && !raw.chars().any(char::is_whitespace)
    })
}

fn display(raw: &str) -> String {
    let mut shown: String = raw.chars().take(128).collect();
    if raw.chars().count() > 128 {
        shown.push_str("…[truncated]");
    }
    shown
}

/// Read at most one bounded record plus the overflow sentinel. Errors never
/// echo raw record contents or arbitrary I/O diagnostics.
pub(super) fn read_record(reader: impl Read) -> Result<FreshnessProof, String> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_RECORD_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "could not read freshness evidence".to_owned())?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err("freshness record exceeds 32768 bytes".to_owned());
    }
    let record =
        String::from_utf8(bytes).map_err(|_| "freshness record is not UTF-8".to_owned())?;
    FreshnessProof::parse(&record)
}

#[cfg(test)]
mod tests;
