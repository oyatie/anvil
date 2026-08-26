//! The facade seal: only a unit's `facade` is importable from outside it.

use std::sync::LazyLock;

use regex::Regex;

use super::paths::unit_of;

/// A unit's interior, as `.anvil/shape.json` declares it.
///
/// The spec names exactly four faces and exactly these directory names:
/// `skeletons.standard.faces = {core: "core/", ports: "ports/", adapters:
/// "adapters/", facade: "facade/"}`. `facade` is absent here because it is the
/// one face that may be named from outside a unit.
///
/// This list first also carried `domain`, `application` and `adapter`, copied
/// from `classify_layer`'s heuristic. That was a widening with no authority
/// behind it. The spec's `layer_suffixes` table does map `domain -> core` and
/// `adapter -> adapters`, but those are CRATE-NAME suffixes for naming units in
/// a workspace (`billing-domain`), not face directories inside one, and
/// `application` appears nowhere in the spec at all. No such directory exists
/// in this tree and none of the recorded bypasses came from one, so the three
/// names bought nothing and cost a false-positive surface: `billing::domain::X`
/// is an ordinary module path, and accusing it in a repository that has no
/// faces is the same wrong answer as accusing a unit of using its own adapters.
///
/// `interior_faces_match_the_shape_spec` holds this to the spec mechanically,
/// so a change there cannot silently leave this behind.
const INTERIOR_FACES: [&str; 3] = ["core", "ports", "adapters"];

/// Every `<unit>::<interior-face>` the line names.
///
/// Matching the shape directly, rather than splitting on `crate::` and reading
/// one path out of the result, is what makes this hold. That earlier spelling
/// had four separate failures, all from the same hand-rolled parse:
///
///   * it read only the FIRST path on the line, so
///     `f(crate::a::facade::X, crate::b::adapters::Y)` was reported clean --
///     a one-line evasion, put a legal reference first;
///   * `use crate::{b::adapters::X, c::core::Y}` yielded a unit named
///     `{b` and never saw `c::core` at all;
///   * because `{b` can never equal `b`, a unit grouping its OWN faces was
///     reported as a bypass;
///   * a nested group had no spelling it could parse.
///
/// An identifier cannot contain `{`, `,` or whitespace, so anchoring on the
/// identifier shape handles grouped, nested and repeated paths without any of
/// them being enumerated as cases.
static FACE_REF: LazyLock<Regex> = LazyLock::new(|| {
    // Rooted at `crate::` deliberately. Matching a bare `<ident>::<face>`
    // also matched paths into crates we do not own -- `uuid::adapter::Compact`
    // is an ordinary third-party path, and accusing it is the same wrong
    // answer as accusing a unit of using its own adapters. Anvil's review of
    // this change raised it; `expand_use_groups` runs first, so a grouped
    // `use crate::{b::adapters::X}` is already `crate::b::adapters::X` here.
    Regex::new(&format!(
        r"\b([A-Za-z_][A-Za-z0-9_]*)::([A-Za-z_][A-Za-z0-9_]*)::({})\b",
        INTERIOR_FACES.join("|")
    ))
    .expect("static pattern")
});

/// `a::{b::X, c::Y}` becomes `a::b::X, a::c::Y`, innermost group first.
///
/// Without this, `use crate::beta::{core::X, ports::Y}` names neither
/// `beta::core` nor `beta::ports` in a form any pattern can see: the brace sits
/// exactly where the `::` between unit and face would be. Expanding is what
/// makes grouped, nested and longhand spellings one case instead of three.
fn expand_use_groups(line: &str) -> String {
    let mut s = line.to_string();
    // Bounded: a pathological line must not spin. Eight levels is far past any
    // real `use`, and stopping early only under-reports.
    for _ in 0..8 {
        let Some(close) = s.find('}') else { break };
        // The last `{` before the first `}` is the innermost group.
        let Some(open) = s[..close].rfind('{') else {
            break;
        };
        let prefix_start = s[..open]
            .rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':'))
            .map(|i| i + 1)
            .unwrap_or(0);
        let prefix = &s[prefix_start..open];
        if !prefix.ends_with("::") {
            break; // a block or struct literal, not a path group
        }
        let expanded = s[open + 1..close]
            .split(',')
            .map(|part| format!("{prefix}{}", part.trim()))
            .collect::<Vec<_>>()
            .join(", ");
        s = format!("{}{}{}", &s[..prefix_start], expanded, &s[close + 1..]);
    }
    s
}

/// What one line offered the seal, and what the seal found in it.
pub(super) struct FaceScan {
    /// Cross-unit bypasses on this line.
    pub(super) bypasses: Vec<(String, String)>,
    /// References naming SOME unit's face, bypass or not.
    ///
    /// This is the seal's subject, and it is not the same as "a Rust file".
    /// A tree can be full of Rust and offer the rule nothing to judge, and
    /// reporting that as a clean pass is the failure the whole report type
    /// exists to prevent. A same-unit reference counts: it proves faces are
    /// present and that the rule looked at one and spared it.
    pub(super) subjects: usize,
}

pub(super) fn scan_faces(line: &str, importing_file: &str, local_crates: &[String]) -> FaceScan {
    let own = unit_of(importing_file);
    let mut out = FaceScan {
        bypasses: Vec::new(),
        subjects: 0,
    };
    let line = expand_use_groups(line);
    for c in FACE_REF.captures_iter(&line) {
        let root = &c[1];
        // The path must be rooted in code we own. A bare `<ident>::<face>`
        // also matched crates we do not own -- `uuid::adapter::Compact` is an
        // ordinary third-party path, and accusing it is the same wrong answer
        // as accusing a unit of using its own adapters. Anvil's review of this
        // change raised it.
        //
        // `crate::` covers the in-crate case. A workspace member's own name
        // covers the cross-crate one: `src/bin/occupancy.rs` reaches
        // `anvil::change_delivery::core`, which is a real bypass spelled
        // without `crate::`. When the member list is unknown the rule narrows
        // to `crate::` only -- it under-reports rather than accusing.
        if root != "crate" && !local_crates.iter().any(|c| c == root) {
            continue;
        }
        let unit = c[2].to_string();
        if matches!(unit.as_str(), "self" | "super") {
            continue;
        }
        out.subjects += 1;
        if own.as_deref() == Some(unit.as_str()) {
            continue; // a unit may reach into its own interior
        }
        let face = c[3].to_string();
        if !out.bypasses.contains(&(unit.clone(), face.clone())) {
            out.bypasses.push((unit, face));
        }
    }
    out
}
