# WS-07 — Developer platform: the cockpit as ticket surface, and the decision registry

**Interview A-2:** human-authority decisions surface as tickets **on the Anvil cockpit** (the Tier-0
control plane that already ships). GitHub issues remain the interim surface until the cockpit queue
lands — #19 ("decision needed", explicitly addressed to Jason) is the standing pilot case.

## What counts as a human-authority decision (all of them become tickets)

Autonomy-rung promotions (WS-06), policy/constitution changes, destructive-action approvals,
`decision-needed` escalations (#19-class), ratchet-baseline changes, absorption/versioning calls
(restructure plan's open 1.0 question), managed-repo onboarding, and every standing exception
(e.g. a quarantine TTL extension). If a human must decide it, it is a ticket with an evidence
packet and an audit trail — never a chat message, never a convention.

## The decision registry (typed, append-only)

Research grounding: decision provenance via event sourcing (append-only logs) and tamper-evident
agent audit trails (hash-chained) are the 2026 pattern. Registry schema v1:

```
DecisionRecord {
  id, opened_at, kind: Promotion|Policy|Exception|Escalation|Baseline|Release,
  subject, evidence: [uri],           // packets, seeded-proof runs, windows
  options_considered, decision, decided_by,   // principal — human for authority decisions
  ticket: uri,                        // cockpit ticket (interim: GitHub issue URL)
  effects: [uri],                     // the PR/policy/ruleset change it authorized
  hash_prev                           // chain — tamper-evident
}
```

Stored in-repo (JSONL under a capability, one record per line, append-only), surfaced by the
cockpit. The registry is the substrate; the cockpit is the view + queue.

## Milestones

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H1-10a | Registry v1 lands with schema + append-only + hash-chain checks | a mutated historical record fails the chain check (seeded); a promotion event without `ticket` fails schema | Architecture |
| H1-10b | Cockpit ticket queue MVP: open/route/decide/close with evidence links; interim GitHub-issue mirror | #19 decided end-to-end through it: registry row + closed issue referencing the row | Human ticket queue |
| H2 | Full audit surface: every pre-action denial, rung change, and standing exception visible with drill-down to evidence; SLAs on decision latency | weekly decision-latency report generated; a decision older than SLA shows red on the cockpit | Observability |
| H3 | Exception-driven operation: at R3+ the queue is the human's primary interface — everything else is ambient | a quarterly report job computes, from registry data alone: (a) human decisions per merged change ≤ the threshold pinned in the R3 promotion ticket (default pin: 0.2), and (b) incident count for the quarter ≤ the R2.5-era baseline recorded in the same ticket; breaching either fires the WS-06 demotion tripwire automatically | Human ticket queue |

## Ratchets

- A tier/policy/baseline change whose diff lacks a registry-row reference is merge-blocked (test
  seeds one and asserts refusal).
- Registry is append-only by construction: the chain check makes silent rewrites unrepresentable.
- The cockpit renders only measured values — the `fix/no-surface-publishes-a-fabricated-figure`
  line (#195) becomes a standing rule: a lookup that resolved nothing renders as *unresolved*,
  never as an observation.

## Non-goals

No external tracker (declined in interview); no approval-fatigue workaround by widening standing
permissions (ADR-0003 names permission laundering — the fix is narrowing scope, never widening
grants); the registry never stores prose doctrine (that stays in ADRs; the registry stores
decisions and evidence pointers).
