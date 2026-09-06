//! Local coordinates observed by the sole diff walk, never a whole-diff proof.

pub struct AddedPostImageLine {
    line: usize,
    text: String,
}

impl AddedPostImageLine {
    pub fn line(&self) -> usize {
        self.line
    }
    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Default)]
pub(super) struct PostImage {
    additions: Vec<AddedPostImageLine>,
    error: Option<&'static str>,
    hunk: Option<Hunk>,
    previous_end: Option<(usize, usize)>,
}

struct Hunk {
    old: usize,
    new: usize,
    old_left: usize,
    new_left: usize,
}

impl PostImage {
    pub(super) fn invalidate(&mut self, reason: &'static str) {
        self.error.get_or_insert(reason);
    }

    pub(super) fn finish(&mut self) {
        if let Some(hunk) = self.hunk.take() {
            if hunk.old_left != 0 || hunk.new_left != 0 {
                self.invalidate("incomplete hunk coordinates");
            }
            self.previous_end = Some((hunk.old, hunk.new));
        }
    }

    pub(super) fn additions(&self) -> Result<&[AddedPostImageLine], &str> {
        match self.error {
            Some(reason) => Err(reason),
            None => Ok(&self.additions),
        }
    }

    pub(super) fn observe(&mut self, line: &str) {
        if line.starts_with("@@") {
            self.finish();
            match parse_hunk(line) {
                Some(hunk) => {
                    if self
                        .previous_end
                        .is_some_and(|(old, new)| hunk.old < old || hunk.new < new)
                    {
                        self.invalidate("overlapping or reversed hunks");
                    }
                    self.hunk = Some(hunk);
                }
                None => self.invalidate("unsupported hunk header"),
            }
            return;
        }
        let Some(hunk) = self.hunk.as_mut() else {
            // Only known ordinary section metadata may precede a hunk.
            if ![
                "--- ",
                "+++ ",
                "index ",
                "new file mode ",
                "deleted file mode ",
                "old mode ",
                "new mode ",
                "similarity index ",
                "dissimilarity index ",
                "rename from ",
                "rename to ",
                "copy from ",
                "copy to ",
            ]
            .iter()
            .any(|prefix| line.starts_with(prefix))
            {
                self.invalidate("text without coordinates or unsupported section metadata");
            }
            return;
        };
        if line == "\\ No newline at end of file" {
            return;
        }
        let (old, new) = match line.as_bytes().first() {
            Some(b' ') => (true, true),
            Some(b'-') => (true, false),
            Some(b'+') => (false, true),
            _ => {
                self.invalidate("unmarked hunk content");
                return;
            }
        };
        if (old && hunk.old_left == 0) || (new && hunk.new_left == 0) {
            self.invalidate("excess hunk content");
            return;
        }
        if new && !old {
            self.additions.push(AddedPostImageLine {
                line: hunk.new,
                text: line[1..].to_owned(),
            });
        }
        if old {
            hunk.old += 1;
            hunk.old_left -= 1;
        }
        if new {
            hunk.new += 1;
            hunk.new_left -= 1;
        }
    }
}

fn range(value: &str) -> Option<(usize, usize)> {
    let (start, count) = value.split_once(',').unwrap_or((value, "1"));
    if start.is_empty()
        || count.is_empty()
        || !start.bytes().all(|b| b.is_ascii_digit())
        || !count.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let start: usize = start.parse().ok()?;
    let count: usize = count.parse().ok()?;
    // A zero-width range is anchored after `start`, not at source line zero.
    let first = if count == 0 {
        start.checked_add(1)?
    } else {
        start
    };
    if first == 0 {
        return None;
    }
    first.checked_add(count)?;
    Some((first, count))
}

fn parse_hunk(line: &str) -> Option<Hunk> {
    let rest = line.strip_prefix("@@ -")?;
    let (ranges, suffix) = rest.split_once(" @@")?;
    if !suffix.is_empty() && !suffix.starts_with(' ') {
        return None;
    }
    let (old, new) = ranges.split_once(" +")?;
    let (old, old_left) = range(old)?;
    let (new, new_left) = range(new)?;
    Some(Hunk {
        old,
        new,
        old_left,
        new_left,
    })
}
