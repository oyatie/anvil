//! The naming law (plan §33.1 / §33.5), applied to the components that
//! **survive absorption** — and only those.
//!
//! # What the law says
//!
//! - **L1** Name what it VERIFIES, not the aspiration it invokes.
//! - **L2** Adopt the established term; never coin a synonym.
//! - **L3** Suffix carries architecture, concept carries meaning.
//! - **L4** No brand in a wide-blast-radius name.
//! - **D33.2** No aspiration or category stamps — "hyperscaler", "cloud-native",
//!   "enterprise". This covers DISPLAY STRINGS AND LOG LINES too, because those
//!   reach pull requests.
//!
//! # Why this file exists alongside `brand_absence_gate_test.rs`
//!
//! That file tests the *gate mechanism* — does the scanner flag a synthetic
//! stamp, does the debt ledger ratchet. It deliberately shipped warn-only with
//! every pre-existing violation recorded as debt, because renaming code that is
//! about to be deleted is waste (plan §36.2/C1).
//!
//! This file is the *other half*: now that `src/migration/registry.rs` records
//! which components actually survive, the law is enforced on the survivors. The
//! gate mechanism is reused; what changes is that the allowlist collapses from
//! "31 debt entries covering whatever the tree happened to contain" to "the
//! enumerated superseded modules, which are deleted rather than renamed".
//!
//! # Why the scan is production-only
//!
//! Every scan in this file strips `#[cfg(test)]` items before scanning. This is
//! not tidiness. A test in this repo was previously satisfied by a call that
//! lived inside a `#[cfg(test)] mod tests` block and reported green while the
//! production path did nothing. The same hole exists here in both directions:
//!
//!   - a violation could be "fixed" by moving the offending string into a test
//!     module, and
//!   - test fixtures inflate the debt counts, so a real production violation can
//!     hide underneath an allowlist ceiling that was sized by test code.
//!
//! Verified instance of test-module text that must not count:
//! `src/hyperscaler_consensus_guard/mod.rs:239` asserts on the literal
//! `"5/5 Hyperscalers Approved"` from inside `#[cfg(test)] mod tests`.
//!
//! # Stage discipline
//!
//! These are red tests written before the rename. They are expected to fail.

use anvil::brand_absence::{
    BrandAbsenceGate, BrandViolation, BrandViolationKind, VOCABULARY_DEFINITION_PATH,
};
use anvil::migration::{MIGRATION_LEDGER, Verdict};
use anvil::pre_merge_guard::{GateStatus, PreMergeCertificationReport};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// The allowlist: enumerated, finite, and only the superseded modules
// ---------------------------------------------------------------------------

/// Components that are **deleted at absorption, never renamed**.
///
/// A superseded component is replaced by an oyatie counterpart. Renaming it
/// spends effort on code with a scheduled deletion date and — worse — makes the
/// freshly-invested code harder to argue for deleting later. So the law does not
/// reach these, and this list is the only exemption channel.
///
/// It is a **hardcoded enumeration on purpose**. Not a prefix, not a glob, not
/// "everything the ledger marks Superseded" — a fixed list of 15 names that a
/// reviewer can read in one screen. A predicate-shaped allowlist ("skip anything
/// Superseded") would silently absorb every future violation the moment somebody
/// flipped a verdict in the ledger, which is exactly the failure mode
/// `allowlist_is_finite_enumerated_and_ledger_backed` below exists to prevent.
const SUPERSEDED_OFF_LIMITS: &[&str] = &[
    "cloud_native_guard",
    "debt_shrink_guard",
    "monorepo_guard",
    "cedar_guard",
    "clean_architecture_guard",
    "adr_drift_ratchet",
    "predictive_test_selector",
    "cross_service_impact",
    "supply_chain_guard",
    "upgrade_train",
    "corpus_auditor",
    "zero_trust_workload",
    "account_pool",
    "cli",
];

/// Surviving components whose name coins a synonym for an established term (L2).
///
/// `(current module dir, required module dir, the established term it must adopt)`.
/// The third field is a path in the oyatie tree, checked when that tree is
/// present locally. `/repos` is gitignored, so the citation cannot be verified
/// in CI; the rename itself is enforced unconditionally either way.
/// (current_module, required_module, the established oyatie term it adopts).
///
/// Empty, and deliberately so. The one candidate was investigated and rejected:
/// `rust_skills_guard` (now `rust_language_policy`) was mapped onto oyatie's
/// `ci/facade/automation-language-policy`, but the two are different gates.
/// oyatie's is a Rust-FIRST AUTOMATION ratchet -- its own header reads "the
/// portable contract that non-Rust automation is either absent or has a
/// documented Rust/Buck2/cloud-native replacement path", i.e. it asks whether
/// your scripts are shell or Rust. Anvil's applies 27 categories of Rust IDIOM
/// rules ("Ownership & Borrowing (12 rules)", "Error Handling (18 rules)")
/// synced from an upstream rust-skills repository, i.e. it asks whether your
/// Rust is good Rust.
///
/// Adopting a term that means something else is a worse L1 violation than
/// coining one, because the name would then misdescribe the code permanently
/// and survive the merge doing so. L2 says "never coin a synonym" -- these are
/// not synonyms.
const ESTABLISHED_TERM_RENAMES: &[(&str, &str, &str)] = &[];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

// ---------------------------------------------------------------------------
// Production-only source collection
// ---------------------------------------------------------------------------

/// Every `.rs` file under `src/`, with `#[cfg(test)]` items blanked out.
///
/// Line numbers are preserved (stripped lines become empty), so a violation's
/// reported line still points at the real line in the file.
fn production_sources() -> Vec<(String, String)> {
    let root = repo_root();
    let mut files = Vec::new();
    collect_rs(&root.join("src"), &mut files);
    files.sort();

    let mut out = Vec::new();
    for file in files {
        let rel = file
            .strip_prefix(&root)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        // The file that defines the forbidden vocabulary unavoidably contains
        // every term in it. Production `scan_tree` skips it for the same reason.
        if rel == VOCABULARY_DEFINITION_PATH {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(&file) else {
            continue;
        };
        out.push((rel, strip_cfg_test_items(&body).0));
    }
    out
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Blanks every `#[cfg(test)]`-annotated item. Returns the stripped source and
/// how many items were removed.
///
/// The item's extent is found by indentation: a `#[cfg(test)]` at indentation
/// `n` is closed by the first later line whose indentation is `n` and which
/// begins with `}`. An annotated item with no brace before its first `;` (e.g.
/// `#[cfg(test)] use super::*;`) is a single statement and only that line goes.
fn strip_cfg_test_items(source: &str) -> (String, usize) {
    let lines: Vec<&str> = source.lines().collect();
    let mut keep = vec![true; lines.len()];
    let mut removed = 0usize;

    let mut i = 0usize;
    while i < lines.len() {
        let trimmed = lines[i].trim_start();
        if !trimmed.starts_with("#[cfg(test)]") && !trimmed.starts_with("#[cfg(all(test") {
            i += 1;
            continue;
        }
        let indent = lines[i].len() - trimmed.len();

        // Find where the annotated item opens a block, or ends as a statement.
        let mut j = i;
        let mut open_line: Option<usize> = None;
        while j < lines.len() {
            let body = lines[j].trim_start();
            if body.contains('{') {
                open_line = Some(j);
                break;
            }
            if body.ends_with(';') && j > i {
                break;
            }
            j += 1;
        }

        let end = match open_line {
            None => j.min(lines.len().saturating_sub(1)),
            Some(open) => {
                let mut k = open + 1;
                let mut found = open;
                while k < lines.len() {
                    let t = lines[k].trim_start();
                    let ind = lines[k].len() - t.len();
                    if ind == indent && t.starts_with('}') {
                        found = k;
                        break;
                    }
                    k += 1;
                }
                if found == open {
                    lines.len() - 1
                } else {
                    found
                }
            }
        };

        for slot in keep.iter_mut().take(end + 1).skip(i) {
            *slot = false;
        }
        removed += 1;
        i = end + 1;
    }

    let out: Vec<&str> = lines
        .iter()
        .zip(keep.iter())
        .map(|(l, k)| if *k { *l } else { "" })
        .collect();
    (out.join("\n"), removed)
}

/// Every violation the production gate finds in production source, with no
/// ledger applied. The empty allowlist is deliberate: this file's allowlist is
/// [`SUPERSEDED_OFF_LIMITS`], applied per check below, not the warn-only debt
/// ledger the gate ships with.
fn scan_production() -> Vec<BrandViolation> {
    let gate = BrandAbsenceGate::with_allowlist(Vec::new());
    let mut out = Vec::new();
    for (path, body) in production_sources() {
        out.extend(gate.scan_source(&path, &body).new_violations);
    }
    out
}

/// Whether any segment of `rel` names an off-limits superseded component.
///
/// Segment-wise rather than top-level-only because `account_pool` lives at
/// `src/self_governance/account_pool/`, which a top-level check would miss.
fn lives_in_an_off_limits_module(rel: &str) -> bool {
    rel.split('/').any(|seg| {
        let stem = seg.strip_suffix(".rs").unwrap_or(seg);
        SUPERSEDED_OFF_LIMITS.contains(&stem)
    })
}

/// One violation on one line.
///
/// The snippet is whitespace-collapsed and truncated: several offenders live
/// inside multi-line HTML and Markdown literals, and pasting one of those raw
/// pushes every other violation off the top of the failure message. `line` is
/// the line the *literal* starts on, which for those blocks is above the
/// offending text.
fn describe(v: &BrandViolation) -> String {
    let flat = v.snippet.split_whitespace().collect::<Vec<_>>().join(" ");
    let snippet: String = flat.chars().take(120).collect();
    let ellipsis = if flat.chars().count() > 120 {
        "..."
    } else {
        ""
    };
    format!(
        "{}:{} [{:?} stamp={}] {snippet}{ellipsis}",
        v.path, v.line, v.kind, v.stamp,
    )
}

fn render(violations: &[BrandViolation]) -> String {
    violations
        .iter()
        .map(|v| format!("\n    {}", describe(v)))
        .collect::<String>()
}

// ---------------------------------------------------------------------------
// 1. Names
// ---------------------------------------------------------------------------

/// # Defect this catches
///
/// A module, type, or function name on a component that **survives absorption**
/// stamps an aspiration or a product category onto itself instead of naming the
/// check it performs. Verified in the tree at the time of writing:
///
///   - `src/ai_driver/stage_router.rs:60` — `StageModelRouter`.
///     Read the body: `get_stage_fallback_chain` returns a
///     `StageFallbackChain` of `ModelExecutionConfig` tiers per `AgenticStage`.
///     It is a per-stage model fallback router. "Enterprise" is not a property
///     of the routing; it is a property of how the author wanted it to sound.
///     `ai_driver` is `Rewired` in the ledger, so this name moves into oyatie.
///   - `src/dashboard/ssr_renderer.rs:96` — `SsrDashboardRenderer`,
///     which renders an HTML page. `dashboard` carries a `Superseded` verdict
///     but only at `Probable` confidence, so it is **not** on the off-limits
///     list, and the law reaches it.
///
/// # Why prompting would not prevent it
///
/// The aspiration arrives *through* the prompt. A model told to build to
/// hyperscaler standard names the artifact `Hyperscaler*`, and the name then
/// reads as evidence that the standard was met. Nothing in a diff distinguishes
/// a truthful name from a boastful one — and the boastful one looks *more*
/// rigorous to a reviewer, so review actively selects for it. The only thing
/// that catches it is a mechanical scan that does not care how the name sounds.
#[test]
fn no_surviving_module_or_type_name_carries_an_aspiration_stamp() {
    let offenders: Vec<BrandViolation> = scan_production()
        .into_iter()
        .filter(|v| v.kind == BrandViolationKind::Name)
        // The off-limits modules are deleted, not renamed.
        .filter(|v| !lives_in_an_off_limits_module(&v.path))
        // `pub mod hyperscaler_consensus_guard;` in `src/lib.rs` is a reference
        // to an off-limits module, not a new name. It disappears with the module.
        .filter(|v| !SUPERSEDED_OFF_LIMITS.contains(&v.snippet.as_str()))
        .collect();

    assert!(
        offenders.is_empty(),
        "{} surviving name(s) stamp an aspiration or category instead of naming what the \
         code verifies (law §33 L1 / D33.2). These components are not deleted at absorption, \
         so the name travels into oyatie:{}\n\
         Allowlist is the {} enumerated superseded modules only.",
        offenders.len(),
        render(&offenders),
        SUPERSEDED_OFF_LIMITS.len(),
    );
}

/// # Defect this catches
///
/// The allowlist quietly widening until it exempts the whole tree. Two distinct
/// ways that happens, both asserted here:
///
///   1. **The list stops being an enumeration.** A glob, a prefix, or a
///      predicate ("skip anything the ledger marks Superseded") lets unbounded
///      future violations in under one line of diff. Nobody reads `src/*` as the
///      licence it is.
///   2. **The list stops matching the ledger.** Verified at the time of writing:
///      `account_pool` is on the off-limits list but has **no entry at all** in
///      `src/migration/registry.rs`. It was moved under
///      `src/self_governance/account_pool/`, and the ledger's audit unit is the
///      top-level module, so nothing covers it. An allowlist entry whose
///      supersession nobody recorded is an exemption wearing a ledger's clothes.
///
/// It also asserts the ratchet direction: every component in the tree that
/// carries a stamped name must already be on the list. That is what makes the
/// list *closed* — a new `enterprise_*` module cannot be waved through by adding
/// a sixteenth line here, because the sixteenth line would have to claim a
/// `Superseded` verdict the ledger does not record.
///
/// # Why prompting would not prevent it
///
/// "Don't widen the allowlist" is an instruction about a future edit, and the
/// edit that widens it always arrives with a reason ("this module is superseded
/// anyway"). The reason is checkable — against the ledger — and the check is
/// what survives the reasoning.
#[test]
fn allowlist_is_finite_enumerated_and_ledger_backed() {
    // (1) finite and enumerated
    assert!(
        !SUPERSEDED_OFF_LIMITS.is_empty(),
        "an empty allowlist means the enumeration was replaced by something else"
    );
    for entry in SUPERSEDED_OFF_LIMITS {
        for pattern_char in ['*', '?', '[', ']', '^', '$', '/'] {
            assert!(
                !entry.contains(pattern_char),
                "allowlist entry '{entry}' contains pattern character '{pattern_char}'; \
                 entries must be exact component names so the list stays finite"
            );
        }
    }
    let unique: BTreeSet<&&str> = SUPERSEDED_OFF_LIMITS.iter().collect();
    assert_eq!(
        unique.len(),
        SUPERSEDED_OFF_LIMITS.len(),
        "the allowlist contains duplicates, so its true size is not the size it appears to be"
    );

    // (2) every entry resolves to something that exists on disk
    let root = repo_root();
    let mut missing_on_disk = Vec::new();
    for entry in SUPERSEDED_OFF_LIMITS {
        if !module_exists(&root.join("src"), entry) {
            missing_on_disk.push(*entry);
        }
    }
    assert!(
        missing_on_disk.is_empty(),
        "allowlist entries that match nothing in src/: {missing_on_disk:?}. A stale entry \
         shields nothing today and an arbitrary future module tomorrow."
    );

    // (3) every entry is backed by a Superseded verdict in the ledger
    let mut unbacked = Vec::new();
    for entry in SUPERSEDED_OFF_LIMITS {
        match ledger_verdict(entry) {
            Some(Verdict::Superseded) => {}
            Some(other) => unbacked.push(format!("{entry}: ledger says {other:?}, not Superseded")),
            None => unbacked.push(format!("{entry}: no entry in MIGRATION_LEDGER at all")),
        }
    }
    assert!(
        unbacked.is_empty(),
        "{} allowlist entr(ies) are not backed by a Superseded verdict in the authoritative \
         ledger (src/migration/registry.rs). An exemption from the naming law is only \
         legitimate because the code is being deleted; if the ledger does not say so, the \
         exemption is unfounded:\n    {}",
        unbacked.len(),
        unbacked.join("\n    ")
    );

    // (4) ratchet: nothing stamped in the tree escapes the list
    let stamped_components: BTreeSet<String> = scan_production()
        .iter()
        .filter(|v| v.kind == BrandViolationKind::Name)
        .filter(|v| !SUPERSEDED_OFF_LIMITS.contains(&v.snippet.as_str()))
        .map(|v| v.path.clone())
        .collect();
    let escaping: Vec<&String> = stamped_components
        .iter()
        .filter(|p| !lives_in_an_off_limits_module(p))
        .collect();
    assert!(
        escaping.is_empty(),
        "{} file(s) carry a stamped name and are not covered by the enumerated allowlist, so \
         the allowlist is not the tree's only exemption channel — the stamps are simply \
         unguarded: {:?}",
        escaping.len(),
        escaping
    );
}

fn module_exists(src: &Path, component: &str) -> bool {
    fn walk(dir: &Path, component: &str) -> bool {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if stem == component {
                return true;
            }
            if path.is_dir() && walk(&path, component) {
                return true;
            }
        }
        false
    }
    walk(src, component)
}

/// The ledger records components as `"foo"`, `"foo.rs"`, or `"foo (dir)"`.
/// Match on the leading identifier, the same way `migration_ledger_test` does.
fn ledger_verdict(component: &str) -> Option<Verdict> {
    MIGRATION_LEDGER
        .iter()
        .find(|e| {
            e.component
                .split(['/', ' ', '.'])
                .next()
                .is_some_and(|head| head == component)
        })
        .map(|e| e.verdict)
}

// ---------------------------------------------------------------------------
// 2. PR-visible display strings
// ---------------------------------------------------------------------------

/// # Defect this catches
///
/// An aspiration or category stamp inside a string literal. D33.2 covers these
/// explicitly **because they reach a pull request**: Anvil posts them as review
/// findings, scorecard headers, and log lines. Unlike names, these are in scope
/// even inside a superseded module — the module is deleted eventually, but the
/// string is on somebody's PR today.
///
/// Verified instances at the time of writing:
///
///   - `src/hyperscaler_consensus_guard/mod.rs:195` —
///     `"✅ UNANIMOUS APPROVAL (5/5 Hyperscalers Approved: AWS, GCP, Meta, Azure, OCI)"`.
///     No hyperscaler approved anything. The module reads a diff for unbounded
///     `tokio` channels and `thread::sleep`. This is a vendor roll-call posted
///     as if it were a finding.
///   - `src/monorepo_guard/disposition.rs:77` —
///     `"Hand-edited YAML catalog detected. Hyperscaler pattern mandates live
///     Rust AST / Protobuf reflection."` — an appeal to an authority that was
///     never consulted.
///   - `src/monorepo_guard/mod.rs:53` — `"Running MonorepoGuard hyperscaler patterns on {}#{}..."`
///   - `src/ai_driver/stage_router.rs:31` — `"7. 16-Lens Code Review & Hyperscaler Consensus"`
///   - `src/modularization_guard.rs:111` — `"Hyperscaler modularization & directory depth verified..."`,
///     which is a **surviving** (`Migrating`) component's PR-visible summary.
///
/// # The one exemption, and why it is principled
///
/// A stamp is permitted where every token carrying it is **identifier-shaped**:
/// all-lowercase, joined by `_ - . /`. That distinguishes a *reference to a
/// named artifact* from a *claim in prose*. `hyperscaler_consensus_guard`,
/// `oya-shared-hyperscaler-metrics-kernel`, and `hyperscaler.doc.v1` name
/// things; `Hyperscalers Approved` and `Cloud-Native violations` assert things.
/// Anvil does not own oyatie's asset names, and its own identifiers are already
/// governed by the Name check above, so re-flagging them here would be double
/// jeopardy with no new information.
///
/// Honest limit: an author could dodge this by writing a stamp as a fake
/// identifier. That is a deliberate evasion rather than the accident this
/// catches, and it would still be visible in review as a stamp.
///
/// # Why prompting would not prevent it
///
/// A linter that reads identifiers passes these files cleanly — the text is
/// inside quotes. A human reviewer reads a log line as flavour text, not as an
/// assertion, and skims it. But the string is precisely the part that escapes
/// the repository and lands in someone else's pull request, which makes it the
/// part that must be checked mechanically rather than the part that can be left
/// to taste.
#[test]
fn no_pr_visible_display_string_carries_an_aspiration_stamp() {
    let offenders: Vec<BrandViolation> = scan_production()
        .into_iter()
        .filter(|v| v.kind == BrandViolationKind::DisplayString)
        .filter(|v| !stamp_occurs_only_as_an_identifier(&v.snippet, &v.stamp))
        .collect();

    assert!(
        offenders.is_empty(),
        "{} PR-visible string(s) carry an aspiration, category, or vendor roll-call stamp \
         instead of stating the check performed (law §33 D33.2). These are posted onto pull \
         requests today, so they are in scope even inside a superseded module:{}",
        offenders.len(),
        render(&offenders),
    );
}

/// Separators that make a token a machine identifier rather than an English word.
const IDENTIFIER_SEPARATORS: [char; 4] = ['_', '-', '.', '/'];

/// Punctuation that appears inside real identifiers in this tree, e.g.
/// `hyperscaler_{maturity_claims,arch_invariants}_gate` in the migration ledger.
const IDENTIFIER_EXTRAS: [char; 4] = ['{', '}', ',', ':'];

/// Whether every token in `literal` that carries any word of `stamp` is
/// identifier-shaped, AND at least one token carries the whole stamp.
///
/// The second condition matters: a two-word stamp split across two prose tokens
/// ("cloud native") has no token carrying the whole thing, and must be treated
/// as prose rather than silently exempted.
fn stamp_occurs_only_as_an_identifier(literal: &str, stamp: &str) -> bool {
    let stamp_words: Vec<String> = stamp
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_ascii_lowercase())
        .collect();
    if stamp_words.is_empty() {
        return false;
    }

    let mut whole_stamp_seen = false;
    for raw in literal.split_whitespace() {
        let token = raw.trim_matches(|c: char| {
            !(c.is_ascii_alphanumeric()
                || IDENTIFIER_SEPARATORS.contains(&c)
                || IDENTIFIER_EXTRAS.contains(&c))
        });
        if token.is_empty() {
            continue;
        }
        let lower = token.to_ascii_lowercase();
        let carries_any = stamp_words.iter().any(|w| lower.contains(w.as_str()));
        if !carries_any {
            continue;
        }
        if !is_identifier_shaped(token) {
            return false;
        }
        if stamp_words.iter().all(|w| lower.contains(w.as_str())) {
            whole_stamp_seen = true;
        }
    }
    whole_stamp_seen
}

fn is_identifier_shaped(token: &str) -> bool {
    let all_machine = token.chars().all(|c| {
        (c.is_ascii_lowercase() || c.is_ascii_digit())
            || IDENTIFIER_SEPARATORS.contains(&c)
            || IDENTIFIER_EXTRAS.contains(&c)
    });
    let has_separator = token.chars().any(|c| IDENTIFIER_SEPARATORS.contains(&c));
    all_machine && has_separator
}

// ---------------------------------------------------------------------------
// 3. Gate-count claims
// ---------------------------------------------------------------------------

/// The gate corpus size, read from the **gate API** rather than from a constant.
///
/// Every `GateStatus` field must be named here, so removing or adding a gate
/// breaks this fixture at compile time — which is the point. The count this test
/// compares strings against cannot drift away from the corpus without the build
/// noticing.
fn live_gate_count() -> usize {
    let report = PreMergeCertificationReport {
        is_certified_ready: true,
        doc_parity_status: GateStatus::Passed,
        cedar_status: GateStatus::Passed,
        compliance_status: GateStatus::Passed,
        api_contract_status: GateStatus::Passed,
        cell_isolation_status: GateStatus::Passed,
        supply_chain_status: GateStatus::Passed,
        clean_arch_status: GateStatus::Passed,
        monorepo_status: GateStatus::Passed,
        debt_shrink_status: GateStatus::Passed,
        modularization_status: GateStatus::Passed,
        coverage_status: GateStatus::Passed,
        rust_skills_status: GateStatus::Passed,
        kani_status: GateStatus::Passed,
        slo_status: GateStatus::Passed,
        adr_status: GateStatus::Passed,
        shuffle_status: GateStatus::Passed,
        trace_status: GateStatus::Passed,
        constant_work_status: GateStatus::Passed,
        idempotency_status: GateStatus::Passed,
        finops_status: GateStatus::Passed,
        ghost_migration_status: GateStatus::Passed,
        gitops_promo_status: GateStatus::Passed,
        gitops_drift_status: GateStatus::Passed,
        canary_status: GateStatus::Passed,
        cluster_audit_status: GateStatus::Passed,
        migration_orch_status: GateStatus::Passed,
        ci_wallclock_status: GateStatus::Passed,
        predictive_test_status: GateStatus::Passed,
        compile_profile_status: GateStatus::Passed,
        remote_cache_status: GateStatus::Passed,
        runner_economics_status: GateStatus::Passed,
        sandbox_status: GateStatus::Passed,
        cross_service_status: GateStatus::Passed,
        ephemeral_secret_status: GateStatus::Passed,
        psa_status: GateStatus::Passed,
        shadow_traffic_status: GateStatus::Passed,
        unresolved_review_status: GateStatus::Passed,
        local_probe_status: GateStatus::Passed,
        semantic_abi_status: GateStatus::Passed,
        zero_day_status: GateStatus::Passed,
        formal_verification_status: GateStatus::Passed,
        deadlock_status: GateStatus::Passed,
        review_verdict_status: GateStatus::Passed,
        brand_absence_status: GateStatus::Passed,
        migration_boundary_status: GateStatus::Passed,
        shape_status: GateStatus::Passed,
        automated_canary_status: GateStatus::Passed,
        progressive_ring_status: GateStatus::Passed,
        hermetic_build_status: GateStatus::Passed,
        openvex_status: GateStatus::Passed,
        cosign_status: GateStatus::Passed,
        chaos_injection_status: GateStatus::Passed,
        stacked_diffs_status: GateStatus::Passed,
        microbench_status: GateStatus::Passed,
        jittered_backoff_status: GateStatus::Passed,
        schema_evolution_status: GateStatus::Passed,
        auto_rollback_status: GateStatus::Passed,
        wasm_sandbox_status: GateStatus::Passed,
        consistency_status: GateStatus::Passed,
        flake_quarantine_status: GateStatus::Passed,
        zero_trust_workload_status: GateStatus::Passed,
        carbon_compute_status: GateStatus::Passed,
        replay_harness_status: GateStatus::Passed,
        upgrade_train_status: GateStatus::Passed,
        mutation_status: GateStatus::Passed,
        feature_flag_status: GateStatus::Passed,
        bench_status: GateStatus::Passed,
        attestation_status: GateStatus::Passed,
        security_scan_status: GateStatus::Passed,
        schema_compat_status: GateStatus::Passed,
        performance_concurrency_status: GateStatus::Passed,
        test_suite_status: GateStatus::Passed,
        unmeasured_gates: Vec::new(),
        summary_markdown: String::new(),
    };
    let (passed, failed) = report.gate_counts();
    assert_eq!(
        passed + failed,
        report.all_statuses().len(),
        "gate_counts() and all_statuses() disagree about the corpus size; the API has two \
         answers and the strings cannot be checked against either"
    );
    passed + failed
}

/// # Defect this catches
///
/// A hardcoded gate count in a PR-visible string that disagrees with the real
/// corpus. Verified at the time of writing: the corpus is **68**
/// (`all_statuses().len()` and `gate_counts()` both), while the tree claims
/// **70** in these string literals:
///
///   - `src/webhook/pipelines/review.rs:21` — `"Executing AI Code Review & 70-Gate Hyperscale Pipeline for {}#{}..."`
///   - `src/webhook/pipelines/review.rs:716` — `"{} ({}/70 Gates)"`, the PR check-run title
///   - `src/pre_merge_guard/evaluator.rs:159` — `"Evaluating ... Gates for {}#{} (70 gates)..."`
///   - `src/cli/server.rs:56` — `"...Dispatched review & 70-gate certification for {}#{}"`
///   - `src/pre_merge_guard/matrix.rs:98` — the posted scorecard header, `"(70 Gates)"`
///   - `src/dashboard/ssr_renderer.rs:187` and `:190` — the governance panel title
///
/// Reported line numbers point at the line the enclosing literal *starts* on
/// (`matrix.rs:97`, `review.rs:713`, `ssr_renderer.rs:132`), because the last
/// three live inside multi-line Markdown and HTML blocks.
///
/// The brief named three log sites. There are seven claims, which is itself the
/// finding: a hardcoded count spreads by copy-paste faster than anyone tracks
/// it. Two further claims live in doc comments (`src/lib.rs:3`,
/// `src/pre_merge_guard/mod.rs:1`) and are out of this check's reach — it reads
/// string literals, because those are what reach a pull request.
///
/// # Why prompting would not prevent it
///
/// The number was **true when it was typed**. It became a lie through an
/// unrelated edit elsewhere in the tree — gates merged or removed — and no
/// prompt, review, or memory notices a constant drifting away from the thing it
/// counts, because nothing in that later diff mentions the string. The same
/// class already caused a measured harm here: `report.rs:246` records that
/// `gate_counts()` was previously hardcoded as `(70, 0)` / `(69, 1)`, which made
/// telemetry report "95% of PRs stuck at 69/70" — an artefact of the constant,
/// not a measurement. Only comparing the claim to the live corpus catches it.
#[test]
fn every_gate_count_claim_equals_the_live_gate_count() {
    let real = live_gate_count();
    assert!(
        real > 0,
        "the gate API reports a corpus of 0, so there is no source of truth to check claims \
         against and every assertion below would be vacuous"
    );
    assert_eq!(
        BrandAbsenceGate::with_allowlist(Vec::new()).real_gate_count(),
        real,
        "the brand-absence gate's corpus size disagrees with the live gate API. Two sources \
         of truth means the check can pass against the wrong one."
    );

    let offenders: Vec<BrandViolation> = scan_production()
        .into_iter()
        .filter(|v| v.kind == BrandViolationKind::GateCountClaim)
        .collect();

    assert!(
        offenders.is_empty(),
        "{} string(s) claim a gate count that is not {real}. Each is a number Anvil posts \
         onto a pull request as if it were measured. Better than correcting them is deriving \
         the count from the gate API so the next corpus change cannot make them wrong \
         again:{}",
        offenders.len(),
        render(&offenders),
    );
}

// ---------------------------------------------------------------------------
// 4. L2 — adopt the established term
// ---------------------------------------------------------------------------

/// # Defect this catches
///
/// A surviving component coining its own synonym for a concept that already has
/// an established name in the tree it is migrating into (L2).
///
/// The table is currently empty. That is a finding, not an omission: the sole
/// candidate mapping was checked against oyatie's source and turned out to pair
/// two gates that do different jobs. See ESTABLISHED_TERM_RENAMES.
///
/// Two names for one concept is the expensive kind of duplication: it survives
/// the merge, and afterwards nobody can tell whether `rust_language_policy` and
/// `automation-language-policy` are the same gate or two gates.
///
/// This is a separate check from the stamp scan because "skills" is not an
/// aspiration stamp — a vocabulary-based scanner will never flag it. It is only
/// wrong relative to a term that already exists somewhere else.
///
/// # Why prompting would not prevent it
///
/// Coining a synonym requires not knowing the established term, and a prompt
/// cannot supply knowledge of a term in a repository the author has not read.
/// The name is also perfectly descriptive in isolation, so review has nothing to
/// object to. The defect only becomes visible when the two trees are put side by
/// side — which is exactly what the migration ledger did, and what this test
/// pins.
#[test]
fn surviving_components_adopt_the_established_term_instead_of_coining_a_synonym() {
    let root = repo_root();
    let mut failures = Vec::new();

    for (current, required, established) in ESTABLISHED_TERM_RENAMES {
        // The component must still be one that survives; renaming a superseded
        // module would be the waste this lane exists to avoid.
        match ledger_verdict(current).or_else(|| ledger_verdict(required)) {
            Some(Verdict::Superseded) | Some(Verdict::Scaffolding) => failures.push(format!(
                "{current}: ledger says it is deleted at absorption, so it must not be renamed \
                 at all — remove it from ESTABLISHED_TERM_RENAMES"
            )),
            None => failures.push(format!(
                "{current}/{required}: no entry in MIGRATION_LEDGER, so whether it survives is \
                 unrecorded and the rename is unjustified"
            )),
            Some(_) => {}
        }

        let established_path = root.join(established);
        if established_path.exists() {
            // Citation verified against the tree.
        } else {
            eprintln!(
                "NotMeasured: '{established}' is not present locally (/repos is gitignored), \
                 so the established-term citation could not be re-verified in this run. The \
                 rename below is still enforced."
            );
        }

        if root.join("src").join(current).exists() {
            failures.push(format!(
                "src/{current}/ still exists; L2 requires adopting the established term \
                 '{established}' rather than the coined '{current}'"
            ));
        }
        if !root.join("src").join(required).exists() {
            failures.push(format!(
                "src/{required}/ does not exist; the rename to the established term has not \
                 happened"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} established-term violation(s):\n    {}",
        failures.len(),
        failures.join("\n    ")
    );
}

// ---------------------------------------------------------------------------
// 5. The production gate must not count test-module text
// ---------------------------------------------------------------------------

/// # Defect this catches
///
/// The shipped gate scanning `#[cfg(test)]` modules as if they were production.
/// Verified: `BrandAbsenceGate::scan_tree` walks every `.rs` file and hands the
/// whole file to `scan_source`, with no notion of test code — so
/// `src/hyperscaler_consensus_guard/mod.rs:239`,
/// `assert!(report.summary.contains("5/5 Hyperscalers Approved"))`, counts as a
/// PR-visible display string. It is not: nothing in a test module reaches a pull
/// request.
///
/// This repo has already been burned by the mirror image of this: a test was
/// satisfied by a call that lived inside a `#[cfg(test)]` module and reported
/// green while the production path did nothing. Both directions are the same
/// root cause — a scanner that cannot tell production from test lets test text
/// stand in for production text.
///
/// Concretely, leaving it means the debt ledger's occurrence ceilings are sized
/// by test fixtures, so a genuinely new production violation can slot in under a
/// ceiling that test code paid for.
///
/// # Why prompting would not prevent it
///
/// "Scan production code only" is an instruction that a scanner satisfies by
/// looking correct: it opens `.rs` files under `src/`, which *is* production
/// source, and the `#[cfg(test)]` blocks inside them are invisible unless
/// somebody thinks to ask. There is no moment in review at which the omission
/// announces itself — the gate runs, produces a plausible number, and the number
/// is wrong in a direction nobody audits.
#[test]
fn the_gate_does_not_count_a_stamp_that_lives_only_in_a_cfg_test_module() {
    let gate = BrandAbsenceGate::with_allowlist(Vec::new());

    let source = r#"
pub struct StageModelRouter;

impl StageModelRouter {
    pub fn describe(&self) -> &'static str {
        "returns the per-stage model fallback chain"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub struct EnterpriseFixture;

    #[test]
    fn fixture_asserts_on_the_legacy_banner() {
        assert!("5/5 Hyperscalers Approved: AWS, GCP, Azure".contains("Approved"));
    }
}
"#;

    let report = gate.scan_source("src/synthetic/stage_model_router.rs", source);

    assert!(
        report.new_violations.is_empty(),
        "the gate counted {} violation(s) that exist only inside a #[cfg(test)] module. Test \
         text never reaches a pull request, so counting it both inflates the debt ledger and \
         lets production violations hide under a ceiling that test fixtures paid for:{}",
        report.new_violations.len(),
        render(&report.new_violations),
    );

    // Paired positive case, so this cannot be satisfied by a gate that stopped
    // flagging anything at all.
    let same_text_in_production = r#"
pub struct EnterpriseFixture;

fn banner() -> &'static str {
    "5/5 Hyperscalers Approved: AWS, GCP, Azure"
}
"#;
    let dirty = gate.scan_source(
        "src/synthetic/stage_model_router.rs",
        same_text_in_production,
    );
    assert!(
        !dirty.new_violations.is_empty(),
        "the identical text outside a test module must still be flagged; a gate that flags \
         neither is inert, not discriminating"
    );
}

/// # Defect this catches
///
/// The `#[cfg(test)]` stripper used by every scan above silently doing nothing,
/// or doing too much. Either way the four tests above would report a number that
/// is not about production code, and a green from them would mean nothing.
///
/// This is the check that makes the other tests' evidence admissible, so it
/// asserts against a named, verified fixture rather than a threshold alone:
/// `src/hyperscaler_consensus_guard/mod.rs` contains both a production type
/// (`pub struct HyperscalerConsensusGuard`) and a test-only function
/// (`fn test_hyperscaler_consensus_approves_clean_pr`), and after stripping
/// exactly one of the two must survive.
///
/// # Why prompting would not prevent it
///
/// A stripper that returns its input unchanged passes every syntactic review —
/// it compiles, it is called, and its output is a `String` that looks like Rust.
/// The failure is silent by construction: nothing downstream can tell a
/// correctly-stripped file from an unstripped one except by looking for text
/// that should have gone.
#[test]
fn the_cfg_test_stripper_removes_test_modules_and_keeps_production_code() {
    // The fixture is written here rather than borrowed from a production file.
    // It used to point at src/hyperscaler_consensus_guard/mod.rs, so a test
    // about the stripper broke whenever that module changed, and the module
    // could not be deleted while this test held it hostage. A test that pins a
    // transformation owns its input.
    let raw = concat!(
        "pub struct HyperscalerConsensusGuard;\n",
        "\n",
        "impl HyperscalerConsensusGuard {\n",
        "    pub fn new() -> Self {\n",
        "        Self\n",
        "    }\n",
        "}\n",
        "\n",
        "#[cfg(test)]\n",
        "mod tests {\n",
        "    use super::*;\n",
        "\n",
        "    #[test]\n",
        "    fn test_hyperscaler_consensus_approves_clean_pr() {\n",
        "        let _ = HyperscalerConsensusGuard::new();\n",
        "    }\n",
        "}\n",
    )
    .to_string();

    assert!(
        raw.contains("fn test_hyperscaler_consensus_approves_clean_pr"),
        "fixture drifted: the test-only function this check depends on is gone, so a green \
         result here would prove nothing"
    );

    let (stripped, removed) = strip_cfg_test_items(&raw);

    assert!(
        removed >= 1,
        "the stripper removed nothing from a file that demonstrably contains a #[cfg(test)] \
         module"
    );
    assert!(
        !stripped.contains("fn test_hyperscaler_consensus_approves_clean_pr"),
        "test-only code survived stripping, so every scan in this file is scanning test text \
         as if it were production"
    );
    assert!(
        stripped.contains("pub struct HyperscalerConsensusGuard"),
        "production code was removed by the stripper, so every scan in this file is blind to \
         part of the tree"
    );
}
