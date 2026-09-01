# WS-11 — Actor identity and credential brokering

**The class (issue #171):** Anvil authenticates as the human who reviews it. `gh` runs as
`jason931225`, so the loop-guard's `me != author` drops every one of Jason's review comments — the
fixer never runs on human feedback on any watched repository. The issue's own analysis is the
doctrine: *no predicate over a single shared login can separate "mine" from "my reviewer's"* — the
defect is upstream of the check, in identity, and string-matching (`contains("bot")`,
`contains("antigravity")`) is a proxy standing in for a principal.

The promotion ladder shares the root: `promotion-open-next` fails 13/13 with
`GitHub Actions is not permitted to create or approve pull requests` because no purpose-built
credential exists (WS-13 consumes this workstream's fix).

Research grounding (2026): non-human identity for agent fleets is a named gap (research backlog
item 3) — per-agent, short-lived, scoped credentials brokered at spawn; the credential a turn *is*
must be separated from the authority the system *has* (ARCHITECTURE.md's Posture/env-allowlist
already states this locally).

## Milestones

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H1-6a | Anvil becomes a GitHub App (machine identity); daemon, fixer, enlister, and promotion workflows run under it | daemon `gh api user` (or app equivalent) resolves the app slug ≠ `jason931225`; approvals/comments authored by the app principal | Security |
| H1-6b | Loop-guard decides on principal: "is this actor me" and "is this my reviewer" become different questions with different answers | seeded Jason-comment fixture reaches the fixer; seeded self-comment fixture does not; both in CI | Security |
| H1-6c | String-identity predicates die: no `contains("bot")`-class checks in authority paths | meta-test over authority modules: zero substring identity predicates (seeded one, proved red) | Security |
| H2 | Credential brokering: per-turn, least-scope tokens leased at the outbound seam (M6 `Posture` carries the lease); installation tokens per repo, never org-wide long-lived PATs | a spawned agent's env contains only the leased token (asserted by the seam's own test); token TTL ≤ turn budget | Security |
| H2 | Approval provenance: at R0/R1 the *human's* approval is the human's — the enlister never approves under a shared identity again; registry rows carry the approving principal | registry row schema requires `decided_by` principal; a merge whose approver == agent principal at rung < R2 is refused (seeded) | Security |

## Ratchets

- Meta-test keeps substring identity predicates unwritable in authority paths.
- The seam's env-allowlist is a frozen baseline (what a turn may see only shrinks).
- Every credential in CI/daemon config is enumerated with owner + scope + rotation date in a
  checked manifest; an unenumerated secret name fails the manifest test.

## Non-goals

No SPIFFE/SPIRE deployment at current fleet size (pattern noted for H3 scale; backlog item 3);
no shared bot account as the "fix" (that reproduces the class one principal over); Jason's own
account never again the daemon identity — that is the class made unwritable, not reconfigured.
