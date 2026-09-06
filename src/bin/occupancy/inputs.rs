//! The wire format between the presubmit workflow and the rule.
//!
//! Newline lists produced from the forge REST API: this hop's paths, the open
//! hops' paths tagged `pr-<n>`, validated base freshness evidence, and the
//! labels on this pull request. Every one of them fails closed -- a list that
//! could not be read, a line that does not parse, an owner in a spelling this
//! binary does not know -- because each of those, read as an empty set, is
//! indistinguishable from "occupies nothing", which is indistinguishable from
//! "no overlap".

use super::freshness::{FreshnessProof, read_record};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub struct Inputs {
    pub this: BTreeSet<String>,
    pub this_pr: u64,
    pub in_flight: Vec<(String, BTreeSet<String>)>,
    pub freshness: FreshnessProof,
    pub override_label: bool,
}

pub fn collect(args: &[String]) -> Result<Inputs, String> {
    let evidence_path = freshness_flag(args)?;
    let evidence = fs::File::open(&evidence_path)
        .map_err(|_| "could not open freshness evidence".to_owned())?;
    let freshness = read_record(evidence)?;
    let this = read(Path::new(&flag(args, "--this")?))?;
    let in_flight = read(Path::new(&flag(args, "--in-flight")?))?;
    let this_pr = parse_pr_number(&flag(args, "--this-pr")?)?;
    // Optional, and absent means absent: a label file that could not be read
    // is an error, but no `--labels` at all is simply no override.
    let override_label = match flag(args, "--labels") {
        Ok(path) => parse_paths(&read(Path::new(&path))?).contains(OVERRIDE_LABEL),
        Err(_) => false,
    };
    Ok(Inputs {
        this: parse_paths(&this),
        this_pr,
        in_flight: parse_in_flight(&in_flight)?,
        freshness,
        override_label,
    })
}

/// The label a human applies to admit a hop the queue rule would hold.
///
/// Audited by the forge rather than here: GitHub records who applied a label
/// and when, on the pull request's own timeline. What this binary owes is that
/// the override is *visible* — it reports `Warning` naming the label, never
/// `Passed`, so an overridden admission never reads as a measured disjointness.
pub const OVERRIDE_LABEL: &str = "occupancy-override";

/// The pull request's own number, which is its place in the queue.
pub fn parse_pr_number(raw: &str) -> Result<u64, String> {
    raw.trim()
        .parse::<u64>()
        .map_err(|_| format!("--this-pr must be a pull request number, got `{raw}`"))
}

/// `pr-<n>` — the owner spelling the collecting step writes.
///
/// An owner that does not parse is an error rather than a hop skipped: a
/// skipped hop is one this change is no longer compared against, which is
/// exactly the false green the whole check exists to prevent.
pub fn owner_number(owner: &str) -> Result<u64, String> {
    owner
        .strip_prefix("pr-")
        .and_then(|n| n.trim().parse::<u64>().ok())
        .ok_or_else(|| format!("in-flight owner `{owner}`: expected `pr-<number>`"))
}

pub fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))
}

pub fn flag(args: &[String], name: &str) -> Result<String, String> {
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == name {
            return rest
                .next()
                .cloned()
                .ok_or_else(|| format!("{name} requires a value"));
        }
    }
    Err(format!("missing {name}"))
}

fn freshness_flag(args: &[String]) -> Result<String, String> {
    if args.iter().any(|arg| arg == "--merge-base-is-trunk") {
        return Err("obsolete --merge-base-is-trunk is not freshness evidence".to_owned());
    }
    if args.iter().filter(|arg| *arg == "--freshness-file").count() != 1 {
        return Err("exactly one --freshness-file is required".to_owned());
    }
    let path = flag(args, "--freshness-file")?;
    if path.is_empty() || path.starts_with("--") {
        return Err("--freshness-file requires a path".to_owned());
    }
    Ok(path)
}

pub fn parse_paths(body: &str) -> BTreeSet<String> {
    body.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `<owner>\t<path>` lines, one per changed file of one open pull request.
pub fn parse_in_flight(body: &str) -> Result<Vec<(String, BTreeSet<String>)>, String> {
    let mut by_owner: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (i, line) in body.lines().enumerate() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        let Some((owner, path)) = line.split_once('\t') else {
            return Err(format!(
                "in-flight line {}: expected `<owner>\\t<path>`, got `{line}`",
                i + 1
            ));
        };
        if owner.trim().is_empty() || path.trim().is_empty() {
            return Err(format!("in-flight line {}: empty owner or path", i + 1));
        }
        by_owner
            .entry(owner.trim().to_owned())
            .or_default()
            .insert(path.trim().to_owned());
    }
    Ok(by_owner.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| (*s).to_string()).collect()
    }

    fn set(paths: &[&str]) -> BTreeSet<String> {
        paths.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn a_flag_without_a_value_is_an_error_not_an_empty_string() {
        assert!(flag(&args(&["--this"]), "--this").is_err());
        assert!(flag(&args(&[]), "--this").is_err());
        assert_eq!(
            flag(&args(&["--this", "a.txt"]), "--this"),
            Ok("a.txt".to_owned())
        );
    }

    #[test]
    fn freshness_requires_one_record_and_rejects_obsolete_or_mixed_flags() {
        for raw in [
            vec![],
            vec!["--freshness-file"],
            vec!["--freshness-file", ""],
            vec!["--freshness-file", "--this"],
            vec!["--merge-base-is-trunk", "true"],
            vec![
                "--freshness-file",
                "proof",
                "--merge-base-is-trunk",
                "false",
            ],
            vec!["--freshness-file", "first", "--freshness-file", "second"],
        ] {
            assert!(freshness_flag(&args(&raw)).is_err());
        }
        assert_eq!(
            freshness_flag(&args(&["--freshness-file", "proof"])),
            Ok("proof".to_owned())
        );
    }

    #[test]
    fn paths_are_trimmed_and_blank_lines_dropped() {
        assert_eq!(
            parse_paths("tests/a.rs\n\n  tests/b.rs  \n"),
            set(&["tests/a.rs", "tests/b.rs"])
        );
    }

    #[test]
    fn in_flight_lines_group_by_owner() {
        let parsed = parse_in_flight("pr-7\ttests/a.rs\npr-7\ttests/b.rs\npr-9\ttests/c.rs\n")
            .expect("well-formed");
        assert_eq!(
            parsed,
            vec![
                ("pr-7".to_owned(), set(&["tests/a.rs", "tests/b.rs"])),
                ("pr-9".to_owned(), set(&["tests/c.rs"])),
            ]
        );
    }

    #[test]
    fn a_malformed_in_flight_line_is_an_error_not_an_empty_set() {
        let err = parse_in_flight("pr-7 tests/a.rs\n").unwrap_err();
        assert!(
            err.contains("line 1"),
            "the reason must locate itself: {err}"
        );
    }

    #[test]
    fn a_pr_number_that_is_not_a_number_is_an_error() {
        assert!(parse_pr_number("").is_err());
        assert!(parse_pr_number("pr-9").is_err());
        assert_eq!(parse_pr_number(" 9 "), Ok(9));
    }

    /// An owner in a spelling this binary does not know is a hop that would
    /// silently stop being compared against.
    #[test]
    fn an_owner_that_is_not_a_pull_request_number_is_an_error() {
        assert_eq!(owner_number("pr-9"), Ok(9));
        let err = owner_number("branch-foo").unwrap_err();
        assert!(err.contains("branch-foo"), "{err}");
    }
}
