use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn expand_member_pattern(root: &Path, pattern: &str) -> Result<Vec<PathBuf>, String> {
    let pattern = pattern
        .trim_end_matches('/')
        .strip_suffix("/Cargo.toml")
        .unwrap_or(pattern);
    validate(pattern, "member")?;
    let parts = pattern
        .split('/')
        .filter(|part| !matches!(*part, "" | "."))
        .collect::<Vec<_>>();
    let mut matches = Vec::new();
    expand_parts(root, &parts, &mut BTreeSet::new(), &mut matches)?;
    matches.sort();
    matches.dedup();
    Ok(matches)
}

pub(super) fn excluded(relative: &str, patterns: &[String]) -> Result<bool, String> {
    let actual = relative.split('/').collect::<Vec<_>>();
    for pattern in patterns {
        let trimmed = pattern.trim_matches('/');
        let normalized = trimmed.strip_suffix("/Cargo.toml").unwrap_or(trimmed);
        validate(normalized, "exclude")?;
        let expected = normalized.split('/').collect::<Vec<_>>();
        let wildcard = normalized.contains(['*', '?', '[', '\\']);
        if path_matches(&expected, &actual)
            || (!wildcard && relative.starts_with(&format!("{normalized}/")))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn validate(pattern: &str, kind: &str) -> Result<(), String> {
    if pattern.contains(['{', '}']) {
        return Err(format!("unsupported workspace {kind} glob `{pattern}`"));
    }
    if pattern.split('/').any(|part| part == "..") || Path::new(pattern).is_absolute() {
        return Err(format!("workspace {kind} `{pattern}` escapes its root"));
    }
    for segment in pattern.split('/') {
        let chars = segment.chars().collect::<Vec<_>>();
        if segment_boundaries(&chars).is_none() {
            return Err(format!("malformed workspace {kind} glob `{pattern}`"));
        }
    }
    Ok(())
}

fn expand_parts(
    root: &Path,
    parts: &[&str],
    visited: &mut BTreeSet<(PathBuf, usize)>,
    matches: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let mut pending = vec![(root.to_path_buf(), 0)];
    while let Some((current, part_at)) = pending.pop() {
        if !visited.insert((current.clone(), part_at)) {
            continue;
        }
        let Some(part) = parts.get(part_at) else {
            if current.is_dir() && current.join("Cargo.toml").is_file() {
                matches.push(current);
            }
            continue;
        };
        if *part == "**" {
            pending.push((current.clone(), part_at + 1));
            pending.extend(
                child_directories(&current)?
                    .into_iter()
                    .map(|directory| (directory, part_at)),
            );
        } else if !part.contains(['*', '?', '[', '\\']) {
            pending.push((current.join(part), part_at + 1));
        } else {
            for directory in child_directories(&current)? {
                let name = directory
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| {
                        format!("workspace member path {} is not UTF-8", directory.display())
                    })?;
                if segment_matches(part, name) {
                    pending.push((directory, part_at + 1));
                }
            }
        }
    }
    Ok(())
}

fn child_directories(current: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = match fs::read_dir(current) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "cannot expand workspace members in {}: {error}",
                current.display()
            ));
        }
    };
    let mut directories = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("cannot read workspace member entry: {error}"))?;
        let kind = entry.file_type().map_err(|error| {
            format!(
                "cannot inspect workspace member {}: {error}",
                entry.path().display()
            )
        })?;
        if kind.is_symlink() {
            return Err(format!(
                "workspace member candidate {} is a symlink",
                entry.path().display()
            ));
        }
        if kind.is_dir() {
            directories.push(entry.path());
        }
    }
    directories.sort();
    Ok(directories)
}

fn path_matches(pattern: &[&str], value: &[&str]) -> bool {
    let mut matched = vec![vec![false; value.len() + 1]; pattern.len() + 1];
    matched[pattern.len()][value.len()] = true;
    for pattern_at in (0..pattern.len()).rev() {
        for value_at in (0..=value.len()).rev() {
            matched[pattern_at][value_at] = if pattern[pattern_at] == "**" {
                matched[pattern_at + 1][value_at]
                    || (value_at < value.len() && matched[pattern_at][value_at + 1])
            } else {
                value_at < value.len()
                    && segment_matches(pattern[pattern_at], value[value_at])
                    && matched[pattern_at + 1][value_at + 1]
            };
        }
    }
    matched[0][0]
}

fn segment_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let boundaries = segment_boundaries(&pattern).expect("validated pattern");
    let value = value.chars().collect::<Vec<_>>();
    let mut matched = vec![vec![false; value.len() + 1]; pattern.len() + 1];
    matched[pattern.len()][value.len()] = true;
    for (pattern_at, next) in boundaries.into_iter().rev() {
        for value_at in (0..=value.len()).rev() {
            matched[pattern_at][value_at] = match pattern.get(pattern_at) {
                Some('*') => {
                    matched[pattern_at + 1][value_at]
                        || (value_at < value.len() && matched[pattern_at][value_at + 1])
                }
                Some('?') => value_at < value.len() && matched[pattern_at + 1][value_at + 1],
                Some('[') => {
                    let close = next - 1;
                    value_at < value.len()
                        && class_matches(&pattern[pattern_at + 1..close], value[value_at])
                        && matched[close + 1][value_at + 1]
                }
                Some('\\') if pattern_at + 1 < pattern.len() => {
                    value_at < value.len()
                        && pattern[pattern_at + 1] == value[value_at]
                        && matched[pattern_at + 2][value_at + 1]
                }
                Some(expected) => {
                    value_at < value.len()
                        && *expected == value[value_at]
                        && matched[pattern_at + 1][value_at + 1]
                }
                None => unreachable!("pattern index is in bounds"),
            };
        }
    }
    matched[0][0]
}

/// The same parsed boundaries drive validation and matching. Class members
/// and escaped characters are data, never independent pattern starts.
fn segment_boundaries(pattern: &[char]) -> Option<Vec<(usize, usize)>> {
    let mut boundaries = Vec::new();
    let mut cursor = 0;
    while cursor < pattern.len() {
        let next = match pattern[cursor] {
            '[' => class_end(pattern, cursor)? + 1,
            '\\' if cursor + 1 < pattern.len() => cursor + 2,
            _ => cursor + 1,
        };
        boundaries.push((cursor, next));
        cursor = next;
    }
    Some(boundaries)
}

/// Cargo's globset grammar permits `]` as the first class member (after an
/// optional `!`). Thus `[!]]` is a negated class containing a literal `]`;
/// the first bracket is data and the second closes the class.
fn class_end(pattern: &[char], open: usize) -> Option<usize> {
    let mut cursor = open + 1;
    if pattern.get(cursor) == Some(&'!') {
        cursor += 1;
    }
    if pattern.get(cursor) == Some(&']') {
        cursor += 1;
    }
    (cursor < pattern.len())
        .then(|| pattern[cursor..].iter().position(|ch| *ch == ']'))
        .flatten()
        .map(|relative| cursor + relative)
}

fn class_matches(class: &[char], value: char) -> bool {
    let (negated, class) = match class.first() {
        Some('!') => (true, &class[1..]),
        _ => (false, class),
    };
    let mut matched = false;
    let mut cursor = 0;
    while cursor < class.len() {
        if cursor + 2 < class.len() && class[cursor + 1] == '-' {
            matched |= class[cursor] <= value && value <= class[cursor + 2];
            cursor += 3;
        } else {
            matched |= class[cursor] == value;
            cursor += 1;
        }
    }
    matched != negated
}

#[cfg(test)]
mod tests;
