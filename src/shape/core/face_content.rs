//! What may live *inside* a face, as opposed to where a crate sits.
//!
//! Placement and naming rules answer "is this crate in a legal directory with a
//! legal name". They cannot answer "is this port code sitting in an adapter".
//! A crate named `foo-adapter` under `adapters/` satisfies every path rule
//! while containing pure domain logic, and nothing today notices.
//!
//! # The boundary being enforced
//!
//! - **ports** — a pure boundary. Value types and the trait seams adapters
//!   implement later. No I/O: no database, HTTP, filesystem or async runtime.
//! - **core** — the rules over those types. Domain and usecase logic, also
//!   free of I/O.
//! - **adapters** — concrete technology behind a port. I/O belongs here.
//! - **facade** — composition of one capability into a surface. A surface
//!   composing two or more *other* capabilities is a misplaced app.
//!
//! # Why there is no exception list
//!
//! Every rule here is a ratchet over changed paths, never an allowlist. An
//! allowlist grows by one entry per violation and converges on permitting
//! everything, which is how a rule stops measuring anything. Pre-existing
//! violations are baselined by the caller and reported, not silently forgiven.
//!
//! # Measured against the reference corpus
//!
//! Each rule was checked against a 438-crate conformant monorepo before being
//! written, because a rule that the reference tree fails is a wrong rule:
//!
//! | rule | violations in the reference tree |
//! |---|---|
//! | no I/O in `ports` | 1 / 56 |
//! | no I/O in `core` | 5 / 224 |
//! | facade composes < 2 other capabilities | 3 / 53 |
//! | cargo name is path-derived | 47 / 438 |
//!
//! The I/O rules are near-universally held, so they are real. The naming rule
//! carries reorg debt (`shared-` and `messaging-` prefixes surviving a move),
//! which is what a ratchet is for.

use std::collections::BTreeSet;

/// Crates whose presence means a unit performs I/O.
///
/// Deliberately the transitive-runtime kind, not utility crates: `serde` and
/// `thiserror` say nothing about purity, whereas an async runtime or a database
/// driver cannot be in a pure boundary by accident.
pub const IO_CRATES: &[&str] = &[
    "tokio",
    "async-std",
    "smol",
    "hyper",
    "reqwest",
    "ureq",
    "axum",
    "actix-web",
    "warp",
    "tonic",
    "sqlx",
    "diesel",
    "sea-orm",
    "tokio-postgres",
    "rusqlite",
    "mongodb",
    "redis",
    "deadpool",
    "aws-sdk-s3",
    "aws-config",
    "rusoto_core",
    "notify",
    "tempfile",
];

/// Faces that must contain no I/O.
pub const PURE_FACES: &[&str] = &["ports", "core"];

/// A crate under judgement. Path facts only; the caller supplies what it read.
#[derive(Debug, Clone)]
pub struct CrateFacts {
    /// Capability or product directly above the face, e.g. `iam`.
    pub owner: String,
    /// One of core, ports, adapters, facade.
    pub face: String,
    /// Directory name of the crate, e.g. `policy-cedar-api`.
    pub leaf: String,
    /// `package.name` as declared.
    pub package_name: String,
    /// Dependency names from `[dependencies]`.
    pub dependencies: BTreeSet<String>,
    /// Capabilities this crate takes a path dependency on, excluding its own.
    pub foreign_capabilities: BTreeSet<String>,
    /// Whether the crate lives beneath `app/<product>/`.
    pub under_app: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentViolation {
    pub rule: &'static str,
    pub unit: String,
    pub detail: String,
}

/// A pure face may not depend on an I/O crate.
pub fn io_in_pure_face(facts: &CrateFacts) -> Option<ContentViolation> {
    if !PURE_FACES.contains(&facts.face.as_str()) {
        return None;
    }
    let found: Vec<&str> = IO_CRATES
        .iter()
        .copied()
        .filter(|io| facts.dependencies.contains(*io))
        .collect();
    (!found.is_empty()).then(|| ContentViolation {
        rule: "io_in_pure_face",
        unit: format!("{}/{}/{}", facts.owner, facts.face, facts.leaf),
        detail: format!(
            "`{}` is a pure boundary and depends on {}",
            facts.face,
            found.join(", ")
        ),
    })
}

/// A facade composing two or more *other* capabilities is a misplaced app.
///
/// A surface under `app/<product>/` is exempt: composing capabilities is what a
/// product is for. The rule catches a capability facade that has quietly become
/// a product.
pub fn facade_composes_foreign_capabilities(facts: &CrateFacts) -> Option<ContentViolation> {
    if facts.face != "facade" || facts.under_app || facts.foreign_capabilities.len() < 2 {
        return None;
    }
    let mut caps: Vec<&str> = facts
        .foreign_capabilities
        .iter()
        .map(String::as_str)
        .collect();
    caps.sort_unstable();
    Some(ContentViolation {
        rule: "facade_composes_foreign_capabilities",
        unit: format!("{}/{}/{}", facts.owner, facts.face, facts.leaf),
        detail: format!(
            "a single-capability surface is a facade; composing {} is an app: {}",
            caps.len(),
            caps.join(", ")
        ),
    })
}

/// The package name must be derivable from the path, and carry no vendor prefix.
pub fn package_name_not_canonical(facts: &CrateFacts) -> Option<ContentViolation> {
    let want_bare = facts.leaf.as_str();
    let want_owned = format!("{}-{}", facts.owner, facts.leaf);
    if facts.package_name == want_bare || facts.package_name == want_owned {
        return None;
    }
    Some(ContentViolation {
        rule: "package_name_not_canonical",
        unit: format!("{}/{}/{}", facts.owner, facts.face, facts.leaf),
        detail: format!(
            "package `{}` is neither `{}` nor `{}`; a name that does not follow \
             its path is a name a move did not update",
            facts.package_name, want_bare, want_owned
        ),
    })
}

/// Every content rule, in one pass.
pub fn content_violations(crates: &[CrateFacts]) -> Vec<ContentViolation> {
    let checks: [fn(&CrateFacts) -> Option<ContentViolation>; 3] = [
        io_in_pure_face,
        facade_composes_foreign_capabilities,
        package_name_not_canonical,
    ];
    crates
        .iter()
        .flat_map(|c| checks.iter().filter_map(move |check| check(c)))
        .collect()
}
