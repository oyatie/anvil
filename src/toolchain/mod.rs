//! What this repository compiles with, and why that is the only such fact.
//!
//! # One pin, named exactly
//!
//! The channel is pinned to a dated nightly. Not to `nightly`, which is a
//! different compiler every day and makes a build unreproducible; not to a
//! stable release, which arrives six weeks after the lints that will break the
//! build are already knowable. A dated nightly is an exact toolchain that
//! moves on a schedule this repository chooses, which is how a fleet is
//! upgraded: pin a revision, roll it forward on a cadence, run a canary ahead
//! of the pin so a break is met before it is adopted.
//!
//! # Why there is no MSRV
//!
//! An MSRV is a contract with consumers: raise it and every downstream that
//! has not moved is stranded. `anvil` is `publish = false` with no dependent
//! in the organisation, so that contract has no counterparty. A promise with
//! nobody on the other side is not caution, it is a second number to keep
//! honest -- and this repository previously carried `rust-version` that no job
//! ever built under, which is a claim rather than a measurement.
//!
//! The principle that number was meant to serve is kept and moved: what is
//! promised must be exercised. The pin is exercised because every CI job
//! builds on it, and the canary is exercised because a person receives its
//! failures.
//!
//! # What a release costs when it is missed
//!
//! 1.98 added `invalid_runtime_symbol_definitions` as DENY-by-default. A new
//! deny lint is a build break scheduled for whenever the pin moves, and the
//! hyperscaler answer is to meet it early -- Google builds the fleet on HEAD
//! so a breaking lint is fixed before it ships. Tracking dated nightly is that
//! answer applied here: the break arrives as one weekly bump PR carrying its
//! own lint fixes, rather than as a wall on the day someone bumps stable.

pub mod bump;

use std::path::Path;

/// A semantic version triple, compared numerically rather than as text.
///
/// String comparison puts `1.100.0` before `1.98.0`, which is exactly the
/// window this module exists to watch: three-digit minors have arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn parse(s: &str) -> Option<Self> {
        let core = s.trim().split(['-', '+']).next()?;
        let mut it = core.split('.');
        Some(Version {
            major: it.next()?.trim().parse().ok()?,
            minor: it.next().unwrap_or("0").trim().parse().ok()?,
            patch: it.next().unwrap_or("0").trim().parse().ok()?,
        })
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// A channel as `rust-toolchain.toml` may name it.
///
/// Two shapes are pinnable and they order differently: a release by version
/// triple, a dated nightly by date. Nothing orders one against the other,
/// because "is this nightly behind that release" is not a question this
/// repository asks -- it tracks one channel at a time, and a move between
/// kinds is a decision rather than a drift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Channel {
    Release(Version),
    /// A nightly pinned to its manifest date, kept as `YYYY-MM-DD` text.
    ///
    /// ISO dates order lexically, so the text is the comparison; parsing it
    /// into a calendar type would buy nothing and add a dependency.
    Nightly(String),
}

impl Channel {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if let Some(date) = s.strip_prefix("nightly-") {
            return is_iso_date(date).then(|| Channel::Nightly(date.to_owned()));
        }
        Version::parse(s).map(Channel::Release)
    }

    /// Whether `self` is newer than `other`, or `None` across kinds.
    ///
    /// `None` is not "equal" and not "unknown": it is "the comparison is not
    /// defined", and a caller that treats it as either would propose a move
    /// between channels as though it were a routine bump.
    pub fn newer_than(&self, other: &Channel) -> Option<bool> {
        match (self, other) {
            (Channel::Release(a), Channel::Release(b)) => Some(a > b),
            (Channel::Nightly(a), Channel::Nightly(b)) => Some(a > b),
            _ => None,
        }
    }
}

/// `YYYY-MM-DD`, shape only. A date rustup does not publish fails at install,
/// which is a better place to find out than a regex that encodes a calendar.
fn is_iso_date(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    matches!(parts.as_slice(), [y, m, d]
        if y.len() == 4 && m.len() == 2 && d.len() == 2
            && [y, m, d].iter().all(|p| p.bytes().all(|b| b.is_ascii_digit())))
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Channel::Release(v) => write!(f, "{v}"),
            Channel::Nightly(date) => write!(f, "nightly-{date}"),
        }
    }
}

/// What is wrong with a repository's pin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Drift {
    /// The pin is not the newest the channel publishes.
    ///
    /// There is no lag budget. A budget made sense when the channel was stable
    /// and moved every six weeks; a dated nightly moves daily and is rolled on
    /// a cadence this repository picks, so "behind" is a fact to report and the
    /// cadence decides what to do about it.
    ChannelBehind { channel: Channel, latest: Channel },
    /// The pin is not declared at all, so nothing can be measured.
    Undeclared { which: &'static str },
    /// The declared pin and the latest are different kinds of channel.
    ///
    /// Moving between a release and a nightly is a decision about how this
    /// repository is built. Reporting it as a bump would let the weekly chore
    /// make that decision on its own, which is not a chore's to make.
    ChannelKindChanged { channel: Channel, latest: Channel },
}

impl Drift {
    pub fn explain(&self) -> String {
        match self {
            Drift::ChannelBehind { channel, latest } => format!(
                "the pin {channel} trails {latest}; every skipped toolchain \
                 carries soundness fixes and deny-by-default lints that become \
                 build breaks the day the pin moves"
            ),
            Drift::Undeclared { which } => {
                format!("{which} is not declared, so it cannot be measured")
            }
            Drift::ChannelKindChanged { channel, latest } => format!(
                "the pin {channel} and the latest {latest} are different kinds \
                 of channel: moving between them is a decision about how this \
                 repository is built, not a bump to propose automatically"
            ),
        }
    }
}

/// The pair a repository declares.
#[derive(Debug, Clone)]
pub struct Declared {
    pub channel: Option<Channel>,
}

/// This binary's own pinned channel, embedded from the repository's
/// `rust-toolchain.toml` when it was compiled.
///
/// Anything that spawns cargo and must not be redirected by the toolchain file
/// of whatever repository it is pointed at needs to name a toolchain. Naming it
/// as a literal makes a copy, and a copy does not move when the pin moves. This
/// repository had eight copies of `1.98.0` -- seven in workflow YAML, one in
/// `authority.rs` -- and the only thing that noticed was a test asserting the
/// literal, which reports the divergence as its own failure rather than as the
/// drift it is.
///
/// `include_str!` makes the copy unrepresentable: there is one pin, and code
/// that needs it reads that one.
pub fn pinned_channel() -> &'static str {
    const FILE: &str = include_str!("../../rust-toolchain.toml");
    channel_text(FILE).expect("this repository's rust-toolchain.toml declares a channel")
}

/// The channel exactly as written, which `channel_from_toml` cannot return.
///
/// A channel is not always a version triple: `nightly-2026-09-10` and `stable`
/// are both valid and neither parses as semver. Callers that pass the channel
/// to rustup need the text; only callers comparing release distance need
/// [`Version`].
pub fn channel_text(text: &str) -> Option<&str> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .find_map(|line| {
            let rest = line.strip_prefix("channel")?.trim_start();
            let rest = rest.strip_prefix('=')?.trim();
            // A trailing comment would otherwise be read as part of the
            // channel, which would read as a pin nobody declared.
            let rest = rest.split('#').next()?.trim();
            let value = rest.strip_prefix('"')?.split('"').next()?;
            (!value.is_empty()).then_some(value)
        })
}

/// `channel = "..."` from a `rust-toolchain.toml`, as a [`Channel`].
pub fn channel_from_toml(text: &str) -> Option<Channel> {
    channel_text(text).and_then(Channel::parse)
}

pub fn read(repo_dir: &Path) -> Declared {
    let channel = std::fs::read_to_string(repo_dir.join("rust-toolchain.toml"))
        .ok()
        .and_then(|t| channel_from_toml(&t));
    Declared { channel }
}

/// Every drift in the pair. Empty means both facts are declared, distinct and
/// current.
///
/// `latest_stable` is passed in rather than fetched: a gate that reaches the
/// network cannot run in a hermetic build, and a verdict that depends on
/// reachability is not deterministic.
pub fn drift(d: &Declared, latest: Option<Channel>) -> Vec<Drift> {
    let mut out = Vec::new();
    let Some(channel) = d.channel.clone() else {
        out.push(Drift::Undeclared {
            which: "the toolchain channel",
        });
        return out;
    };
    let Some(latest) = latest else {
        return out;
    };
    match channel.newer_than(&latest) {
        // Behind is the only direction worth reporting: a pin ahead of what
        // the channel publishes is a pin someone chose deliberately.
        Some(false) if channel != latest => out.push(Drift::ChannelBehind { channel, latest }),
        Some(_) => {}
        None => out.push(Drift::ChannelKindChanged { channel, latest }),
    }
    out
}
