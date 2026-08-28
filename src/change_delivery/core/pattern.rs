//! CODEOWNERS pattern matching, owned by the unit whose semantics they are.
//!
//! These rules are gitignore-like and are not shape's glob: a pattern with no
//! slash matches a basename at any depth, a trailing slash means the
//! directory's contents, and a leading slash anchors to the root. Borrowing a
//! matcher with different semantics costs a rewrite of the pattern at every
//! call site, and the rewrite is the tell — a contract does not need its
//! argument bent to fit.

/// One token inside a path segment.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Lit(String),
    Star,
}

/// One element of a pattern: a segment, or `**`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Seg {
    Any,
    Pat(Vec<Tok>),
}

fn tokenize(seg: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut lit = String::new();
    for c in seg.chars() {
        if c == '*' {
            if !lit.is_empty() {
                out.push(Tok::Lit(std::mem::take(&mut lit)));
            }
            if out.last() != Some(&Tok::Star) {
                out.push(Tok::Star);
            }
        } else {
            lit.push(c);
        }
    }
    if !lit.is_empty() {
        out.push(Tok::Lit(lit));
    }
    out
}

/// Whether one segment's tokens match one path segment.
fn segment_matches(toks: &[Tok], seg: &str) -> bool {
    match toks.split_first() {
        None => seg.is_empty(),
        Some((Tok::Lit(l), rest)) => seg
            .strip_prefix(l.as_str())
            .is_some_and(|tail| segment_matches(rest, tail)),
        // `*` is greedy but backtracks: it spans any run within the segment.
        Some((Tok::Star, rest)) => {
            if rest.is_empty() {
                return true;
            }
            (0..=seg.len()).any(|i| seg.is_char_boundary(i) && segment_matches(rest, &seg[i..]))
        }
    }
}

fn parse(pattern: &str) -> Vec<Seg> {
    pattern
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            if s == "**" {
                Seg::Any
            } else {
                Seg::Pat(tokenize(s))
            }
        })
        .collect()
}

/// Whether a segment list matches a path's segments, `**` spanning any run.
fn walk(pat: &[Seg], path: &[&str]) -> bool {
    match pat.split_first() {
        None => path.is_empty(),
        Some((Seg::Any, rest)) => (0..=path.len()).any(|i| walk(rest, &path[i..])),
        Some((Seg::Pat(toks), rest)) => path
            .split_first()
            .is_some_and(|(head, tail)| segment_matches(toks, head) && walk(rest, tail)),
    }
}

/// Whether `path` is owned by `pattern` under CODEOWNERS rules.
pub fn matches(pattern: &str, path: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let anchored = pattern.starts_with('/');
    let trimmed = pattern.trim_start_matches('/');
    let dir_form = pattern.ends_with('/');
    let bare = trimmed.trim_end_matches('/');

    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    // No slash inside the pattern: a basename, or a directory name, anywhere.
    if !bare.contains('/') {
        let toks = tokenize(bare);
        let hit = segments.iter().any(|s| segment_matches(&toks, s));
        return if dir_form {
            hit && path.contains('/')
        } else {
            hit
        };
    }

    let pat = parse(bare);
    // A directory form owns everything beneath it.
    let with_contents = |p: &[Seg]| {
        let mut v = p.to_vec();
        v.push(Seg::Any);
        v
    };
    let effective = if dir_form { with_contents(&pat) } else { pat };

    if walk(&effective, &segments) {
        return true;
    }
    // Unanchored path patterns may begin at any depth.
    !anchored && (1..=segments.len()).any(|i| walk(&effective, &segments[i..]))
}
