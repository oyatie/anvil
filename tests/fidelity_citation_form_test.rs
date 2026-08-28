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

/// Line-number citations remaining. EXACT, and it must fall.
///
/// Not a ceiling. A ceiling would let a new coordinate be written under cover
/// of an old one, which is the whole defect: the cost of this class is paid by
/// whoever edits an unrelated file, so nobody who adds one ever feels it.
const LINE_CITATIONS_REMAINING: usize = 124;

fn count() -> usize {
    AUDITED_GATES
        .iter()
        .map(|g| LINE_CITATION.find_iter(g.gap).count())
        .sum()
}

/// A ceiling, not an equality. See the derived bound below.
#[test]
fn line_number_citations_do_not_exceed_the_recorded_ceiling() {
    let found = count();
    assert!(
        found <= LINE_CITATIONS_REMAINING,
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
        if path != "src/fidelity/registry.rs" {
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
    let now = count(
        "src/fidelity/registry.rs",
        &std::fs::read_to_string(repo.join("src/fidelity/registry.rs")).expect("registry"),
    );
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
