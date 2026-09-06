//! A small path-glob matcher for satellite forms and artifact patterns:
//! `*` within a segment, `**` for any number of segments, `{a,b}` alternation
//! within a segment, and literal text. Matches are over '/'-separated
//! relative paths.

#[derive(Debug, Clone, PartialEq, Eq)]
enum Seg {
    Any,           // **
    Pat(Vec<Tok>), // one segment
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Lit(String),
    Star,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glob {
    alternatives: Vec<Vec<Seg>>,
}

impl Glob {
    pub fn new(pattern: &str) -> Glob {
        let pattern = pattern.trim_matches('/');
        let alternatives = expand_braces(pattern)
            .into_iter()
            .map(|p| {
                p.split('/')
                    .filter(|s| !s.is_empty())
                    .map(|s| {
                        if s == "**" {
                            Seg::Any
                        } else {
                            Seg::Pat(tokenize(s))
                        }
                    })
                    .collect()
            })
            .collect();
        Glob { alternatives }
    }

    pub fn matches(&self, rel: &str) -> bool {
        let segs: Vec<&str> = rel.split('/').filter(|s| !s.is_empty()).collect();
        self.alternatives.iter().any(|alt| match_segs(alt, &segs))
    }
}

fn expand_braces(p: &str) -> Vec<String> {
    let Some(open) = p.find('{') else {
        return vec![p.to_string()];
    };
    let Some(close) = p[open..].find('}').map(|i| open + i) else {
        return vec![p.to_string()];
    };
    let head = &p[..open];
    let tail = &p[close + 1..];
    let mut out = Vec::new();
    for alt in p[open + 1..close].split(',') {
        for rest in expand_braces(tail) {
            out.push(format!("{head}{alt}{rest}"));
        }
    }
    out
}

fn tokenize(seg: &str) -> Vec<Tok> {
    let mut toks = Vec::new();
    let mut lit = String::new();
    for c in seg.chars() {
        if c == '*' {
            if !lit.is_empty() {
                toks.push(Tok::Lit(std::mem::take(&mut lit)));
            }
            if toks.last() != Some(&Tok::Star) {
                toks.push(Tok::Star);
            }
        } else {
            lit.push(c);
        }
    }
    if !lit.is_empty() {
        toks.push(Tok::Lit(lit));
    }
    toks
}

fn match_segs(pat: &[Seg], segs: &[&str]) -> bool {
    match pat.first() {
        None => segs.is_empty(),
        Some(Seg::Any) => (0..=segs.len()).any(|k| match_segs(&pat[1..], &segs[k..])),
        Some(Seg::Pat(toks)) => {
            segs.first().is_some_and(|s| match_toks(toks, s)) && match_segs(&pat[1..], &segs[1..])
        }
    }
}

fn match_toks(toks: &[Tok], s: &str) -> bool {
    match toks.first() {
        None => s.is_empty(),
        Some(Tok::Lit(l)) => s.starts_with(l.as_str()) && match_toks(&toks[1..], &s[l.len()..]),
        Some(Tok::Star) => s
            .char_indices()
            .map(|(i, _)| i)
            .chain(std::iter::once(s.len()))
            .any(|k| match_toks(&toks[1..], &s[k..])),
    }
}

#[cfg(test)]
mod tests {
    use super::Glob;

    #[test]
    fn segments_stars_and_braces() {
        assert!(Glob::new("*.openslo.yaml").matches("a.openslo.yaml"));
        assert!(!Glob::new("*.openslo.yaml").matches("x/a.openslo.yaml"));
        assert!(Glob::new("**").matches("a/b/c"));
        assert!(Glob::new("{openapi,asyncapi,proto}/**").matches("proto/v1/x.proto"));
        assert!(!Glob::new("{openapi,asyncapi,proto}/**").matches("graphql/x"));
        assert!(Glob::new("IP-*.md").matches("IP-ADR-0339-Shared.md"));
        assert!(Glob::new("dpia.md").matches("dpia.md"));
        assert!(!Glob::new("dpia.md").matches("dpia.txt"));
        assert!(Glob::new("**/*.rs").matches("src/a/b.rs"));
        assert!(Glob::new("third-party/**").matches("third-party/BUCK"));
    }

    #[test]
    fn star_walks_char_boundaries_not_bytes() {
        // A path segment with a multi-byte char panicked the byte-indexed
        // matcher on oyatie's tree ("µ" at bytes 50..52).
        assert!(Glob::new("*.md").matches("latency-µs.md"));
        assert!(Glob::new("*µ*").matches("aµb"));
        assert!(!Glob::new("*.rs").matches("µ.md"));
    }
}
