//! The review must examine what Google's reviewer guidance names, not only
//! hold stances toward it.
//!
//! Sixteen lenses were all *stances* — ways of looking. Four of the ten named
//! *aspects* had no lens between them: tests, naming, comments, documentation.
//! Measured consequence: this review passed 15 of 16 lenses on a change
//! carrying five real defects, four of which were about the tests being wrong.

use anvil::reviewer::rubric::{REVIEW_ASPECTS, REVIEW_STANCES, rubric_prompt};

/// The ten named in `google.github.io/eng-practices/review/reviewer/looking-for.html`.
const NAMED_BY_THE_SOURCE: &[&str] = &[
    "Design",
    "Functionality",
    "Complexity",
    "Tests",
    "Naming",
    "Comments",
    "Style",
    "Consistency",
    "Documentation",
    "Every line",
];

#[test]
fn every_aspect_the_source_names_is_asked_for() {
    for want in NAMED_BY_THE_SOURCE {
        assert!(
            REVIEW_ASPECTS.iter().any(|(a, _)| a == want),
            "`{want}` is a named review aspect and nothing asks about it. A \
             reviewer can hold every stance in the rubric and still never look \
             at it — which is how four real defects about the tests passed 15 \
             of 16 lenses."
        );
    }
}

#[test]
fn the_prompt_actually_carries_them() {
    let prompt = rubric_prompt();
    for (aspect, ask) in REVIEW_ASPECTS {
        assert!(
            prompt.contains(aspect),
            "`{aspect}` is declared and never reaches the prompt"
        );
        assert!(
            prompt.contains(ask.split(&['?', '.'][..]).next().unwrap_or(ask).trim()),
            "`{aspect}` reaches the prompt as a bare label with nothing asked \
             of it, which is a heading rather than a question"
        );
    }
    for stance in REVIEW_STANCES {
        assert!(prompt.contains(stance), "a stance was dropped: {stance}");
    }
}

/// Aspects come first. The order is the argument: what to examine decides
/// what gets seen; how to look only shapes what is made of it.
#[test]
fn what_to_examine_is_stated_before_how_to_look() {
    let prompt = rubric_prompt();
    let lowered = prompt.to_lowercase();
    let aspects_at = lowered.find("must examine").expect("aspects section");
    let stances_at = lowered.find("how to look").expect("stances section");
    assert!(
        aspects_at < stances_at,
        "the stances are stated before the aspects, so the reviewer is told \
         how to look before it is told what at"
    );
}

/// The count in the prose must be the count in the list. A rubric that says
/// sixteen and ships fifteen is the stale-count defect this repo already
/// gates elsewhere.
#[test]
fn the_stated_lens_count_matches_the_list() {
    let prompt = rubric_prompt();
    assert!(
        prompt.contains(&format!("{}-Lens", REVIEW_STANCES.len())),
        "the rubric names a lens count that is not {}",
        REVIEW_STANCES.len()
    );
}

/// The Tests aspect must ask the question that matters, not merely mention
/// tests. "Are there tests" is satisfied by a test that cannot fail.
#[test]
fn the_tests_aspect_asks_whether_a_test_could_fail() {
    let (_, ask) = REVIEW_ASPECTS
        .iter()
        .find(|(a, _)| *a == "Tests")
        .expect("Tests aspect");
    let lowered = ask.to_lowercase();
    assert!(
        lowered.contains("fail"),
        "the Tests aspect asks for the presence of tests and not whether they \
         would FAIL if the code were wrong. Every defect found in this \
         codebase by seeding one was invisible to the first question: {ask}"
    );
}
