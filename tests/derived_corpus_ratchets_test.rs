//! Two corpus-wide counts that may fall and may not rise, derived rather than
//! written down.
//!
//! `NOT_PROVISIONED_COUNT = 26` and `GATES_WITHOUT_PROOF = 23` were literals
//! cross-checked against the tables they summarise, in one build. That makes
//! them global variables every lane has to edit, with two costs:
//!
//!   - Two branches that both lower one write the same line. Git merges that
//!     cleanly, so the merged tree carries a number wrong by one and there is
//!     no conflict to catch it.
//!   - Any *partial* move of the gate corpus disagrees with the literal and is
//!     red. That is why converting the corpus had to be one unmergeable pull
//!     request rather than fifteen small ones — the literal, not the work,
//!     was the thing that could not be sharded.
//!
//! The bound is the same bound: exact, one-way, over the whole corpus. It is
//! measured against *this change's own merge-base* instead, so nothing is
//! written down, nothing conflicts, and a fall needs no bookkeeping commit.
//!
//! The merge-base measurement is textual, because the tree at the merge-base
//! cannot be compiled here. That is only safe while the text agrees with the
//! compiled truth, so each ratchet asserts both: the textual count at HEAD
//! equals what the library computes, and the textual count has not risen.

use std::path::Path;

/// `Absence::NotProvisioned` rows, counted in source text.
fn not_provisioned_sites(path: &str, body: &str) -> usize {
    if path != "src/pre_merge_guard/admission.rs" {
        return 0;
    }
    body.matches("Absence::NotProvisioned")
        .count()
        // The enum's own definition and the `matches!` arms that read it are
        // not rows in the table.
        .saturating_sub(body.matches("matches!(a, Absence::NotProvisioned").count())
}

/// `GateProof` rows in the ledger, counted in source text.
fn gate_proof_sites(path: &str, body: &str) -> usize {
    if path != "src/gate_proof/mod.rs" {
        return 0;
    }
    body.matches("    GateProof {").count()
}

/// Gates in the corpus, counted in source text.
///
/// `GATE_LABELS` is pinned to `named_statuses()` in order and in length by
/// `matrix::every_named_gate_has_exactly_one_label_and_vice_versa`, so counting
/// its rows counts the corpus.
fn gate_label_sites(path: &str, body: &str) -> usize {
    if path != "src/pre_merge_guard/matrix.rs" {
        return 0;
    }
    body.matches("\n    (\n        \"").count()
}

/// The join the consumer makes: `pre_merge_guard` owns the corpus and the
/// "can this gate fire here" predicate, `gate_proof` owns the ledger.
fn owing_a_proof() -> Vec<&'static str> {
    anvil::gate_proof::gates_owing_a_proof(
        &anvil::pre_merge_guard::matrix::GATE_LABELS
            .iter()
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>(),
        anvil::pre_merge_guard::admission::absence_blocks,
    )
}

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn head_sites(count: impl Fn(&str, &str) -> usize) -> usize {
    let mut total = 0;
    let mut stack = vec![repo().join("src")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs")
                && let Ok(body) = std::fs::read_to_string(&p)
            {
                let rel = p
                    .strip_prefix(repo())
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .replace('\\', "/");
                total += count(&rel, &body);
            }
        }
    }
    total
}

fn at_merge_base(count: impl Fn(&str, &str) -> usize + Copy) -> Option<usize> {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    rt.block_on(anvil::ratchet::facade::derived::source_sites_at_merge_base(
        repo(),
        "origin/dev",
        "HEAD",
        count,
    ))
    .map(|d| d.at_merge_base)
}

#[test]
fn the_unprovisionable_count_does_not_rise() {
    use anvil::pre_merge_guard::admission::{ABSENCE_POLICY, Absence};
    let compiled = ABSENCE_POLICY
        .iter()
        .filter(|(_, a)| matches!(a, Absence::NotProvisioned { .. }))
        .count();
    let here = head_sites(not_provisioned_sites);
    assert_eq!(
        here, compiled,
        "the text-level count of `Absence::NotProvisioned` rows ({here}) disagrees \
         with what the table compiles to ({compiled}). The merge-base half of this \
         ratchet is textual — a tree at another revision cannot be compiled here — \
         so a disagreement means the bound below is measuring something other than \
         the table."
    );

    let Some(base) = at_merge_base(not_provisioned_sites) else {
        eprintln!("skipped: no merge-base against origin/dev in this checkout");
        return;
    };
    assert!(
        here <= base,
        "{here} gate(s) are declared unprovisionable, up from {base} at the \
         merge-base. A table that only grows switches the corpus off one gate at a \
         time. Stand the capability up, or argue the row on the pull request."
    );
}

/// A gate arrives with its proof, or the obligation grew.
///
/// The two counts above bound the ledger's own size in each direction and miss
/// the thing that actually matters: three gates entering the corpus without
/// proofs raises the number owing by three while the ledger neither grew nor
/// shrank, so both of those assertions pass. Measured, not reasoned about --
/// wiring three callerless guards is exactly what exposed it.
///
/// `GATE_LABELS.len() - GATE_PROOFS.len()` is the obligation as a single
/// number, textual on both sides so the merge-base half can compute it, and it
/// may not rise.
#[test]
fn a_gate_arrives_with_its_proof() {
    let labels = head_sites(gate_label_sites);
    let proofs = head_sites(gate_proof_sites);
    assert_eq!(
        labels,
        anvil::pre_merge_guard::matrix::GATE_LABELS.len(),
        "the text-level count of GATE_LABELS rows ({labels}) disagrees with the \
         compiled table ({}), so the bound below measures something else",
        anvil::pre_merge_guard::matrix::GATE_LABELS.len()
    );
    let here = labels.saturating_sub(proofs);

    let Some(base_labels) = at_merge_base(gate_label_sites) else {
        eprintln!("skipped: no merge-base against origin/dev in this checkout");
        return;
    };
    let base_proofs = at_merge_base(gate_proof_sites).unwrap_or(0);
    let base = base_labels.saturating_sub(base_proofs);

    assert!(
        here <= base,
        "{here} gate(s) in the corpus have no proof, up from {base} at the \
         merge-base. A gate that has never been seeded with the defect it \
         exists to catch cannot be believed, so it arrives with its proof or it \
         does not arrive.\n\
         Corpus {base_labels} -> {labels}, ledger {base_proofs} -> {proofs}."
    );
}

#[test]
fn the_gates_owing_a_proof_do_not_rise() {
    // Two readings of the ledger, in opposite directions. The compiled set is
    // gates that CAN fire and have no proof; the textual measure counts the
    // proofs themselves, which must never fall. A gate added without a proof
    // raises the first; a proof deleted lowers the second.
    let owing = owing_a_proof();
    assert!(
        !owing.is_empty() || anvil::gate_proof::GATE_PROOFS.len() > 1,
        "fixture sanity: the ledger must hold something for this to bound"
    );

    let here = head_sites(gate_proof_sites);
    assert_eq!(
        here,
        anvil::gate_proof::GATE_PROOFS.len(),
        "the text-level count of `GateProof` rows ({here}) disagrees with the \
         compiled ledger ({}). The merge-base half of this ratchet is textual, so \
         a disagreement means the bound below is measuring something else.",
        anvil::gate_proof::GATE_PROOFS.len()
    );

    let Some(base) = at_merge_base(gate_proof_sites) else {
        eprintln!("skipped: no merge-base against origin/dev in this checkout");
        return;
    };
    assert!(
        here >= base,
        "the proof ledger fell from {base} entries at the merge-base to {here}. \
         A gate that was shown to fire does not stop having been shown it; \
         deleting its proof returns it to a tick.\n\
         Still owing a proof: {}",
        owing.join(", ")
    );
}
