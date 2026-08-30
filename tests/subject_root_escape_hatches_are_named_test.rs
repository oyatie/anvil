//! Only test code may assert a subject root as a fixture.
//!
//! `SubjectRoot::asserted` is the way out of the type, so the type is worth
//! exactly as much as the discipline on that one symbol. `TestFixture` is the
//! variant a production caller would reach for first, and it is the one that
//! means nothing was verified.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Files a parent declares as `#[cfg(test)] mod name;`.
///
/// Their contents never reach a release build, so a fixture in one is not a
/// production caller even though it carries no `#[cfg(test)]` of its own.
fn test_only_modules(files: &[PathBuf]) -> HashSet<PathBuf> {
    let mut out = HashSet::new();
    for f in files {
        let Ok(body) = fs::read_to_string(f) else {
            continue;
        };
        let lines: Vec<&str> = body.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if !line.trim().starts_with("#[cfg(test)]") {
                continue;
            }
            let Some(next) = lines.get(i + 1) else {
                continue;
            };
            let Some(name) = next
                .trim()
                .strip_prefix("mod ")
                .and_then(|r| r.strip_suffix(';'))
            else {
                continue;
            };
            let dir = f.parent().unwrap_or(Path::new("."));
            out.insert(dir.join(format!("{name}.rs")));
            out.insert(dir.join(name).join("mod.rs"));
        }
    }
    out
}

/// Which lines of a file sit inside a `#[cfg(test)]` module, or `None` when
/// this scan cannot tell.
///
/// Depth is counted on `code_only`, which blanks comments AND literal bodies,
/// so a brace inside a string cannot move it. `without_commentary` keeps
/// literal bodies and was tried first: one unbalanced brace in a fixture
/// string left `inside` latched open to the end of the file, and every line
/// below the test module read as test code. That is a false NEGATIVE, and it
/// let a seeded production fixture through.
///
/// `code_only` has its own blind spot -- it does not model raw strings, and a
/// single `r#"..."#` leaves its scanner inside a string for the rest of the
/// file. That is why this returns `None` when the depth does not return to
/// zero at the end: an unbalanced count means the scan was fooled, and a
/// caller must refuse rather than guess. Refusing is the only direction that
/// cannot hide the defect.
fn test_only_lines(body: &str) -> Option<Vec<bool>> {
    let code = anvil::source_scan::code_only(body);
    let code_lines: Vec<&str> = code.lines().collect();
    let raw_lines: Vec<&str> = body.lines().collect();
    let mut out = vec![false; raw_lines.len()];

    let mut depth: i32 = 0;
    let mut inside: Option<i32> = None;
    let mut pending = false;

    for (i, raw) in raw_lines.iter().enumerate() {
        if inside.is_some() {
            out[i] = true;
        }
        if raw.trim().starts_with("#[cfg(test)]") {
            pending = true;
        }
        let line = code_lines.get(i).copied().unwrap_or("");
        if pending && line.contains("mod ") && line.contains('{') {
            inside = Some(depth);
            out[i] = true;
            pending = false;
        }
        depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
        if inside.is_some_and(|d| depth <= d) {
            inside = None;
        }
    }

    (depth == 0).then_some(out)
}

#[test]
fn only_test_code_asserts_a_fixture_subject_root() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&src, &mut files);
    assert!(
        !files.is_empty(),
        "no sources found; this would pass vacuously"
    );

    let test_only = test_only_modules(&files);
    let mut offences = Vec::new();
    let mut unreadable = Vec::new();

    for f in &files {
        if test_only.contains(f) {
            continue;
        }
        let Ok(body) = fs::read_to_string(f) else {
            continue;
        };
        if !body.contains("Uncloned::TestFixture") {
            continue;
        }
        let Some(regions) = test_only_lines(&body) else {
            unreadable.push(f.display().to_string());
            continue;
        };
        for (i, line) in body.lines().enumerate() {
            if line.contains("Uncloned::TestFixture") && !regions[i] {
                offences.push(format!("{}:{}", f.display(), i + 1));
            }
        }
    }

    assert!(
        offences.is_empty(),
        "`Uncloned::TestFixture` claims a subject root nobody cloned and nobody \
         checked. In production, name the real reason -- `SelfMeasurement` for \
         anvil's own tree, `OperatorSupplied` for a path off the command line, \
         `NoTreeBehindThisDiff` for a corpus with no tree -- or take the root \
         from `ensure_repo_cloned`, which is the only thing that puts one on \
         disk.\n  {}",
        offences.join("\n  ")
    );
    assert!(
        unreadable.is_empty(),
        "brace depth did not return to zero in these files, so the scan could \
         not tell test code from production and refused to guess. `code_only` \
         does not model raw strings; move the fixture to `tests/`, or to a file \
         a parent declares `#[cfg(test)] mod name;`.\n  {}",
        unreadable.join("\n  ")
    );
}

#[test]
fn the_region_scan_ends_at_the_test_module_and_not_at_the_file() {
    let body = "\
fn production() {}

#[cfg(test)]
mod tests {
    fn fixture() { let _ = \"}\"; }
}

fn appended_below() {}
";
    let r = test_only_lines(body).expect("balanced fixture");
    assert!(!r[0], "a function above the test module is production");
    assert!(r[4], "a line inside the test module is test code");
    assert!(
        !r[7],
        "a function appended BELOW the test module is production again; a \
         latching flag calls it test code and lets the defect through"
    );
}

#[test]
fn a_cfg_test_module_in_another_file_is_recognised() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&src, &mut files);
    let test_only = test_only_modules(&files);

    // `clean_architecture_guard` declares `#[cfg(test)] mod tests;`, and its
    // fixtures live in that separate file. Without cross-file resolution this
    // check would report five false offences.
    assert!(
        test_only.contains(&src.join("clean_architecture_guard").join("tests.rs")),
        "cross-file `#[cfg(test)] mod tests;` was not resolved; found {} test-only modules",
        test_only.len()
    );
}
