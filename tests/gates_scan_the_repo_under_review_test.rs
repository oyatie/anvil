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

use std::fs;
use std::path::{Path, PathBuf};

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn a_gate_holding_a_diff_context_does_not_scope_at_its_own_build_directory() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_sources(&src, &mut files);
    assert!(
        !files.is_empty(),
        "no sources found; this check would pass vacuously"
    );

    let mut offences = Vec::new();
    for path in &files {
        let Ok(body) = fs::read_to_string(path) else {
            continue;
        };
        // Production only. A test module legitimately scopes at anvil's tree;
        // it is anvil's tree that it is testing.
        let ships = body.split("#[cfg(test)]").next().unwrap_or(&body);
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
