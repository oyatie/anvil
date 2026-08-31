# WS-06 — The autonomy ladder and pre-action authorization

**End-state (hard constraint, not regressed):** sandbox policy, allow/deny (including destructive
actions), and merge decisions are ultimately made by agents under **deterministic pre-action
authorization** — policy evaluated before the tool call, deny-by-default, destructive actions always
tiered highest. Today's doctrine ("green is not merge authority; Jason reviews first") is the bottom
rung, not the ceiling. Rungs are keyed to **blast radius and reversibility** (ADR-0003's three
questions: detection, reversibility, verification), promotion is **earned with evidence and ratified
by Jason via ticket** (interview A-3), and no rung is skipped.

Research grounding (2026): deterministic pre-action authorization is a named field with measured
results (deny-by-default drops adversarial attack success ~75%→0%; OAP, Cedar-fronted Bedrock tool
calls, ACP with TLA+-verified admission); merge authority in industry remains almost exclusively
human and autonomy where granted is **per change class, not per agent** — which is exactly how the
rungs below are cut; trust is windowed with hysteresis and instant demotion (AWS graduated-autonomy
pattern); Claude Code's own classifier-based authorizer has a published 17% false-negative rate on
overeager actions — the gap a *deterministic* authorizer exists to close.

## Measured starting point

The bottom rung is currently a convention, not a policy: dev ruleset requires **0** approving
reviews (`gh api repos/oyatie/anvil/rulesets/21064279`), and Anvil approves + arms auto-merge while
authenticated as `jason931225` (#171). Rung 0 must be made mechanically true before anything climbs.

## The rungs

| Rung | Authority granted | Keyed to | Enter when (evidence, ratified by ticket) |
|---|---|---|---|
| R0 (now) | Agent suggests, reviews, certifies; human approves and merge queue lands | zero delegated blast radius — nothing merges without a human, so reversibility is the human's | H1-9: ruleset requires ≥1 human approval; machine identity (WS-11) so approval provenance is honest |
| R1 | Agent enlists to queue autonomously post-certification; human approval still required per PR | reversible (revert), detected (rehearsal+postsubmit) | 30 days of R0 with zero enlist-door incidents; doors behaviourally tested (WS-05) |
| R2 | Auto-merge lanes per change class: docs-only, lockfile bumps, generated code — under canary + auto-revert | low blast radius, cheap reversal | H2-6: Cedar lane policy; seeded out-of-class PR denied pre-action; 30-day R1 incident-free window |
| R2.5 | Class expansion (tests-only, config-with-schema, single-capability code) with human batch-review after the fact | medium blast radius, still cheaply reversible | per-class evidence packet: seeded-defect coverage on the class, revert drill, 60-day R2 window |
| R3 | General code changes auto-merge on dev behind full certification + rehearsal + post-merge canary; human is exception-handler | trunk blast radius, revert rehearsed | H3-2: 90-day R2.5 window; replayable evidence packet per merge; demotion drill passes |
| R4 | Promotion rungs (staging→canary→production) agent-advanced on health evidence; agents propose policy changes | production blast radius | H3-3/H3-5: 180-day R3 incident-free window (default pin; registry row at promotion); post-merge loop proven (WS-13); policy-change dry-run diffs through registry |

Destructive actions (force-push, branch deletion on shared refs, secret access, ruleset edits,
production rollout) sit in the **highest tier at every rung** — always pre-action authorized,
deny-by-default, human-ticketed until R4, and even at R4 the kill switch and Jason's constitution
remain above them.

## Deterministic pre-action authorization (the enforcement substrate)

- Policy engine: Cedar (already in-tree as `CedarGuard`; oyatie carries `cedar/` faces). Policies
  are data, versioned, and evaluated **before** the tool call in the outbound seam —
  ARCHITECTURE.md M6's `exec::gh()` / `agent(tool, cwd, posture)` is the choke point; `Posture` has
  no `Default`, so a spawn site cannot omit the isolation decision.
- Deny-by-default: an action with no matching permit is refused; refusals are observable
  (answerability, ADR-0003) and logged to the decision registry (WS-07).
- The authorizer is itself a guard: seeded forbidden actions (one per tier) must be denied in CI
  before any rung promotion — prove the check, then trust it.

## Milestones

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H1-9 | R0 made real (ruleset + machine identity + registry row per merge) | ruleset shows ≥1 approval; merges recorded with approver principal ≠ agent principal | Human ticket queue |
| WS06-H1a | M6 outbound seam + Cedar pre-action check wired for `gh` and agent spawns; deny-by-default on destructive verbs | seeded `git push --force` / ruleset-edit attempt denied and logged | Security |
| WS06-H1b | **R1 activation**: 30-day R0 window complete, enlist doors behaviourally tested (WS-05), evidence packet ratified by ticket | registry row for the R1 promotion exists and references the window + door-test proof; enlistment without human per-PR approval still impossible (ruleset) | Human ticket queue |
| H2-6 | R1→R2 with lane policy + canary + auto-revert (R2.5 follows per the rung table's own windows) | see rung table; registry windows recorded | Human ticket queue |
| H3-2 | R3 promotion | see rung table; roadmap H3-2 | Human ticket queue |
| H3-3 | Agentic policy plane mechanism (dry-run diffs, deny-by-default evaluation); each *policy adoption* still ratified via ticket | see rung table; roadmap H3-3 | Security (mechanism) — adoption decisions via Human ticket queue |

## Ratchets

- Promotion is schema-unrepresentable without a ticket reference + evidence packet (registry schema,
  WS-07).
- Demotion is instant and cheaper than promotion: any tripwire (roadmap §5) or incident drops the
  rung one level without a ticket; re-promotion needs a new packet. Hysteresis, not flapping.
- Kill switch: one action halts queue, daemon, codemod fan-out, and all agent spawns; drilled
  quarterly; a failed drill freezes all rungs at current level.

## Non-goals

Agents never ratify their own promotions; no rung skipped on model-capability arguments (autonomy is
a property of the task's verification cost, ADR-0003); `main` keeps its human gate until a registry
decision retires it; no probabilistic/classifier authorizer as the *binding* layer (it may advise;
the deterministic policy decides).
