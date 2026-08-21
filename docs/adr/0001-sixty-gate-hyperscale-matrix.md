# ADR-0001: Live-Corpus Pre-Merge Certification Matrix

## Status
Accepted

## Schema
Achieves: Autonomous, deterministic, zero-human-bottleneck pre-merge quality certification, automated merge queue gating, and trunk healing.
Origin: Hyperscale engineering practices synthesized from Google Borg/Piper, AWS Zelkova/Cellular, Meta Sapling/Buck2, and Netflix Spinnaker/Kayenta.
Rule: Every pull request must satisfy every gate on `PreMergeCertificationReport` before entering the merge train. The count is `TOTAL_GATES` (`all_statuses().len()`), asserted by test. The founding name was "60-Gate". That number is historical. The filename of this ADR still says sixty-gate; that is the founding name, not the live count.
Ensure: Automated gate evaluation completes in sub-minute wallclock using DAG-aware predictive test selection and Sccache remote compilation caching.
Overturn-When: A verified formal mathematical proof demonstrates that a strict subset of gates provides identical invariant safety guarantees.

## Context
As the Oyatie platform scales across multiple cell partitions, monorepo packages, and distributed microservices, human review alone cannot guarantee zero downtime, mathematical safety, or sub-5-minute CI wallclock times. Handwritten counts on this page were a second source of truth. They drifted (60, then 68, then 70, then 72) and became false greens.

## Decision
We deploy Anvil as a standalone, autonomous daemon enforcing the live certification corpus over GitHub webhooks. Published pages (README, doctrine, this ADR, OpenAPI, CLI copy) must match `TOTAL_GATES` or DocGuard fails closed after attempting an auto-update. Absent evidence is never a pass. A shallow check may not wear a hyperscaler name.
