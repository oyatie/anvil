//! Two facts that are not one number: what we compile WITH, and what we
//! promise to compile UNDER.
//!
//! # Why they are different
//!
//! The toolchain channel is an operational choice. It should track stable,
//! because every release carries soundness fixes, new deny-by-default lints
//! that are future build breaks, and stdlib APIs that let hand-rolled code be
//! deleted. Lagging is debt that compounds.
//!
//! MSRV is a contract with consumers. It should rise rarely and deliberately,
//! because raising it strands every downstream that has not moved. Lagging is
//! a feature.
//!
//! They move in opposite directions for opposite reasons, so one number cannot
//! serve both. Anvil carried `1.97.1` in `rust-toolchain.toml` and `1.97.1` as
//! `rust-version` while stable was `1.98.0`: not a coincidence, an unmanaged
//! pair. Neither fact was being decided, and nothing in the tree could tell.
//!
//! # What a release costs when it is missed
//!
//! 1.98 added `invalid_runtime_symbol_definitions` as DENY-by-default. A new
//! deny lint is a build break scheduled for whenever the pin moves, and the
//! hyperscaler answer is to meet it early -- Google builds the fleet on HEAD
//! so a breaking lint is fixed before it ships. The equivalent here is that
//! the channel lag is measured and published rather than discovered on the
//! day someone bumps it.

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

    /// Releases between two versions, counting minors only.
    ///
    /// Rust ships a minor every six weeks and patches out of band, so the
    /// minor is the unit of "how far behind" a channel is.
    pub fn minors_behind(self, newer: Version) -> u32 {
        if newer.major != self.major {
            return u32::MAX;
        }
        newer.minor.saturating_sub(self.minor)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// What is wrong with a repository's toolchain pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Drift {
    /// One number for both facts. Neither is being managed.
    Conflated { at: Version },
    /// The channel trails stable by more than the budget.
    ChannelBehind { channel: Version, by: u32 },
    /// MSRV promises more than the channel can deliver: unbuildable.
    MsrvAheadOfChannel { msrv: Version, channel: Version },
    /// A fact is not declared at all, so nothing can be measured.
    Undeclared { which: &'static str },
}

impl Drift {
    pub fn explain(&self) -> String {
        match self {
            Drift::Conflated { at } => format!(
                "`rust-toolchain.toml` channel and `rust-version` are both {at}. \
                 They are different promises -- one is what we compile with and \
                 should track stable, the other is what consumers may compile \
                 under and should rise rarely. One number means neither was \
                 decided."
            ),
            Drift::ChannelBehind { channel, by } => format!(
                "the toolchain channel {channel} is {by} release(s) behind \
                 stable; every skipped release carries soundness fixes and \
                 deny-by-default lints that become build breaks the day the \
                 pin moves"
            ),
            Drift::MsrvAheadOfChannel { msrv, channel } => format!(
                "MSRV {msrv} is newer than the channel {channel}: the promised \
                 minimum cannot build here at all"
            ),
            Drift::Undeclared { which } => {
                format!("{which} is not declared, so it cannot be measured")
            }
        }
    }
}

/// The pair a repository declares.
#[derive(Debug, Clone)]
pub struct Declared {
    pub channel: Option<Version>,
    pub msrv: Option<Version>,
}

/// `channel = "..."` from a `rust-toolchain.toml`.
pub fn channel_from_toml(text: &str) -> Option<Version> {
    field_after(text, "channel")
}

/// `rust-version = "..."` from a `Cargo.toml`.
pub fn msrv_from_manifest(text: &str) -> Option<Version> {
    field_after(text, "rust-version")
}

fn field_after(text: &str, key: &str) -> Option<Version> {
    text.lines()
        .map(str::trim)
        // A whole-line comment cannot match anyway -- its key carries the `#`
        // -- but a TRAILING one silently breaks parsing:
        // `channel = "1.98.0" # bumped` leaves `1.98.0" # bumped` after the
        // quote trim, whose patch component does not parse, so the pin reads
        // as UNDECLARED. A version this module cannot see is one it cannot
        // report as behind, which is the quiet direction of the failure.
        .filter(|l| !l.starts_with('#'))
        .find_map(|l| {
            let (k, rest) = l.split_once('=')?;
            if k.trim() != key {
                return None;
            }
            let value = rest.split('#').next().unwrap_or(rest);
            Version::parse(value.trim().trim_matches('"'))
        })
}

pub fn read(repo_dir: &Path) -> Declared {
    let channel = std::fs::read_to_string(repo_dir.join("rust-toolchain.toml"))
        .ok()
        .and_then(|t| channel_from_toml(&t));
    let msrv = std::fs::read_to_string(repo_dir.join("Cargo.toml"))
        .ok()
        .and_then(|t| msrv_from_manifest(&t));
    Declared { channel, msrv }
}

/// How many releases the channel may trail stable before it is a finding.
///
/// Two, not zero. A release lands and a fleet needs a window to absorb it; a
/// budget of zero would make every Tuesday a finding and teach readers to
/// ignore the gate. Two six-week trains is twelve weeks of slack and is still
/// inside the window where the next deny-lint has not yet shipped.
pub const CHANNEL_LAG_BUDGET: u32 = 2;

/// Every drift in the pair. Empty means both facts are declared, distinct and
/// current.
///
/// `latest_stable` is passed in rather than fetched: a gate that reaches the
/// network cannot run in a hermetic build, and a verdict that depends on
/// reachability is not deterministic.
pub fn drift(d: &Declared, latest_stable: Option<Version>) -> Vec<Drift> {
    let mut out = Vec::new();
    let Some(channel) = d.channel else {
        out.push(Drift::Undeclared {
            which: "the toolchain channel",
        });
        return out;
    };
    let Some(msrv) = d.msrv else {
        out.push(Drift::Undeclared {
            which: "MSRV (`rust-version`)",
        });
        return out;
    };
    if channel == msrv {
        out.push(Drift::Conflated { at: channel });
    }
    if msrv > channel {
        out.push(Drift::MsrvAheadOfChannel { msrv, channel });
    }
    if let Some(stable) = latest_stable {
        let by = channel.minors_behind(stable);
        if by > CHANNEL_LAG_BUDGET {
            out.push(Drift::ChannelBehind { channel, by });
        }
    }
    out
}
