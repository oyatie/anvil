# Oyatie Anvil Platform Doctrine

## 1. Principle of Autonomous Verification
Every pull request and trunk commit across the Oyatie monorepo and microservices ecosystem must undergo continuous, deterministic evaluation across the live Anvil certification corpus (`TOTAL_GATES` on `PreMergeCertificationReport`). The founding name said sixty gates. That number is historical. The field list is the authority.

## 2. Zero Unresolved Review Threads Invariant
Pull requests may never enter the merge queue or be certified if there are open review threads (`isResolved: false`).

## 3. Native Kubernetes PSA & Zero Third-Party Admission
All pod workloads strictly enforce Kubernetes Native Pod Security Admission (`pod-security.kubernetes.io/enforce: restricted`). No third-party mutating admission webhooks or unmanaged Kyverno dependencies are permitted.

## 4. Wallclock Latency & FinOps Economic Ratchet
PR CI wallclock must target $\le 5\text{min}$ with $\ge 90\%$ compilation cache hit rates. Heavy soak/chaos workloads are partitioned to Nightly/Weekly crons.
