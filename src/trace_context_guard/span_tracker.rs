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
/// which will never carry a span. And a match preceded by `fn` is a
/// *declaration*, not a call -- see [`is_declaration`].
///
/// It remains a nominal match: nothing here resolves a type, so any non-empty
/// call whose final path segment is one of those three names is treated as a
/// task boundary, and there is no allowlist. Disclosed in
/// `src/fidelity/registry.rs`.
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

/// What one pass over one hunk's shipped lines found.
#[derive(Debug, Default)]
pub struct ScanOutcome {
    /// Boundaries whose region opened *and* closed within the hunk, so a
    /// verdict about them rests on an extent that was actually established.
    pub classified: usize,
    /// Boundaries whose region never closed inside the hunk that opened it.
    /// Neither verdict is available over one of these, so they are counted here
    /// rather than judged -- see [`SpanTracker::scan`]. A count above zero is
    /// published: `src/trace_context_guard/mod.rs` reports it as `NotMeasured`,
    /// which names the number and blocks merge-queue admission, because a
    /// boundary that was seen and not judged is not an empty measurement.
    pub unresolved: usize,
    /// Classified boundaries that carry no span.
    pub detached: Vec<DetachedSpanFinding>,
}

/// A boundary's region while it is still open: where the call was written,
/// what to tell the author if it turns out to carry no span, and the text
/// written at *its* depth so far -- the text a nested boundary owns goes to
/// that boundary's own frame instead.
struct Frame {
    line: usize,
    issue: String,
    text: String,
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

    /// Inspects the lines of **one diff hunk**.
    ///
    /// One forward pass, carrying a stack with an entry per open parenthesis:
    /// `Some(frame)` when a boundary call opened it, `None` otherwise. Every
    /// other character is appended to the innermost `Some`, which is exactly
    /// "this boundary's region minus the regions its children own" -- a span
    /// attached to a child task belongs to the child, and a span attached at a
    /// parent's own close belongs to the parent even when a child sits between
    /// the two. A frame is judged when its parenthesis closes.
    ///
    /// The caller passes one hunk at a time, and the stack starts and ends with
    /// it. That is what bounds a region: a boundary whose parenthesis never
    /// closes inside its own hunk is **not classified**. The gate reads diff
    /// hunks, not files, and the hunks of a file are disjoint windows onto it --
    /// a walk that ran past the end of one and into the next would close a
    /// region with a parenthesis from code hundreds of lines away, and clear it
    /// with an `.instrument(...)` belonging to an unrelated call. Both verdicts
    /// available there are claims about an extent that was never established,
    /// so those boundaries are counted in [`ScanOutcome::unresolved`] and appear
    /// in neither `classified` nor `detached`.
    pub fn scan(&self, file_path: &str, lines: &[(usize, &str)]) -> ScanOutcome {
        let mut outcome = ScanOutcome::default();
        // The lexer's state runs across lines, so a literal or a block comment
        // that spans them is noise for its whole length. It starts at `Code`,
        // which is a guess: a hunk is a window onto a file and may open inside
        // either. Disclosed in `src/fidelity/registry.rs`.
        let mut lex = Lex::Code;
        let mut stack: Vec<Option<Frame>> = Vec::new();
        // The indices in `stack` of the frames that hold a region, innermost
        // last. Every character of the hunk is appended to the innermost open
        // region, and finding that frame by walking the stack costs O(depth)
        // per character -- quadratic in unmatched-open depth over text that
        // arrives from a webhook, and this gate bounds its input at no size.
        // Carried alongside, the lookup is O(1) -- see [`push`].
        let mut opens: Vec<usize> = Vec::new();

        for (idx, (_, raw)) in lines.iter().enumerate() {
            let code = strip_noise(raw, &mut lex);
            // Where each boundary call on this line begins, and where it opens
            // its parenthesis. Every match, not merely the first: a second
            // spawn on the same line is a second boundary, and skipping it
            // clears a task that ships detached.
            let boundaries: Vec<(usize, usize)> = BOUNDARY_RE
                .find_iter(&code)
                .map(|m| (m.start(), m.end() - 1))
                .filter(|(start, _)| !is_declaration(&code[..*start]))
                .collect();

            for (col, ch) in code.char_indices() {
                match ch {
                    '(' => {
                        push(&mut stack, &opens, ch);
                        let opened = boundaries
                            .iter()
                            .find(|(_, paren)| *paren == col)
                            .filter(|_| !code[col + 1..].starts_with(')'));
                        stack.push(opened.map(|(start, paren)| Frame {
                            line: idx,
                            // Which kind of boundary this is comes from *this*
                            // match's own callee text, not from the whole line
                            // prefix: with the prefix, the second spawn on a
                            // line reads its kind off the first one's callee
                            // and its author is told the wrong fix.
                            issue: issue_for(callee_of(&code, *start, *paren), &code[col + 1..]),
                            text: String::new(),
                        }));
                        if opened.is_some() {
                            opens.push(stack.len() - 1);
                        }
                    }
                    ')' => {
                        // A `)` with nothing open belongs to code above this
                        // hunk. It closes no region here.
                        if let Some(slot) = stack.pop() {
                            if opens.last() == Some(&stack.len()) {
                                opens.pop();
                            }
                            if let Some(frame) = slot {
                                outcome.classified += 1;
                                if !INSTRUMENT_RE.is_match(&frame.text) {
                                    let (line_number, text) = lines[frame.line];
                                    outcome.detached.push(DetachedSpanFinding {
                                        file_path: file_path.to_string(),
                                        line_number,
                                        snippet: text.trim().to_string(),
                                        issue: frame.issue,
                                    });
                                }
                            }
                        }
                        push(&mut stack, &opens, ch);
                    }
                    _ => push(&mut stack, &opens, ch),
                }
            }
            push(&mut stack, &opens, '\n');
        }

        outcome.unresolved = opens.len();
        // Findings close in the order their regions end, which puts an outer
        // task after the inner one it contains. A reader reads down the file.
        outcome.detached.sort_by_key(|f| f.line_number);
        outcome
    }
}

/// Appends one character to the region of the innermost boundary still open.
///
/// `opens` names that frame directly, so this is O(1) and the region walk is
/// linear in the text it reads. Searching the stack for it instead --
/// `.iter_mut().rev().flatten().next()` -- costs O(parenthesis depth) on every
/// character, the synthetic newline at each line end included, which is
/// quadratic on a deeply nested line. The input is `git diff` output for a pull
/// request on a watched repository, a file need not compile to be diffed and
/// scanned, and unlike `src/cedar_guard.rs` and `src/doc_guard/mod.rs` this gate
/// caps its input at no size -- so that cost is reachable from outside.
fn push(stack: &mut [Option<Frame>], opens: &[usize], ch: char) {
    if let Some(&innermost) = opens.last()
        && let Some(frame) = stack[innermost].as_mut()
    {
        frame.text.push(ch);
    }
}

/// Whether the text preceding a match is the `fn` of a declaration.
///
/// A definition named `spawn` is not a call and opens no task, but the matcher
/// is nominal, so `fn spawn(task: BoxFuture) {` and a `fn spawn(&self, f: F);`
/// trait row both read as calls with a non-empty argument list. That is not a
/// corner case: the natural remedy for a fleet of detached boundaries is a
/// wrapper that attaches the span, the obvious name for it is `spawn`, and its
/// declaration line is the one place where a span actually is attached.
fn is_declaration(before: &str) -> bool {
    let Some(rest) = before.trim_end().strip_suffix("fn") else {
        return false;
    };
    // `fn` has to be a whole keyword: `myfn spawn(..)` is a call.
    rest.chars()
        .next_back()
        .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
}

/// The callee expression one match belongs to: back to the nearest statement or
/// argument separator, forward to the parenthesis the call opens.
///
/// Read per match rather than off the whole line prefix. With the prefix, the
/// second and later boundaries on a line include the earlier calls, so
/// `let a = std::thread::spawn(..); let b = tokio::spawn(..)` reads the tokio
/// task's kind off the thread's callee. Both findings then carry one string,
/// `remedies()` dedupes them to it, and the sentence tells the author of an
/// async task to hold a span guard across an await -- the anti-pattern -- while
/// the advice that fits it is never published at all.
fn callee_of(code: &str, start: usize, paren: usize) -> &str {
    let cut = code[..start]
        .rfind([';', '{', '}', ',', '='])
        .map_or(0, |i| i + 1);
    &code[cut..paren]
}

/// What to tell the author, which depends on the shape of what they spawned.
///
/// `Instrument` is blanket-implemented for every `T: Sized`, but `Instrumented<T>`
/// implements `Future` only when `T` does -- never when `T` is the `FnOnce()` a
/// thread or a blocking pool is handed. So the combinator does not compile on
/// one of those, and a gate that blocks a merge on advice the compiler rejects
/// is worse than one that says nothing. Such a boundary carries the caller's
/// span by entering it inside the closure instead.
///
/// The decision is made on the callee's shape rather than on the one substring
/// `thread::spawn`: `spawn_blocking` takes `FnOnce() -> R`,
/// `std::thread::Builder::new().spawn(..)` never contains `thread::spawn`, and
/// gate 4 (`src/rust_language_policy/engine.rs`) actively instructs authors to
/// move blocking calls onto `tokio::task::spawn_blocking` -- so under a
/// substring rule gate 4 prescribes the form gate 17 then blocks with a fix that
/// will not build. `argument` is the text the call opens with, which catches a
/// closure-taking boundary this list does not name, `rayon::spawn(move || ..)`
/// among them.
///
/// Every string this returns is folded into the published summary by
/// `src/trace_context_guard/mod.rs`, because `summary` is the only field of the
/// report that reaches a reader.
fn issue_for(callee: &str, argument: &str) -> String {
    let head = argument.trim_start();
    let closure_taking = callee.contains("spawn_blocking")
        || callee.contains("thread::")
        || callee.contains("Builder")
        || head.starts_with('|')
        || head.starts_with("move |");
    if closure_taking {
        "Work handed to a closure-taking boundary without carrying the caller's \
         tracing span: the `Instrument` combinator produces a future and a \
         closure is not one, so capture `tracing::Span::current()` outside the \
         closure and enter it inside, with `let _enter = span.enter();`"
            .to_string()
    } else {
        "Asynchronous task spawned without attaching tracing span via \
         `.instrument(...)` or `.in_current_span()`"
            .to_string()
    }
}

/// What the lexer is in the middle of at a given point.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lex {
    Code,
    /// An ordinary or byte string literal.
    Str,
    /// A raw string literal, holding the number of `#` that closes it.
    Raw(usize),
    /// A block comment, holding its nesting depth.
    Block(usize),
}

/// Blanks everything on a line that is not live code -- string literals, raw
/// strings, character literals, line comments and block comments -- so that
/// none of them can look like a spawn it is not, hide one it is, or contribute
/// a parenthesis to the region walk.
///
/// The parenthesis half is why this is a lexer rather than a pair of rules.
/// `scan` decides where a region ends by counting `(` and `)`, so a `)` written
/// inside `')'`, inside `/* returns ) on failure */`, or inside a raw string
/// closes the region early -- and an early close is fully classified, which
/// publishes a finding against a boundary whose span is attached on the very
/// line the finding says carries none.
///
/// State is carried across lines by the caller, so a literal continued with a
/// trailing `\` -- this repository's dominant multi-line string idiom, used by
/// this module's own source -- is noise for its whole length rather than live
/// code from its second line on.
///
/// Known limits: the state starts at `Code` at the top of every hunk, so a hunk
/// whose first line is already inside a literal or a block comment is lexed as
/// code until it closes. A `'` that is neither a character literal nor a
/// lifetime is read as a lifetime. Both are disclosed in
/// `src/fidelity/registry.rs`.
fn strip_noise(line: &str, state: &mut Lex) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;

    while let Some(c) = rest.chars().next() {
        let mut step = c.len_utf8();
        match *state {
            Lex::Block(depth) => {
                if rest.starts_with("/*") {
                    *state = Lex::Block(depth + 1);
                    step = 2;
                } else if rest.starts_with("*/") {
                    *state = if depth <= 1 {
                        Lex::Code
                    } else {
                        Lex::Block(depth - 1)
                    };
                    step = 2;
                }
                out.push(' ');
            }
            Lex::Str => {
                if c == '\\' {
                    // The escape and whatever it escapes are both noise. A
                    // backslash with nothing after it is a line continuation:
                    // the literal carries on into the next line.
                    step = 1 + rest[1..].chars().next().map_or(0, char::len_utf8);
                } else if c == '"' {
                    *state = Lex::Code;
                }
                out.push(' ');
            }
            Lex::Raw(hashes) => {
                if c == '"' && rest[1..].bytes().take_while(|b| *b == b'#').count() >= hashes {
                    *state = Lex::Code;
                    step = 1 + hashes;
                }
                out.push(' ');
            }
            Lex::Code => {
                if rest.starts_with("//") {
                    // The rest of the line is a comment, and the state a
                    // comment leaves behind is the state it found.
                    break;
                } else if rest.starts_with("/*") {
                    *state = Lex::Block(1);
                    step = 2;
                    out.push(' ');
                } else if let Some(hashes) = raw_string_opener(rest) {
                    *state = Lex::Raw(hashes);
                    step = 1 + hashes + 1;
                    out.push(' ');
                } else if c == '"' {
                    *state = Lex::Str;
                    out.push(' ');
                } else if let Some(len) = char_literal_len(rest) {
                    step = len;
                    out.push(' ');
                } else {
                    out.push(c);
                }
            }
        }
        // Every branch advances by a whole number of characters, so this is
        // always a character boundary.
        rest = &rest[step..];
    }

    out
}

/// The `#` count of a raw string opening here, if one does.
///
/// `r` is only a raw-string prefix at the start of a token, which is what the
/// `b` case covers too: `br"..."` reaches this with `rest` at the `r`.
fn raw_string_opener(rest: &str) -> Option<usize> {
    let after_r = rest.strip_prefix('r')?;
    let hashes = after_r.bytes().take_while(|b| *b == b'#').count();
    after_r[hashes..].starts_with('"').then_some(hashes)
}

/// The byte length of a character literal starting here, if this `'` opens one
/// rather than naming a lifetime.
fn char_literal_len(rest: &str) -> Option<usize> {
    let body = rest.strip_prefix('\'')?;
    if let Some(escape) = body.strip_prefix('\\') {
        // An escape of any length -- `\n`, `\'`, `\x41`, `\u{1F600}` -- ends at
        // the next quote *after the character it escapes*: in `'\''` that
        // character is itself a quote, and stopping at it leaves the real
        // closing quote behind as live code.
        let escaped = escape.chars().next()?.len_utf8();
        return escape[escaped..]
            .find('\'')
            .map(|i| 1 + 1 + escaped + i + 1);
    }
    let mut chars = body.chars();
    let first = chars.next()?;
    (chars.next() == Some('\'')).then(|| 1 + first.len_utf8() + 1)
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
        // never attached to it. Keeping the child's text in the child's own
        // frame is what makes the two verdicts independent.
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

    #[test]
    fn a_character_literal_holding_a_quote_is_blanked_whole() {
        // `'\''` is the one character literal whose closing quote is not the
        // next one after the opening: the quote in the middle is escaped. Ended
        // there, the real closing quote is left behind as live code, and it is
        // then free to pair with a later quote and blank whatever lies between
        // them -- which can be a parenthesis the region walk needs.
        let mut lex = Lex::Code;
        let stripped = strip_noise("let quote = '\\''; work(quote);", &mut lex);
        assert!(
            !stripped.contains('\''),
            "the literal was not consumed whole: {stripped:?}"
        );
        assert_eq!(stripped.matches('(').count(), 1);
        assert_eq!(stripped.matches(')').count(), 1);
    }

    #[test]
    fn a_boundary_that_does_not_close_in_its_hunk_is_unresolved() {
        // `scan` is handed one hunk, and the hunk is the whole of what it may
        // read. A boundary left open at the end of it is a region whose extent
        // was never established.
        let lines = shipped(&["tokio::spawn(async move {", "    do_work().await;"]);
        let outcome = SpanTracker::new().scan("src/handler.rs", &lines);
        assert_eq!(outcome.classified, 0);
        assert_eq!(outcome.unresolved, 1);
        assert!(outcome.detached.is_empty());
    }
}
