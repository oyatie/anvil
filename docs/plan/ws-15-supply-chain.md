# WS-15 — Supply chain, provenance, and the agent-config attack surface

**Why front-of-mind (research gap 2):** a Rust fleet with agents authoring and eventually landing
changes is a high-value injection target, and deny-by-default tool authorization does nothing about
a malicious crate, a poisoned toolchain, or a tampered receipt. Measured starting point: the README
already claims SBOM/SLSA ambitions, `supply_chain_guard` reads the real lockfile and resolves OSV
(one of the 5-of-64 gates that invoke real tooling, #59), `AttestationGuard` honestly reports
`NotMeasured` because "nothing signs or attests" the lane receipts, and `deny.toml` +
`supply-chain-weekly.yml` exist. The gap is signing, provenance, and treating **agent configuration
as supply chain**.

Research grounding (2026): agent config (skills, MCP servers, instruction files) is a managed,
deny-by-default supply chain; provenance attestation (sigstore/in-toto-style) and
proof-of-execution — evidence-carrying, replayable authorization for what actually ran — are the
patterns that make "the gate ran" verifiable rather than asserted.

## Milestones

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H2-8a | Receipts signed: certification receipts and `LscReport`s carry sigstore-style attestations; an independent verifier job re-checks them | seeded unsigned/tampered receipt refused by the verifier (red proof archived); `AttestationGuard` flips from `NotMeasured` to `Measured` honestly | Security |
| H2-8b | Agent-config supply chain: skills, MCP servers, harness instruction files, and model/prompt versions are an enumerated, reviewed allowlist per repo; changes go through a review lane | unenumerated config source fails the census; seeded rogue MCP entry red | Security |
| H2-8c | Rust-native certification additions (research gap 5), each landing as an M2 rule with fixture: `cargo-semver-checks` on published surfaces, MSRV verification (channel and `rust-version` are separate promises — verified separately), `cargo-deny` advisories as `Evaluated` | each new rule's fixture red-then-green before registration (WS-08 mandate applies — no gate without proof) | Security |
| H3 | SLSA-lane provenance for built artifacts (daemon binary, any published crate): builds reproducible or provenance-attested; `swap-installs-the-new-binary`-class checks bound to attested artifacts | verifier refuses an unattested binary swap (seeded); provenance chain from source rev to running daemon demonstrated in a drill | Security |

## Ratchets

- Verifier-required: once signing lands, an unsigned receipt is unrepresentable in the report
  schema (not a warning).
- Config allowlist is baseline-frozen; additions require a registry ticket (WS-07).
- Dependency policy: `deny.toml` + lockfile `--locked` discipline stays; a new dependency in an
  authority path (webhook signature, admin auth — the `hmac`/`sha2` bump is already flagged
  security-relevant in the restructure plan) requires the security review lane.

## Non-goals

No full SLSA L3+ build farm before target-keyed certification exists (WS-03 order); no signing
theater — a signature over an unmeasured claim is worse than none (WS-08's honesty law applies to
attestations too: sign `Evaluated`, never `Passed`-shaped assertions).
