//! Per-section evidence resolution. Text markers are interpreted only by
//! `diffs_by_path`; this module neither walks nor reparses a diff.

use super::FileChangeKind::{self, Added, Copied, Deleted, Modified, Renamed};

#[derive(Default)]
pub(super) struct Observation {
    pub header: Option<(String, String)>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub source: Option<String>,
    pub destination: Option<String>,
    pub hint: Option<FileChangeKind>,
    pub old_mode: bool,
    pub new_mode: bool,
    pub invalid: bool,
}

impl Observation {
    pub fn hint(&mut self, kind: FileChangeKind) {
        self.invalid |= self.hint.is_some_and(|old| old != kind);
        self.hint = Some(kind);
    }

    pub fn set(&mut self, field: usize, value: String) {
        // Empty is the explicit absent endpoint, not a filename. Unsupported
        // quoted/escaped identity remains unknown rather than guessed.
        self.invalid |= value.contains(['"', '\\', '\t', '\n']);
        let slot = match field {
            0 => &mut self.before,
            1 => &mut self.after,
            2 => &mut self.source,
            _ => &mut self.destination,
        };
        self.invalid |= slot.as_ref().is_some_and(|old| old != &value);
        *slot = Some(value);
    }

    pub fn resolve(&self, path: &str) -> Option<FileChangeKind> {
        if self.invalid || path.is_empty() {
            return None;
        }
        let before = self.before.as_deref();
        let after = self.after.as_deref();
        let kind = match self.hint {
            Some(kind) => kind,
            None => match (before, after) {
                (Some(""), Some(new)) if !new.is_empty() => Added,
                (Some(old), Some("")) if !old.is_empty() => Deleted,
                (Some(old), Some(new)) if !old.is_empty() && old == new => Modified,
                (None, None) if self.old_mode && self.new_mode => Modified,
                _ => return None,
            },
        };
        if self.old_mode != self.new_mode || (self.old_mode && matches!(kind, Added | Deleted)) {
            return None;
        }
        let endpoints_match = match kind {
            Added => before.is_none_or(str::is_empty) && after.is_none_or(|p| p == path),
            Deleted => before.is_none_or(|p| p == path) && after.is_none_or(str::is_empty),
            Modified => before.is_none_or(|p| p == path) && after.is_none_or(|p| p == path),
            Renamed | Copied => {
                let (Some(old), Some(new)) = (self.source.as_deref(), self.destination.as_deref())
                else {
                    return None;
                };
                !old.is_empty()
                    && old != new
                    && new == path
                    && before.is_none_or(|p| p == old)
                    && after.is_none_or(|p| p == new)
            }
        };
        if !endpoints_match {
            return None;
        }
        if !matches!(kind, Renamed | Copied)
            && (self.source.is_some() || self.destination.is_some())
        {
            return None;
        }
        if let Some((old, new)) = &self.header {
            let expected_old = if matches!(kind, Renamed | Copied) {
                self.source.as_deref()?
            } else {
                path
            };
            if old != expected_old || new != path {
                return None;
            }
        } else if self.before.is_none() || self.after.is_none() {
            // A mode marker alone does not supply a repository path identity.
            return None;
        }
        Some(kind)
    }

    pub fn previous(&self, kind: Option<FileChangeKind>, path: &str) -> Option<String> {
        match kind? {
            FileChangeKind::Added => None,
            FileChangeKind::Renamed | FileChangeKind::Copied => self.source.clone(),
            FileChangeKind::Deleted | FileChangeKind::Modified => Some(path.to_string()),
        }
    }
}
