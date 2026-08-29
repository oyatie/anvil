# Oyatie Anvil Platform Doctrine

## 1. Principle of Autonomous Verification
Every pull request and trunk commit across the Oyatie monorepo and microservices ecosystem must undergo continuous, deterministic evaluation across the live Anvil certification corpus (`TOTAL_GATES` on `PreMergeCertificationReport`). The founding name said sixty gates. That number is historical. The field list is the authority.

## 2. Zero Unresolved Review Threads Invariant
Pull requests may never enter the merge queue or be certified if there are open review threads (`isResolved: false`).

## 3. Native Kubernetes PSA & Zero Third-Party Admission
All pod workloads strictly enforce Kubernetes Native Pod Security Admission (`pod-security.kubernetes.io/enforce: restricted`). No third-party mutating admission webhooks or unmanaged Kyverno dependencies are permitted.

## 4. Wallclock Latency & FinOps Economic Ratchet
PR CI wallclock must target $\le 5\text{min}$ with $\ge 90\%$ compilation cache hit rates. Heavy soak/chaos workloads are partitioned to Nightly/Weekly crons.

## 5. Rust, and only Rust — and Anvil is judged by it too

Every standard in this section applies to Anvil's own tree exactly as it
applies to a managed repository. Not by intent: by construction. The guards
run against `ANVIL_SOURCE_TREE` on startup and against a managed repo's
worktree on a pull request, through the *same* entrypoints — so a rule Anvil
would enforce on oyatie and not on itself is a rule that does not compile.

Where that has failed, it failed loudly and is recorded: `unit_missing_face`
and `cross_unit_non_facade` are `advisory-until-infra` in `.anvil/shape.json`,
which is Anvil exempting itself from two rules it would apply elsewhere. That
is debt with a name, not a standard with an exception.

`.anvil/shape.json` declares two language profiles — `rust-cargo` and
`rust-module-tree` — and no others. A `TsWorkspace` adapter exists in
`shape::adapters` for repositories that need it; it is not declared here.

Ephemeral helpers in other languages are fine for a one-off measurement. They
are not the pipeline, they are not committed as the pipeline, and nothing
Anvil enforces is written in them. A probe written in Python during this
session reported ten dead stages; the Rust implementation of the same check
reported six, and the Rust one was right. That is the standing reason.

What is enforced, and where:

| Property | Mechanism | Rung |
|---|---|---|
| No `unsafe` at all | `#![forbid(unsafe_code)]` in `lib.rs` | unspellable |
| Memory and type safety | safe Rust, no `unsafe` to opt out of it | unspellable |
| No `unwrap` in production | `rust_language_policy` `err-no-unwrap-prod` | presubmit |
| Borrow over clone, slice over `Vec` | `own-borrow-over-clone`, `own-slice-over-vec` | presubmit |
| No lock held across `await` | `async-no-lock-await` | presubmit |
| Blocking work off the async runtime | `async-spawn-blocking` | presubmit |
| No needless `format!` | `mem-avoid-format` | presubmit |
| `// SAFETY:` on any `unsafe` | `unsafe-safety-comment` | presubmit |
| Idiomatic, lint-clean | `clippy -D warnings` | pre-push and presubmit |
| Formatted | `rustfmt --check`, edition read from the manifest | pre-commit, pre-push, presubmit |
| 100–300 lines per hand-written file | `modularization_guard` | presubmit |

`forbid` rather than `deny`: an inner `allow` cannot reopen it.

## 6. `#[cfg(test)]`, `#[test]`, and `#[cfg(test)] mod tests`

Three environments, and the attributes are how a build is told which one it is
compiling. Choosing wrongly is not a style question: one form defeated twelve
scanners in this repository at once, and another would ship test dependencies
into every running container.

| Environment | Attributes | Why it exists at scale |
|---|---|---|
| **Production build** | neither | Zero overhead. Everything behind `#[cfg(test)]` or marked `#[test]` is stripped entirely — smaller image, faster cold start, smaller attack surface. |
| **Inline unit test** | `#[cfg(test)]` + `#[test]` | Fast, local, reaches private items. `#[cfg(test)]` is what keeps mocking frameworks and bulk fixtures out of the production binary. |
| **Integration test** | `#[test]` only | End-to-end against the public API. Cargo already compiles `tests/` as a test-only target, so `#[cfg(test)]` there is redundant noise. |

The cost argument is the one that makes this non-negotiable rather than
tidiness. A `mod tests` that forgets `#[cfg(test)]` compiles its mock
libraries and its fixture structs into the live service — wasted resident
memory multiplied by every instance, plus every one of those test
dependencies now inside the production attack surface. The attribute is not
decoration; it is the boundary between what ships and what does not.

Three forms, three places.

**Unit tests, in the same file as the code.** `#[cfg(test)] mod tests { … }`
at the bottom. Keeps the namespace clean, strips test helpers from the
production build, and reaches private items.

**Unit tests that would push the file past the 300-line budget.**
`#[cfg(test)] mod tests;` with the body in a sibling `tests.rs`. Still a unit
test, still reaches private items — but **the attribute now lives in the
parent and the test file carries no marker of its own**. Every scanner that
strips test code by searching for the literal `#[cfg(test)]` *inside* a file
goes blind. Twelve in this repository did, and five unit tests were counted as
production diff parsers. Use `source_scan::is_cfg_test_module_file`, never a
hand-rolled search.

**Integration tests, in `tests/`.** A bare `#[test]`. The directory is already
compiled only for `cargo test`, so `#[cfg(test)]` there is redundant noise.
Each file is its own crate and sees only the public API — which is the point:
if a test in `tests/` cannot reach it, neither can a caller.

**A test-only method on a production type.** `#[cfg(test)]` on the `impl`
block, not on the whole type.

**The rule that decides it:** does the test need a private item? Then it is a
unit test and lives with its module. Does it exercise the public surface?
Then it belongs in `tests/`, where being unable to reach an internal is
information rather than an obstacle.

Two consequences worth stating, both measured here:

- A file reached by `#[cfg(test)] mod tests;` compiles **only** under test.
  Nothing in it is production code, and a scanner that treats it as such
  inflates every ratchet it feeds.
- A caller that exists only inside `#[cfg(test)]` does **not** make a stage
  live. `gate_proof` and `postmortem` are both in exactly that state:
  fully tested, and run by nothing in production.
