//! A count published in prose must be one the code can still support.
//!
//! `doc_guard::corpus_sync` has owned this for markdown since the published
//! counts drifted 23 / 60 / 68 / 70. It scanned `docs/**.md` and nothing else,
//! which is why the same defect went on living in Rust doc comments:
//!
//!   * `report.rs` said the registry exemption "covers thirty-seven of the
//!     seventy-two gates". It covered eighteen. Every gate audited since it was
//!     written moved the number, and no gate PR touched the sentence.
//!   * `fidelity/mod.rs` said adding a field "would touch all fifty-one
//!     entries". The table held fifty-four.
//!
//! Both were true when written. That is the whole character of the class: prose
//! is correct at the moment of writing and decays silently afterwards, and the
//! only counts that do not decay are the ones a symbol derives.
//!
//! This is the same detector, not a second one. Two corpora, one rule.

use anvil::doc_guard::corpus_sync::{numeral, remaining_claim};
use anvil::pre_merge_guard::report::TOTAL_GATES;
use std::fs;
use std::path::{Path, PathBuf};

fn rust_sources() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::from("src")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "rs") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Doc comments and line comments, with quoted spans removed.
///
/// Code is excluded deliberately: `SECRET_RULES` may legitimately contain the
/// digits of a length bound, and a regex may contain anything. The claim this
/// gate judges is one made to a READER, and that lives in the commentary.
///
/// Backticked and double-quoted spans are removed for the opposite reason: a
/// count inside them is a CITATION, not a claim. `brand_absence` documents the
/// forbidden strings by quoting them -- ``70-Gate``, ``70 gates`` -- and
/// `report.rs` narrates the incident as `published "70 gates" in seven
/// PR-visible strings`. Judging a quotation as an assertion would force this
/// tree to stop being able to describe its own history, which is a worse
/// outcome than the drift.
fn commentary(path: &Path) -> String {
    let text = fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::trim_start)
        .filter(|l| l.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut out = String::with_capacity(text.len());
    let mut quote: Option<char> = None;
    for ch in text.chars() {
        match (quote, ch) {
            (None, '`' | '"') => quote = Some(ch),
            // A newline closes an unterminated quote, so one stray backtick
            // cannot blank the rest of the file and report it clean.
            (Some(_), '\n') => {
                quote = None;
                out.push('\n');
            }
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, c) => out.push(c),
        }
    }
    out
}

#[test]
fn no_doc_comment_publishes_a_stale_gate_count() {
    let files = rust_sources();
    assert!(
        files.len() > 50,
        "only {} source files were read; the scan did not cover what it claims",
        files.len()
    );

    let drift: Vec<String> = files
        .iter()
        .filter_map(|p| {
            remaining_claim(&commentary(p), TOTAL_GATES)
                .map(|why| format!("{}: {why}", p.display()))
        })
        .collect();

    assert!(
        drift.is_empty(),
        "{} doc comment(s) publish a gate count the code does not support:\n  {}\n\
         A count a symbol already derives has one honest home, and it is the symbol.",
        drift.len(),
        drift.join("\n  ")
    );
}

#[test]
fn the_detector_reads_spelled_out_counts_and_not_ordinary_words() {
    // The old detector had a hardcoded `\bsixty-gate\b` regex: an N+1 patch for
    // the one spelled-out number that had drifted, which by construction could
    // never catch the next one. Two of the three counts that actually rotted
    // were spelled out.
    assert_eq!(numeral("72"), Some(72));
    assert_eq!(numeral("sixty"), Some(60));
    assert_eq!(numeral("Seventy-Two"), Some(72));
    assert_eq!(numeral("eighteen"), Some(18));
    assert_eq!(numeral("ninety-nine"), Some(99));

    // Not numerals, so a word merely sitting in front of "gates" is not a claim.
    assert_eq!(numeral("pre-merge"), None);
    assert_eq!(numeral("the"), None);
    assert_eq!(numeral("twenty-ten"), None);
    assert_eq!(numeral("hundred"), None);
}

/// The English for a total under a hundred.
///
/// Exists so the fixtures below DERIVE the agreeing and disagreeing spellings
/// from `TOTAL_GATES` instead of hardcoding them. The comment promising that
/// was already there; the code said `"the seventy-four gates"`, so removing one
/// gate from the corpus broke a test about prose drift by drifting.
fn spell(n: usize) -> String {
    const ONES: [&str; 20] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    const TENS: [&str; 10] = [
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    if n < 20 {
        return ONES[n].to_string();
    }
    let (t, o) = (n / 10, n % 10);
    if o == 0 {
        TENS[t].to_string()
    } else {
        format!("{}-{}", TENS[t], ONES[o])
    }
}

#[test]
fn a_spelled_out_total_that_disagrees_is_caught() {
    assert!(remaining_claim("the sixty-gate matrix", TOTAL_GATES).is_some());
    assert!(remaining_claim("all seventy gates run", TOTAL_GATES).is_some());
    assert!(remaining_claim("all 70 gates run", TOTAL_GATES).is_some());
    // Derived from `TOTAL_GATES`, so a corpus that grows or shrinks does not
    // make the "agrees with the code" case unwritable — which is what the
    // hardcoded spelling did the first time a gate was removed.
    assert!(remaining_claim(&format!("the {} gates", spell(TOTAL_GATES)), TOTAL_GATES).is_none());
    assert!(
        remaining_claim(
            &format!("the {} gates", spell(TOTAL_GATES + 1)),
            TOTAL_GATES
        )
        .is_some()
    );
    assert!(remaining_claim("the pre-merge gates", TOTAL_GATES).is_none());
}

#[test]
fn a_subset_count_is_refused_even_when_its_total_is_right() {
    // This is the sentence that rotted. It contains a CORRECT total, so a
    // detector that only validates totals reads it as clean -- which is exactly
    // what happened for however long "thirty-seven" stood.
    let sentence = format!(
        "That exemption covers thirty-seven of the {} gates.",
        spell(TOTAL_GATES)
    );
    let why = remaining_claim(&sentence, TOTAL_GATES).expect("a subset count must be refused");
    assert!(why.contains("subset"), "{why}");

    // Correct arithmetic does not rescue it: no total can verify a subset, so
    // the shape is refused rather than checked.
    assert!(remaining_claim("eighteen of the seventy-two gates", TOTAL_GATES).is_some());
}

#[test]
fn a_small_cardinal_is_an_enumeration_not_a_claim() {
    // "the two gates this path used to assert absent" names a pair it has just
    // described. Treating it as a claim about the matrix would force ordinary
    // English out of the comments, or force an allowlist -- and an allowlist is
    // the N+1 shape this check exists to avoid.
    assert!(remaining_claim("the two gates this path asserts absent", TOTAL_GATES).is_none());
    assert_eq!(remaining_claim("the other two gates", TOTAL_GATES), None);
    assert!(remaining_claim("seven gates were rebuilt", TOTAL_GATES).is_none());

    // The threshold is stated, and above it the check is live. Twenty-one was a
    // real stale claim, in `--help`, where a user reads it.
    assert!(remaining_claim("Domain Gates on a PR (21 gates)", TOTAL_GATES).is_some());
}

#[test]
fn a_count_explicitly_marked_historical_is_allowed_to_stand() {
    // ADR-0001 and doctrine.md both narrate the founding number and hand the
    // authority to the symbol. That is the honest form of a stale count, and a
    // check that forbade it would make this tree unable to describe its past.
    assert!(
        remaining_claim(
            "The founding name said sixty gates. That number is historical. \
             The field list is the authority.",
            TOTAL_GATES
        )
        .is_none()
    );

    // The disclaimer must be adjacent. A page that says the word somewhere far
    // below does not get to publish a wrong number at the top.
    let far = format!(
        "We run sixty gates.{}Some of this is historical.",
        " ".repeat(400)
    );
    assert!(remaining_claim(&far, TOTAL_GATES).is_some());

    // And it is a disclaimer, not a blanket pardon: without it the same
    // sentence is drift.
    assert!(remaining_claim("The founding name said sixty gates.", TOTAL_GATES).is_some());
}
