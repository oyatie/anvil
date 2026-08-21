use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetachedSpanFinding {
    pub file_path: String,
    pub line_number: usize,
    pub snippet: String,
    pub issue: String,
}

/// A call that opens an async boundary: the callee's final path segment is
/// `spawn`, `spawn_blocking` or `spawn_local`, and it is passed something.
///
/// The name has to be the whole identifier, not a prefix of one: widening this
/// to `spawn\w*` picks up `spawn_monitoring_daemon()` and
/// `spawn_continuous_poller(..)`, both live in `src/cli/server.rs` and neither a
/// traced task. The argument list has to be non-empty for the same reason in the
/// other direction: `Command::new("cargo").spawn()?` starts a child process,
/// which will never carry a span.
static BOUNDARY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[^A-Za-z0-9_])(?:spawn_blocking|spawn_local|spawn)\s*\(").unwrap()
});

static INSTRUMENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.instrument\s*\(").unwrap());

pub struct SpanTracker;

impl Default for SpanTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl SpanTracker {
    pub fn new() -> Self {
        Self
    }

    /// Scans source text for detached async tasks, treating every line as one
    /// the caller ships. Callers holding a diff should classify its lines first
    /// and call [`SpanTracker::scan`], so that a line being deleted is neither
    /// counted nor held against the author.
    pub fn scan_detached_tasks(&self, file_path: &str, content: &str) -> Vec<DetachedSpanFinding> {
        let lines: Vec<(usize, &str)> = content
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l))
            .collect();
        self.scan(file_path, &lines).1
    }

    /// Inspects the lines a pull request ships, returning how many async
    /// boundaries were inspected and which of them attach no span.
    ///
    /// Each line arrives with the position it should be reported at. A boundary
    /// owns the region from the call it opens to the parenthesis that closes it,
    /// stopping at the next boundary: a span belongs to the call it was attached
    /// to and not to the next one down, so a lookahead that simply runs on
    /// clears a detached task with its neighbour's span.
    pub fn scan(
        &self,
        file_path: &str,
        lines: &[(usize, &str)],
    ) -> (usize, Vec<DetachedSpanFinding>) {
        // String literals are blanked before line comments are stripped: this
        // repository carries lines with a `://` inside a literal, and stripping
        // first would swallow the rest of every one of them.
        let code: Vec<String> = lines.iter().map(|(_, text)| strip_noise(text)).collect();

        // (index of the line, byte offset just past the call's opening paren)
        let sites: Vec<(usize, usize)> = code
            .iter()
            .enumerate()
            .filter_map(|(idx, line)| {
                BOUNDARY_RE
                    .find(line)
                    .filter(|m| !line[m.end()..].starts_with(')'))
                    .map(|m| (idx, m.end()))
            })
            .collect();

        let mut findings = Vec::new();
        for (nth, &(idx, opened_at)) in sites.iter().enumerate() {
            let stops_before = sites.get(nth + 1).map_or(code.len(), |&(next, _)| next);
            let mut depth: i64 = 1;
            let mut last = idx;
            for (offset, line) in code[idx..stops_before].iter().enumerate() {
                let text = if offset == 0 {
                    &line[opened_at..]
                } else {
                    &line[..]
                };
                depth += text.matches('(').count() as i64 - text.matches(')').count() as i64;
                last = idx + offset;
                if depth <= 0 {
                    break;
                }
            }

            if !code[idx..=last].iter().any(|l| INSTRUMENT_RE.is_match(l)) {
                findings.push(DetachedSpanFinding {
                    file_path: file_path.to_string(),
                    line_number: lines[idx].0,
                    snippet: lines[idx].1.trim().to_string(),
                    issue: "Asynchronous task spawned without attaching tracing span via `.instrument(...)`".to_string(),
                });
            }
        }

        (sites.len(), findings)
    }
}

/// Blanks the contents of double-quoted string literals, then drops any `//`
/// line comment, so that neither can make a line look like a spawn it is not --
/// nor hide one it is.
///
/// Known limits: `/* */` block comments and character literals holding a quote
/// are not modelled.
fn strip_noise(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    let mut in_string = false;

    while let Some(c) = chars.next() {
        if in_string {
            if c == '\\' {
                chars.next();
                out.push(' ');
            } else if c == '"' {
                in_string = false;
                out.push('"');
                continue;
            }
            out.push(' ');
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push('"');
            }
            '/' if chars.peek() == Some(&'/') => break,
            _ => out.push(c),
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_uninstrumented_spawn() {
        let tracker = SpanTracker::new();
        let code = r#"
pub async fn handle_event() {
    tokio::spawn(async move {
        do_work().await;
    });
}
"#;
        let findings = tracker.scan_detached_tasks("src/handler.rs", code);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn test_passes_instrumented_spawn() {
        let tracker = SpanTracker::new();
        let code = r#"
pub async fn handle_event() {
    tokio::spawn(async move {
        do_work().await;
    }.instrument(tracing::info_span!("event_worker")));
}
"#;
        let findings = tracker.scan_detached_tasks("src/handler.rs", code);
        assert_eq!(findings.len(), 0);
    }
}
