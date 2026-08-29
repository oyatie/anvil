//! Prove the target toolchain builds this tree, then move the pin. In that
//! order, and never the other.
//!
//! # Why the probe is the product
//!
//! Moving a pin is one line. The reason it does not happen is that nobody
//! knows what breaks, and finding out by moving it turns a routine bump into
//! an outage on the branch everyone shares. A release brings new
//! deny-by-default lints -- 1.98 added `invalid_runtime_symbol_definitions` --
//! and turns warnings into hard errors, so a bump is a build break with a
//! scheduled date.
//!
//! The hyperscaler answer is to meet it early rather than avoid it: build the
//! fleet on the new compiler before it is the compiler. This is that, one
//! repository at a time. The probe runs the whole gate under the TARGET
//! toolchain with the pin untouched, and the edit is only computed if it
//! passed.
//!
//! # What it refuses to do
//!
//! Apply an unproven bump. `Safety::Unproven` carries the first failing
//! command and its output; there is no flag that skips the probe, because a
//! bump applied without one is exactly the change this module exists to stop
//! somebody making by hand.

use crate::exec::{ExecClass, run_bounded};
// `Fix` through the harness, which already re-exports it. Importing it
// from `shape::core` directly would be a second cross-unit facade bypass,
// and the seal counts them exactly.
use crate::harness::{Finding, Fix};
use crate::toolchain::Version;
use std::path::Path;
use tokio::process::Command;

/// Whether the target toolchain can build and test this tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Safety {
    /// Every probe command succeeded under the target.
    Proven { ran: Vec<String> },
    /// A probe command failed. Carries which, and what it said.
    Unproven { command: String, detail: String },
    /// The toolchain is not installed, so nothing was measured.
    ///
    /// Not a failure of the tree, and deliberately not `Unproven`: one says
    /// the bump is unsafe, the other says nobody looked.
    ToolchainAbsent { toolchain: String, detail: String },
}

impl Safety {
    pub fn permits_bump(&self) -> bool {
        matches!(self, Safety::Proven { .. })
    }

    pub fn explain(&self) -> String {
        match self {
            Safety::Proven { ran } => {
                format!("probed under the target toolchain: {}", ran.join(", "))
            }
            Safety::Unproven { command, detail } => {
                format!("`{command}` failed under the target toolchain: {detail}")
            }
            Safety::ToolchainAbsent { toolchain, detail } => format!(
                "toolchain `{toolchain}` is not installed, so the bump was not \
                 measured rather than judged safe: {detail}"
            ),
        }
    }
}

/// The gate, run under `toolchain` with the repository's pin overridden.
///
/// `RUSTUP_TOOLCHAIN` rather than a `+toolchain` argument: the plus form is
/// consumed by the rustup shim and is silently ignored when cargo is invoked
/// directly, which would probe the OLD compiler and report success.
pub async fn probe(repo_dir: &Path, toolchain: &str) -> Safety {
    let steps: [(&str, &[&str]); 3] = [
        ("cargo build", &["build", "--all-targets"]),
        (
            "cargo clippy",
            &["clippy", "--all-targets", "--", "-D", "warnings"],
        ),
        ("cargo test", &["test", "--no-run"]),
    ];
    let mut ran = Vec::new();
    for (label, args) in steps {
        let mut cmd = Command::new("cargo");
        // Captured, not inherited. Without this the child's build log floods
        // A gate whose answer is buried in the build log of the thing it
        // judges has not reported.
        cmd.current_dir(repo_dir)
            .env("RUSTUP_TOOLCHAIN", toolchain)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .args(args);
        match run_bounded(cmd, ExecClass::Build, label).await {
            Err(e) => {
                let detail = e.to_string();
                // "toolchain not installed" is a different answer from "the
                // code does not compile", and collapsing them would report a
                // missing rustup component as a broken tree.
                if detail.contains("is not installed") || detail.contains("no such") {
                    return Safety::ToolchainAbsent {
                        toolchain: toolchain.to_string(),
                        detail,
                    };
                }
                return Safety::Unproven {
                    command: label.to_string(),
                    detail,
                };
            }
            Ok(out) if !out.status.success() => {
                let err = String::from_utf8_lossy(&out.stderr);
                if err.contains("is not installed") {
                    return Safety::ToolchainAbsent {
                        toolchain: toolchain.to_string(),
                        detail: err.trim().chars().take(400).collect(),
                    };
                }
                return Safety::Unproven {
                    command: label.to_string(),
                    detail: err.trim().chars().take(1200).collect(),
                };
            }
            Ok(_) => ran.push(label.to_string()),
        }
    }
    Safety::Proven { ran }
}

/// The edit that moves the channel, as a finding the codemod can apply.
///
/// `Fix::DependOnInstead` rather than a new variant: it is already an exact
/// anchored replacement that refuses with `AnchorNotFound` when the text it
/// expects is absent, which is what a version bump is. The anchor carries the
/// key, so `channel = "1.97.1"` cannot be confused with any other line that
/// happens to hold the same version string.
pub fn channel_bump(from: Version, to: Version) -> Finding {
    Finding {
        rule: "toolchain_channel_behind",
        key: format!("channel:{from}"),
        subject: "rust-toolchain.toml".to_string(),
        detail: format!("channel {from} -> {to}, proven by probe under the target"),
        fix: Some(Fix::DependOnInstead {
            replace: format!("channel = \"{from}\""),
            with: format!("channel = \"{to}\""),
        }),
    }
}

/// The edit that moves the toolchain the CI workflow installs.
///
/// A pin lives in more than one place. `rust-toolchain.toml` is the source of
/// truth, but `dtolnay/rust-toolchain` at a pinned SHA needs the version named
/// explicitly -- omitting it installs `''` and rustup falls back to stable, so
/// CI would silently build with a different compiler than every developer.
///
/// A bump that moves only part of the pin leaves CI and developers on
/// different compilers.
pub fn ci_toolchain_bump(from: Version, to: Version) -> Finding {
    Finding {
        rule: "toolchain_channel_behind",
        key: format!("ci-toolchain:{from}"),
        subject: ".github/workflows/ci.yml".to_string(),
        detail: format!("CI toolchain {from} -> {to}"),
        fix: Some(Fix::DependOnInstead {
            replace: format!("toolchain: \"{from}\""),
            with: format!("toolchain: \"{to}\""),
        }),
    }
}

/// The edit that moves MSRV.
///
/// Separate function, deliberately. MSRV rises for a different reason and on a
/// different schedule, and a single "bump the toolchain" that moved both would
/// re-create the conflated pair this module exists to separate.
pub fn msrv_bump(from: Version, to: Version) -> Finding {
    Finding {
        rule: "toolchain_msrv",
        key: format!("msrv:{from}"),
        subject: "Cargo.toml".to_string(),
        detail: format!("rust-version {from} -> {to}"),
        fix: Some(Fix::DependOnInstead {
            replace: format!("rust-version = \"{from}\""),
            with: format!("rust-version = \"{to}\""),
        }),
    }
}
