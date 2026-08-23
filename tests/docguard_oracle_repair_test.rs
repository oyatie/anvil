//! The certification oracle must not lie about, or vandalise, documentation.
//!
//! Issue #27: the corpus sync rewrites the gate counts of whatever repository
//! it is pointed at, using Anvil's private `TOTAL_GATES`, and the pipeline then
//! commits and pushes the result onto the contributor's branch.
//!
//! Issue #28: the exemption-marker deletion in `rewrite_page` takes its start
//! from the previous sentence boundary but its end from end-of-line, so it
//! destroys any prose that follows the marker on the same line and fuses the
//! surviving prefix with the next line.
//!
//! Issue #29's live path sits behind the `agy` doc-parity probe, so **no test in
//! this file calls `DocGuard::ensure_documentation_parity` at all**. Every case
//! here drives either a pure function (`corpus_sync::sync_published_counts`,
//! `evaluator::doc_parity_status`) or the exemption rewriter through the sync,
//! or — in the last section — `PreMergeGuard::evaluate_pre_merge_gates`, which
//! is arithmetic over the sixty-nine guard reports it is handed plus two gates
//! that read Anvil's own source tree. None of them can reach the probe, so none
//! of them can spawn `agy`.
//!
//! ## Why the gate cases are not in this binary
//!
//! They used to be, routed through `DocGuard::with_probe_override`, and this
//! header claimed that made an `agy` spawn "structurally unreachable rather than
//! merely unlikely". That was false, and it was the kind of false assurance this
//! branch exists to remove. `Probe` constrains where the probe's outcome is
//! *stored*; it does not oblige `evaluate_doc_parity` to read it. An implementer
//! who writes `Probe::Overridden(_) => "low".to_string()` — meaning to consult
//! the override somewhere else, and missing a path — turns a parallel test
//! binary into up to eighteen concurrent invocations of
//! `agy --print <prompt> --effort low --dangerously-skip-permissions`, each on a
//! 120-second budget, because `agy` is resolved through the inherited `PATH` and
//! is installed on developer machines.
//!
//! So those cases moved to `tests/docguard_oracle_repair_gate_test.rs`, which
//! empties `PATH` before any gate call. A fall-through spawn there fails in
//! microseconds instead of invoking a model. `PATH` is process-global, so that
//! binary contains exactly one `#[test]` — mutating the environment beside
//! parallel readers is a data race, which is the same reason
//! `tests/docguard_oracle_repair_probe_seam_test.rs` and
//! `tests/docguard_oracle_repair_self_repo_test.rs` are binaries of their own.
//!
//! ## The probe seam is still part of the specification
//!
//! `DocGuard::with_probe_override` supplies the *outcome* the `agy` probe would
//! have produced — `Ok(judgement)` or `Err(reason)`. Its signature is pinned by
//! this suite. It carried an `agy_effort` parameter as originally specified,
//! dropped on the floor behind an `#[allow(unused_variables)]`; that is a named
//! input nothing measures, which is the defect class this branch exists to
//! remove, so review directed it be deleted and the fixtures changed with it.
//! The `Err` arm carries as much weight as the `Ok` arm: it is the arm
//! whose historical collapse into `is_doc_sufficient: true` made gate 1
//! unfailable (the comment recording it still sits in `evaluate_doc_parity`), so
//! `Err(reason)` must be delivered as an `Err` out of `evaluate_doc_parity` —
//! the same path a real probe failure takes.
//!
//! The override must be consulted **inside `evaluate_doc_parity`**, and it is a
//! stored value rather than a slot that empties. Both requirements, and the
//! cases that fence them, live with the gate cases in
//! `tests/docguard_oracle_repair_gate_test.rs` and
//! `tests/docguard_oracle_repair_probe_seam_test.rs`.
//!
//! ## Which of these cases is red evidence, and which is a fence
//!
//! This file was written before the implementation existed. At test-review time
//! it reported 9 passed and 19 failed, and the nine were disclosed as fences
//! rather than red evidence. That disclosure is kept here in its measured form,
//! now that the implementation exists and every case is green: red-by-assertion
//! is claimed only where a mutant demonstrates it.
//!
//! MEASURED — each mutant applied alone to the repaired implementation, run, and
//! reverted; all six compile and all six are killed:
//!
//! * The exemption deletion taking the LINE instead of the sentence (issue #28)
//!   turns THIRTEEN cases in this file red, on their own assertions. That is the
//!   bulk of the red evidence here, and it is the defect the pre-#12 README
//!   reproduces.
//! * `is_anvils_own_repository -> true` (issue #27) turns four red:
//!   `a_corpus_sync_that_did_not_apply_says_so_instead_of_passing_silently`,
//!   `a_slug_that_merely_resembles_anvils_is_still_somebody_elses_repository`,
//!   `the_corpus_sync_rewrites_anvils_own_published_counts_but_not_a_watched_repositorys`,
//!   `anvil_is_recognised_case_insensitively_and_only_as_a_whole_slug`.
//! * Reading `files_created_or_updated` before `is_sufficient` at
//!   `evaluator::doc_parity_status` — the live defect on `main` — turns two red:
//!   `a_diff_the_probe_judged_under_documented_does_not_certify_because_a_stub_was_written`
//!   and
//!   `a_stub_written_for_an_under_documented_diff_does_not_certify_through_the_evaluator`.
//!   The second is the one that closes the requirement the first cannot: that
//!   `evaluate_pre_merge_gates` keeps reaching gate 1's verdict through
//!   `doc_parity_status` rather than through a second, private copy.
//! * The three remaining mutants (`is_sufficient: true` on the generate path,
//!   `let _ = tokio::fs::write(..)`, and a never-amended file pushed onto the
//!   updated list) turn nothing in this file red. They need the gate, and they
//!   are killed in `tests/docguard_oracle_repair_gate_test.rs`.
//!
//! FENCES, disclosed: the four `classify_probe_output` cases at the end of this
//! file and four of the five `DocGuardReport` -> `GateStatus` mapping cases —
//!
//! * `a_diff_..._does_not_certify_when_no_file_was_written`
//! * `a_probe_that_produced_no_judgement_does_not_certify`
//! * `an_errored_gate_does_not_certify_even_when_a_page_was_rewritten`
//! * `a_sufficient_diff_certifies_and_a_rewritten_owned_page_does_not_block_it`
//!
//! — pin arms that were already correct before this branch. Both
//! `doc_parity_status` and `classify_probe_output` were EXTRACTED from
//! production verbatim, defect and all, rather than being written here; the
//! extraction is disclosed at each of them. The last of the four is the
//! counterweight that stops "never accept a non-empty file list" from being the
//! cheapest repair. Their falsifiability is not assumed either: dropping the
//! `Errored` arm fails two of them, turning the `AutoUpdated` arm into `Failed`
//! fails the fourth, and blocking with an empty reason fails the fifth case's
//! pass-through assertion.
//!
//! ## Ownership is a compile-time constant
//!
//! Which repository is Anvil's own is settled by this suite as a property of the
//! build, not of the process environment. The companion binary
//! `tests/docguard_oracle_repair_self_repo_test.rs` pins it: with
//! `SELF_REPO=oyatie/console` set, `oyatie/console` is still skipped and
//! `oyatie/anvil` is still synced. It lives in its own test binary because it
//! mutates the environment, which is a data race in a parallel one.

use anvil::doc_guard::corpus_sync::sync_published_counts;
use anvil::doc_guard::{DocGuardReport, classify_probe_output};
use anvil::git_manager::PrDiffContext;
use anvil::pre_merge_guard::evaluator::{PreMergeGuard, doc_parity_status};
use anvil::pre_merge_guard::report::{GateStatus, PreMergeCertificationReport, TOTAL_GATES};
use std::path::Path;
use tempfile::tempdir;

/// Anvil's own repository: the only one whose published gate counts are
/// `TOTAL_GATES`.
///
/// The literal slug is deliberate, and it is a decision rather than a deferral.
/// `Config::self_repo` reads `SELF_REPO` from the environment, and
/// `Config::from_env` calls `dotenvy::dotenv()`, which mutates the process
/// environment. Resolving ownership that way would make these cases depend on
/// whatever `.env` sits in the developer's working directory and would make a
/// mis-set `SELF_REPO` rewrite and push a watched repository's docs — issue #27
/// verbatim. See `tests/docguard_oracle_repair_self_repo_test.rs`.
///
/// STATED CONSEQUENCE of that decision, recorded so it reads as a known cost
/// rather than an oversight: ownership is a compile-time constant in the
/// implementation too, so renaming or moving this repository switches gate 1's
/// corpus enforcement off — silently, with this whole suite still green, because
/// every case here supplies the slug it expects. The failure direction is safe
/// (Anvil stops repairing its own pages; no watched repository is ever
/// corrupted), and a test cannot distinguish "renamed" from "correctly scoped"
/// without inventing a second source of truth for the name. A rename must
/// therefore update the constant in the same commit.
const ANVIL: &str = "oyatie/anvil";

/// The same repository, in the case a GitHub event might deliver it in.
/// See `anvil_is_recognised_case_insensitively_and_only_as_a_whole_slug`.
const ANVIL_SLUGS: &[&str] = &["oyatie/anvil", "Oyatie/Anvil"];

/// Two of the repositories Anvil reviews. `TOTAL_GATES` is meaningless in both.
const WATCHED: &[&str] = &["oyatie/oyatie", "oyatie/console"];

/// Slugs that are near misses for Anvil's own. Every one of them belongs to
/// somebody else, so the sync must treat them exactly as it treats
/// `oyatie/console`. A substring, prefix, suffix, or owner-blind predicate
/// separates `oyatie/anvil` from `oyatie/console` and still corrupts these.
const NEAR_MISSES: &[&str] = &[
    "oyatie/anvil-sdk",
    "oyatie/anvildocs",
    "attacker/anvil",
    // `"oyatie/anvil"` as a proper SUFFIX of somebody else's slug. Every other
    // entry here dies to a `contains`, a `starts_with` or an owner-blind
    // predicate, but an `ends_with("oyatie/anvil")` survives all of them and
    // hands this repository Anvil's corpus rules.
    "notoyatie/anvil",
    // No owner at all: not a slug Anvil can recognise as its own.
    "anvil",
    // Nothing is known about the repository under review — and a slug that is
    // only whitespace is the shape a trimmed-but-unchecked event payload
    // produces, which a `repo.trim() == ""`-blind predicate then compares
    // against nothing.
    "   ",
    // The boundary: an empty slug.
    "",
];

/// Every page `collect_owned_pages` claims, so a partial skip is caught too.
const OWNED_PAGES: &[&str] = &[
    "README.md",
    "docs/doctrine.md",
    "openapi/openapi.yaml",
    "docs/adr/0001-console.md",
    "docs/decisions/0001-console.md",
];

/// Pages that exist in Anvil's own checkout and are deliberately **not** part
/// of the corpus the sync owns.
///
/// The counter-pressure to `OWNED_PAGES`, and until now the suite had none of
/// it on Anvil's own side: every Anvil fixture in every one of the four binaries
/// wrote only paths that were already owned, so the pressure ran one way —
/// "reach every owned page" — with nothing at all fencing what must be left
/// alone. The cheapest way to satisfy that one-directional pressure is to stop
/// maintaining a list and walk the checkout instead (`walkdir`, or a hand-rolled
/// `read_dir` recursion filtering `*.md` / `*.yaml`), which reaches every owned
/// page by construction and passes every assertion in every binary — because
/// under it the tempdir contains nothing BUT owned pages, so even
/// `assert_eq!(sync.rewritten.len(), OWNED_PAGES.len())` still matches.
///
/// What that ships is issue #27's harm aimed inward: on Anvil's own pull
/// requests the sync rewrites gate-count claims in, and deletes exemption
/// sentences from, `CHANGELOG.md`, `.github/**`, and every `docs/**` page that
/// is not doctrine — and the pipeline commits and pushes those edits onto the
/// contributor's branch.
///
/// Each of the four kills a different way of not maintaining a list, and each
/// exists because the ones before it do not.
///
/// * `CHANGELOG.md` is chosen deliberately: it is named in the historical
///   exemption sentence itself (`such as README.md or CHANGELOG.md`), so
///   excluding it is a real decision about the corpus boundary rather than an
///   arbitrary path. It is at the checkout root, so it kills a walk rooted
///   there.
/// * The nested `docs/notes/roadmap.md` sits under `docs/`, beside the two
///   directories `collect_owned_pages` really does enumerate, which is exactly
///   where a recursive walk stops being distinguishable from the fixed list.
/// * `docs/runbook.md` is a SIBLING of `docs/doctrine.md` — a `*.md` sitting
///   directly under `docs/`, at the same depth as an owned page and in the same
///   directory. It is here because the other two leave the second natural way
///   to stop maintaining a list completely alive: not a recursive walk but a
///   SHALLOW glob of the directories the corpus already lives in —
///   `README.md`, `openapi/openapi.yaml`, every `*.md` directly under `docs/`,
///   plus the two ADR directories. That reaches all five `OWNED_PAGES`
///   (`docs/doctrine.md` sits directly under `docs/`), never touches a root
///   file, and never descends into `docs/notes/`, so `CHANGELOG.md` and
///   `docs/notes/roadmap.md` both survive it and every byte-identity and
///   `rewritten.len()` assertion in all four binaries holds.
///
/// * `openapi/components.yaml` is a SIBLING of `openapi/openapi.yaml` — the
///   same shape of entry as `docs/runbook.md`, aimed at the OTHER directory the
///   corpus lives in, and it is here because the first three leave that
///   directory completely unfenced. `OWNED` names exactly one page under
///   `openapi/`, so a glob of `openapi/*.yaml` (or `openapi/**`) beside the
///   README, doctrine and the two ADR directories reaches all five
///   `OWNED_PAGES`, never touches a root file, never descends into
///   `docs/notes/`, and never sees `docs/runbook.md`. Every byte-identity
///   assertion, every `!rewritten.contains(unowned)` assertion and every
///   `rewritten.len() == OWNED_PAGES.len()` assertion in all four binaries
///   holds under it.
///
/// In the live checkout both of those globs are currently INDISTINGUISHABLE
/// from the fixed list, which is what makes them dangerous rather than merely
/// wrong: `docs/` today contains only `doctrine.md`, `adr/` and `decisions/`,
/// and `openapi/` today contains only `openapi.yaml`. The moment anyone adds
/// `docs/runbook.md`, or `openapi/webhooks.yaml`, or any other page beside the
/// ones the corpus names, the sync starts rewriting that page's gate-count
/// claims and deleting its exemption sentences on every Anvil pull request, the
/// page is reported in `files_created_or_updated` as a documentation update, and
/// the pipeline commits and pushes the edit onto the contributor's branch. That
/// is issue #27's harm aimed inward — latent, silent, and triggered by an
/// ordinary documentation commit rather than by any change to this code.
///
/// All four carry the same drifting bytes as the owned pages, written in the
/// same checkout in the same run, so the only thing that can separate any of
/// them from `docs/doctrine.md` or `openapi/openapi.yaml` is the corpus boundary
/// this constant fences.
const NOT_OWNED_PAGES: &[&str] = &[
    "CHANGELOG.md",
    "docs/notes/roadmap.md",
    "docs/runbook.md",
    "openapi/components.yaml",
];

const EXEMPTION_MARKER: &str = "does **not** yet amend existing documents";
const PLAIN_EXEMPTION_MARKER: &str = "does not yet amend existing documents";

/// The reason a probe gives for judging a diff under-documented. Held as a
/// constant so the assertions that it reaches `DocGuardReport::summary` pin
/// pass-through rather than wording.
const MISSING_REASON: &str = "newly_public is a new public API with no reference page";

/// The five ways `evaluate_doc_parity` can come back with no judgement at all.
/// The strings are the shapes the real call site produces (spawn failure,
/// non-zero exit, timeout, unparseable output, watchdog supervision failure);
/// their wording is the test's, and it is only ever asserted as pass-through.
///
/// These strings therefore say nothing about whether the product produces `Err`
/// in those five situations — they are the CONSUMPTION side only. The production
/// side is pinned separately and against the product's own output, by
/// `classify_probe_output` in the last section of this file and by
/// `a_live_probe_that_could_not_run_is_errored_through_the_real_call_path` in
/// `tests/docguard_oracle_repair_gate_test.rs`.
const PROBE_FAILURES: &[&str] = &[
    "failed to run doc parity probe: No such file or directory (os error 2)",
    "doc parity probe exited with status exit status: 1: permission check failed for command",
    "doc parity probe timed out after 120s",
    "doc parity probe returned no parseable evaluation (stdout 4096 bytes)",
    "doc parity probe supervision failed: watchdog channel closed",
];

/// An Anvil page publishing claims that are deliberately *not* `TOTAL_GATES`,
/// so the fixture stays a drift fixture whatever `TOTAL_GATES` becomes. Used
/// only where the repository under review **is** Anvil's.
///
/// Both of `rewrite_page`'s count mutations are represented: the numeric
/// gate-count rewrite and the `sixty-gate` rewrite. The second has no other
/// positive coverage anywhere — the in-module unit tests only use the digit
/// form, and the live corpus contains no `sixty-gate` occurrence for
/// `published_*_matches_live_corpus` to catch — so without it an implementer
/// scoping `rewrite_page` could delete the `sixty_regex` replacement outright
/// while keeping `remaining_claim`'s `sixty-gate` check, leaving Anvil's own
/// gate 1 to hard-fail the moment any owned page reintroduces the word.
fn drifting_page() -> String {
    format!(
        "# Anvil\n\
         \n\
         The fabric ships behind a {}-gate release check.\n\
         It replaced the sixty-gate pilot programme.\n",
        TOTAL_GATES + 1
    )
}

/// The same drift as `drifting_page`, plus an exemption sentence, so one
/// fixture exercises **all three** of `rewrite_page`'s mutations on a page of
/// Anvil's own — the positive-direction mirror of `watched_repo_page`.
///
/// Used by `the_corpus_sync_rewrites_every_owned_page_not_only_the_readme` to
/// drive every entry of `OWNED_PAGES` at once. `OWNED_PAGES` was until now only
/// ever used against repositories that are NOT Anvil's; on Anvil's side every
/// fixture in every binary wrote to `README.md` and nothing else, so narrowing
/// what the sync actually rewrites — `if rel != "README.md" { continue; }`, or
/// applying the exemption deletion only to `.md` pages — passed the whole suite
/// while leaving Anvil's own doctrine, OpenAPI document and ADRs publishing
/// stale counts and surviving markers.
fn drifting_page_with_exemption() -> String {
    format!(
        "# Anvil\n\
         \n\
         The fabric ships behind a {}-gate release check.\n\
         It replaced the sixty-gate pilot programme.\n\
         Roadmap. DocGuard does **not** yet amend existing documents such as `README.md`. Support is planned.\n",
        TOTAL_GATES + 1
    )
}

/// A page belonging to a repository that is **not** Anvil's.
///
/// `rewrite_page` performs three independent mutations on a page it owns: the
/// gate-count rewrite, the `sixty-gate` rewrite, and the exemption-sentence
/// deletion. A fixture whose only mutable content is a gate count would let an
/// implementation scope the count rewrite by `repo` and leave the other two
/// unscoped — which still deletes a sentence from a watched repository's README
/// and pushes it to the contributor's branch, the precise harm issue #27
/// describes. This fixture triggers all three, so `assert_eq!(got, page)` means
/// what it says: *no owned page in that repository is modified*.
fn watched_repo_page() -> String {
    format!(
        "# Console\n\
         \n\
         The console ships behind a {}-gate release check.\n\
         It replaced the sixty-gate pilot programme.\n\
         Roadmap. The console does **not** yet amend existing documents such as `README.md`. Support is planned.\n\
         \n\
         None of this is Anvil's corpus.\n",
        TOTAL_GATES + 1
    )
}

/// The real pre-#12 README table, verbatim (`git show 508a66e^:README.md`).
/// The exemption marker sits mid-line, inside the DocGuard row's cell — the
/// only layout these markers were ever written for.
const HISTORICAL_README_TABLE: &str = "| Quality Gate | Description |\n\
|---|---|\n\
| **📚 Documentation & ADR Parity** | Verifies public APIs and platform doctrine, and creates missing ADRs (`DocGuard`). Note: it does **not** yet amend existing documents such as `README.md` or `CHANGELOG.md` — see the roadmap. |\n\
| **🛡️ Cedar Policy & IAM Boundaries** | Verifies AWS Cedar authorization policy coverage & tenant bounds (`CedarGuard`) |\n";

const DOCGUARD_ROW_PREFIX: &str = "| **📚 Documentation & ADR Parity** |";

/// The DocGuard row with the exemption *sentence* gone — not merely the marker
/// phrase. `Note: it` opens that sentence and `— see the roadmap.` closes it;
/// both belong to it and both go.
const DOCGUARD_ROW_AFTER: &str = "| **📚 Documentation & ADR Parity** | Verifies public APIs and platform doctrine, and creates missing ADRs (`DocGuard`). |";

const CEDAR_ROW: &str = "| **🛡️ Cedar Policy & IAM Boundaries** | Verifies AWS Cedar authorization policy coverage & tenant bounds (`CedarGuard`) |";

fn write(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, body).unwrap();
}

/// Collapses runs of whitespace inside each line to a single space and trims
/// the line.
///
/// Assertions compare normalised text so they pin *which words survive* without
/// dictating whether the implementation leaves one space or two where a
/// sentence used to be. Line count and line boundaries are asserted separately
/// and unnormalised, because those are load-bearing.
fn normalise(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

/// Runs the sync over a single Anvil-owned page at `rel` and returns the bytes
/// left on disk together with the reported drift.
///
/// Parameterised by relative path rather than hardcoded to `README.md` so the
/// issue-#28 fixtures can be driven through a page that is **not** markdown.
/// `OWNED_PAGES` carries five kinds and `rewrite_page` is handed all five, so a
/// rewriter that grows a markdown-table-aware sentence scanner for #28 and then
/// applies the exemption deletion only to `.md` pages leaves
/// `openapi/openapi.yaml` publishing the marker forever — see
/// `the_exemption_rewriter_is_not_scoped_to_markdown_pages`.
fn rewrite_anvil_page(rel: &str, body: &str) -> (String, Vec<String>) {
    let dir = tempdir().unwrap();
    write(&dir.path().join(rel), body);
    let sync = sync_published_counts(ANVIL, dir.path(), TOTAL_GATES).unwrap();
    let got = std::fs::read_to_string(dir.path().join(rel)).unwrap();
    (got, sync.remaining_drift)
}

/// `rewrite_anvil_page` on Anvil's own `README.md`, which is the page most of
/// the issue-#28 layouts came off.
fn rewrite_anvil_readme(body: &str) -> (String, Vec<String>) {
    rewrite_anvil_page("README.md", body)
}

/// Builds `docs/adr` with one ADR in it and then makes the directory itself
/// unreadable, so the sync's **first** filesystem read fails: not the per-page
/// `read_to_string` loop, but `collect_owned_pages`'s `read_dir`, which runs
/// before it.
///
/// Returns the directory's original permissions so the caller can restore them
/// (an unreadable directory defeats `TempDir`'s cleanup), or `None` when this
/// process is privileged enough to read a `0o000` directory — under a root
/// container the fixture cannot be built at all, and the caller says so rather
/// than passing vacuously.
///
/// STATED COST: this fixture is unix-only, and under a root CI container it
/// fails with a diagnostic instead of running. Both are recorded rather than
/// worked around, because the alternative — skipping quietly — would leave the
/// read path looking fenced when only one of its two branches was tested.
#[cfg(unix)]
fn make_adr_dir_unreadable(repo_dir: &Path) -> Option<std::fs::Permissions> {
    use std::os::unix::fs::PermissionsExt;

    let adr = repo_dir.join("docs/adr");
    std::fs::create_dir_all(&adr).unwrap();
    std::fs::write(adr.join("0001-x.md"), "# ADR\n").unwrap();

    let original = std::fs::metadata(&adr).unwrap().permissions();
    std::fs::set_permissions(&adr, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_dir(&adr).is_ok() {
        std::fs::set_permissions(&adr, original).unwrap();
        return None;
    }
    Some(original)
}

/// Restores `docs/adr`'s permissions on drop, so a panic *between* the fixture
/// being built and the assertions running cannot leave a `0o000` directory
/// behind — `TempDir`'s own cleanup cannot remove one, so every such panic
/// otherwise leaks an undeletable directory into `TMPDIR`.
///
/// A plain restore call at the end of the case is not enough, and during the red
/// phase it is actively wrong: a byte-index bug in the new `rewrite_page` panics
/// *inside* `sync_published_counts`, which is upstream of any line the case
/// could put the restore on. The gate binary already uses this shape; both
/// places now do.
#[cfg(unix)]
struct RestoreAdrDir<'a> {
    repo_dir: &'a Path,
    original: Option<std::fs::Permissions>,
}

#[cfg(unix)]
impl Drop for RestoreAdrDir<'_> {
    fn drop(&mut self) {
        if let Some(original) = self.original.take() {
            let _ = std::fs::set_permissions(self.repo_dir.join("docs/adr"), original);
        }
    }
}

// =========================================================================
// Issue #27 — the corpus sync is scoped to Anvil's own repository
// =========================================================================

#[test]
fn the_corpus_sync_rewrites_anvils_own_published_counts_but_not_a_watched_repositorys() {
    let anvil_dir = tempdir().unwrap();
    write(&anvil_dir.path().join("README.md"), &drifting_page());

    let anvil = sync_published_counts(ANVIL, anvil_dir.path(), TOTAL_GATES).unwrap();
    let anvil_readme = std::fs::read_to_string(anvil_dir.path().join("README.md")).unwrap();

    assert_eq!(
        anvil.rewritten,
        vec!["README.md".to_string()],
        "Anvil's own published counts are still the sync's business"
    );
    // The page published two gate-count claims — `TOTAL_GATES + 1` in digits and
    // `sixty-gate` spelled out — and both are claims about the same number. Both
    // must still be *there* afterwards, and both must now read `TOTAL_GATES`.
    //
    // Counted rather than matched literally, and case-insensitively, because the
    // behaviour is "every published claim now states TOTAL_GATES", not "the
    // rewriter capitalises the way today's `sixty_regex` replacement happens to".
    // A rewriter that normalises `sixty-gate` to `72-gate` instead of `72-Gate`
    // is different but correct and must pass; a rewriter that *deletes* the
    // spelled-out claim rather than repairing it is not, and the count is what
    // separates them — `!contains("sixty-gate")` alone accepts deletion, and
    // `contains("{TOTAL_GATES}-gate")` alone is already satisfied by the digit
    // claim's repair.
    let lowered = anvil_readme.to_lowercase();
    let repaired = format!("{TOTAL_GATES}-gate");
    assert_eq!(
        lowered.matches(&repaired).count(),
        2,
        "both published gate-count claims must survive the rewrite and both must \
         now read TOTAL_GATES={TOTAL_GATES}; deleting a claim is not repairing \
         it: {anvil_readme}"
    );
    assert!(
        !lowered.contains(&format!("{}-gate", TOTAL_GATES + 1)),
        "the drifting digit claim must be gone: {anvil_readme}"
    );
    // `remaining_claim` fails the gate on a surviving `sixty-gate`, so a rewriter
    // that stops repairing it turns the word into an unfixable hard failure on
    // Anvil's own PRs.
    assert!(
        !lowered.contains("sixty-gate"),
        "no page of Anvil's may go on publishing `sixty-gate`: {anvil_readme}"
    );
    assert!(
        anvil.remaining_drift.is_empty(),
        "every claim on this page is one the rewriter knows how to repair: {:?}",
        anvil.remaining_drift
    );
    assert!(
        anvil.not_applicable.is_none(),
        "the sync did apply to Anvil, so it must not report otherwise: {:?}",
        anvil.not_applicable
    );

    let page = watched_repo_page();
    for repo in WATCHED {
        let dir = tempdir().unwrap();
        for owned in OWNED_PAGES {
            write(&dir.path().join(owned), &page);
        }

        let sync = sync_published_counts(repo, dir.path(), TOTAL_GATES).unwrap();

        for owned in OWNED_PAGES {
            let got = std::fs::read_to_string(dir.path().join(owned)).unwrap();
            assert_eq!(
                got, page,
                "{repo}: {owned} is not Anvil's page and TOTAL_GATES says nothing \
                 about it, so the sync must leave it byte-identical — not its gate \
                 count, not its `sixty-gate` phrasing, and not its prose"
            );
        }
        assert!(
            sync.rewritten.is_empty(),
            "{repo}: nothing in another repository may be reported as rewritten: {:?}",
            sync.rewritten
        );
        assert!(
            sync.remaining_drift.is_empty(),
            "{repo}: another repository's gate counts are not drift against \
             Anvil's TOTAL_GATES and must not fail its PR: {:?}",
            sync.remaining_drift
        );
    }
}

#[test]
fn the_corpus_sync_rewrites_every_owned_page_not_only_the_readme() {
    // The positive-direction mirror of the `OWNED_PAGES` loops above, and until
    // now the suite had none: every Anvil fixture in every one of the four
    // binaries wrote drift to `README.md` alone, and `rewrite_anvil_readme` —
    // the helper the whole of issue #28 runs through — was hardcoded to it.
    //
    // The wrong implementation that closed, and it is not contrived: while
    // restructuring `sync_published_counts` around the ownership early-return
    // required by issue #27, narrow what actually gets rewritten —
    // `if rel != "README.md" { continue; }`, or
    // `const OWNED: &[&str] = &["README.md"];`, or (the more natural variant once
    // `rewrite_page` grows a markdown-table-aware sentence scanner for #28)
    // apply the rewrite only to `.md` pages so `openapi/openapi.yaml` is skipped.
    // Every assertion in every one of the four binaries passed that, because
    // nothing on Anvil's side ever handed the sync a second page.
    //
    // What ships: Anvil's own `docs/doctrine.md`, `openapi/openapi.yaml` and ADRs
    // go on publishing stale gate counts and surviving exemption markers,
    // `remaining_claim` never sees them because the rewriter never reached them,
    // and gate 1 reports the corpus clean — the exact silent-drift defect
    // `corpus_sync` was written to end. The `docs/adr` fence elsewhere in this
    // file does NOT catch it: it only requires `collect_owned_pages` to keep
    // LISTING the ADR directories, never that their contents are rewritten.
    //
    // The counter-pressure runs in the SAME sync: `NOT_OWNED_PAGES` carries the
    // identical drifting bytes, in the same checkout, so the only thing that can
    // separate them is the corpus boundary itself. Without it this case pushes
    // one way only — "reach every owned page" — and the cheapest way to satisfy
    // that is to stop maintaining a list and walk the checkout instead, which
    // passes every assertion in all four binaries while rewriting Anvil's
    // CHANGELOG, its `.github/` pages and its `docs/` notes on every pull
    // request. See `NOT_OWNED_PAGES`.
    let page = drifting_page_with_exemption();
    let dir = tempdir().unwrap();
    for owned in OWNED_PAGES {
        write(&dir.path().join(owned), &page);
    }
    for unowned in NOT_OWNED_PAGES {
        write(&dir.path().join(unowned), &page);
    }

    let sync = sync_published_counts(ANVIL, dir.path(), TOTAL_GATES).unwrap();

    assert!(
        sync.not_applicable.is_none(),
        "the sync did apply to Anvil, so it must not report otherwise: {:?}",
        sync.not_applicable
    );
    assert!(
        sync.remaining_drift.is_empty(),
        "every claim on every one of these pages is one the rewriter knows how to \
         repair: {:?}",
        sync.remaining_drift
    );

    // Reported as a set, not a sequence: `collect_owned_pages` sorts, but the
    // order it reports in is not a behaviour this suite has any reason to pin.
    for owned in OWNED_PAGES {
        assert!(
            sync.rewritten.contains(&(*owned).to_string()),
            "{owned} is one of Anvil's own published pages and it carried drift, so \
             the sync must report having rewritten it: {:?}",
            sync.rewritten
        );
    }
    assert_eq!(
        sync.rewritten.len(),
        OWNED_PAGES.len(),
        "exactly the pages that were written may be reported: {:?}",
        sync.rewritten
    );

    let repaired = format!("{TOTAL_GATES}-gate");
    for owned in OWNED_PAGES {
        let got = std::fs::read_to_string(dir.path().join(owned)).unwrap();
        let lowered = got.to_lowercase();

        assert_eq!(
            lowered.matches(&repaired).count(),
            2,
            "{owned}: both published gate-count claims must survive the rewrite and \
             both must now read TOTAL_GATES={TOTAL_GATES}: {got}"
        );
        assert!(
            !lowered.contains(&format!("{}-gate", TOTAL_GATES + 1)),
            "{owned}: the drifting digit claim must be gone: {got}"
        );
        assert!(
            !lowered.contains("sixty-gate"),
            "{owned}: no page of Anvil's may go on publishing `sixty-gate`: {got}"
        );
        assert!(
            !got.contains(EXEMPTION_MARKER),
            "{owned}: the exemption marker must be removed from every owned page, \
             not only from the one the fixtures happened to use: {got}"
        );
        assert!(
            got.contains("Roadmap.") && got.contains("Support is planned."),
            "{owned}: the prose either side of the exemption sentence must survive \
             on every owned page: {got}"
        );
        assert_eq!(
            got.lines().count(),
            page.lines().count(),
            "{owned}: line structure must be preserved: {got}"
        );
    }

    // And the other direction, in the same run and on the same bytes: a page of
    // Anvil's that the corpus does not own is not the sync's to edit either.
    for unowned in NOT_OWNED_PAGES {
        let got = std::fs::read_to_string(dir.path().join(unowned)).unwrap();
        assert_eq!(
            got, page,
            "{unowned} is not one of Anvil's published corpus pages. It carries the \
             same drifting claims and the same exemption sentence as the five that \
             are, so the only thing that may separate them is the corpus boundary — \
             and a sync that walks the checkout instead of enumerating the corpus \
             rewrites this file, and the pipeline commits and pushes that edit onto \
             the contributor's branch. That is issue #27's harm, aimed at Anvil"
        );
        assert!(
            !sync.rewritten.contains(&(*unowned).to_string()),
            "{unowned} is not owned, so it may not be reported as rewritten — a \
             non-empty file list is read as AutoUpdated at gate 1 and AutoUpdated \
             certifies: {:?}",
            sync.rewritten
        );
        assert!(
            !sync.remaining_drift.iter().any(|d| d.contains(unowned)),
            "{unowned} is not measured against Anvil's TOTAL_GATES, so its counts \
             are not drift and must never fail a pull request: {:?}",
            sync.remaining_drift
        );
    }
}

#[test]
fn the_exemption_rewriter_is_not_scoped_to_markdown_pages() {
    // `openapi/openapi.yaml` is an owned page and it is not markdown. Issue #28's
    // repair replaces the end-of-line scan with a sentence scan that has to
    // understand `|` as a markdown cell delimiter, and the natural way to keep
    // that from misfiring on other formats is to gate the whole deletion on
    // `rel.ends_with(".md")`. Anvil's OpenAPI document then publishes the
    // exemption forever, `remaining_claim` never runs on it because the rewriter
    // skipped it, and gate 1 calls the corpus clean.
    //
    // The fixture is shaped like the file it stands for — a YAML block scalar,
    // indented, with no table row anywhere near it — so it also pins that the
    // sentence scan does not depend on markdown structure being present.
    let page = format!(
        "openapi: 3.1.0\n\
         info:\n\
         \x20 title: Anvil\n\
         \x20 description: |\n\
         \x20   Runs the {}-gate certification. DocGuard does **not** yet amend existing documents. Support is planned.\n",
        TOTAL_GATES + 1
    );
    let (got, remaining_drift) = rewrite_anvil_page("openapi/openapi.yaml", &page);

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption marker must be removed from a page that is not markdown \
         too: {got:?}"
    );
    assert!(
        got.contains(&format!("{TOTAL_GATES}-gate")),
        "the gate-count claim on a page that is not markdown must be repaired \
         too: {got:?}"
    );
    assert!(
        !got.contains(&format!("{}-gate", TOTAL_GATES + 1)),
        "the drifting claim must be gone: {got:?}"
    );
    assert_eq!(
        normalise(&got),
        normalise(&format!(
            "openapi: 3.1.0\n\
             info:\n\
             \x20 title: Anvil\n\
             \x20 description: |\n\
             \x20   Runs the {TOTAL_GATES}-gate certification. Support is planned.\n"
        )),
        "exactly the exemption sentence goes; the surrounding YAML is not the \
         rewriter's to touch: {got:?}"
    );
    assert_eq!(
        got.lines().count(),
        page.lines().count(),
        "a YAML document that loses a line stops parsing; line structure must be \
         preserved: {got:?}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");
}

#[test]
fn a_slug_that_merely_resembles_anvils_is_still_somebody_elses_repository() {
    let page = watched_repo_page();

    for repo in NEAR_MISSES {
        let dir = tempdir().unwrap();
        for owned in OWNED_PAGES {
            write(&dir.path().join(owned), &page);
        }

        let sync = sync_published_counts(repo, dir.path(), TOTAL_GATES).unwrap();

        for owned in OWNED_PAGES {
            let got = std::fs::read_to_string(dir.path().join(owned)).unwrap();
            assert_eq!(
                got, page,
                "{repo:?} is not oyatie/anvil; {owned} must be byte-identical in \
                 every respect. A predicate that only separates oyatie/anvil from \
                 oyatie/console still rewrites and pushes this repository's docs"
            );
        }
        assert!(
            sync.rewritten.is_empty(),
            "{repo:?}: nothing may be reported as rewritten: {:?}",
            sync.rewritten
        );
        assert!(
            sync.remaining_drift.is_empty(),
            "{repo:?}: this repository's counts are not drift against Anvil's \
             TOTAL_GATES and must not fail its PR: {:?}",
            sync.remaining_drift
        );
        let reason = sync.not_applicable.as_deref().unwrap_or_else(|| {
            panic!("{repo:?}: the skip must be stated, not read as a clean page")
        });
        assert!(
            !reason.trim().is_empty(),
            "{repo:?}: the stated reason must actually say something — \
             `Some(String::new())` states nothing, and the empty slug is exactly \
             where a reason built out of the slug degenerates to it"
        );
    }
}

#[test]
fn anvil_is_recognised_case_insensitively_and_only_as_a_whole_slug() {
    // GitHub slugs are case-insensitive identities and arrive in whatever case
    // the event carried, so `Oyatie/Anvil` is Anvil. Case-insensitivity is safe
    // only because it applies to the whole slug: `ATTACKER/ANVIL` and
    // `Oyatie/Anvil-SDK` are still somebody else's. Both halves are pinned here
    // because they are one decision, and that decision is settled: this suite
    // is the specification, so an exact-case match is out of scope for the
    // implementation.
    for repo in ANVIL_SLUGS {
        let dir = tempdir().unwrap();
        write(&dir.path().join("README.md"), &drifting_page());

        let sync = sync_published_counts(repo, dir.path(), TOTAL_GATES).unwrap();
        let got = std::fs::read_to_string(dir.path().join("README.md")).unwrap();

        assert_eq!(
            sync.rewritten,
            vec!["README.md".to_string()],
            "{repo} is Anvil's own repository and its counts are the sync's business"
        );
        assert!(
            got.contains(&format!("{TOTAL_GATES}-gate")),
            "{repo}: Anvil's README must be rewritten to TOTAL_GATES: {got}"
        );
        assert!(
            sync.not_applicable.is_none(),
            "{repo}: the sync applied, so it must not report otherwise: {:?}",
            sync.not_applicable
        );
    }

    let page = watched_repo_page();
    for repo in ["ATTACKER/ANVIL", "Oyatie/Anvil-SDK"] {
        let dir = tempdir().unwrap();
        for owned in OWNED_PAGES {
            write(&dir.path().join(owned), &page);
        }

        let sync = sync_published_counts(repo, dir.path(), TOTAL_GATES).unwrap();

        for owned in OWNED_PAGES {
            assert_eq!(
                std::fs::read_to_string(dir.path().join(owned)).unwrap(),
                page,
                "{repo}: ignoring case must not turn into ignoring the rest of the \
                 slug; {owned} is not Anvil's and must be byte-identical"
            );
        }
        let reason = sync
            .not_applicable
            .as_deref()
            .unwrap_or_else(|| panic!("{repo}: the skip must be stated, not read as a clean page"));
        assert!(
            !reason.trim().is_empty(),
            "{repo}: the stated reason must actually say something"
        );
    }
}

#[test]
fn a_corpus_sync_that_did_not_apply_says_so_instead_of_passing_silently() {
    let anvil_dir = tempdir().unwrap();
    write(&anvil_dir.path().join("README.md"), &drifting_page());
    let anvil = sync_published_counts(ANVIL, anvil_dir.path(), TOTAL_GATES).unwrap();
    assert!(
        anvil.not_applicable.is_none(),
        "the sync did apply to Anvil, so it must not report otherwise: {:?}",
        anvil.not_applicable
    );

    let page = watched_repo_page();
    for repo in WATCHED {
        let dir = tempdir().unwrap();
        write(&dir.path().join("README.md"), &page);

        let sync = sync_published_counts(repo, dir.path(), TOTAL_GATES).unwrap();

        let reason = sync.not_applicable.as_deref().unwrap_or_else(|| {
            panic!(
                "{repo}: a skipped sync that reports an empty rewrite list and no \
                 drift is indistinguishable from a clean page; the skip must be stated"
            )
        });
        assert!(
            !reason.trim().is_empty(),
            "{repo}: the stated reason must actually say something"
        );
    }

    // The fail-closed mirror, and the one every fixture above misses: each of
    // them hands the sync a clean, fully readable tempdir, so "the sync did not
    // apply" and "the sync applied and found nothing to do" are still
    // distinguishable only by `not_applicable`. A corpus the sync CANNOT READ in
    // a repository that is not Anvil's separates them properly.
    //
    // The wrong implementation this catches is the natural edit, not a contrived
    // one: apply the ownership predicate per page rather than as an early
    // return, because that is where you already are when you reach for it —
    //
    //     for rel in pages {
    //         let original = std::fs::read_to_string(&path)?;   // runs for EVERY repo
    //         let updated = rewrite_page(&original, total_gates);
    //         if is_anvil(repo) && updated != original { write; rewritten.push(rel) }
    //         if is_anvil(repo) && let Some(why) = remaining_claim(..) { drift.push(..) }
    //     }
    //
    // Every case above still passes: the foreign pages stay byte-identical,
    // `rewritten` and `remaining_drift` are empty, `not_applicable` is `Some`.
    // But a watched repository whose `README.md` is a directory, or whose
    // `docs/adr` is unreadable, now returns `Err` from a sync that does not even
    // apply to it. `ensure_documentation_parity` maps that to `errored`, gate 1
    // goes Errored, and every pull request on that repository is blocked by
    // Anvil's private gate count — issue #27's harm arrived at from the
    // fail-closed side.
    //
    // The fixture is the one from
    // `a_corpus_sync_that_could_not_run_at_all_is_errored_at_the_gate` (now in
    // `tests/docguard_oracle_repair_gate_test.rs`), where the same unreadable
    // page is (correctly) `Err` for Anvil, so the pair pins the whole decision:
    // whose corpus it is settles whether it is read at all.
    for repo in WATCHED {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("README.md")).unwrap();
        assert!(
            sync_published_counts(ANVIL, dir.path(), TOTAL_GATES).is_err(),
            "fence: this same fixture must remain unreadable for Anvil, or the case \
             below is not about a corpus the sync could not read"
        );

        let sync = sync_published_counts(repo, dir.path(), TOTAL_GATES).unwrap_or_else(|e| {
            panic!(
                "{repo}: an unreadable page in somebody else's repository is not \
                 Anvil's business either. Reading it at all is the defect; failing \
                 the sync on it blocks every pull request on {repo} at gate 1 with \
                 Anvil's private gate count. got: {e}"
            )
        });

        assert!(
            sync.rewritten.is_empty(),
            "{repo}: nothing may be reported as rewritten: {:?}",
            sync.rewritten
        );
        assert!(
            sync.remaining_drift.is_empty(),
            "{repo}: a corpus the sync does not own is not drift against Anvil's \
             TOTAL_GATES, readable or not: {:?}",
            sync.remaining_drift
        );
        let reason = sync.not_applicable.as_deref().unwrap_or_else(|| {
            panic!("{repo}: the skip must be stated here too, not read as a clean page")
        });
        assert!(
            !reason.trim().is_empty(),
            "{repo}: the stated reason must actually say something"
        );
    }

    // The SECOND filesystem read, and the one the loop above does not fence.
    // `sync_published_counts` opens the corpus twice:
    //
    //     let pages = collect_owned_pages(repo_dir)?;   // std::fs::read_dir
    //     for rel in pages { std::fs::read_to_string(..)?; .. }
    //
    // A `README.md` that is a directory only fails the SECOND one, so it pins
    // the ownership decision as "before the page loop" and nothing more. The
    // wrong implementation that survives it is one line lower than the right
    // one, and it is entirely natural because it is where you already are when
    // you reach for the guard:
    //
    //     let pages = collect_owned_pages(repo_dir)?;   // runs for EVERY repo
    //     if !is_anvil(repo) {
    //         return Ok(CorpusSync { rewritten: vec![], remaining_drift: vec![],
    //                                not_applicable: Some(reason) });
    //     }
    //
    // Every assertion above still passes. But a watched repository whose
    // `docs/adr` exists and is unreadable then makes
    // `sync_published_counts("oyatie/console", ..)` return `Err("list docs/adr")`;
    // `ensure_documentation_parity` maps that to `errored`, gate 1 goes
    // `Errored`, `Errored.is_acceptable()` is false, and every pull request on
    // that repository is blocked forever by a directory Anvil had no business
    // opening. That is the fail-closed mirror of #27 the comment above claims to
    // have closed, arriving through the read the comment does not cover.
    //
    // The gate-level twin of this fixture is in
    // `the_gate_summary_for_a_non_anvil_repository_carries_the_skipped_syncs_reason`
    // (`tests/docguard_oracle_repair_gate_test.rs`).
    #[cfg(unix)]
    {
        for repo in WATCHED {
            let dir = tempdir().unwrap();
            let Some(original) = make_adr_dir_unreadable(dir.path()) else {
                panic!(
                    "fixture: this process can read a 0o000 directory, so the \
                     unreadable-ADR fixture cannot be built. That is a root container, \
                     not a passing implementation — run this suite as a non-root user. \
                     The `README.md`-is-a-directory fixture above still ran, but it \
                     fences only the per-page read, never `collect_owned_pages`."
                );
            };

            // Both calls are made while the directory is unreadable, and the guard
            // restores the permissions however this block leaves — assertion,
            // panic inside the sync, or ordinary fall-through — because an
            // unreadable directory also defeats `TempDir`'s own cleanup.
            let _restore = RestoreAdrDir {
                repo_dir: dir.path(),
                original: Some(original),
            };
            let anvil_result = sync_published_counts(ANVIL, dir.path(), TOTAL_GATES);
            let watched_result = sync_published_counts(repo, dir.path(), TOTAL_GATES);

            assert!(
                anvil_result.is_err(),
                "fence: an unreadable `docs/adr` must remain a real failure for Anvil's \
                 own corpus, or the case below is not about a corpus the sync could not \
                 read"
            );

            let sync = watched_result.unwrap_or_else(|e| {
                panic!(
                    "{repo}: `docs/adr` in somebody else's checkout is not Anvil's \
                     corpus, and LISTING it is not something this sync had any reason \
                     to do. Deciding ownership after `collect_owned_pages` blocks every \
                     pull request on {repo} at gate 1 on a directory Anvil should never \
                     have opened. got: {e}"
                )
            });

            assert!(
                sync.rewritten.is_empty(),
                "{repo}: nothing may be reported as rewritten: {:?}",
                sync.rewritten
            );
            assert!(
                sync.remaining_drift.is_empty(),
                "{repo}: a corpus the sync does not own is not drift against Anvil's \
                 TOTAL_GATES, listable or not: {:?}",
                sync.remaining_drift
            );
            let reason = sync.not_applicable.as_deref().unwrap_or_else(|| {
                panic!("{repo}: the skip must be stated here too, not read as a clean page")
            });
            assert!(
                !reason.trim().is_empty(),
                "{repo}: the stated reason must actually say something"
            );
        }
    }
}

// WHERE THE CORPUS SYNC'S `remaining_drift` ARM IS PINNED, and why it is not
// pinned here.
//
// `ensure_documentation_parity` matches three ways on the sync: `Err` (absent
// evidence), non-empty `remaining_drift` (hard fail, never AutoUpdated), and the
// ordinary success. This branch restructures that match to thread
// `not_applicable` through it, so all three arms are load-bearing rather than
// dead. All three are driven, and all three are driven at the GATE, in
// `tests/docguard_oracle_repair_gate_test.rs` —
// `a_corpus_sync_that_could_not_run_at_all_is_errored_at_the_gate` and
// `published_drift_the_sync_could_not_repair_fails_anvils_own_gate` — because
// that is where the arms exist. Nothing here needs to change for them; this note
// exists so an implementer flattening the match knows to look there.
//
// The drift arm needed a fixture that had to be BUILT rather than written down,
// and the reason is worth stating because an earlier round of this suite got it
// wrong and declared the arm unreachable:
//
//   * `rewrite_page` and `remaining_claim` share `count_regex`, `sixty_regex`
//     and `EXEMPTION_MARKERS`, so no page can simply *arrive* carrying drift
//     that survives its own repair. The count rewrite emits `TOTAL_GATES`
//     followed by the captured suffix verbatim, so a repaired claim always
//     re-parses.
//   * But the deletion CAN manufacture a claim at its junction, and that is the
//     whole of the arm's reachability. Removing a sentence splices the text
//     either side of it together, and `count_regex`'s `\s*-\s*gate` spans a
//     newline — so `Anvil ships 12` followed by an exemption sentence followed
//     by `-gate release check.` becomes a `12`-gate claim that nobody wrote.
//     The ORDER rule in the next section is what settles the consequence: the
//     count passes run first, so the spliced claim is reported as drift rather
//     than silently normalised. `unrepairable_drift_page()` in the gate binary
//     is that fixture, and it depends on the rule.
//
// Every issue-#28 case below asserts `remaining_drift.is_empty()`, which fences
// the same net from the other side: if the new sentence deletion misses an
// occurrence on a pinned layout, those assertions say so rather than letting the
// page be published mangled and reported clean.

// =========================================================================
// Issue #28 — removing the exemption removes one sentence, not one line
// =========================================================================
//
// The rule these cases collectively pin, stated once so the implementer is not
// reverse-engineering it from failures:
//
//   * A sentence is bounded by `.`, `?`, `!`, `。`, by a markdown cell
//     delimiter `|`, by a newline, or by the ends of the page. The set is the
//     same walking backwards from the marker and walking forwards from it;
//     `a_sentence_before_the_exemption_survives_whatever_terminates_it` and
//     `the_exemption_sentence_ends_at_whatever_terminates_it` pin each member
//     on both sides — `\n` included, which is the member a scan written over
//     "the terminator set" alone quietly loses — because two scan helpers with
//     different boundary sets is the shape that passes half a suite.
//   * `.` is a terminator unless the next character is ASCII alphanumeric —
//     this is what keeps `README.md`, `CHANGELOG.md` and `v1.2` from ending the
//     sentence mid-word. The exception is specific to ASCII `.`; `。` is a
//     terminator regardless of what follows it, because Korean and Japanese
//     prose does not put a space after it. `is_ascii_alphanumeric`, not
//     `is_alphanumeric`: `.고시` ends a sentence and `.md` does not.
//   * A terminator belongs to the sentence it ends; `|` and `\n` are clamps and
//     stay where they are.
//   * `start` walks back from the marker to the nearest boundary, then forward
//     over spaces. `end` runs to the nearest boundary after the marker.
//   * Ranges are byte ranges over UTF-8: a terminator that belongs to the
//     sentence is consumed by its own encoded length, never by one byte.
//   * The trailing newline is consumed only when `start` landed at a line start
//     *and* nothing survives on that line after `end` — otherwise the surviving
//     prefix or suffix would be fused with the next line.
//   * Both halves of that condition are WHITESPACE-BLIND, in the same direction,
//     and each carries its own whitespace with the newline when the newline
//     goes. Stated as two bullets because they are two separate decisions an
//     implementation makes at two separate indices, and a suite that decided one
//     and left the other open would let a whole published line differ.
//   * "`start` landed at a line start" means nothing but WHITESPACE precedes it
//     on that line, and when the newline is consumed that leading whitespace
//     goes with it. Anvil's own `openapi/openapi.yaml` is an owned page and it
//     is indented, so the exemption sentence occupying a whole *indented* line
//     is a layout the corpus really has; a rule that required `start` to be at
//     byte-column zero would leave a whitespace-only line behind there and a
//     rule that did not say which way it went would let either happen. Decided
//     rather than left open, and the cost is stated: it forbids leaving the
//     indentation, which on a page nobody re-indents is only diff noise —
//     but diff noise on a page the pipeline commits and pushes is still a
//     change nobody asked for. The converse is the same decision seen from the
//     other side and is pinned with it: when the newline is NOT consumed the
//     leading whitespace is NOT touched, because the line survives. Both
//     directions are byte-exact fixtures in
//     `the_trailing_newline_goes_only_when_the_deletion_started_at_a_line_start`,
//     because an implementation that takes the start adjustment as a separate
//     unconditional tidy-up — rather than as one of two assignments guarded by
//     one condition — gets the first direction right and destroys a YAML block
//     scalar's indentation in the second.
//   * "nothing survives on that line after `end`" means nothing but WHITESPACE
//     follows it on that line, and when the newline is consumed that trailing
//     whitespace goes with it. This is the exact mirror of the bullet above and
//     it is decided here for the same reason. The literal reading —
//     `end == line_end` — is what an implementer writes first, and on
//
//         Alpha line.
//         DocGuard does **not** yet amend existing documents.··
//         Gamma line.
//
//     (`··` being an ordinary markdown hard line break, two trailing spaces,
//     which Anvil's own README really can carry) it leaves `"··\n"` behind: the
//     published page keeps a whitespace-only line where the sentence was, one
//     line MORE than the whitespace-blind reading produces, on a page the
//     pipeline commits and pushes onto the contributor's branch. Both readings
//     satisfy every fixture that ends its exemption line flush, which is why
//     `the_trailing_newline_goes_only_when_the_deletion_started_at_a_line_start`
//     now carries a fixture that does not.
//   * The mirror of the mirror, and the reason both bullets say "when the
//     newline is consumed": whitespace on a line that SURVIVES is not touched,
//     at EITHER end of it — the indentation in front of a surviving suffix as
//     much as the hard break behind a surviving prefix.
//     A hard line break at the end of a line whose prefix survives belonged to
//     that line before the deletion and still does after it — removing it would
//     change how the page renders, on a line the deletion was never asked to
//     edit. That is issue #28's own requirement ("text before and after the
//     marker on the same line survives") applied to the text that happens to be
//     whitespace. STATED COST: it forbids an implementation that tidies
//     trailing whitespace on surviving lines, which some editors would call an
//     improvement — but an unrequested improvement committed and pushed onto a
//     contributor's branch is the same class of act as issue #27.
//   * Every occurrence is removed, not only the first — including two on one
//     line, and including one of each marker variant on the same line.
//   * ORDER: the gate-count rewrite and the `sixty-gate` rewrite run BEFORE the
//     exemption-sentence deletion, and neither is re-run over the deleted text
//     afterwards. This is a requirement, not an artefact of today's code, and
//     it is stated here because a fixture depends on it
//     (`unrepairable_drift_page()` in `tests/docguard_oracle_repair_gate_test.rs`).
//
//     Deleting a sentence SPLICES the text either side of it together, and the
//     splice can form a gate-count claim that was never written on the page: a
//     line ending `Anvil ships 12`, an exemption sentence, and a line opening
//     `-gate release check.` become `Anvil ships 12\n-gate release check.`, which
//     `count_regex`'s `\s*-\s*gate` reads as a `12`-gate claim. A rewriter that
//     normalises counts AFTER the deletion silently manufactures that claim and
//     then repairs it to `TOTAL_GATES`, publishing a number the author never
//     wrote and never saw. A rewriter that normalises BEFORE it leaves the
//     spliced claim standing, `remaining_claim` reports it as drift, and gate 1
//     hands it back to a human — which is the fail-closed answer, and the one
//     this suite requires.
//
//     STATED COST, so this reads as a decision rather than an accident of the
//     code it replaces: re-running the count pass to a fixpoint after the
//     deletion would be a defensible design, and it is forbidden here. The
//     reason is that its output on this layout is a published claim nobody
//     authored, which is the class of statement this whole branch exists to
//     stop the oracle making.

#[test]
fn an_exemption_marker_inside_a_table_row_leaves_the_row_and_its_neighbour_intact() {
    let (got, remaining_drift) = rewrite_anvil_readme(HISTORICAL_README_TABLE);

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption marker is the thing being removed: {got}"
    );
    assert_eq!(
        got.lines().count(),
        HISTORICAL_README_TABLE.lines().count(),
        "the table must keep every row it started with:\n{got}"
    );

    let docguard_row = got
        .lines()
        .find(|l| l.starts_with(DOCGUARD_ROW_PREFIX))
        .unwrap_or_else(|| panic!("the DocGuard row must survive:\n{got}"));

    // The whole sentence goes, not just the marker phrase. `Note: it` opens the
    // sentence and `— see the roadmap.` closes it; leaving either behind writes
    // mangled English to a published page and reports it clean.
    for orphan in [
        "Note: it",
        "such as `README.md`",
        "or `CHANGELOG.md`",
        "see the roadmap",
    ] {
        assert!(
            !got.contains(orphan),
            "{orphan:?} belongs to the exemption sentence and must go with it, \
             not survive as a fragment:\n{got}"
        );
    }
    assert_eq!(
        normalise(docguard_row),
        normalise(DOCGUARD_ROW_AFTER),
        "the row must lose exactly the exemption sentence and keep everything else"
    );
    assert!(
        docguard_row.ends_with('|'),
        "the DocGuard row must keep its closing pipe and stay a table row: {docguard_row}"
    );
    assert!(
        got.lines().any(|l| l == CEDAR_ROW),
        "the following row must not be fused into the one above it:\n{got}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");
}

#[test]
fn an_exemption_sentence_with_no_terminator_is_clamped_and_takes_nothing_beyond_it() {
    // The historical row happens to end its cell with `.` before the closing
    // pipe. A cell that simply runs to the pipe, and a line that simply runs to
    // the newline, carry no terminator at all — the commoner markdown shape,
    // and the one where "delete to the next `. `" deletes the rest of the file.
    let table = "| Gate | It does **not** yet amend existing documents |\n\
                 | Next | Row survives |\n";
    let (got, remaining_drift) = rewrite_anvil_readme(table);

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption marker is the thing being removed: {got:?}"
    );
    let rows: Vec<&str> = got.lines().collect();
    assert_eq!(
        rows.len(),
        2,
        "neither row may be swallowed or fused: {got:?}"
    );
    assert!(
        rows[0].starts_with("| Gate |"),
        "the row's untouched first cell must survive: {:?}",
        rows[0]
    );
    assert!(
        rows[0].ends_with('|'),
        "the row must keep its closing pipe and stay a table row: {:?}",
        rows[0]
    );
    assert_eq!(
        rows[1], "| Next | Row survives |",
        "the following row must be untouched: {got:?}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");

    // `|` IS AN END BOUNDARY, and until now nothing in this suite said so.
    //
    // The rule at the head of this section states the boundary set is "the same
    // walking backwards from the marker and walking forwards from it", `|`
    // included. `|` was pinned as a START boundary — `rows[0].starts_with("|
    // Gate |")` above fails if the backward scan does not stop at the cell
    // delimiter — but NO fixture anywhere required it to stop the FORWARD scan,
    // because in every table fixture in this file the exemption sentence is the
    // last content in its cell:
    //
    //   * `HISTORICAL_README_TABLE` — the cell ends "… — see the roadmap. |", so
    //     `end` lands on the ASCII `.` before ever reaching the pipe.
    //   * the fixture above — nothing survives after the marker except the
    //     closing pipe itself.
    //   * `a_page_that_ends_at_the_exemption_sentence_is_not_overrun`'s
    //     "| Gate | It does **not** yet amend existing documents" — no pipe after
    //     the marker at all.
    //
    // The wrong implementation, and it is the MINIMAL edit to today's
    // `let end_rel = rest.find('\n').unwrap_or(rest.len());`: add the sentence
    // terminators to the forward scan and keep `\n` as the only clamp, so `|`
    // never enters the end-boundary set. Against the fixture above that yields
    // `rows[0] == "| Gate |"` instead of the correct `"| Gate ||"` — the closing
    // pipe of the DocGuard cell has been eaten — and BOTH surviving assertions
    // still hold: `starts_with("| Gate |")` is true because the string equals it,
    // and `ends_with('|')` is true because the row's own opening-cell pipe is now
    // the last character.
    //
    // What ships: on any owned page where a table cell's exemption sentence
    // carries no `.`/`?`/`!`/`。` — the commoner markdown shape, as this case's
    // own comment says — everything from the marker to end-of-line is destroyed:
    // the cell delimiter and every cell to its right. That is issue #28's
    // headline harm reached through the one boundary member no fixture exercised.
    //
    // So: the exemption sentence is no longer the last content in its row. The
    // forward scan must stop at `|`, and the cell to its right must survive.
    // Unterminated first, which is the case that has no other boundary to fall
    // back on.
    let table = "| Gate | It does **not** yet amend existing documents | Support is planned |\n\
                 | Next | Row survives | Third cell |\n";
    let (got, remaining_drift) = rewrite_anvil_readme(table);

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption marker is the thing being removed: {got:?}"
    );
    let rows: Vec<&str> = got.lines().collect();
    assert_eq!(
        rows.len(),
        2,
        "neither row may be swallowed or fused: {got:?}"
    );
    assert!(
        rows[0].contains("Support is planned"),
        "the cell to the RIGHT of the exemption sentence is a different cell and \
         must survive: the forward scan stops at `|`, it does not run to the end \
         of the line: {:?}",
        rows[0]
    );
    assert!(
        rows[0].starts_with("| Gate |"),
        "the row's untouched first cell must survive: {:?}",
        rows[0]
    );
    assert!(
        rows[0].ends_with('|'),
        "the row must keep its closing pipe and stay a table row: {:?}",
        rows[0]
    );
    assert_eq!(
        rows[0].matches('|').count(),
        rows[1].matches('|').count(),
        "the row must keep its full cell count — a scan that eats the cell \
         delimiter merges two cells into one and the table stops parsing:\n\
         got:      {:?}\nneighbour: {:?}",
        rows[0],
        rows[1]
    );
    assert_eq!(
        normalise(rows[0]),
        normalise("| Gate | | Support is planned |"),
        "the row loses exactly the exemption sentence: its own cell is emptied and \
         nothing beyond the delimiter is touched: {:?}",
        rows[0]
    );
    assert_eq!(
        rows[1], "| Next | Row survives | Third cell |",
        "the following row must be untouched: {got:?}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");

    // The same row shape with the sentence TERMINATED, so `end` has a `.` to
    // stop on and the rest of the cell — not just the rest of the row — has to
    // survive as well. `HISTORICAL_README_TABLE` cannot pin this: its exemption
    // sentence is the last thing in its cell.
    let table = "| Gate | It does **not** yet amend existing documents. Support is planned. | Third |\n\
                 | Next | Row survives | Cell |\n";
    let (got, remaining_drift) = rewrite_anvil_readme(table);

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption marker is the thing being removed: {got:?}"
    );
    let rows: Vec<&str> = got.lines().collect();
    assert_eq!(
        rows.len(),
        2,
        "neither row may be swallowed or fused: {got:?}"
    );
    assert_eq!(
        normalise(rows[0]),
        normalise("| Gate | Support is planned. | Third |"),
        "the sentence after the exemption keeps its cell, and the cell after that \
         keeps its place: {:?}",
        rows[0]
    );
    assert_eq!(
        rows[0].matches('|').count(),
        rows[1].matches('|').count(),
        "the row must keep its full cell count:\ngot:      {:?}\nneighbour: {:?}",
        rows[0],
        rows[1]
    );
    assert_eq!(
        rows[1], "| Next | Row survives | Cell |",
        "the following row must be untouched: {got:?}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");

    // The same clamp in plain prose. The marker opens the line, so the emptied
    // line goes with its newline — but the line below it is not part of the
    // sentence and must survive whole.
    let (got, remaining_drift) = rewrite_anvil_readme(
        "DocGuard does **not** yet amend existing documents\nNext line here. Tail.\n",
    );
    assert_eq!(
        normalise(&got),
        normalise("Next line here. Tail.\n"),
        "an unterminated sentence ends at the newline; the following line is a \
         different sentence and survives whole: {got:?}"
    );
    assert!(
        got.lines().any(|l| l == "Next line here. Tail."),
        "the following line must stay a line of its own: {got:?}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");

    // And mid-line, where a surviving prefix keeps the line alive.
    let (got, remaining_drift) = rewrite_anvil_readme(
        "Alpha. DocGuard does **not** yet amend existing documents\nNext line here. Tail.\n",
    );
    assert_eq!(
        normalise(&got),
        normalise("Alpha.\nNext line here. Tail.\n"),
        "the prefix keeps its own line and the line below is untouched: {got:?}"
    );
    assert_eq!(
        got.lines().count(),
        2,
        "no line may be lost or fused: {got:?}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");
}

#[test]
fn a_sentence_before_the_exemption_survives_whatever_terminates_it() {
    // The start boundary is a SENTENCE boundary, not a period boundary. A start
    // that scans back only for `['.', '\n']` — the shape the current code has,
    // and the one a repair that touches only `end` leaves in place — swallows
    // the contributor's preceding sentence whole and reports the page clean.
    // Anvil's own corpus carries Korean, so the ideographic full stop is not an
    // exotic case here.
    let cases: &[(&str, &str)] = &[
        (
            "Is that so? DocGuard does **not** yet amend existing documents. Beta.\n",
            "Is that so? Beta.\n",
        ),
        (
            "Wonderful! DocGuard does **not** yet amend existing documents. Beta.\n",
            "Wonderful! Beta.\n",
        ),
        // The ASCII-alphanumeric exception belongs to `.` ALONE, on the START
        // side too. Both fixtures above put a space after their terminator, so a
        // backward scan that carries the `.`-only exception onto `?` and `!`
        // passes them — `。` is the only one of the three pinned flush against
        // ASCII here (`고시 관련입니다。DocGuard …`, below), and it is pinned on
        // the END side by three fixtures in
        // `the_exemption_sentence_ends_at_whatever_terminates_it`. With that
        // mutation the scan walks back past `?` to byte 0 and deletes the
        // contributor's question along with the exemption: mangled prose,
        // reported clean, which is issue #28's harm reached from the start side.
        (
            "Is that so?DocGuard does **not** yet amend existing documents. Beta.\n",
            "Is that so? Beta.\n",
        ),
        (
            "Wonderful!DocGuard does **not** yet amend existing documents. Beta.\n",
            "Wonderful! Beta.\n",
        ),
        // Korean and Japanese prose puts no space after `。`, so the exemption
        // sentence starts flush against the sentence before it. The marker's
        // sentence ends this line, which keeps the case about the START
        // boundary alone: there is no junction after the deletion for a
        // whitespace convention to argue about.
        (
            "고시 관련입니다。DocGuard does **not** yet amend existing documents.\nBeta.\n",
            "고시 관련입니다。\nBeta.\n",
        ),
        // The `.`-is-not-always-a-terminator rule, on the START side. The
        // orphan assertions in the table case pin it on the END side only, so a
        // repair that touches only `end` — the cheapest possible fix for #28 —
        // leaves the naive `rfind(['.', '\n'])` start in place and passes
        // everything else. Here the LAST `.` before the marker is the one
        // inside `v1.2`, so that start lands between `1` and `2` and publishes
        // "Alpha. See CHANGELOG.md and v1 Beta." — mangled prose, reported
        // clean, which is issue #28's harm exactly.
        (
            "Alpha. See CHANGELOG.md and v1.2 for details, but DocGuard does **not** yet amend existing documents. Beta.\n",
            "Alpha. Beta.\n",
        ),
        // The same rule where the sentence terminator does follow the intra-word
        // dots, so a correct scan must walk past `CHANGELOG.md` and `v1.2` and
        // still stop at `notes.`.
        (
            "See CHANGELOG.md for v1.2 notes. DocGuard does **not** yet amend existing documents. Beta.\n",
            "See CHANGELOG.md for v1.2 notes. Beta.\n",
        ),
        // `is_ascii_alphanumeric`, not `is_alphanumeric`, on the START side.
        // The rule at the head of this section says `.고시` ends a sentence and
        // `.md` does not; until now that distinction was pinned on the END side
        // only (`a_multibyte_character_touching_the_deletion_boundary_is_not_split`
        // carries every `.`+non-ASCII fixture, and all of them sit *after* the
        // marker). A backward scan written with the obvious `is_alphanumeric` —
        // the method name the rule's own warning is about — refuses to stop at
        // the `.` after `Alpha`, walks back to byte 0, and deletes
        // `Alpha.고시 …` wholesale: a mangled page, reported clean. Anvil's own
        // corpus carries Korean (`docs/adr/0002-…`), so `.` flush against
        // Hangul is a real layout here.
        //
        // `고시` opens the marker's sentence — the terminator before it is the
        // `.` after `Alpha` — so it goes with the sentence, exactly as
        // `고시 detail follows.` survives on the END side for the mirror reason.
        (
            "Alpha.고시 DocGuard does **not** yet amend existing documents. Beta.\n",
            "Alpha. Beta.\n",
        ),
        // `\n` IS A START BOUNDARY, and until now nothing here said so. Every
        // other fixture above puts a `.`, `?`, `!` or `。` immediately before the
        // newline the deletion starts after (`Alpha line.\n`, `… Beta.\nGamma.`),
        // so the terminator boundary and the newline boundary coincide and the
        // suite cannot tell them apart. A refactor into `sentence_start()` /
        // `sentence_end()` where the END scan treats `\n` and `|` as *clamps* —
        // a genuinely different concept, because a clamp is not consumed — and
        // the START scan is then written over the terminator set alone,
        // `['.', '?', '!', '。', '|']`, is exactly the "two scan helpers with
        // different boundary sets" shape the rule above says it is defending
        // against, and it passed every case in this file.
        //
        // Here there is no terminator anywhere before the marker, so the only
        // boundary available is the newline that ends `## Roadmap`. Without it
        // the backward scan runs to byte 0 and the page becomes
        // `" Support is planned.\n"` — the heading, the blank line and the title
        // all destroyed, with `remaining_drift` empty, so the gate reports the
        // page clean. That is issue #28 verbatim, reached from the start side, on
        // the most ordinary markdown layout there is: a heading directly above a
        // paragraph.
        (
            "# Anvil\n\n## Roadmap\nDocGuard does **not** yet amend existing documents. Support is planned.\n",
            "# Anvil\n\n## Roadmap\nSupport is planned.\n",
        ),
        // The same start boundary where the line ABOVE is a table row rather
        // than a heading, so the nearest character the terminator-only scan can
        // see is the row's closing `|`. It stops there, one byte before the
        // newline, and consumes the newline with the deleted sentence: the
        // DocGuard note is fused onto the end of the table's last row and the
        // page loses a line. The `got.lines().count()` assertion below is what
        // catches that.
        (
            "| Quality Gate | Notes |\n\
             |---|---|\n\
             | **Docs** | Verifies public APIs |\n\
             DocGuard does **not** yet amend existing documents. Support is planned.\n",
            "| Quality Gate | Notes |\n\
             |---|---|\n\
             | **Docs** | Verifies public APIs |\n\
             Support is planned.\n",
        ),
    ];

    for (input, expected) in cases {
        let (got, remaining_drift) = rewrite_anvil_readme(input);
        assert!(
            !got.contains(EXEMPTION_MARKER),
            "input {input:?}: the marker is the thing being removed, got {got:?}"
        );
        assert_eq!(
            normalise(&got),
            normalise(expected),
            "input {input:?} must lose exactly the exemption sentence and leave the \
             sentence before it whole"
        );
        assert_eq!(
            got.lines().count(),
            expected.lines().count(),
            "input {input:?} must keep its line structure, got {got:?}"
        );
        assert!(
            remaining_drift.is_empty(),
            "input {input:?}: {remaining_drift:?}"
        );
    }
}

#[test]
fn the_exemption_sentence_ends_at_whatever_terminates_it() {
    // The mirror of the case above, and the reason it exists: every other
    // END-boundary assertion in this suite terminates on `.`, `|`, `\n` or the
    // end of the page. An implementation with two scan helpers — a start scan
    // over ['.', '?', '!', '。', '|', '\n'] and an end scan over ['.', '|',
    // '\n'] — satisfies the whole of issue #28's coverage without ever handling
    // `?`, `!` or `。` at the end of the deleted sentence, and swallows the
    // following sentence when the exemption ends in one.
    let cases: &[(&str, &str)] = &[
        (
            "Alpha. It does **not** yet amend existing documents! Beta. Gamma.\n",
            "Alpha. Beta. Gamma.\n",
        ),
        (
            "Alpha. It does **not** yet amend existing documents? Beta. Gamma.\n",
            "Alpha. Beta. Gamma.\n",
        ),
        // Doubly load-bearing: `。` is three bytes, so an end scan that consumes
        // its terminator with `end += 1` instead of `end += ch.len_utf8()`
        // lands mid-character and panics inside `replace_range` — in the review
        // pipeline, on Anvil's own corpus, which carries Korean. The byte-index
        // defect is pinned today on the start side only.
        (
            "Anvil does **not** yet amend existing documents。고시 관련. Beta.\n",
            "고시 관련. Beta.\n",
        ),
        // The ASCII-alphanumeric exception belongs to `.` ALONE. `。`, `?` and
        // `!` terminate regardless of what follows them, and until now that was
        // pinned on the START side only (`고시 관련입니다。DocGuard …`, where `。`
        // is followed by ASCII `D`); every END-side fixture put a space or a
        // non-ASCII character after the terminator, so an end scan carrying the
        // `.`-only exception onto the other three passed the whole suite.
        //
        // With that mutation, `。` stops terminating here, `end` runs on to the
        // `.` after `detail`, and the contributor's next sentence is deleted —
        // issue #28's harm, reached from the terminator the rule says is
        // unconditional.
        (
            "Anvil does **not** yet amend existing documents。Beta detail. Gamma.\n",
            "Beta detail. Gamma.\n",
        ),
        (
            "Anvil does **not** yet amend existing documents!Beta detail. Gamma.\n",
            "Beta detail. Gamma.\n",
        ),
        (
            "Anvil does **not** yet amend existing documents?Beta detail. Gamma.\n",
            "Beta detail. Gamma.\n",
        ),
    ];

    for (input, expected) in cases {
        let (got, remaining_drift) = rewrite_anvil_readme(input);
        assert!(
            !got.contains(EXEMPTION_MARKER),
            "input {input:?}: the marker is the thing being removed, got {got:?}"
        );
        assert_eq!(
            normalise(&got),
            normalise(expected),
            "input {input:?}: the exemption sentence ends at its own terminator, and \
             the sentence after it survives whole"
        );
        assert_eq!(
            got.lines().count(),
            expected.lines().count(),
            "input {input:?} must keep its line structure, got {got:?}"
        );
        assert!(
            remaining_drift.is_empty(),
            "input {input:?}: {remaining_drift:?}"
        );
    }
}

#[test]
fn prose_following_the_exemption_sentence_on_the_same_line_survives() {
    let (got, remaining_drift) = rewrite_anvil_readme(
        "# Anvil\n\nAlpha sentence. DocGuard does **not** yet amend existing documents. Beta sentence.\nGamma line.\n",
    );

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption marker is the thing being removed: {got}"
    );
    // `DocGuard` opens the marker's sentence. A deletion that starts at the
    // marker rather than at the sentence boundary leaves it stranded.
    assert!(
        !got.contains("DocGuard"),
        "the head of the exemption sentence must go with the rest of it: {got}"
    );

    let surviving = got
        .lines()
        .find(|l| l.contains("Alpha sentence."))
        .unwrap_or_else(|| panic!("the sentence before the exemption must survive: {got}"));
    assert_eq!(
        normalise(surviving),
        vec!["Alpha sentence. Beta sentence.".to_string()],
        "removing the exemption sentence must leave its neighbours whole and on \
         one line, with nothing of the deleted sentence left behind"
    );
    assert!(
        got.lines().any(|l| l == "Gamma line."),
        "the next line must stay a line of its own, not be fused upward: {got}"
    );
    assert_eq!(
        got.lines().count(),
        4,
        "line structure must be preserved:\n{got}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");
}

#[test]
fn the_trailing_newline_goes_only_when_the_deletion_started_at_a_line_start() {
    // Two halves of one rule, pinned together because an implementation can only
    // get them both right by looking at where `start` landed.
    //
    // The first layout — the sentence alone on its line — is the one the current
    // range computation happens to get right, and it is exactly what a rewrite
    // that narrows `end` to the sentence terminator can regress: narrow `end`
    // without keeping the newline rule and a blank line is left behind.
    let own_line = "# Anvil\n\nAlpha line.\nDocGuard does **not** yet amend existing documents.\nGamma line.\n";
    let (got, remaining_drift) = rewrite_anvil_readme(own_line);

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption marker is the thing being removed: {got}"
    );
    assert_eq!(
        got.lines().count(),
        own_line.lines().count() - 1,
        "the exemption line must be removed entirely, not blanked:\n{got}"
    );
    assert_eq!(
        got.lines().filter(|l| l.trim().is_empty()).count(),
        1,
        "the only blank line is the one under the heading; no blank may be left \
         where the exemption line was:\n{got}"
    );
    assert!(
        got.contains("Alpha line.\nGamma line.\n"),
        "the neighbouring lines must be unchanged, unfused, and adjacent:\n{got}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");

    // The second layout: the marker's sentence ends the line but does not begin
    // it. The line has a surviving prefix, so its newline must stay or the
    // prefix is fused with the line below.
    let shares_a_line = "# Anvil\n\nAlpha line.\nPreamble. DocGuard does **not** yet amend existing documents.\nGamma line.\n";
    let (got, remaining_drift) = rewrite_anvil_readme(shares_a_line);

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption marker is the thing being removed: {got}"
    );
    assert_eq!(
        normalise(&got),
        normalise("# Anvil\n\nAlpha line.\nPreamble.\nGamma line.\n"),
        "the surviving prefix keeps its own line; the newline is only consumed \
         when the deletion started at a line start"
    );
    assert_eq!(
        got.lines().count(),
        shares_a_line.lines().count(),
        "no line may be lost here — the exemption line had other prose on it:\n{got}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");

    // The third layout, and the one that settles what "at a line start" means:
    // the sentence occupies a whole INDENTED line. `openapi/openapi.yaml` is an
    // owned page and it is indented throughout, so this is a layout Anvil's own
    // corpus really has — and a scan that requires `start` at byte-column zero
    // leaves `    \n` behind, while one that accepts "only whitespace precedes
    // start" removes the line. Both satisfy every other case in this file, which
    // is why the rule block states which one is required.
    let indented = "openapi: 3.1.0\n\
                    info:\n\
                    \x20 description: |\n\
                    \x20   Alpha line.\n\
                    \x20   DocGuard does **not** yet amend existing documents.\n\
                    \x20   Gamma line.\n";
    let (got, remaining_drift) = rewrite_anvil_page("openapi/openapi.yaml", indented);

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption marker is the thing being removed: {got:?}"
    );
    assert_eq!(
        got,
        "openapi: 3.1.0\n\
         info:\n\
         \x20 description: |\n\
         \x20   Alpha line.\n\
         \x20   Gamma line.\n",
        "the indented exemption line goes whole — its own indentation and its \
         newline with it — leaving neither a blank line nor a line of trailing \
         spaces on a page the pipeline commits and pushes: {got:?}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");

    // The leading-side MIRROR of the layout above, and what makes the indented
    // case pin a CONDITION rather than a single index. Same indented owned page,
    // same sentence beginning the line's content — but this time prose SURVIVES
    // after the sentence's terminator on the same line, so the newline is not
    // consumed and therefore neither is the indentation.
    //
    // The rule is two assignments made at two indices, and each is guarded by
    // BOTH halves of one condition. The natural error is to hoist the start
    // adjustment out of the guard, because on its own it reads like a separate
    // tidy-up:
    //
    //     let line_start = out[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    //     if out[line_start..start].trim().is_empty() { start = line_start; }
    //     if nothing_after_on_line { end = (line_end + 1).min(out.len()); }
    //
    // Every other fixture in all four binaries survives that. The two with a
    // surviving suffix are at byte 0 with no indentation to lose, or are
    // compared through `normalise`, which trims each line and so cannot see
    // leading whitespace at all. These two are the only place a hoisted start
    // is visible.
    //
    // What the hoisted start ships: Anvil's own `openapi/openapi.yaml` is an
    // owned page and it is indented throughout. A block scalar whose indentation
    // is destroyed stops the OpenAPI document parsing; the rewriter still
    // reports the page in `rewritten`, `remaining_drift` is empty so gate 1
    // reports the corpus clean, and the pipeline commits and pushes the broken
    // document onto the contributor's branch.
    //
    // Driven twice, because the two questions are separable and this suite
    // decides only one of them. `normalise`'s own contract is that interior
    // spacing — "whether the implementation leaves one space or two where a
    // sentence used to be" — is NOT decided here, and in this layout the space
    // that followed the deleted sentence's terminator sits immediately after the
    // line's indentation, so a single byte-exact assertion over a
    // space-separated fixture would decide it by accident. So:
    //
    //   * the space-separated fixture asserts the indentation byte-exactly and
    //     the words through `normalise`, which is exactly the leading-whitespace
    //     question and nothing else; and
    //   * a fixture whose sentence ends in `。`, which the rule makes a
    //     terminator regardless of what follows it and which therefore leaves no
    //     separator to argue about, is asserted byte-exactly end to end.
    //
    // Together they pin the surviving direction of the leading half without
    // this suite deciding a question it elsewhere declares open.
    let indented_with_a_survivor = "openapi: 3.1.0\n\
                                    info:\n\
                                    \x20 description: |\n\
                                    \x20   DocGuard does **not** yet amend existing documents. Support is planned.\n\
                                    \x20   Gamma line.\n";
    let (got, remaining_drift) =
        rewrite_anvil_page("openapi/openapi.yaml", indented_with_a_survivor);

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption marker is the thing being removed: {got:?}"
    );
    assert_eq!(
        normalise(&got),
        normalise(
            "openapi: 3.1.0\n\
             info:\n\
             \x20 description: |\n\
             \x20   Support is planned.\n\
             \x20   Gamma line.\n"
        ),
        "the exemption sentence goes and the prose after it on the same line \
         stays: {got:?}"
    );
    assert_eq!(
        got.lines().count(),
        indented_with_a_survivor.lines().count(),
        "no line may be lost here — the exemption line had other prose on \
         it: {got:?}"
    );
    let survivor = got
        .lines()
        .find(|l| l.contains("Support is planned."))
        .unwrap_or_else(|| panic!("the prose after the sentence must survive: {got:?}"));
    assert!(
        survivor.starts_with("    "),
        "the exemption sentence began this line's CONTENT, but prose survives \
         after its terminator, so the line survives — and a line that survives \
         keeps its own leading whitespace. This is the assertion `normalise` \
         cannot make, because it trims each line before comparing, and it is the \
         only one in the suite that can see a start adjustment applied outside \
         the guard: on this page, a YAML block scalar, the four spaces are what \
         keep the document parsing, and the pipeline commits and pushes whatever \
         the rewriter leaves: {survivor:?}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");

    // The same layout with `。` closing the sentence, so there is no separator
    // between the deletion's end and the surviving prose and the whole page can
    // be compared byte for byte. `。` terminates regardless of what follows it —
    // that is the rule, pinned on both sides by
    // `the_exemption_sentence_ends_at_whatever_terminates_it` — and Anvil's own
    // corpus really does carry Korean.
    let indented_cjk = "openapi: 3.1.0\n\
                        info:\n\
                        \x20 description: |\n\
                        \x20   DocGuard does **not** yet amend existing documents。Support is planned.\n\
                        \x20   Gamma line.\n";
    let (got, remaining_drift) = rewrite_anvil_page("openapi/openapi.yaml", indented_cjk);

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption marker is the thing being removed: {got:?}"
    );
    assert_eq!(
        got,
        "openapi: 3.1.0\n\
         info:\n\
         \x20 description: |\n\
         \x20   Support is planned.\n\
         \x20   Gamma line.\n",
        "byte-exact, and the only fixture in the suite that is byte-exact over a \
         surviving INDENTED line: the sentence's own bytes go, its terminator \
         goes with it, and the four spaces in front of it — which belong to a \
         line that survives — do not. An implementation that moves `start` back \
         to the line start unconditionally, rather than only when it is also \
         about to consume the newline, emits this page with the block scalar's \
         indentation gone: {got:?}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");

    // The last two layouts, and the ones that settle what "nothing
    // survives on that line after `end`" means: the sentence occupies a whole
    // line and that line ends in WHITESPACE before its newline.
    //
    // This is the exact mirror of the indented case above, and it was left open
    // while that one was decided. `end < line_end` — the literal reading of the
    // rule, and the one an implementer writes first — leaves `"  \n"` behind
    // here: a whitespace-only line where the sentence was, one line MORE than
    // the correct result, on a page the pipeline commits and pushes onto the
    // contributor's branch. `out[end..line_end].trim().is_empty()` removes the
    // line. Every other fixture in all four binaries ends its exemption line
    // flush, so both readings survive them, and the two outputs differ by a
    // whole published line.
    //
    // MEASURED, and disclosed for the same reason the indented case's status is
    // not stated by omission: these two fixtures PASS against `main` today, and
    // so does the indented one. Today's `end` runs to end-of-line and consumes
    // the newline, which swallows trailing whitespace by accident — it is the
    // same over-reach that fuses lines in the negative below. They are here to
    // constrain the REPAIR, which narrows `end` to the sentence terminator and
    // must then decide this question explicitly, not to report a live defect.
    // The red evidence in this case is the second layout, the two
    // leading-side mirrors above, and the last layout below.
    //
    // Two trailing spaces first, because that is not a typo in markdown: it is
    // an ordinary hard line break, and Anvil's own README carries them.
    for (label, tail) in [("markdown hard break", "  "), ("tab", "\t")] {
        let trailing = format!(
            "# Anvil\n\nAlpha line.\nDocGuard does **not** yet amend existing documents.{tail}\nGamma line.\n"
        );
        let (got, remaining_drift) = rewrite_anvil_readme(&trailing);

        assert!(
            !got.contains(EXEMPTION_MARKER),
            "{label}: the exemption marker is the thing being removed: {got:?}"
        );
        assert_eq!(
            got, "# Anvil\n\nAlpha line.\nGamma line.\n",
            "{label}: the exemption line goes whole — its trailing whitespace and \
             its newline with it — leaving neither a blank line nor a \
             whitespace-only line where the sentence was. Byte-exact, because the \
             two readings of \"nothing survives after `end`\" differ by exactly \
             that line and nothing else in this suite can tell them apart: {got:?}"
        );
        assert!(remaining_drift.is_empty(), "{label}: {remaining_drift:?}");
    }

    // The mirror negative, in the same case because it is the same decision seen
    // from the other side: trailing whitespace after `end` when a prefix
    // SURVIVES on the line. The newline must stay — fusing `Preamble.` with
    // `Gamma line.` is issue #28's own headline — and the hard break must stay
    // with it, because it belonged to a line that survives and nothing asked the
    // rewriter to edit that line's rendering.
    for (label, tail) in [("markdown hard break", "  "), ("tab", "\t")] {
        let kept = format!(
            "# Anvil\n\nAlpha line.\nPreamble. DocGuard does **not** yet amend existing documents.{tail}\nGamma line.\n"
        );
        let (got, remaining_drift) = rewrite_anvil_readme(&kept);

        assert!(
            !got.contains(EXEMPTION_MARKER),
            "{label}: the exemption marker is the thing being removed: {got:?}"
        );
        assert_eq!(
            normalise(&got),
            normalise("# Anvil\n\nAlpha line.\nPreamble.\nGamma line.\n"),
            "{label}: the surviving prefix keeps its own line; whitespace after \
             `end` is not a licence to consume the newline: {got:?}"
        );
        assert_eq!(
            got.lines().count(),
            kept.lines().count(),
            "{label}: no line may be lost here — the exemption line had other \
             prose on it, so its newline is what keeps `Gamma line.` a line of \
             its own: {got:?}"
        );
        let surviving = got
            .lines()
            .find(|l| l.trim_start().starts_with("Preamble."))
            .unwrap_or_else(|| panic!("{label}: the prefix must survive: {got:?}"));
        assert!(
            surviving.ends_with(tail),
            "{label}: the line survives, so its own trailing hard break survives \
             with it. Consuming it changes how a line the deletion was never \
             asked to edit renders, on a page the pipeline commits and pushes: \
             {surviving:?}"
        );
        assert!(remaining_drift.is_empty(), "{label}: {remaining_drift:?}");
    }
}

#[test]
fn a_multibyte_character_touching_the_deletion_boundary_is_not_split() {
    // Anvil's own corpus is not ASCII (`docs/adr/0002-…` carries Korean, and
    // the README table carries em dashes and emoji). A range computed in bytes
    // that walks one position past a sentence terminator lands mid-character
    // and panics inside `replace_range`, in the review pipeline.
    let cases: &[(&str, &str)] = &[
        // Multi-byte immediately after the sentence terminator, no space.
        (
            "Anvil does **not** yet amend existing documents.고시 detail follows.\n",
            "고시 detail follows.\n",
        ),
        (
            "Anvil does **not** yet amend existing documents.— trailing note.\n",
            "— trailing note.\n",
        ),
        (
            "Anvil does **not** yet amend existing documents.“quoted tail”.\n",
            "“quoted tail”.\n",
        ),
        // Multi-byte immediately before the deletion start.
        (
            "고시 관련. DocGuard does **not** yet amend existing documents. Beta.\n",
            "고시 관련. Beta.\n",
        ),
    ];

    for (input, expected) in cases {
        let (got, remaining_drift) = rewrite_anvil_readme(input);
        assert_eq!(
            normalise(&got),
            normalise(expected),
            "input {input:?} must lose exactly the exemption sentence"
        );
        assert_eq!(
            got.lines().count(),
            expected.lines().count(),
            "input {input:?} must keep its line structure, got {got:?}"
        );
        assert!(
            remaining_drift.is_empty(),
            "input {input:?}: {remaining_drift:?}"
        );
    }
}

#[test]
fn a_page_that_ends_at_the_exemption_sentence_is_not_overrun() {
    // No trailing newline to consume, so an `end` that walks one past the
    // sentence terminator indexes out of bounds instead of stopping. The first
    // case is the degenerate one the current code survives; the second is the
    // same file boundary with prose after the sentence, which it destroys.
    let (got, remaining_drift) =
        rewrite_anvil_readme("Anvil does **not** yet amend existing documents.");
    assert_eq!(
        got, "",
        "the page was one exemption sentence and nothing else; removing it leaves \
         nothing behind"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");

    let (got, remaining_drift) =
        rewrite_anvil_readme("Alpha. DocGuard does **not** yet amend existing documents. Beta.");
    assert_eq!(
        normalise(&got),
        normalise("Alpha. Beta."),
        "an unterminated last line is still a line; prose after the exemption \
         sentence survives there too"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");

    // The shape neither case above actually exercises, and the one the required
    // rewrite makes reachable: the marker's own sentence runs to end-of-page with
    // NO terminator and NO trailing newline. Both cases above end the marker's
    // sentence on a `.`, so `end` lands on a real terminator; every fixture in
    // `an_exemption_sentence_with_no_terminator_is_clamped_and_takes_nothing_beyond_it`
    // has a `\n` or a `|` to clamp on.
    //
    // This branch must replace `rest.find('\n').unwrap_or(rest.len())` with
    // "find the terminator and consume it", and the natural shapes of that —
    // `rest.find(is_boundary).map(|i| i + boundary_len).unwrap_or(rest.len() + 1)`,
    // or an `.unwrap()` on the `find` — pass the whole of the rest of this suite
    // and then panic (`byte index N is out of bounds` inside `replace_range`, or
    // an unwrap panic) the first time an owned page of Anvil's ends here. That
    // panic happens inside the review pipeline, on a real PR.
    let (got, remaining_drift) =
        rewrite_anvil_readme("Alpha. DocGuard does **not** yet amend existing documents");
    assert_eq!(
        normalise(&got),
        normalise("Alpha."),
        "a marker sentence that simply runs out of page ends at the end of the \
         page: there is no terminator to consume and none to walk past: {got:?}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");

    // The same file boundary inside a table row that never closes its cell, so
    // the clamp the row would normally supply is absent too.
    let (got, remaining_drift) =
        rewrite_anvil_readme("| Gate | It does **not** yet amend existing documents");
    assert_eq!(
        normalise(&got),
        normalise("| Gate |"),
        "the row's untouched first cell survives; the unterminated, unclamped \
         sentence ends at the end of the page: {got:?}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");
}

#[test]
fn an_exemption_marker_at_the_start_of_the_page_takes_only_its_own_sentence() {
    let (got, remaining_drift) =
        rewrite_anvil_readme("does **not** yet amend existing documents. Beta sentence.\nGamma.\n");

    assert_eq!(
        normalise(&got),
        normalise("Beta sentence.\nGamma.\n"),
        "a marker at byte 0 has no preceding sentence to reach back to; only its \
         own sentence goes"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");
}

#[test]
fn every_occurrence_of_the_exemption_is_removed_not_just_the_first() {
    // `remaining_claim` fails the gate on any surviving marker, so a
    // first-occurrence-only deletion hard-fails Anvil's own gate 1 on a page
    // the sync was supposed to have fixed.
    let cases: &[(&str, &str)] = &[
        // On separate lines.
        (
            "# Anvil\n\nAlpha. DocGuard does **not** yet amend existing documents. Beta.\n\
             Gamma. Anvil does **not** yet amend existing documents. Delta.\n",
            "# Anvil\n\nAlpha. Beta.\nGamma. Delta.\n",
        ),
        // Two occurrences of the SAME variant on ONE line. The first deletion
        // shifts the second's byte offset, and the first sentence's terminator
        // becomes the second sentence's start boundary. Two implementations
        // that pass every other case here fail this one: a per-line pass
        // (`for line in text.lines() { if line.contains(marker) { delete one
        // sentence } }`) removes only the first and leaves a marker for
        // `remaining_claim` to hard-fail on; and collecting `match_indices`
        // up front, then deleting in ascending order without adjusting for the
        // bytes already removed, slices at stale offsets and can land
        // mid-character.
        (
            "Alpha. X does **not** yet amend existing documents. Beta. Y does **not** yet amend existing documents. Gamma.\n",
            "Alpha. Beta. Gamma.\n",
        ),
        // One of each variant on one line. `EXEMPTION_MARKERS` is a loop over
        // markers, so each pass sees a string the previous pass already edited.
        (
            "Alpha. X does **not** yet amend existing documents. Beta. Y does not yet amend existing documents. Gamma.\n",
            "Alpha. Beta. Gamma.\n",
        ),
    ];

    for (input, expected) in cases {
        let (got, remaining_drift) = rewrite_anvil_readme(input);

        assert!(
            !got.contains(EXEMPTION_MARKER),
            "input {input:?}: every occurrence must be removed, not only the \
             first:\n{got}"
        );
        assert!(
            !got.contains(PLAIN_EXEMPTION_MARKER),
            "input {input:?}: the unbolded variant counts as an occurrence too:\n{got}"
        );
        assert_eq!(
            normalise(&got),
            normalise(expected),
            "input {input:?}: each occurrence takes exactly its own sentence"
        );
        assert_eq!(
            got.lines().count(),
            expected.lines().count(),
            "input {input:?} must keep its line structure, got {got:?}"
        );
        assert!(
            remaining_drift.is_empty(),
            "input {input:?}: a surviving marker is reported as drift and fails the \
             gate: {remaining_drift:?}"
        );
    }
}

#[test]
fn the_plain_text_exemption_variant_is_removed_the_same_way() {
    let (got, remaining_drift) = rewrite_anvil_readme(
        "# Anvil\n\nAlpha sentence. DocGuard does not yet amend existing documents. Beta sentence.\nGamma line.\n",
    );

    assert!(
        !got.contains(PLAIN_EXEMPTION_MARKER),
        "the unbolded variant is an exemption marker too: {got}"
    );
    assert_eq!(
        normalise(&got),
        normalise("# Anvil\n\nAlpha sentence. Beta sentence.\nGamma line.\n"),
        "the unbolded variant loses exactly its own sentence"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");
}

// =========================================================================
// Issue #29 at the gate that actually decides the merge
// =========================================================================
//
// `tests/docguard_oracle_repair_gate_test.rs` pins `DocGuardReport`.
// `DocGuardReport` is a value; the merge decision is not made there. `PreMergeGuard::evaluate_pre_merge_gates` maps the
// report onto gate 1's `GateStatus`, and `PreMergeCertificationReport::seal()`
// sets `is_certified_ready = all_statuses().all(is_acceptable)`.
//
// The mapping, today, is:
//
//     if let Some(err) = &report.errored                  { Errored(err) }
//     else if !report.files_created_or_updated.is_empty() { AutoUpdated }
//     else if report.is_sufficient                        { Passed }
//     else                                                { Failed(summary) }
//
// `is_sufficient` is never consulted once the file list is non-empty, and
// `AutoUpdated.is_acceptable()` is `true`. So the report
// `an_under_documented_diff_does_not_pass_through_the_public_gate` demands —
// `is_sufficient: false`, `errored: None`,
// `files_created_or_updated: ["docs/reference/newly-public.md"]` — certifies the
// pull request as ready. An engineer implements exactly what that case asks for,
// every case in this suite goes green, and gate 1 still passes every
// under-documented diff the probe flagged: DocGuard dutifully wrote a stub, the
// stub makes the file list non-empty, and the evaluator calls that AUTO-SYNCED.
//
// So the mapping is pinned here, in the same branch as the report it consumes.
// `pre_merge_guard::evaluator::doc_parity_status` is the seam: the scaffolding
// extracted the evaluator's inline mapping into a public function, unchanged,
// and `evaluate_pre_merge_gates` calls it. Nothing else in `tests/` pins this —
// `scorecard_wiring_test.rs` is the only other file that touches
// `doc_parity_status`, and only by assigning a `GateStatus` to it directly.
//
// Asserted through `is_acceptable()` and through a sealed report's
// `is_certified_ready`, never against a particular `GateStatus` variant: the
// behaviour under test is the merge decision, not the enum. An implementation
// that adds a variant, or that reports an under-documented diff as `Errored`
// rather than `Failed`, is free to.

/// A `DocGuardReport` with the exact field combination under test.
fn doc_report(
    is_sufficient: bool,
    errored: Option<&str>,
    files: &[&str],
    summary: &str,
) -> DocGuardReport {
    DocGuardReport {
        is_sufficient,
        errored: errored.map(|s| s.to_string()),
        files_created_or_updated: files.iter().map(|f| (*f).to_string()).collect(),
        summary: summary.to_string(),
    }
}

/// Whether a certification report carrying this gate-1 status still certifies.
///
/// Every other gate is `NotMeasured`, which is acceptable, so the verdict is
/// `doc_parity_status`'s alone.
fn certifies_with(status: GateStatus) -> bool {
    let mut report = PreMergeCertificationReport::unmeasured("not evaluated in this fixture");
    report.doc_parity_status = status;
    report.seal();
    report.is_certified_ready
}

/// The gate's stated reason, however the status chooses to carry it.
///
/// Read through `Debug` rather than by matching a variant, so this pins that the
/// reason reaches the certification report at all without dictating which
/// variant carries it or what that variant is called.
fn stated_reason(status: &GateStatus) -> String {
    format!("{status:?}")
}

#[test]
fn a_diff_the_probe_judged_under_documented_does_not_certify_because_a_stub_was_written() {
    // The decisive case, and the live defect: this is the exact report
    // `an_under_documented_diff_does_not_pass_through_the_public_gate` requires
    // DocGuard to produce. The stub DocGuard wrote is a real file and listing it
    // is honest; what is not honest is reading "a file was written" as "the
    // documentation gap is closed". An auto-generated stub carrying the symbol's
    // name in a heading is evidence of the gap, not its repair.
    let status = doc_parity_status(&doc_report(
        false,
        None,
        &["docs/reference/newly-public.md"],
        MISSING_REASON,
    ));

    assert!(
        !status.is_acceptable(),
        "the probe judged this diff under-documented and DocGuard wrote a stub; a \
         stub is not documentation, and gate 1 must not accept it. status: {status:?}"
    );
    assert!(
        !certifies_with(status.clone()),
        "with every other gate unmeasured, this status alone decides the verdict, \
         and an under-documented diff must not certify as ready to merge: {status:?}"
    );
    assert!(
        stated_reason(&status).contains(MISSING_REASON),
        "a blocked pull request must carry the probe's finding to the scorecard; a \
         status that blocks without saying why is unactionable: {status:?}"
    );
}

#[test]
fn a_diff_the_probe_judged_under_documented_does_not_certify_when_no_file_was_written() {
    // The complement, and a regression fence: this arm is correct today
    // (`Failed(summary)`), and the repair for the case above must not break it by
    // keying the decision on the file list from the other side.
    let status = doc_parity_status(&doc_report(false, None, &[], MISSING_REASON));

    assert!(
        !status.is_acceptable(),
        "the probe judged the diff under-documented and nothing was written to \
         change that: {status:?}"
    );
    assert!(
        !certifies_with(status.clone()),
        "an under-documented diff must not certify: {status:?}"
    );
    assert!(
        stated_reason(&status).contains(MISSING_REASON),
        "the finding must reach the certification report: {status:?}"
    );
}

#[test]
fn a_probe_that_produced_no_judgement_does_not_certify() {
    // Absent evidence, at the gate. Correct today, and fenced here for the same
    // reason: the repair above rearranges this chain, and `Errored` is the arm
    // whose historical collapse into a pass made gate 1 unfailable.
    for failure in PROBE_FAILURES {
        let status = doc_parity_status(&doc_report(
            false,
            Some(failure),
            &[],
            "Documentation parity could not be evaluated",
        ));

        assert!(
            !status.is_acceptable(),
            "{failure:?}: no judgement was obtained, and absent evidence is never a \
             pass: {status:?}"
        );
        assert!(
            !certifies_with(status.clone()),
            "{failure:?}: a gate that could not measure must not certify: {status:?}"
        );
        assert!(
            stated_reason(&status).contains(failure),
            "{failure:?}: the reason the gate could not judge must reach the \
             certification report, so it can be told apart from a documentation \
             finding: {status:?}"
        );
    }
}

#[test]
fn an_errored_gate_does_not_certify_even_when_a_page_was_rewritten() {
    // The pairing that produces the wrong answer if the chain is reordered to
    // consult the file list first: the corpus sync really did rewrite Anvil's
    // README, and the probe really did fail. Work done is not evidence about the
    // diff, so the file list must not out-rank `errored`.
    let failure = PROBE_FAILURES[0];
    let status = doc_parity_status(&doc_report(
        false,
        Some(failure),
        &["README.md"],
        "Documentation parity could not be evaluated",
    ));

    assert!(
        !status.is_acceptable(),
        "a rewritten README says nothing about whether the diff is documented: \
         {status:?}"
    );
    assert!(
        !certifies_with(status.clone()),
        "absent evidence must not certify, whatever else the gate got done: {status:?}"
    );
    assert!(
        stated_reason(&status).contains(failure),
        "the probe failure must still be the stated reason: {status:?}"
    );
}

#[test]
fn a_sufficient_diff_certifies_and_a_rewritten_owned_page_does_not_block_it() {
    // The fence on the other side, and the reason it matters: the corpus sync's
    // whole purpose is to repair Anvil's own published counts in the course of
    // certifying a pull request, and gate 1 accepting that is what makes the
    // repair land instead of blocking every Anvil PR that touches a drifted page.
    // A repair for the decisive case above that simply stops accepting a
    // non-empty file list breaks this.
    let rewritten = doc_parity_status(&doc_report(
        true,
        None,
        &["README.md"],
        &format!("Published docs rewritten to TOTAL_GATES={TOTAL_GATES}: README.md"),
    ));
    assert!(
        rewritten.is_acceptable(),
        "the probe judged the diff documented and the sync repaired a page of \
         Anvil's own; that is a pass, not a finding: {rewritten:?}"
    );
    assert!(
        certifies_with(rewritten.clone()),
        "an auto-synced page must not block certification: {rewritten:?}"
    );

    // And the plain pass, so the assertions above are not satisfied by an
    // implementation that has simply stopped distinguishing anything.
    let clean = doc_parity_status(&doc_report(
        true,
        None,
        &[],
        "Documentation and SSOT frontmatters satisfy the required fields and parity rules.",
    ));
    assert!(
        clean.is_acceptable(),
        "nothing adverse was found and nothing needed doing: {clean:?}"
    );
    assert!(
        certifies_with(clean.clone()),
        "a documented diff on a clean corpus certifies: {clean:?}"
    );
}

// =========================================================================
// Issue #29 at the function that actually makes the merge decision
// =========================================================================
//
// The five cases above pin `pre_merge_guard::evaluator::doc_parity_status`. That
// is where the decision is *written*, and it is not the same claim as "that is
// where the decision is *made*". The branch's own scaffolding doc comment on
// that function records the difference —
//
//     "must keep being called from `evaluate_pre_merge_gates` — a second,
//      private copy of it inside the evaluator would put the decision back out
//      of the suite's reach"
//
// — as a REQUIREMENT FOR THE IMPLEMENTER that, until this case, nothing
// enforced. Nothing else in `tests/` touches it either: `scorecard_wiring_test`
// and `naming_law_survivors_test` only assign a `GateStatus` to the report field
// directly.
//
// The wrong implementation is not contrived. While repairing the mapping the
// implementer wants gate 1's `Failed` reason to also carry the corpus-sync note,
// finds it reads better beside the other sixty-nine gates that are mapped
// inline, and folds the logic back into `evaluate_pre_merge_gates` — leaving
// `doc_parity_status` correctly repaired, publicly exported, and uncalled. All
// five cases above go green, `certifies_with` still returns the right answers,
// and gate 1 in production goes on reading a stub-driven non-empty
// `files_created_or_updated` as `AutoUpdated` and certifying every
// under-documented diff the probe flagged.
//
// On a branch whose subject is the honesty law, a specification requirement that
// lives only in a doc comment is that same defect one level up — which is
// exactly the argument this branch used to justify making `Probe` an enum
// rather than trusting a comment about `take()`.
//
// So the decision is pinned at the function that makes it. The other
// sixty-nine guard reports are neutral: every one of them is the value that
// leaves its own gate acceptable, so the only thing that varies between the two
// runs below is the `DocGuardReport`.
//
// STATED LIMIT, measured rather than assumed, and the reason this case does not
// assert `!report.is_certified_ready` even though that is the sentence one would
// like to write. `evaluate_pre_merge_gates` runs two gates against Anvil's OWN
// tree rather than against the reports it is handed: `brand_absence_status` and
// `migration_boundary_status`. On this tree the brand-absence gate is
// `Failed("12 name(s) or PR-visible string(s) ...")` — twelve pre-existing
// violations in `src/migration/registry.rs`, none of them this branch's, and the
// gate is a real measurement rather than a fixture input, so no test can supply
// a value that clears it. `is_certified_ready` is therefore `false` for every
// report this fixture can build, and asserting it would be a test that passes
// whether or not gate 1 is correct — the exact defect this suite exists to stop.
//
// What replaces it is stronger, not weaker: the two runs differ in nothing but
// the `DocGuardReport`, so the number of gates withholding certification must
// differ by exactly one. That is independent of the tree's own brand-absence
// debt, it is false today, and no "block everything" or "accept everything"
// implementation satisfies it. The merge arithmetic itself —
// `GateStatus::is_acceptable()` conjoined by `seal()` — is pinned by
// `certifies_with` in the five cases above.

/// The other sixty-nine guard reports, each carrying the value that leaves its
/// own gate acceptable.
///
/// Held in one struct rather than passed as a sixty-nine-element tuple so the
/// two runs below are visibly identical apart from the `DocGuardReport`. Every
/// field is the neutral value for its gate: nothing here is evidence about
/// anything, and no assertion in this file reads any of it. `NEUTRAL` is the
/// summary string on every report that has one, so a summary that leaked into
/// gate 1's reason would be visible rather than plausible.
struct NeutralGuardReports {
    cedar: anvil::cedar_guard::CedarGuardReport,
    compliance: anvil::compliance_guard::ComplianceGuardReport,
    api_contract: anvil::api_contract_guard::ApiContractReport,
    cell: anvil::cell_isolation_guard::CellIsolationReport,
    supply_chain: anvil::supply_chain_guard::SupplyChainReport,
    clean_arch: anvil::clean_architecture_guard::CleanArchitectureReport,
    monorepo: anvil::monorepo_guard::MonorepoGuardReport,
    debt: anvil::debt_shrink_guard::DebtShrinkReport,
    modular: anvil::modularization_guard::ModularizationReport,
    coverage: anvil::coverage_guard::CoverageReport,
    rust_skills: anvil::rust_language_policy::RustSkillsReport,
    kani: anvil::kani_guard::KaniGuardReport,
    slo: anvil::slo_canary_guard::SloCanaryReport,
    adr: anvil::adr_drift_ratchet::AdrReport,
    shuffle: anvil::shuffle_shard_simulator::ShuffleShardReport,
    trace: anvil::trace_context_guard::TraceContextReport,
    constant_work: anvil::constant_work_guard::ConstantWorkReport,
    idempotency: anvil::idempotency_guard::IdempotencyReport,
    finops: anvil::finops_ratchet::FinOpsReport,
    ghost_migration: anvil::ghost_migration_harness::GhostMigrationReport,
    gitops_promo: anvil::gitops_promotion::GitOpsPromotionReport,
    gitops_drift: anvil::gitops_drift_reconciler::GitOpsDriftReport,
    canary: anvil::canary_rollout::CanaryRolloutReport,
    cluster_audit: anvil::cluster_state_auditor::ClusterAuditReport,
    migration_orch: anvil::migration_orchestrator::MigrationLifecycleReport,
    ci_wallclock: anvil::ci_wallclock_ratchet::CiWallclockReport,
    predictive_test: anvil::predictive_test_selector::PredictiveTestReport,
    compile_profile: anvil::compile_time_profiler::CompileProfileReport,
    remote_cache: anvil::remote_cache_optimizer::CacheReport,
    runner_economics: anvil::ci_runner_economics::RunnerEconomicsReport,
    sandbox: anvil::ephemeral_sandbox::SandboxReport,
    cross_service: anvil::cross_service_impact::ServiceImpactReport,
    secret_policy: anvil::ephemeral_secrets::SecretPolicyReport,
    psa: anvil::psa_admission_guard::PsaAdmissionReport,
    shadow_traffic: anvil::shadow_traffic_harness::ShadowTrafficReport,
    unresolved_review: anvil::unresolved_review_guard::UnresolvedReviewReport,
    local_probe: anvil::local_inner_loop::LocalProbeReport,
    semantic_abi: anvil::semantic_abi_ratchet::SemanticAbiReport,
    zero_day: anvil::zero_day_patcher::ZeroDayReport,
    formal: anvil::formal_verification::FormalVerificationReport,
    deadlock: anvil::deadlock_analyzer::DeadlockReport,
    aca: anvil::automated_canary::AutomatedCanaryReport,
    ring: anvil::progressive_rollout::ProgressiveRingReport,
    hermetic: anvil::hermetic_build::HermeticBuildReport,
    openvex: anvil::vex_scanner::OpenVexReport,
    cosign: anvil::cosign_signer::CosignReport,
    chaos_inj: anvil::chaos_injector::ChaosInjectorReport,
    stacked: anvil::stacked_diffs::StackedDiffsReport,
    microbench: anvil::microbenchmark_ratchet::MicrobenchmarkReport,
    jittered: anvil::jittered_backoff::JitteredBackoffReport,
    schema_evo: anvil::schema_evolution::SchemaEvolutionReport,
    auto_rollback: anvil::auto_rollback::AutoRollbackReport,
    wasm: anvil::wasm_sandbox::WasmSandboxReport,
    consistency: anvil::consistency_guard::ConsistencyReport,
    flake_quarantine: anvil::flake_quarantine::FlakeQuarantineReport,
    zero_trust: anvil::zero_trust_workload::ZeroTrustWorkloadReport,
    carbon: anvil::carbon_aware::CarbonComputeReport,
    replay: anvil::replay_harness::ReplayHarnessReport,
    upgrade_train: anvil::upgrade_train::UpgradeTrainReport,
    mutation: anvil::chaos_mutation_guard::MutationAdequacyReport,
    feature_flag: anvil::feature_flag_ratchet::FeatureFlagReport,
    bench: anvil::criterion_bench_ratchet::BenchmarkReport,
    attestation: anvil::attestation_guard::AttestationReport,
    shape: anvil::shape::facade::gate::ShapeGateOutcome,
}

/// The summary string carried by every neutral report that has one.
const NEUTRAL: &str = "neutral in this fixture; it is not evidence about anything";

fn neutral_guard_reports() -> NeutralGuardReports {
    use anvil::clean_architecture_guard::ArchMeasurement;
    use anvil::coverage_guard::CoverageMeasurement;

    let n = || NEUTRAL.to_string();
    NeutralGuardReports {
        cedar: anvil::cedar_guard::CedarGuardReport {
            is_compliant: true,
            files_created_or_updated: Vec::new(),
            summary: n(),
        },
        compliance: anvil::compliance_guard::ComplianceGuardReport {
            is_compliant: true,
            violations: Vec::new(),
            evaluation_date: "2026-08-21".to_string(),
            jurisdictions_evaluated: Vec::new(),
            active_rules_count: 0,
            summary: n(),
        },
        api_contract: anvil::api_contract_guard::ApiContractReport {
            is_intact: true,
            auto_synced_files: Vec::new(),
            summary: n(),
        },
        cell: anvil::cell_isolation_guard::CellIsolationReport {
            is_isolated: true,
            violations: Vec::new(),
            summary: n(),
        },
        supply_chain: anvil::supply_chain_guard::SupplyChainReport {
            is_secure: true,
            audited_packages: 0,
            patched_packages: Vec::new(),
            slsa_provenance_generated: true,
            summary: n(),
        },
        clean_arch: anvil::clean_architecture_guard::CleanArchitectureReport {
            is_clean: true,
            violations: Vec::new(),
            summary: n(),
            measurement: ArchMeasurement::Measured {
                files_inspected: 0,
                files_classified: 0,
            },
            scope: n(),
        },
        monorepo: anvil::monorepo_guard::MonorepoGuardReport {
            is_compliant: true,
            violations: Vec::new(),
            summary: n(),
        },
        debt: anvil::debt_shrink_guard::DebtShrinkReport {
            status: GateStatus::Passed,
            is_acceptable: true,
            total_debt_shrunk: 0,
            violations: Vec::new(),
            summary: n(),
        },
        modular: anvil::modularization_guard::ModularizationReport {
            is_modular: true,
            oversized_files: Vec::new(),
            summary: n(),
        },
        coverage: anvil::coverage_guard::CoverageReport {
            is_sufficient: true,
            estimated_diff_coverage_percent: 100.0,
            executable_lines_added: 0,
            test_lines_added: 0,
            findings: Vec::new(),
            summary: n(),
            measurement: CoverageMeasurement::NothingToMeasure,
        },
        rust_skills: anvil::rust_language_policy::RustSkillsReport {
            is_idiomatic: true,
            findings: Vec::new(),
            rules_evaluated_count: 0,
            categories_evaluated: Vec::new(),
            summary: n(),
            measurement: anvil::rust_language_policy::RustSkillsMeasurement::NothingToMeasure,
        },
        kani: anvil::kani_guard::KaniGuardReport {
            all_unsafe_blocks_documented: true,
            unsafe_blocks_found: 0,
            unsafe_blocks_with_safety_comment: 0,
            violations: Vec::new(),
            summary: n(),
        },
        slo: anvil::slo_canary_guard::SloCanaryReport {
            status: GateStatus::Passed,
            is_compliant: true,
            slos_evaluated: 0,
            violations: Vec::new(),
            summary: n(),
        },
        adr: anvil::adr_drift_ratchet::AdrReport {
            is_compliant: true,
            adrs_evaluated: 0,
            scaffolded_adrs: Vec::new(),
            violations: Vec::new(),
            summary: n(),
        },
        shuffle: anvil::shuffle_shard_simulator::ShuffleShardReport {
            is_isolated: true,
            total_cells: 0,
            cells_per_tenant: 0,
            blast_radius_ratio: 0.0,
            max_tenant_overlap: 0,
            violations: Vec::new(),
            summary: n(),
        },
        trace: anvil::trace_context_guard::TraceContextReport {
            status: GateStatus::Passed,
            is_propagated: true,
            tasks_scanned: 0,
            detached_findings: Vec::new(),
            summary: n(),
        },
        constant_work: anvil::constant_work_guard::ConstantWorkReport {
            is_bounded: true,
            unbounded_findings: Vec::new(),
            summary: n(),
        },
        idempotency: anvil::idempotency_guard::IdempotencyReport {
            is_idempotent: true,
            findings: Vec::new(),
            summary: n(),
        },
        finops: anvil::finops_ratchet::FinOpsReport {
            status: GateStatus::Passed,
            is_cost_optimal: true,
            findings: Vec::new(),
            summary: n(),
        },
        ghost_migration: anvil::ghost_migration_harness::GhostMigrationReport {
            status: GateStatus::Passed,
            is_safe: true,
            migrations_evaluated: 0,
            violations: Vec::new(),
            summary: n(),
        },
        gitops_promo: anvil::gitops_promotion::GitOpsPromotionReport {
            is_pinned: true,
            unpinned_findings: Vec::new(),
            summary: n(),
        },
        gitops_drift: anvil::gitops_drift_reconciler::GitOpsDriftReport {
            status: GateStatus::Passed,
            is_safe: true,
            orphan_findings: Vec::new(),
            summary: n(),
        },
        canary: anvil::canary_rollout::CanaryRolloutReport {
            is_healthy: true,
            current_traffic_percent: 100,
            burn_rate: 0.0,
            summary: n(),
        },
        cluster_audit: anvil::cluster_state_auditor::ClusterAuditReport {
            status: GateStatus::Passed,
            is_synchronized: true,
            drift_findings: Vec::new(),
            summary: n(),
        },
        migration_orch: anvil::migration_orchestrator::MigrationLifecycleReport {
            status: GateStatus::Passed,
            is_ordered: true,
            findings: Vec::new(),
            summary: n(),
        },
        ci_wallclock: anvil::ci_wallclock_ratchet::CiWallclockReport {
            status: GateStatus::Passed,
            is_acceptable: true,
            summary: n(),
        },
        predictive_test: anvil::predictive_test_selector::PredictiveTestReport {
            status: GateStatus::Passed,
            is_optimized: true,
            selected_packages: Vec::new(),
            skipped_packages_count: 0,
            pruning_ratio: 0.0,
            summary: n(),
        },
        compile_profile: anvil::compile_time_profiler::CompileProfileReport {
            is_lean: true,
            findings: Vec::new(),
            summary: n(),
        },
        remote_cache: anvil::remote_cache_optimizer::CacheReport {
            status: GateStatus::Passed,
            is_cache_aligned: true,
            summary: n(),
        },
        runner_economics: anvil::ci_runner_economics::RunnerEconomicsReport {
            is_cost_optimal: true,
            findings: Vec::new(),
            summary: n(),
        },
        sandbox: anvil::ephemeral_sandbox::SandboxReport {
            status: GateStatus::Passed,
            is_hermetic: true,
            sandboxes_allocated: 0,
            average_spinup_ms: 0,
            summary: n(),
        },
        cross_service: anvil::cross_service_impact::ServiceImpactReport {
            is_compatible: true,
            breaking_findings: Vec::new(),
            summary: n(),
        },
        secret_policy: anvil::ephemeral_secrets::SecretPolicyReport {
            is_zero_trust: true,
            findings: Vec::new(),
            summary: n(),
        },
        psa: anvil::psa_admission_guard::PsaAdmissionReport {
            is_compliant: true,
            findings: Vec::new(),
            summary: n(),
        },
        shadow_traffic: anvil::shadow_traffic_harness::ShadowTrafficReport {
            status: GateStatus::Passed,
            is_verified: true,
            summary: n(),
        },
        unresolved_review: anvil::unresolved_review_guard::UnresolvedReviewReport {
            is_clean: true,
            unresolved_threads: Vec::new(),
            summary: n(),
        },
        local_probe: anvil::local_inner_loop::LocalProbeReport {
            is_valid: true,
            latency_ms: 0,
            findings: Vec::new(),
            summary: n(),
        },
        semantic_abi: anvil::semantic_abi_ratchet::SemanticAbiReport {
            status: GateStatus::Passed,
            is_abi_stable: true,
            breaking_findings: Vec::new(),
            summary: n(),
        },
        zero_day: anvil::zero_day_patcher::ZeroDayReport {
            is_clean: true,
            advisories_detected: Vec::new(),
            summary: n(),
        },
        formal: anvil::formal_verification::FormalVerificationReport {
            passed: true,
            findings: Vec::new(),
        },
        deadlock: anvil::deadlock_analyzer::DeadlockReport {
            passed: true,
            findings: Vec::new(),
        },
        aca: anvil::automated_canary::AutomatedCanaryReport {
            status: GateStatus::Passed,
            passed: true,
            verdict: anvil::automated_canary::CanaryVerdict::Pass,
        },
        ring: anvil::progressive_rollout::ProgressiveRingReport {
            passed: true,
            state: anvil::progressive_rollout::RingRolloutState {
                current_ring: anvil::progressive_rollout::DeploymentRing::Ring3GlobalProd,
                target_ring: anvil::progressive_rollout::DeploymentRing::Ring3GlobalProd,
                traffic_pct: 100,
                is_healthy: true,
            },
        },
        hermetic: anvil::hermetic_build::HermeticBuildReport {
            status: GateStatus::Passed,
            passed: true,
            result: anvil::hermetic_build::ReproducibilityResult::DeterministicBitForBit,
        },
        openvex: anvil::vex_scanner::OpenVexReport {
            status: GateStatus::Passed,
            passed: true,
            statements: Vec::new(),
        },
        cosign: anvil::cosign_signer::CosignProvenanceSigner::new()
            .evaluate_without_signing_backend(),
        chaos_inj: anvil::chaos_injector::ChaosInjectorReport {
            passed: true,
            trials: Vec::new(),
        },
        stacked: anvil::stacked_diffs::StackedDiffsReport {
            status: GateStatus::Passed,
            passed: true,
            plan: anvil::stacked_diffs::StackSyncPlan {
                stack_depth: 0,
                rebase_order: Vec::new(),
                atomic_merge_ready: true,
            },
        },
        microbench: anvil::microbenchmark_ratchet::MicrobenchmarkReport {
            status: GateStatus::Passed,
            passed: true,
            verdict: anvil::microbenchmark_ratchet::BenchmarkRegressionVerdict::Optimal,
        },
        jittered: anvil::jittered_backoff::JitteredBackoffReport {
            passed: true,
            unjittered_retries_detected: 0,
            missing_deadline_calls: 0,
            summary: n(),
        },
        schema_evo: anvil::schema_evolution::SchemaEvolutionReport {
            passed: true,
            breaking_field_changes: 0,
            tag_renumbering_detected: false,
            summary: n(),
        },
        auto_rollback: anvil::auto_rollback::AutoRollbackReport {
            status: GateStatus::Passed,
            passed: true,
            rollback_triggered: false,
            summary: n(),
        },
        wasm: anvil::wasm_sandbox::WasmSandboxReport {
            passed: true,
            active_wasm_plugins: 0,
            policy_violations: Vec::new(),
            summary: n(),
        },
        consistency: anvil::consistency_guard::ConsistencyReport {
            passed: true,
            split_brain_risks: 0,
            unversioned_mutations: 0,
            summary: n(),
        },
        flake_quarantine: anvil::flake_quarantine::FlakeQuarantineReport {
            status: GateStatus::Passed,
            passed: true,
            quarantined_tests_isolated: 0,
            rehabilitated_tests_restored: 0,
            summary: n(),
        },
        zero_trust: anvil::zero_trust_workload::ZeroTrustWorkloadReport {
            passed: true,
            spiffe_id_verified: true,
            mtls_enforced: true,
            unauthenticated_endpoints: 0,
            summary: n(),
        },
        carbon: anvil::carbon_aware::CarbonComputeReport {
            status: GateStatus::Passed,
            passed: true,
            estimated_joules_per_build: 0.0,
            green_window_scheduled: true,
            summary: n(),
        },
        replay: anvil::replay_harness::ReplayHarnessReport {
            status: GateStatus::Passed,
            passed: true,
            replayed_fixtures_count: 0,
            divergence_detected: false,
            summary: n(),
        },
        upgrade_train: anvil::upgrade_train::UpgradeTrainReport {
            status: GateStatus::Passed,
            passed: true,
            pending_upgrades_available: 0,
            breaking_major_upgrades: 0,
            summary: n(),
        },
        mutation: anvil::chaos_mutation_guard::MutationAdequacyReport {
            is_adequate: true,
            surviving_findings: Vec::new(),
            summary: n(),
            // #77 replaced the filename-substring check with a real
            // cargo-mutants run, so the count field is gone. This fixture is a
            // neutral stand-in that no case reads back.
            measurement: anvil::chaos_mutation_guard::MutationMeasurement::NothingToMeasure,
        },
        feature_flag: anvil::feature_flag_ratchet::FeatureFlagReport {
            is_clean: true,
            flags_scanned_count: 0,
            violations: Vec::new(),
            summary: n(),
        },
        bench: anvil::criterion_bench_ratchet::BenchmarkReport {
            is_within_budget: true,
            hot_paths_evaluated: 0,
            violations: Vec::new(),
            summary: n(),
        },
        attestation: anvil::attestation_guard::AttestationReport {
            status: anvil::pre_merge_guard::GateStatus::Passed,
            stamped_receipt_path: None,
            summary: n(),
        },
        shape: anvil::shape::facade::gate::ShapeGateOutcome::NoSpec { reason: n() },
    }
}

/// Runs the full gate matrix over `doc` and the neutral reports.
///
/// Nothing here spawns anything: `evaluate_pre_merge_gates` is arithmetic over
/// the reports it is handed, plus two self-directed gates that read Anvil's own
/// source tree.
fn certification_report_for(doc: &DocGuardReport) -> PreMergeCertificationReport {
    let r = neutral_guard_reports();
    // Never the developer's own checkout: the fixture's working directory is a
    // tempdir, so an implementation that decided to act on `repo_working_dir`
    // cannot reach the tree this suite is running out of.
    let workdir = tempdir().unwrap();
    let ctx = PrDiffContext {
        repo: ANVIL.to_string(),
        pr_number: 77,
        base_branch: "main".to_string(),
        base_sha: "base-sha".to_string(),
        head_sha: "head-sha".to_string(),
        is_incremental: false,
        previous_head_sha: None,
        diff_content: "+pub fn newly_public() {}\n".to_string(),
        changed_files: vec!["src/lib.rs".to_string()],
        repo_working_dir: workdir.path().to_path_buf(),
    };

    PreMergeGuard::new()
        .evaluate_pre_merge_gates(
            &ctx,
            doc,
            &r.cedar,
            &r.compliance,
            &r.api_contract,
            &r.cell,
            &r.supply_chain,
            &r.clean_arch,
            &r.monorepo,
            &r.debt,
            &r.modular,
            &r.coverage,
            &r.rust_skills,
            &r.kani,
            &r.slo,
            &r.adr,
            &r.shuffle,
            &r.trace,
            &r.constant_work,
            &r.idempotency,
            &r.finops,
            &r.ghost_migration,
            &r.gitops_promo,
            &r.gitops_drift,
            &r.canary,
            &r.cluster_audit,
            &r.migration_orch,
            &r.ci_wallclock,
            &r.predictive_test,
            &r.compile_profile,
            &r.remote_cache,
            &r.runner_economics,
            &r.sandbox,
            &r.cross_service,
            &r.secret_policy,
            &r.psa,
            &r.shadow_traffic,
            &r.unresolved_review,
            &r.local_probe,
            &r.semantic_abi,
            &r.zero_day,
            &r.formal,
            &r.deadlock,
            &r.aca,
            &r.ring,
            &r.hermetic,
            &r.openvex,
            &r.cosign,
            &r.chaos_inj,
            &r.stacked,
            &r.microbench,
            &r.jittered,
            &r.schema_evo,
            &r.auto_rollback,
            &r.wasm,
            &r.consistency,
            &r.flake_quarantine,
            &r.zero_trust,
            &r.carbon,
            &r.replay,
            &r.upgrade_train,
            &r.mutation,
            &r.feature_flag,
            &r.bench,
            &r.attestation,
            Some(true),
            "APPROVE",
            &r.shape,
        )
        .expect("the evaluator is arithmetic over the reports it is handed")
}

/// How many of the report's gates withhold certification.
fn gates_withholding_certification(report: &PreMergeCertificationReport) -> usize {
    report
        .all_statuses()
        .iter()
        .filter(|s| !s.is_acceptable())
        .count()
}

#[test]
fn a_stub_written_for_an_under_documented_diff_does_not_certify_through_the_evaluator() {
    let under_documented = doc_report(
        false,
        None,
        &["docs/reference/newly-public.md"],
        MISSING_REASON,
    );
    let documented = doc_report(
        true,
        None,
        &["README.md"],
        &format!("Published docs rewritten to TOTAL_GATES={TOTAL_GATES}: README.md"),
    );

    let blocked = certification_report_for(&under_documented);
    let certified = certification_report_for(&documented);

    // 1. The decision itself, at the gate that makes it.
    assert!(
        !blocked.doc_parity_status.is_acceptable(),
        "the probe judged this diff under-documented and DocGuard wrote a stub. \
         A stub carrying the symbol's name in a heading is evidence of the gap, \
         not its repair, and this is the function where that stops being a merge. \
         gate 1: {:?}",
        blocked.doc_parity_status
    );
    assert!(
        stated_reason(&blocked.doc_parity_status).contains(MISSING_REASON),
        "the scorecard row a contributor reads is composed from this status, so \
         the probe's finding must reach it; a gate that blocks without saying why \
         is unactionable: {:?}",
        blocked.doc_parity_status
    );

    // 2. The counterweight, so none of this is satisfied by an evaluator that
    //    has simply stopped accepting anything. The corpus sync's whole purpose
    //    is to repair Anvil's own published counts while certifying a pull
    //    request, and gate 1 accepting that is what makes the repair land
    //    instead of blocking every Anvil PR that touches a drifted page.
    assert!(
        certified.doc_parity_status.is_acceptable(),
        "the probe judged the diff documented and the sync repaired a page of \
         Anvil's own; that is a pass, not a finding: {:?}",
        certified.doc_parity_status
    );

    // 3. The requirement that had no test: the evaluator must reach gate 1's
    //    verdict through the same decision this file pins directly. A private
    //    re-inlined copy that disagrees with the exported one is caught here,
    //    and an unrepaired copy disagrees on exactly the report above.
    //
    //    Asserted as equality with `doc_parity_status`, not against a named
    //    variant, so the implementer stays free to choose how the block is
    //    reported — only that the evaluator and the exported decision cannot
    //    diverge.
    assert_eq!(
        blocked.doc_parity_status,
        doc_parity_status(&under_documented),
        "gate 1 in a certification report must be the status \
         `pre_merge_guard::evaluator::doc_parity_status` maps the DocGuardReport \
         to. A second, private copy inside `evaluate_pre_merge_gates` puts the \
         merge decision back out of this suite's reach and leaves the exported \
         function correctly repaired, publicly visible, and uncalled"
    );
    assert_eq!(
        certified.doc_parity_status,
        doc_parity_status(&documented),
        "the same, on the arm that must keep certifying"
    );

    // 4. The merge arithmetic, expressed as a difference rather than an
    //    absolute. `is_certified_ready` cannot carry weight here — see the
    //    STATED LIMIT above this section — but the two runs differ in NOTHING
    //    except the `DocGuardReport`, so exactly one more gate must withhold
    //    certification in the first. An implementation that blocks everything or
    //    accepts everything fails it, and it does not depend on how much
    //    unrelated debt the tree happens to be carrying.
    assert_eq!(
        gates_withholding_certification(&blocked),
        gates_withholding_certification(&certified) + 1,
        "the only difference between these two certification reports is gate 1's \
         DocGuardReport, so exactly one more gate must withhold certification when \
         the probe judged the diff under-documented.\n\
         under-documented gate 1: {:?}\n\
         documented gate 1:       {:?}",
        blocked.doc_parity_status,
        certified.doc_parity_status
    );
}

// =========================================================================
// Issue #29 at the source — whether a probe run produced a judgement at all
// =========================================================================
//
// `PROBE_FAILURES` above, and its twin in the gate binary, are five strings a
// test wrote. Every case that uses them pins what the gate does with a probe
// failure; not one of them runs the code that decides whether a probe run IS a
// failure. That code is `doc_guard::classify_probe_output`, and the arm of it
// that returns `Err` is the arm whose historical collapse into
// `is_doc_sufficient: true` made gate 1 unfailable — the comment recording that
// is still in `evaluate_doc_parity`. Pinning the consumption of `Err` five ways
// over while leaving its production unpinned is the same shape of gap as the
// defects this branch exists to close.
//
// These cases pin it. They need no environment, no tempdir and no process:
// `classify_probe_output` is pure, which is why they live in this binary rather
// than with the gate cases.
//
// FENCES, disclosed: they pinned an already-correct part of the oracle before
// this branch, for the same reason and with the same justification as the four
// `doc_parity_status` cases named at the head of this file. The classification
// was EXTRACTED out of `evaluate_doc_parity`'s probe closure verbatim, defect
// and all, rather than being written here. What they buy is that the repair for
// issues #27, #28 and #29 — which reaches into this function's caller — cannot
// quietly restore the collapse while the rest of the suite stays green. Their
// falsifiability is not assumed: each case names the mutation it kills.
//
// There is exactly one call site: the probe closure in `evaluate_doc_parity`
// hands `run_bounded_for`'s output straight to this function, so there is no
// second, private copy for the gate to reach its verdict through instead. The
// seam that used to supply a completed probe run to the gate — so that a test
// could bind the two together — was removed on review: it bought this same
// assertion a second time, at the price of a third `Probe` arm and a second
// public constructor.

/// A completed process' exit status, built rather than obtained.
///
/// STATED COST: there is no portable constructor, so on a non-unix target this
/// fixture cannot be built. It panics there rather than letting the cases that
/// need it disappear from the run — a vanished test publishes the same false
/// assurance as a vacuous one, which is the defect class this suite exists to
/// prevent.
#[cfg(unix)]
fn exit_status(code: i32) -> std::process::ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code << 8)
}

#[cfg(not(unix))]
fn exit_status(_code: i32) -> std::process::ExitStatus {
    panic!(
        "this fixture needs a constructed `ExitStatus`, which only the unix \
         extension trait provides. The behaviour is not satisfied on this \
         platform, it is unmeasured on it"
    )
}

/// The judgement body the probe's own prompt asks the model to print. Held as
/// text, not as a `DocParityEvaluation`, because the thing under test is the
/// reading of it.
const PRINTED_JUDGEMENT: &str = "{\"is_doc_sufficient\": false, \
     \"missing_doc_summary\": \"newly_public is a new public API with no reference page\", \
     \"doc_files_to_update\": [\"docs/reference/newly-public.md\", \"CHANGELOG.md\"], \
     \"suggested_adr_title\": null}";

#[test]
fn a_probe_run_that_printed_a_judgement_is_that_judgement() {
    // The counterweight to the `Err` cases below, and it is not optional:
    // without it, "classify everything as Err" satisfies all of them and gate 1
    // becomes unpassable instead of unfailable. Both failures publish a
    // scorecard that is not about the diff.
    //
    // Three layouts, because all three are what a model really prints against
    // this prompt: the fenced block the prompt asks for, that block with the
    // model's own commentary around it, and the bare object a model prints when
    // it takes "output strictly valid JSON" literally.
    let layouts: &[(&str, String)] = &[
        (
            "the fenced block the prompt asks for",
            format!("```json\n{PRINTED_JUDGEMENT}\n```\n"),
        ),
        (
            "the same block with the model's commentary around it",
            format!(
                "Here is my assessment.\n\n```json\n{PRINTED_JUDGEMENT}\n```\n\nHope that helps.\n"
            ),
        ),
        ("the bare object", format!("{PRINTED_JUDGEMENT}\n")),
    ];

    for (label, stdout) in layouts {
        let eval = classify_probe_output(exit_status(0), stdout, "").unwrap_or_else(|e| {
            panic!(
                "{label}: the probe ran, exited zero and printed a judgement. \
                 Reporting no judgement here turns every well-behaved probe run \
                 into a blocked pull request: {e}"
            )
        });

        // The verdict, the reason and the file list are what the rest of the
        // gate acts on, so all three must survive unaltered — a classifier that
        // returned a DEFAULT judgement satisfies an assertion on the verdict
        // alone.
        assert!(
            !eval.is_doc_sufficient,
            "{label}: the probe said this diff is under-documented"
        );
        assert_eq!(
            eval.missing_doc_summary.as_deref(),
            Some(MISSING_REASON),
            "{label}: the probe's stated reason is what reaches the contributor's \
             scorecard, so it must survive"
        );
        assert_eq!(
            eval.doc_files_to_update,
            vec![
                "docs/reference/newly-public.md".to_string(),
                "CHANGELOG.md".to_string()
            ],
            "{label}: the files the probe named are what the gate acts on"
        );
        assert!(
            eval.suggested_adr_title.is_none(),
            "{label}: the probe suggested no ADR title: {:?}",
            eval.suggested_adr_title
        );
    }

    // The other verdict, so nothing here is satisfied by a classifier that reads
    // every judgement as adverse.
    let printed_sufficient = "```json\n{\"is_doc_sufficient\": true, \
         \"missing_doc_summary\": null, \"doc_files_to_update\": [], \
         \"suggested_adr_title\": null}\n```\n";
    let eval = classify_probe_output(exit_status(0), printed_sufficient, "")
        .expect("a probe that printed a judgement of sufficiency produced a judgement");
    assert!(
        eval.is_doc_sufficient,
        "the probe judged this diff documented, and a classifier that cannot \
         report that blocks every pull request Anvil reviews"
    );
    assert!(
        eval.doc_files_to_update.is_empty(),
        "{:?}",
        eval.doc_files_to_update
    );
}

#[test]
fn a_probe_run_that_printed_no_judgement_is_not_a_judgement() {
    // THE case. A probe that exited zero has told the gate nothing unless it
    // printed something the gate can read, and "nothing" is not "sufficient".
    // The mutation this kills is the historical one: an
    // `Ok(DocParityEvaluation { is_doc_sufficient: true, .. })` on the
    // unparseable path, which makes gate 1 unfailable for every model outage,
    // refusal, truncation and prompt regression at once — silently, because the
    // run looks successful from the outside.
    //
    // Six shapes, all of them things a real probe prints.
    let printed_nothing_usable: &[(&str, String)] = &[
        ("nothing at all", String::new()),
        ("only whitespace", "   \n\n".to_string()),
        (
            "prose with no JSON in it — a refusal, or an outage message",
            "I was unable to review this diff.\n".to_string(),
        ),
        (
            "a block cut off mid-object, which is what a truncated stream leaves",
            "```json\n{\n  \"is_doc_sufficient\": tr".to_string(),
        ),
        (
            "a fenced block whose contents are not JSON",
            "```json\nis_doc_sufficient: false\n```\n".to_string(),
        ),
        (
            "valid JSON that answers a different question",
            "```json\n{\"answer\": \"the docs look fine to me\"}\n```\n".to_string(),
        ),
    ];

    for (label, stdout) in printed_nothing_usable {
        let err = classify_probe_output(exit_status(0), stdout, "")
            .err()
            .unwrap_or_else(|| {
                panic!(
                    "{label}: the probe exited zero but said nothing about this \
                     diff. A judgement obtained from this is a judgement nobody \
                     made, and reading it as sufficiency is the collapse that made \
                     gate 1 unfailable. stdout was: {stdout:?}"
                )
            });
        assert!(
            !err.to_string().trim().is_empty(),
            "{label}: a gate blocked on absent evidence must say what was absent, \
             or a contributor has nothing to act on"
        );
    }
}

#[test]
fn a_probe_run_that_exited_non_zero_is_not_a_judgement() {
    const STDERR: &str = "permission check failed for command\n";

    // A judgement on stdout AND a failure exit. The run failed, so its answer is
    // not taken — an implementation that reads stdout first and consults the
    // status only when the parse fails passes every other case in this file
    // while certifying diffs on the strength of a probe invocation that did not
    // work.
    let printed_a_judgement_anyway = format!("```json\n{PRINTED_JUDGEMENT}\n```\n");
    let err = classify_probe_output(exit_status(1), &printed_a_judgement_anyway, STDERR)
        .err()
        .unwrap_or_else(|| {
            panic!(
                "the probe exited non-zero. Whatever it printed on the way out is \
                 not a judgement the gate may act on"
            )
        });
    let message = err.to_string();
    assert!(
        !message.trim().is_empty(),
        "a gate blocked on a failed probe must say so"
    );
    assert!(
        message.contains(STDERR.trim()),
        "the run's own stderr is what tells an operator whether this was a broken \
         invocation or a broken repository, and it is the only diagnostic that \
         exists on this path. got: {message}"
    );

    // Which status it exited with is part of that diagnostic. Asserted as a
    // difference rather than against a literal, so this says "the exit status
    // reaches the message" without pinning how it is rendered.
    let one = classify_probe_output(exit_status(1), "", STDERR)
        .expect_err("non-zero is never a judgement")
        .to_string();
    let two = classify_probe_output(exit_status(2), "", STDERR)
        .expect_err("non-zero is never a judgement")
        .to_string();
    assert_ne!(
        one, two,
        "two probe runs that failed differently must not produce the same line on \
         a contributor's scorecard: the exit status is the distinguishing detail"
    );
}

#[test]
fn the_ways_a_probe_can_fail_are_told_apart_in_what_it_reports() {
    // Absent evidence blocks, and it blocks the same way whichever shape it
    // took — so the only thing that tells an operator what to do about it is
    // what the gate says. A classifier that reports one fixed string for
    // everything satisfies every "non-empty" assertion above while leaving a
    // model outage, a broken invocation and an abandoned run looking identical
    // on the scorecard.
    let printed_nothing = classify_probe_output(exit_status(0), "I could not review this.", "")
        .expect_err("no parseable judgement is no judgement")
        .to_string();
    let exited_non_zero = classify_probe_output(exit_status(1), "", "permission check failed")
        .expect_err("a non-zero exit is no judgement")
        .to_string();

    let reported = [
        ("ran, printed nothing usable", printed_nothing),
        ("exited non-zero", exited_non_zero),
    ];
    for (i, (a_label, a)) in reported.iter().enumerate() {
        for (b_label, b) in reported.iter().skip(i + 1) {
            assert_ne!(
                a, b,
                "a probe that {a_label} and a probe that {b_label} are different \
                 failures with different remedies, and the gate's report is the \
                 only place that difference can survive"
            );
        }
    }
}
