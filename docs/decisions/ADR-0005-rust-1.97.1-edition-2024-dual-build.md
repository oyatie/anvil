---
status: Accepted
date: 2026-08-20
---

# ADR-0005 — Rust 1.97.1, edition 2024, Cargo + Buck2 dual build

## Context

Anvil enforces a toolchain pin, an edition, and a dual-track build on the
repositories it manages (`dual_track_build_guard`, `rust_language_policy`) while
carrying none of them itself: no `rust-toolchain.toml`, edition 2021, zero Buck2
files. The host happened to have 1.97.1 installed, so CI's `toolchain: stable`
was 1.97.1 by accident, not by decision. oyatie pins `1.97.1` in
`rust-toolchain.toml` and `oya-deps.toml`, builds edition 2024 workspace-wide,
and treats Cargo as the merge path with Buck2 as local hermeticity (ADR-0716).

## Decision

1. **Toolchain is pinned**: `rust-toolchain.toml` with `channel = "1.97.1"`,
   `rustfmt` + `clippy`, minimal profile — byte-identical to oyatie's.
   `[package] rust-version = "1.97.1"`. CI installs `"1.97.1"`, never `stable`.
2. **Lockfile is authoritative**: every CI and hook invocation passes
   `--locked`; `Cargo.lock` stays at format `version = 4`.
3. **The release profile is compile-checked post-submit**: `cargo build
   --release --locked` runs on push to the trunk (`dev`) and to every promotion
   rung — `ci.yml` triggers on all of them. It uploads nothing and deploys
   nothing — `scripts/start.sh` builds its own binary on the host. Its only job
   is to prove the release profile still compiles, which the pre-merge legs
   (`cargo nextest` + clippy, dev profile, amd64) do not cover. It is not a
   merge-queue gate.
4. **Edition 2024** follows in its own change (behaviour-class under I8: the
   `gen` keyword, `if let` temporary scope, and impl-trait capture rules change
   semantics, so it is reviewed apart from any file move).
5. **Dual build** follows once the tree is a workspace: `.buckconfig`,
   `toolchains/BUCK`, reindeer-vendored `third-party/`, and first-party `BUCK`
   files that are generated from `cargo metadata` and checked for drift.
   Cargo remains merge authority; Buck2 is local hermeticity and a weekly
   smoke, as in oyatie.

## Consequences

- A contributor on a different toolchain gets the pinned one from rustup
  automatically; CI and local builds can no longer diverge silently.
- `tests/lockfile_policy_test.rs` pins the lockfile format and asserts that
  `rust-toolchain.toml` and `rust-version` agree, so the two cannot drift.
- This is the first self-application of the standard Anvil applies to its
  tenants. Every later shape rule is enabled on `oyatie/anvil` before it is
  enabled anywhere else.
