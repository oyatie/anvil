# Anvil Platform Doctrine

## 1. Principle of Autonomous Verification
Every pull request and trunk commit across the watched monorepo and microservices ecosystem must undergo continuous, deterministic evaluation across the **60-Gate Hyperscale Delivery Fabric**.

## 2. Zero Unresolved Review Threads Invariant
Pull requests may never enter the merge queue or be certified if there are open review threads (`isResolved: false`).

## 3. Native Kubernetes PSA & Zero Third-Party Admission
All pod workloads strictly enforce Kubernetes Native Pod Security Admission (`pod-security.kubernetes.io/enforce: restricted`). No third-party mutating admission webhooks or unmanaged Kyverno dependencies are permitted.

## 4. Wallclock Latency & FinOps Economic Ratchet
PR CI wallclock must target $\le 5\text{min}$ with $\ge 90\%$ compilation cache hit rates. Heavy soak/chaos workloads are partitioned to Nightly/Weekly crons.

## 5. Default engineering practice
Every capability and every `app/<product>/` is hexagonal: `core/`, `ports/`, `adapters/`, `facade/` (plus `cedar/`, `observability/`, `iac/`, `docs/`). Unknown or dump roots (`plan/`, `tasks/`, `specs/`, `libs/`, …) fail. Absence of an allowed name is not a failure. Anvil's own tree is the control plane, not this layout.

## 6. Operator intake
Client need (ambiguous or wrong) is received by the **human operator** working with Anvil. Research and existing docs are verified into an **ephemeral artifact package**. Raw client text never reaches implement. The package hands off to **Product** (`app/`) **xor** **Program** (capability roots). Mixed packages fail. Dump-root requests are rejected.

## 7. Deterministic lanes, no spawn, no fold
Anvil computes ready hops. It does not spawn agents. N disjoint implement-ready slices are N implement hops. Binding k < N agents is a fold and is illegal. Completing a hop unblocks successor roles on that slice (fan-out, not a serial mega-pipeline) and frees that role for other slices. Each hop binds a **fresh** agent id.

## 8. Cross-slice amendment without a committee
A team that does not own a path **must not write it**. They file a `ports/draft/` or `adapters/draft/` on **their** path-set (commutes). The owner gets a `ContractAmend` hop when those owner paths are free. Breaks (presubmit / merge_group red) quarantine **that path-set**; other disjoint slices continue. No ticket board. No waiting on a lock.
