//! Citations must name a symbol, not a line number.
//!
//! # The class
//!
//! A `file.rs:133-137` citation is a cached search result. Anything inserted
//! above line 133 invalidates it, and nothing that inserts those lines is
//! looking at the registry. This is the same decoupling the citation test
//! itself was written to close, one level up: there, the quoted evidence drifts
//! from the code; here, the coordinate drifts from the evidence.
//!
//! It is not hypothetical. Wiring `gate_proof` into the scorecard added sixteen
//! lines to `publish/scorecard.rs` and broke a `trace_status` citation that had
//! nothing to do with the change -- a tax charged to an unrelated author, on a
//! file with a hundred and twenty-four such coordinates across sixty-nine
//! files.
//!
//! # The remedy, which already existed
//!
//! `file.rs::symbol` was already supported and already used nine times. The
//! mechanism was not missing; the obligation was. A symbol moves with its code,
//! so the citation survives every edit that does not delete what it cites --
//! and when it DOES delete it, that is a real finding rather than a renumbering
//! chore.
//!
//! This is the ratchet that makes the next line-range citation unwritable and
//! obliges the existing ones to fall.

use anvil::fidelity::registry::AUDITED_GATES;
use std::path::Path;
use std::sync::LazyLock;

/// A citation pinned to a line number rather than to a symbol.
///
/// `\.rs:` followed by a digit. The symbol form is `\.rs::name`, whose second
/// colon is never a digit, so the two forms cannot be confused.
static LINE_CITATION: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"[A-Za-z0-9_/]+\.rs:[0-9]").expect("static pattern"));

/// Line-number citations remaining. Zero, and it may never rise.
///
/// Not a ceiling with slack in it. Slack is what lets a new coordinate be
/// written under cover of an old one, which is the whole defect: the cost of
/// this class is paid by whoever edits an unrelated file, so nobody who adds
/// one ever feels it. The registry holds none on either side of the merge base,
/// so this says what the derived bound below says -- and says it in a checkout
/// with no merge base to ask.
const LINE_CITATIONS_REMAINING: usize = 0;

fn count() -> usize {
    AUDITED_GATES
        .iter()
        .map(|g| LINE_CITATION.find_iter(g.gap).count())
        .sum()
}

/// An equality, which at zero is the only form that bounds anything.
#[test]
fn line_number_citations_match_the_recorded_count() {
    assert_eq!(
        count(),
        LINE_CITATIONS_REMAINING,
        "line-number citations rose. Migrate the citation to `file.rs::symbol`: \
         a coordinate is a cached search result that the next unrelated \
         edit will invalidate."
    );
}

#[test]
fn the_symbol_form_is_in_real_use_and_not_merely_permitted() {
    let symbol_citations: usize = AUDITED_GATES
        .iter()
        .map(|g| g.gap.matches(".rs::").count())
        .sum();
    assert!(
        symbol_citations >= 9,
        "the remedy must stay reachable: {symbol_citations} symbol citation(s) \
         found. A remedy nothing uses is a remedy nobody will reach for."
    );
}

/// Every `.rs` file under `src/fidelity/`, not one named file.
///
/// The subject is the registry corpus, and a single path is a proxy for it that
/// stops being one the moment an entry moves. `registry.rs` held all of them
/// until it was split across `registry/entries_*.rs`; a counter keyed to the
/// old filename would have read zero on this side, reported a fall of 124 and
/// been blind to every coordinate written into a new file afterwards. Both
/// sides ask the same question of the directory, so a move inside it is not a
/// change in the count.
fn is_registry_source(path: &str) -> bool {
    path.starts_with("src/fidelity/") && path.ends_with(".rs")
}

/// The line-citation count over the working tree's own registry sources.
fn line_citations_in_the_tree(root: &Path, out: &mut usize) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            line_citations_in_the_tree(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let body = std::fs::read_to_string(&path).expect("registry source is readable");
            *out += LINE_CITATION.find_iter(&body).count();
        }
    }
}

/// The bound that holds without a committed literal.
///
/// `LINE_CITATIONS_REMAINING` above stays as a floor for checkouts with no
/// merge-base. Measured on the registry's source text on both sides, so the
/// two counts are the same measure even though only one side can be parsed.
#[test]
fn line_citations_do_not_grow_against_the_merge_base() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let count = |path: &str, body: &str| {
        if !is_registry_source(path) {
            return 0;
        }
        LINE_CITATION.find_iter(body).count()
    };
    let Some(base) = rt.block_on(anvil::ratchet::facade::derived::source_sites_at_merge_base(
        repo,
        "origin/dev",
        "HEAD",
        count,
    )) else {
        eprintln!("skipped: no merge-base against origin/dev");
        return;
    };
    let mut now = 0usize;
    line_citations_in_the_tree(&repo.join("src/fidelity"), &mut now);
    assert!(
        now <= base.at_merge_base,
        "line-number citations grew from {} at merge-base {} to {}. A \
         `file.rs:133-137` citation is a cached search result that the next \
         unrelated edit invalidates; `file.rs::symbol` moves with the code.",
        base.at_merge_base,
        &base.merge_base[..12],
        now
    );
}
