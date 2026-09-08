//! A gate handed a pull request scopes from that pull request's tree.
//!
//! `CARGO_MANIFEST_DIR` is anvil's own source directory, fixed at compile time.
//! A gate that scopes there while holding a `PrDiffContext` reports on anvil to
//! an author who cannot act on it, and reports it identically for every pull
//! request in every repository.
//!
//! Self-conformance checks legitimately scope there — they exist to measure
//! anvil. What separates them is the `PrDiffContext`: holding one means the
//! subject is someone else's tree.

use std::path::Path;

#[path = "source_acquisition/mod.rs"]
mod source_acquisition;

#[test]
fn a_gate_holding_a_diff_context_does_not_scope_at_its_own_build_directory() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let files = source_acquisition::rust_sources(&src).expect("source corpus");
    assert!(
        !files.is_empty(),
        "no sources found; this check would pass vacuously"
    );

    let mut offences = Vec::new();
    for file in &files {
        let path = &file.path;
        let body = &file.text;
        // Production only. A test module legitimately scopes at anvil's tree;
        // it is anvil's tree that it is testing.
        let ships = body.split("#[cfg(test)]").next().unwrap_or(body);
        // `without_commentary`, not `code_only`: the name is only ever spelled as
        // the string literal inside `env!(...)`, and `code_only` blanks literals --
        // it would erase exactly what this looks for. The first draft did.
        let code = anvil::source_scan::without_commentary(ships);
        if !code.contains("PrDiffContext") {
            continue;
        }
        for (i, line) in code.lines().enumerate() {
            // Scoping at anvil's own tree is legitimate for a self-conformance
            // check, and the way to say so is to name it: a `const` whose name
            // carries ANVIL declares the subject at the site, where a reader of
            // the scope sees it. An allowlist kept elsewhere would not.
            let names_itself = line.contains("const") && line.contains("ANVIL");
            if line.contains("CARGO_MANIFEST_DIR") && !names_itself {
                let rel = path.strip_prefix(&src).unwrap_or(path);
                offences.push(format!("src/{}:{}", rel.display(), i + 1));
            }
        }
    }

    assert!(
        offences.is_empty(),
        "these gates hold a `PrDiffContext` and scope at anvil's own tree, so every \
         verdict is about anvil rather than the change under review. Scope from \
         `diff_ctx.repo_working_dir`, which the certification pipeline already \
         populates -- or, if anvil really is the subject, bind it to a `const` \
         whose name says so.\n  {}",
        offences.join("\n  ")
    );
}

/// #218. A filesystem walk in this suite must not descend into another
/// checkout.
///
/// Anvil keeps agent worktrees under `.claude/worktrees/` and a `devtree`
/// beside them. Every walk rooted at the repository descended into each one, so
/// censuses that call themselves CLOSED counted every real site once per
/// checkout, and the reported set changed when worktrees were removed while
/// nothing about the source under review did.
///
/// Fixing the three walkers that were measured leaves the fourth to be written
/// the same way. This is the rule that makes the fourth fail here instead: a
/// file that calls `fs::read_dir` either carries the exclusion, or is listed
/// below with the bounded subtree it walks. The list only shrinks.
///
/// Not a lint against `read_dir` itself -- walking is legitimate, and a walk
/// bounded to `src/` cannot reach `.claude/` or `devtree/` at all.
#[test]
fn every_filesystem_walk_either_skips_nested_checkouts_or_says_why_it_need_not() {
    // Each entry is a file whose walk cannot reach a nested checkout, with the
    // reason. Anvil's checkouts live at `.claude/worktrees/` and `devtree/`,
    // both outside `src/` and outside `tests/`.
    const BOUNDED: &[(&str, &str)] = &[(
        "source_acquisition_test.rs",
        "drives the shared walker with an in-memory access; the checkout rule is what two of its cases assert",
    )];

    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut unguarded = Vec::new();
    let mut walkers = 0usize;
    let mut stack = vec![tests.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("tests listable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("readable");
            let code = anvil::source_scan::without_commentary(&text);
            if !code.contains("read_dir(") {
                continue;
            }
            walkers += 1;
            // Either the exclusion is spelled here, or the walk is rooted at a
            // subtree that cannot contain one. `join("src")` / `join("tests")`
            // is that root, spelled the way this suite spells it.
            let excludes =
                code.contains(".join(\".git\").exists()") || code.contains("is_separate_checkout");
            let bounded = code.contains("CARGO_MANIFEST_DIR\")).join(\"src\")")
                || code.contains("CARGO_MANIFEST_DIR\").join(\"src\")")
                || code.contains("CARGO_MANIFEST_DIR\")).join(\"tests\")")
                || code.contains("CARGO_MANIFEST_DIR\").join(\"tests\")");
            let name = path
                .file_name()
                .expect("named")
                .to_string_lossy()
                .to_string();
            let listed = BOUNDED.iter().any(|(f, _)| *f == name);
            if !excludes && !bounded && !listed {
                unguarded.push(name);
            }
        }
    }

    // The instrument first: a scan finding no walkers would pass by being blind.
    assert!(
        walkers >= 20,
        "the walk census found {walkers} files calling `read_dir`, far fewer than \
         this suite has -- the scan is broken, not the code"
    );

    unguarded.sort();
    unguarded.dedup();
    assert!(
        unguarded.is_empty(),
        "these walk the filesystem without excluding nested checkouts, so anything \
         left inside the repository inflates what they report:\n  {}\nSkip a \
         directory whose `.git` entry exists (`p.join(\".git\").exists()`), root the \
         walk at a bounded subtree, or add it to BOUNDED with the reason.",
        unguarded.join("\n  ")
    );
}
