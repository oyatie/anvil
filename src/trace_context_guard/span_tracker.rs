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

/// The two ways a call site carries a span across a boundary. Both are methods
/// on `tracing::Instrument`: `.instrument(span)` attaches a named one,
/// `.in_current_span()` carries the caller's, and the second is the canonical
/// way to keep a spawned task inside the span that spawned it.
///
/// The third standard form, `#[tracing::instrument]` on the spawned `async fn`,
/// instruments it at its *definition* and leaves no `.` before the name at the
/// call site. It is not matched here and not matched anywhere else either: a
/// task spawned that way is reported detached. That blind spot is disclosed in
/// `src/fidelity/registry.rs` rather than papered over, because widening this
/// pattern to the bare word would clear every call in the same region.
static INSTRUMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.(?:instrument|in_current_span)\s*\(").unwrap());

/// A position in the stripped source: `(line index, byte offset into that line)`.
/// Ordered lexicographically, which is source order.
type Pos = (usize, usize);

/// What one pass over a file's shipped lines found.
#[derive(Debug, Default)]
pub struct ScanOutcome {
    /// Boundaries whose region opened *and* closed within the lines available,
    /// so a verdict about them rests on an extent that was actually established.
    pub classified: usize,
    /// Boundaries whose region never closed in the lines available. Neither
    /// verdict is available over one of these, so they are counted here and
    /// reported nowhere else -- see [`SpanTracker::scan`].
    pub unresolved: usize,
    /// Classified boundaries that carry no span.
    pub detached: Vec<DetachedSpanFinding>,
}

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

    /// Inspects the lines a pull request ships.
    ///
    /// Each line arrives with the position it should be reported at. A boundary
    /// owns the region between the parenthesis its call opens and the one that
    /// closes it, minus the regions owned by the boundaries nested inside it: a
    /// span attached to a child task belongs to that child, and a span attached
    /// at a parent's own close belongs to the parent even when a child sits
    /// between the two.
    ///
    /// A boundary whose parenthesis never closes in the lines available is
    /// **not classified**. The gate reads diff hunks, not files, so a hunk can
    /// carry a spawn and stop three lines later; both verdicts it could publish
    /// there would be claims about a region whose extent was never established,
    /// and the one that clears the boundary would do so with an
    /// `.instrument(...)` written somewhere else entirely. Those boundaries are
    /// counted in [`ScanOutcome::unresolved`] and appear in neither
    /// `classified` nor `detached`.
    pub fn scan(&self, file_path: &str, lines: &[(usize, &str)]) -> ScanOutcome {
        // String literals are blanked before line comments are stripped: this
        // repository carries lines with a `://` inside a literal, and stripping
        // first would swallow the rest of every one of them.
        let code: Vec<String> = lines.iter().map(|(_, text)| strip_noise(text)).collect();

        // Every boundary, with the position just past the parenthesis it opens
        // and the position of the one that closes it -- `None` when the lines
        // available never balance it.
        let regions: Vec<(Pos, Option<Pos>)> = code
            .iter()
            .enumerate()
            .filter_map(|(idx, line)| {
                BOUNDARY_RE
                    .find(line)
                    .filter(|m| !line[m.end()..].starts_with(')'))
                    .map(|m| ((idx, m.end()), close_of(&code, (idx, m.end()))))
            })
            .collect();

        let mut outcome = ScanOutcome::default();
        for (nth, &(start, close)) in regions.iter().enumerate() {
            let Some(close) = close else {
                outcome.unresolved += 1;
                continue;
            };
            outcome.classified += 1;
            if INSTRUMENT_RE.is_match(&region_text(&code, &regions, nth, start, close)) {
                continue;
            }
            let idx = start.0;
            outcome.detached.push(DetachedSpanFinding {
                file_path: file_path.to_string(),
                line_number: lines[idx].0,
                snippet: lines[idx].1.trim().to_string(),
                issue: issue_for(&code[idx]),
            });
        }

        outcome
    }
}

/// The position of the parenthesis that closes the one opened just before
/// `from`, or `None` when the lines available never balance it.
fn close_of(code: &[String], from: Pos) -> Option<Pos> {
    let mut depth: i64 = 1;
    for (offset, line) in code[from.0..].iter().enumerate() {
        let at = if offset == 0 { from.1 } else { 0 };
        for (col, ch) in line[at..].char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((from.0 + offset, at + col));
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// The text a boundary's region spans, with the regions of the boundaries
/// nested inside it cut out.
///
/// The cut is what attributes a span to one boundary rather than another. Left
/// in, a child task's `.instrument(...)` clears the parent that merely contains
/// it; cut out, the parent is judged on what is written at its own depth --
/// including at its closing line, which is where the combinator is applied when
/// a child sits in between.
fn region_text(
    code: &[String],
    regions: &[(Pos, Option<Pos>)],
    nth: usize,
    start: Pos,
    close: Pos,
) -> String {
    let mut out = String::new();
    let mut cur = start;

    for &(child_start, child_close) in &regions[nth + 1..] {
        if child_start > close {
            break;
        }
        // A grandchild: already inside a region this loop has skipped past.
        if child_start < cur {
            continue;
        }
        let Some(child_close) = child_close else {
            continue;
        };
        push_span(code, cur, child_start, &mut out);
        // `)` is one ASCII byte, so the position just past it is a char boundary.
        cur = (child_close.0, child_close.1 + 1);
    }
    push_span(code, cur, close, &mut out);

    out
}

/// Appends `code[from..to]` to `out`, newline-separated when it spans lines.
fn push_span(code: &[String], from: Pos, to: Pos, out: &mut String) {
    if from >= to {
        return;
    }
    if from.0 == to.0 {
        out.push_str(&code[from.0][from.1..to.1]);
        return;
    }
    out.push_str(&code[from.0][from.1..]);
    for line in &code[from.0 + 1..to.0] {
        out.push('\n');
        out.push_str(line);
    }
    out.push('\n');
    out.push_str(&code[to.0][..to.1]);
}

/// What to tell the author, which depends on what they spawned.
///
/// `Instrument` is blanket-implemented for every `T: Sized`, but `Instrumented<T>`
/// implements `Future` only when `T` does -- never when `T` is the `FnOnce()` a
/// thread is handed. So `std::thread::spawn(closure.instrument(span))` does not
/// compile, and a gate that blocks a merge on advice the compiler rejects is
/// worse than one that says nothing. A thread carries the caller's span by
/// entering it inside the closure instead.
fn issue_for(line: &str) -> String {
    if line.contains("thread::spawn") {
        "Thread spawned without carrying the caller's tracing span: capture \
         `tracing::Span::current()` outside the closure and enter it inside, \
         with `let _enter = span.enter();`"
            .to_string()
    } else {
        "Asynchronous task spawned without attaching tracing span via \
         `.instrument(...)` or `.in_current_span()`"
            .to_string()
    }
}

/// Blanks the contents of double-quoted string literals, then drops any `//`
/// line comment, so that neither can make a line look like a spawn it is not --
/// nor hide one it is.
///
/// Known limits: `/* */` block comments, character literals holding a quote, and
/// raw strings (`r"..."`, `r#"..."#`) are not modelled. A raw string is read one
/// line at a time like any other, so its second and later lines are lexed as
/// live code: a multi-line fixture written as `r#"..."#` with a spawn in it is
/// read as a spawn. This repository's convention is diff fixtures as ordinary
/// `"..."` literals, which are blanked correctly, and no raw string in `src/` or
/// `tests/` carries a spawn call today. The limit is disclosed in
/// `src/fidelity/registry.rs` rather than fixed here, because modelling raw
/// strings means growing this into a real lexer.
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

    /// The shape `scan` is called with: every line paired with the position it
    /// should be reported at. Written out here rather than behind a helper on
    /// `SpanTracker`, because a `pub fn` that treats every line it is handed as
    /// one the pull request ships is the removed-line defect back on the API
    /// surface for anyone to pick up.
    fn shipped<'a>(code: &[&'a str]) -> Vec<(usize, &'a str)> {
        code.iter().enumerate().map(|(i, l)| (i + 1, *l)).collect()
    }

    #[test]
    fn an_uninstrumented_spawn_is_reported() {
        let lines = shipped(&[
            "pub async fn handle_event() {",
            "    tokio::spawn(async move {",
            "        do_work().await;",
            "    });",
            "}",
        ]);
        let outcome = SpanTracker::new().scan("src/handler.rs", &lines);
        assert_eq!(outcome.classified, 1);
        assert_eq!(outcome.unresolved, 0);
        assert_eq!(outcome.detached.len(), 1);
    }

    #[test]
    fn a_spawn_carrying_its_span_is_not() {
        let lines = shipped(&[
            "pub async fn handle_event() {",
            "    tokio::spawn(async move {",
            "        do_work().await;",
            "    }.instrument(tracing::info_span!(\"event_worker\")));",
            "}",
        ]);
        let outcome = SpanTracker::new().scan("src/handler.rs", &lines);
        assert_eq!(outcome.classified, 1);
        assert!(outcome.detached.is_empty());
    }

    #[test]
    fn a_span_attached_earlier_on_the_opening_line_is_not_this_boundary_s() {
        // The region begins just past the parenthesis the call opens, so text
        // written to the left of it belongs to whatever call that was. Reading
        // the whole line clears a genuinely detached spawn with an
        // `.instrument(...)` attached to an unrelated value beside it.
        let lines = shipped(&[
            "let g = other().instrument(sp()); tokio::spawn(async move { detached().await; });",
        ]);
        let outcome = SpanTracker::new().scan("src/handler.rs", &lines);
        assert_eq!(outcome.classified, 1);
        assert_eq!(outcome.detached.len(), 1);
    }

    #[test]
    fn a_child_task_s_span_does_not_clear_the_parent_that_contains_it() {
        // The parent opens a region the child's whole call sits inside, so a
        // parent judged on its region as written is cleared by a span that was
        // never attached to it. Cutting the child's region out is what makes
        // the two verdicts independent.
        let lines = shipped(&[
            "tokio::spawn(async move {",
            "    set.spawn(child().instrument(tracing::info_span!(\"child\")));",
            "    set.join_all().await;",
            "});",
        ]);
        let outcome = SpanTracker::new().scan("src/supervisor.rs", &lines);
        assert_eq!(outcome.classified, 2);
        assert_eq!(outcome.detached.len(), 1);
        assert!(outcome.detached[0].snippet.starts_with("tokio::spawn("));
    }
}
