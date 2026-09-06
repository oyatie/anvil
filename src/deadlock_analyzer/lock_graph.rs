//! A lock-order graph over the text of a change, with cycle detection.
//!
//! # What this is, and what it is not
//!
//! Deadlock detection worth the name resolves *lock identity*. Meta's Infer
//! `Starvation` checker and RacerD build interprocedural summaries over a call
//! graph and reason about the abstract addresses guards are taken on; a runtime
//! detector such as `parking_lot`'s `deadlock_detection` feature or
//! ThreadSanitizer watches the real blocked-thread graph and needs no static
//! identity at all. Both tiers need infrastructure this repository does not
//! have: the first needs a call graph and points-to analysis, the second needs
//! the program to actually run.
//!
//! What is available here is the text of one diff. So the graph below is built
//! at the only tier that text supports: **nodes are receiver expressions, spelled
//! exactly as they appear in the source**, and an edge `a -> b` records that a
//! guard bound from `a` was still in scope when `b` was acquired. Cycle
//! detection over that graph is the same algorithm the real analyses use -- a
//! cycle is a lock-order inversion -- run over a weaker notion of identity.
//!
//! The consequences of that weakness are stated rather than hidden:
//!
//! - Two distinct locks written the same way (`acc_arc` in different loops) are
//!   one node, and one lock reached through two spellings (`self.pools` and a
//!   `&self.pools` binding) is two nodes. Aliasing is not resolved.
//! - The order is only visible where the source shows it. A lock taken inside a
//!   function this change does not touch is invisible, so an inversion split
//!   across a call boundary is not found. This is the missing call graph, and no
//!   amount of text scanning replaces it.
//! - Conditional acquisition is approximated by brace scope, so two locks in
//!   mutually exclusive branches of the same `if` are correctly not paired, but
//!   two locks whose exclusivity comes from a runtime flag rather than the
//!   syntax are paired.
//! - Scope is tracked one line at a time with no state between lines, so a raw
//!   string or a block comment spanning several lines is read as code, and a
//!   `drop(g)` sharing a line with the acquisition it precedes releases one
//!   statement late.
//! - A guard rebound by shadowing (`let g = a.lock(); let g = b.lock();`) is
//!   not released by the shadowing; both stay held until the block closes.
//!
//! This list is the disclosed set, not a claim to be exhaustive. Two entries
//! were added after review found them live: `drop` was not tracked at all, so
//! the idiomatic early release read as a self-deadlock, and braces inside
//! string literals corrupted the depth, so a format escape in one function
//! could manufacture a cycle against another.
//!
//! The bias is deliberately toward silence, because a false red on this gate
//! blocks a merge. An acquisition is only treated as *held* when it is bound by
//! a plain `let`: an unbound guard is a temporary and Rust drops it at the end
//! of its statement. And a finding requires a **cycle**, never a nesting --
//! holding two locks at once is how correct code works, and this repository does
//! it (`self.pools` is held while each account is write-locked). Only an
//! inconsistent order, or one lock taken twice while already held, is reported.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockOrderFinding {
    /// The locks on the cycle, as a set in `BTreeSet` order.
    ///
    /// Not a sequence. `cycles()` returns a strongly connected component --
    /// which locks lie on a cycle together -- and nothing reconstructs a
    /// witness path through it, so the order these are emitted in is
    /// alphabetical, not the order they are acquired in. The field was called
    /// `lock_sequence`, which claimed the ordering the mechanism does not
    /// produce, in a gate whose whole subject is that class of fault.
    pub locks: Vec<String>,
    pub file_path: String,
    pub description: String,
}

/// Method calls that acquire a guard. All are zero-argument, which is what
/// separates `RwLock::read` from `io::Read::read`.
const ACQUISITIONS: [&str; 3] = [".lock()", ".read()", ".write()"];

/// A guard that is still in scope, and the brace depth of the block that owns
/// it. It is released when that block closes, or earlier by an explicit
/// `drop(binding)`.
struct HeldGuard {
    lock: String,
    depth: i32,
    /// The name the guard was `let`-bound to, so `drop(g)` can find it.
    binding: String,
}

#[derive(Debug, Clone, Default)]
pub struct LockOrderGraph;

impl LockOrderGraph {
    pub fn new() -> Self {
        Self
    }

    /// Builds the lock-order graph for `content` and reports every cycle in it.
    ///
    /// `content` may be a unified diff -- which is what the certification
    /// pipeline hands it -- or plain Rust source, which is what the
    /// false-positive test feeds it from this repository's own tree.
    pub fn find_lock_order_cycles(&self, file_path: &str, content: &str) -> Vec<LockOrderFinding> {
        let edges = self.acquisition_edges(content);
        cycles(&edges)
            .into_iter()
            .map(|locks| LockOrderFinding {
                description: describe(&locks),
                locks,
                file_path: file_path.to_string(),
            })
            .collect()
    }

    /// `a -> b`: a guard bound from `a` was in scope when `b` was acquired.
    fn acquisition_edges(&self, content: &str) -> BTreeSet<(String, String)> {
        let mut edges = BTreeSet::new();
        let mut held: Vec<HeldGuard> = Vec::new();
        let mut depth: i32 = 0;

        for raw in content.lines() {
            let line = match diff_payload(raw) {
                DiffLine::Code(l) => l,
                // A hunk or file header ends the fragment: the next hunk is
                // disjoint code, and a guard does not survive the seam.
                DiffLine::Boundary => {
                    held.clear();
                    depth = 0;
                    continue;
                }
                DiffLine::Removed => continue,
            };
            let line = &without_literals(line);

            for acq in acquisitions(line, depth) {
                for guard in &held {
                    edges.insert((guard.lock.clone(), acq.lock.clone()));
                }
                if let Some(binding) = acq.bound {
                    held.push(HeldGuard {
                        lock: acq.lock,
                        depth: acq.depth,
                        binding,
                    });
                }
            }
            // `drop(g)` ends the guard's scope before its block does. Not
            // honouring it turns the idiomatic early release -- which
            // `telemetry_store/mod.rs` already writes twice -- into a
            // self-deadlock accusation.
            //
            // Applied after the line's acquisitions, so a `drop(g); let h =`
            // written on one line releases `g` one statement late. Line
            // granularity is the ceiling of the whole scanner.
            for name in dropped_bindings(line) {
                held.retain(|g| g.binding != name);
            }

            depth += brace_delta(line);
            if depth < 0 {
                // A hunk can start mid-function, so the depth it starts at is
                // unknown and counted as zero. Closing past that means the
                // fragment left the scope it opened in: nothing it saw is held.
                held.clear();
                depth = 0;
            }
            held.retain(|g| g.depth <= depth);
        }

        edges
    }
}

enum DiffLine<'a> {
    Code(&'a str),
    Removed,
    Boundary,
}

/// The new-file content of one unified-diff line.
///
/// Removed lines describe the code as it was: an inversion that appears only
/// there is one this change *deletes*, and reporting it accuses the author of
/// the bug they just fixed.
fn diff_payload(raw: &str) -> DiffLine<'_> {
    if raw.starts_with("@@")
        || raw.starts_with("diff --git ")
        || raw.starts_with("+++")
        || raw.starts_with("---")
        || raw.starts_with("index ")
    {
        return DiffLine::Boundary;
    }
    match raw.as_bytes().first() {
        Some(b'-') => DiffLine::Removed,
        Some(b'+') | Some(b' ') => DiffLine::Code(&raw[1..]),
        _ => DiffLine::Code(raw),
    }
}

/// The line with string and character literal contents removed, and anything
/// after a `//` outside a literal cut.
///
/// Both halves are the same problem: text that looks like code and is not. A
/// `println!("{{")` pushed the brace depth to 2 and returned it to 1, so a
/// guard bound in that function was never released, survived into the next one,
/// and manufactured an edge -- and a cycle -- between two unrelated functions.
/// The comment cut used to be `line.split("//")`, which is the same bug in the
/// other direction: a `//` inside a string truncated the line.
///
/// Raw strings and literals spanning several lines are not handled; this is a
/// line scanner with no state between lines. A `'` that is not a complete
/// character literal is left alone, because it is a lifetime.
fn without_literals(line: &str) -> String {
    let b = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => break,
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
                out.push_str("\"\"");
            }
            b'\'' => {
                // `'a'`, `'\n'` -- but not `&'a T`, where the quote opens a
                // lifetime and never closes.
                let end = if b.get(i + 1) == Some(&b'\\') {
                    b[i + 2..]
                        .iter()
                        .position(|&c| c == b'\'')
                        .map(|p| i + 3 + p)
                } else if b.get(i + 2) == Some(&b'\'') {
                    Some(i + 3)
                } else {
                    None
                };
                match end {
                    Some(end) => {
                        out.push_str("''");
                        i = end;
                    }
                    None => {
                        out.push('\'');
                        i += 1;
                    }
                }
            }
            _ => {
                let ch = line[i..].chars().next().expect("index is a char boundary");
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    out
}

/// The names passed to a `drop(...)` call on this line, when the argument is a
/// plain identifier. `drop(v[0])` and `drop(foo())` name no binding, and
/// `self.drop(x)` is not `mem::drop`.
fn dropped_bindings(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(at) = rest.find("drop(") {
        let follows_a_path = rest[..at]
            .bytes()
            .next_back()
            .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.');
        let arg = &rest[at + "drop(".len()..];
        rest = arg;
        let Some(close) = arg.find(')') else { continue };
        let name = arg[..close].trim();
        if !follows_a_path
            && !name.is_empty()
            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            && !name.as_bytes()[0].is_ascii_digit()
        {
            out.push(name.to_string());
        }
    }
    out
}

fn brace_delta(line: &str) -> i32 {
    line.bytes()
        .map(|b| match b {
            b'{' => 1,
            b'}' => -1,
            _ => 0,
        })
        .sum()
}

struct Acquisition {
    lock: String,
    /// The name a plain `let` bound the guard to, so the guard lives to the end
    /// of its block and `drop` can name it. `None` for an unbound acquisition,
    /// which is a temporary dropped at the end of its statement.
    bound: Option<String>,
    /// Brace depth *at the acquisition*, not at the start of the line, so
    /// `if x { let g = a.lock(); }` releases `g` when that line ends.
    depth: i32,
}

/// Every guard acquisition on one line, in source order.
fn acquisitions(line: &str, line_depth: i32) -> Vec<Acquisition> {
    let mut out = Vec::new();
    for (at, _) in line.char_indices() {
        if !ACQUISITIONS.iter().any(|c| line[at..].starts_with(c)) {
            continue;
        }
        let prefix = &line[..at];
        let Some(lock) = receiver(prefix) else {
            continue;
        };
        out.push(Acquisition {
            lock,
            bound: guard_binding(prefix),
            depth: (line_depth + brace_delta(prefix)).max(0),
        });
    }
    out
}

/// The name the acquisition ending this prefix is bound to, if any.
///
/// `let g = x.lock()` holds until its block ends. A bare `x.lock()` is a
/// temporary dropped at the end of the statement, and `let _ = x.lock()` is
/// dropped immediately -- `_` is a wildcard pattern, not a binding, which is
/// the classic Rust guard bug. None of the three can be told apart by looking
/// at the call alone.
fn guard_binding(prefix: &str) -> Option<String> {
    let statement = prefix.rsplit(';').next().unwrap_or("").trim_start();
    let pattern = statement.strip_prefix("let ")?;
    let pattern = pattern.split('=').next().unwrap_or("").trim();
    let name = pattern.strip_prefix("mut ").unwrap_or(pattern).trim();
    (!name.is_empty()
        && name != "_"
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
    .then(|| name.to_string())
}

/// The dotted path immediately left of an acquisition -- `self.pools`,
/// `acc_arc`. `None` when the receiver is not a plain path (`foo().lock()`,
/// `v[0].lock()`), because a call or an index result cannot be named as a node
/// and guessing one would fabricate an edge.
fn receiver(prefix: &str) -> Option<String> {
    let bytes = prefix.as_bytes();
    let mut start = prefix.len();
    while start > 0 {
        let b = bytes[start - 1];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
            start -= 1;
        } else {
            break;
        }
    }
    let path = &prefix[start..];
    let head = path.split('.').next().unwrap_or("");
    let named = !head.is_empty()
        && head
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && !path.ends_with('.');
    named.then(|| path.to_string())
}

/// The locks lying on a cycle, grouped into strongly connected components.
///
/// A node sits on a cycle exactly when it reaches itself, and two such nodes
/// belong to the same cycle when each reaches the other -- which is the
/// definition of an SCC. Tarjan's linear algorithm computes the same partition;
/// the transitive closure below is used instead because a lock graph built from
/// one diff has a handful of nodes, and a reachability map is far less code to
/// be wrong in.
// ponytail: O(n^2) closure over a graph of a few nodes; swap in Tarjan if a
// whole-repository graph is ever fed to this.
fn cycles(edges: &BTreeSet<(String, String)>) -> Vec<Vec<String>> {
    let mut adjacency: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (from, to) in edges {
        adjacency.entry(from).or_default().insert(to);
        adjacency.entry(to).or_default();
    }

    let reach: BTreeMap<&str, BTreeSet<&str>> = adjacency
        .keys()
        .map(|node| (*node, reachable(node, &adjacency)))
        .collect();

    let mut remaining: BTreeSet<&str> = reach
        .iter()
        .filter(|(node, seen)| seen.contains(*node))
        .map(|(node, _)| *node)
        .collect();

    let mut out = Vec::new();
    while let Some(node) = remaining.iter().next().copied() {
        let component: Vec<String> = remaining
            .iter()
            .filter(|other| reach[node].contains(**other) && reach[**other].contains(node))
            .map(|other| other.to_string())
            .collect();
        remaining.retain(|other| !component.iter().any(|c| c == other));
        out.push(component);
    }
    out
}

fn reachable<'a>(
    from: &'a str,
    adjacency: &BTreeMap<&'a str, BTreeSet<&'a str>>,
) -> BTreeSet<&'a str> {
    let mut seen = BTreeSet::new();
    let mut stack: Vec<&str> = adjacency.get(from).into_iter().flatten().copied().collect();
    while let Some(node) = stack.pop() {
        if !seen.insert(node) {
            continue;
        }
        stack.extend(adjacency.get(node).into_iter().flatten().copied());
    }
    seen
}

fn describe(locks: &[String]) -> String {
    match locks {
        [one] => format!(
            "`{one}` is acquired again while a guard on it is still in scope. Mutex and RwLock in \
             std, tokio and parking_lot are not reentrant, so the second acquisition can block on \
             the first with nothing left to release it."
        ),
        many => format!(
            "Lock-order cycle over {} locks ({}). Each is acquired while another in the set is \
             held, and the order is not consistent across the acquisition sites, so two tasks can \
             each hold what the other is waiting for. Fix by acquiring them in one documented \
             order everywhere.",
            many.len(),
            many.join(", ")
        ),
    }
}
