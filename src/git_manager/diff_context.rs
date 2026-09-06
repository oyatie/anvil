use super::subject::SubjectRoot;
mod change_kind;
use change_kind::Observation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangeKind {
    Added,
    Deleted,
    Modified,
    Renamed,
    Copied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrDiffContext {
    pub repo: String,
    pub pr_number: u64,
    pub base_branch: String,
    pub base_sha: String,
    pub head_sha: String,
    pub is_incremental: bool,
    pub previous_head_sha: Option<String>,
    pub diff_content: String,
    pub changed_files: Vec<String>,
    pub repo_working_dir: SubjectRoot,
}

/// One file's portion of a unified diff, split by what the change does.
///
/// Three gates each carried their own copy of this parsing, and all three
/// copies were wrong in the same two ways, because they were the same lines
/// pasted three times.
/// Why a rule needs the side of the diff a change REMOVES.
///
/// A closed set, so asking for both sides is a named, reviewable act rather
/// than a field access. Adding a variant is a diff someone can challenge --
/// which is the whole mechanism: seven times a scanner read the removed side
/// by accident, and once (#115) a fresh scanner arrived with the same defect
/// after the first two were fixed. Reading the whole diff is what you get by
/// NOT thinking about it, so the type stops being silent about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BothSides {
    /// A wire contract is compared before and after: a `required` field that
    /// disappears IS the finding, so the rule cannot work from additions.
    ContractComparesRemovedFields,
}

pub struct FileDiff {
    /// Destination path, or the before-path for a deletion.
    pub path: String,
    kind: Option<FileChangeKind>,
    previous: Option<String>,
    /// Only the lines this change ADDS, without their `+`.
    added: String,
    /// Every line of this file's hunk, additions and context alike, without
    /// the leading marker.
    ///
    /// Separate from `added` because the two answer different questions. "Does
    /// this change introduce a mutating route" is about `added`; "does the file
    /// reference an Idempotency-Key" is about `all`, since a key already
    /// present is context the diff never adds.
    all: String,
    /// Lines added minus lines removed for this file.
    ///
    /// Counted while parsing, so a rule asking "did this change grow the file"
    /// needs neither the removed side nor a second parser. `both_sides` is
    /// reserved for rules whose SUBJECT is a removal; size is not one.
    net_lines: i64,
    /// This file's hunk lines exactly as the diff spells them, `+` and `-`
    /// markers intact.
    ///
    /// For the rules whose subject IS the removal -- `removed_required_fields`
    /// compares the two sides of a wire contract, and a field disappearing is
    /// the entire finding. Reaching for this is opting out of the added/removed
    /// distinction on purpose, and a rule that takes it should say why.
    raw: String,
}

impl FileDiff {
    /// None means incomplete, unsupported, or contradictory observation.
    pub fn change_kind(&self) -> Option<FileChangeKind> {
        self.kind
    }

    pub fn previous_path(&self) -> Option<&str> {
        self.previous.as_deref()
    }

    fn new(path: String) -> Self {
        Self {
            path,
            kind: None,
            previous: None,
            added: String::new(),
            all: String::new(),
            net_lines: 0,
            raw: String::new(),
        }
    }
    /// Lines added minus lines removed. Negative means the file shrank.
    pub fn net_lines(&self) -> i64 {
        self.net_lines
    }

    /// The lines this change ADDS, without their `+`.
    ///
    /// What an ordinary rule wants. It contains no removed line, so a rule
    /// working from it cannot refuse the change that deletes what it is
    /// looking for.
    pub fn added(&self) -> &str {
        &self.added
    }

    /// The file as it stands AFTER this change: additions plus the context they
    /// sit in, removals excluded.
    ///
    /// For a rule asking what the file says now -- "a Namespace declared
    /// without the enforce label", "an image not pinned to a digest" -- where a
    /// line the change does not touch still counts.
    pub fn after_change(&self) -> &str {
        &self.all
    }

    /// Both sides, markers intact.
    ///
    /// Requires naming a reason from [`BothSides`], because this is the only
    /// corpus containing removed lines and reading it by accident is the
    /// inversion defect. The parameter is deliberately unused at runtime: its
    /// job is to make the caller state, in code a reviewer reads, that the
    /// removal is the subject rather than something swept in.
    pub fn both_sides(&self, _why: BothSides) -> &str {
        &self.raw
    }
}

/// The sole diff walk: content projections and change identity share section
/// boundaries. Unsupported sections reset state; missing evidence is not Modified.
pub fn diffs_by_path(diff: &str) -> Vec<FileDiff> {
    let mut out = Vec::new();
    let mut current: Option<FileDiff> = None;
    let mut observed = Observation::default();
    let mut in_hunk = false;
    for line in diff.lines() {
        if line.starts_with("diff ") {
            finish_section(&mut out, current.take(), &observed);
            observed = Observation::default();
            in_hunk = false;
            if let Some((old, new)) = line
                .strip_prefix("diff --git a/")
                .and_then(|rest| rest.split_once(" b/"))
            {
                observed.invalid = old.is_empty()
                    || new.is_empty()
                    || old.contains(['"', '\\', '\t'])
                    || new.contains(['"', '\\', '\t'])
                    || new.contains(" b/");
                observed.header = Some((old.to_string(), new.to_string()));
                current = Some(FileDiff::new(new.to_string()));
            } else {
                observed.invalid = true;
            }
            continue;
        }
        if !in_hunk {
            if let Some(endpoint) = line.strip_prefix("+++ ") {
                // Preserve repeated plus-header-only attribution without
                // carrying the preceding section's metadata forward.
                if observed.after.is_some() {
                    let invalid = observed.header.is_some();
                    observed.invalid |= invalid;
                    finish_section(&mut out, current.take(), &observed);
                    observed = Observation {
                        invalid,
                        ..Observation::default()
                    };
                }
                let value = if endpoint == "/dev/null" {
                    Some("")
                } else {
                    endpoint.strip_prefix("b/")
                };
                if let Some(value) = value {
                    observed.set(1, value.to_string());
                    let path = if value.is_empty() {
                        observed.before.clone()
                    } else {
                        Some(value.to_string())
                    };
                    if let Some(path) = path.filter(|p| !p.is_empty()) {
                        let file = current.get_or_insert_with(|| FileDiff::new(path.clone()));
                        file.path = path;
                    }
                } else {
                    observed.invalid = true;
                }
                continue;
            }
            if let Some(endpoint) = line.strip_prefix("--- ") {
                let value = if endpoint == "/dev/null" {
                    Some("")
                } else {
                    endpoint.strip_prefix("a/")
                };
                if let Some(value) = value {
                    observed.set(0, value.to_string());
                    if current.is_none() && !value.is_empty() {
                        current = Some(FileDiff::new(value.to_string()));
                    }
                } else {
                    observed.invalid = true;
                }
                continue;
            }
            let marker = if line.starts_with("new file mode ") {
                Some(FileChangeKind::Added)
            } else if line.starts_with("deleted file mode ") {
                Some(FileChangeKind::Deleted)
            } else {
                None
            };
            if let Some(kind) = marker {
                observed.hint(kind);
                continue;
            }
            if line.starts_with("old mode ") {
                observed.old_mode = true;
                continue;
            }
            if line.starts_with("new mode ") {
                observed.new_mode = true;
                continue;
            }
            let movement = [
                ("rename from ", FileChangeKind::Renamed, 2),
                ("rename to ", FileChangeKind::Renamed, 3),
                ("copy from ", FileChangeKind::Copied, 2),
                ("copy to ", FileChangeKind::Copied, 3),
            ]
            .into_iter()
            .find_map(|(prefix, kind, field)| line.strip_prefix(prefix).map(|p| (kind, field, p)));
            if let Some((kind, field, path)) = movement {
                observed.hint(kind);
                observed.set(field, path.to_string());
                continue;
            }
        }
        if line.starts_with("@@") {
            in_hunk = true;
            continue;
        }
        let Some(file) = current.as_mut() else {
            continue;
        };
        // Header metadata is not hunk content, including for empty/binary files.
        if !line.starts_with(['+', '-', ' ', '\\']) {
            continue;
        }
        file.raw.push_str(line);
        file.raw.push('\n');
        if let Some(body) = line.strip_prefix('+') {
            file.net_lines += 1;
            file.added.push_str(body);
            file.added.push('\n');
            file.all.push_str(body);
            file.all.push('\n');
        } else if let Some(body) = line.strip_prefix(' ') {
            file.all.push_str(body);
            file.all.push('\n');
        } else if line.starts_with('-') {
            file.net_lines -= 1;
        }
    }
    finish_section(&mut out, current, &observed);
    out
}

fn finish_section(out: &mut Vec<FileDiff>, file: Option<FileDiff>, observed: &Observation) {
    let Some(mut file) = file else { return };
    file.kind = observed.resolve(&file.path);
    file.previous = observed.previous(file.kind, &file.path);
    if let Some(existing) = out.iter_mut().find(|existing| existing.path == file.path) {
        // Unknown/contradictory repeated sections stay unknown; later evidence
        // may not overwrite the ambiguity with a falsely definite verdict.
        if existing.kind != file.kind || existing.previous != file.previous {
            existing.kind = None;
            existing.previous = None;
        }
        existing.added.push_str(&file.added);
        existing.all.push_str(&file.all);
        existing.raw.push_str(&file.raw);
        existing.net_lines += file.net_lines;
    } else {
        out.push(file);
    }
}
