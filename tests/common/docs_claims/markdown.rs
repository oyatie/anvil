//! Finite standalone-fence recognition, not Markdown container parsing.
//!
//! Delimiters follow CommonMark 0.31.2 section 4.5: zero to three leading
//! spaces, matching character, sufficient closing length, whitespace-only
//! closing suffix, and no backtick in a backtick opener's info string.
//! List/blockquote structure and container continuation are not interpreted.

use std::path::{Path, PathBuf};

pub struct Claim {
    pub file: PathBuf,
    pub line: usize,
    pub spec: String,
    pub expected: String,
    /// A supported fenced body, excluding the opening/info delimiter line.
    pub fenced: bool,
}

#[derive(Clone, Copy)]
struct Fence {
    delimiter: u8,
    length: usize,
}

fn delimiter_line(raw: &str) -> Option<(Fence, &str)> {
    let spaces = raw.bytes().take_while(|c| *c == b' ').count();
    if spaces > 3 {
        return None;
    }
    let text = &raw[spaces..];
    let delimiter = *text.as_bytes().first()?;
    if !matches!(delimiter, b'`' | b'~') {
        return None;
    }
    let length = text.bytes().take_while(|c| *c == delimiter).count();
    (length >= 3).then_some((Fence { delimiter, length }, &text[length..]))
}

/// Every marker and original line number, including unsupported/unfenced ones.
/// EOF does not implicitly close a supported block. No claim is evaluated here.
pub fn claims_from_text(path: &Path, text: &str) -> Vec<Claim> {
    let mut out = Vec::new();
    let mut open: Option<Fence> = None;
    for (i, raw) in text.lines().enumerate() {
        let candidate = delimiter_line(raw);
        let fenced = match open {
            Some(fence) => {
                let closes = candidate.is_some_and(|(candidate, suffix)| {
                    candidate.delimiter == fence.delimiter
                        && candidate.length >= fence.length
                        && suffix.bytes().all(|c| matches!(c, b' ' | b'\t'))
                });
                if closes {
                    open = None;
                }
                !closes
            }
            None => {
                if let Some((fence, suffix)) = candidate
                    && (fence.delimiter != b'`' || !suffix.contains('`'))
                {
                    open = Some(fence);
                }
                false
            }
        };
        if let Some((spec, expected)) = raw.split_once("#=") {
            out.push(Claim {
                file: path.to_path_buf(),
                line: i + 1,
                // Keep delimiter characters: malformed text must not turn into
                // a valid count specification by stripping its prefix.
                spec: spec.trim().to_string(),
                expected: expected.trim().to_string(),
                fenced,
            });
        }
    }
    out
}

/// Read failures are evidence failures, never a default empty claim list.
pub fn claims_in(path: &Path) -> Result<Vec<Claim>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("{}: cannot be read ({e})", path.display()))?;
    Ok(claims_from_text(path, &text))
}
