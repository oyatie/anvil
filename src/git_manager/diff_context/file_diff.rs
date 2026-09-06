use super::{BothSides, FileChangeKind};

/// One file's portion of a unified diff, split by what the change does.
///
/// Three gates each carried their own copy of this parsing, and all three
/// copies were wrong in the same two ways, because they were the same lines
/// pasted three times.
pub struct FileDiff {
    /// Destination path, or the before-path for a deletion.
    pub path: String,
    pub(super) positions: super::post_image::PostImage,
    pub(super) kind: Option<FileChangeKind>,
    pub(super) previous: Option<String>,
    /// Only the lines this change ADDS, without their `+`.
    pub(super) added: String,
    /// Every line of this file's hunk, additions and context alike, without
    /// the leading marker.
    ///
    /// Separate from `added` because the two answer different questions. "Does
    /// this change introduce a mutating route" is about `added`; "does the file
    /// reference an Idempotency-Key" is about `all`, since a key already
    /// present is context the diff never adds.
    pub(super) all: String,
    /// Lines added minus lines removed for this file.
    ///
    /// Counted while parsing, so a rule asking "did this change grow the file"
    /// needs neither the removed side nor a second parser. `both_sides` is
    /// reserved for rules whose SUBJECT is a removal; size is not one.
    pub(super) net_lines: i64,
    /// This file's hunk lines exactly as the diff spells them, `+` and `-`
    /// markers intact.
    ///
    /// For the rules whose subject IS the removal -- `removed_required_fields`
    /// compares the two sides of a wire contract, and a field disappearing is
    /// the entire finding. Reaching for this is opting out of the added/removed
    /// distinction on purpose, and a rule that takes it should say why.
    pub(super) raw: String,
}

impl FileDiff {
    /// Local textual coordinates, not whole-diff or checkout certification.
    pub fn added_post_image_lines(&self) -> Result<&[super::AddedPostImageLine], &str> {
        if self.kind.is_none() {
            return Err("unknown file change identity");
        }
        self.positions.additions()
    }

    /// None means incomplete, unsupported, or contradictory observation.
    pub fn change_kind(&self) -> Option<FileChangeKind> {
        self.kind
    }

    pub fn previous_path(&self) -> Option<&str> {
        self.previous.as_deref()
    }

    pub(super) fn new(path: String) -> Self {
        Self {
            path,
            positions: super::post_image::PostImage::default(),
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
