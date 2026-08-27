# Changelog

All notable changes to the **Anvil** Autonomous Delivery Fabric will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed — clean-architecture boundary enforcement

- **clean_architecture_guard**: reported `0 violations across 55 classified files` while `git_manager` held a
  direct reference to `change_delivery::adapters::git_vcs`. Two blind spots: it classified a file by *its own*
  path, so a file belonging to no layer — most of the tree — had no forbidden direction and could bind to any
  other unit's interior; and an edge only counted when spelled `use`, so the offending expression was filtered
  out before any rule saw it. Added the rule the four faces exist to create: a unit's `core`, `ports` and
  `adapters` are its interior, and only its `facade` is importable from outside. **19** cross-unit bypasses
  recorded as an exact ratchet, `FACADE_BYPASSES_IN_ANVIL`.
- **clean_architecture_guard**: five further defects found by running the new rule against the spellings it
  would meet, two of which *accused conformant code* — Cargo workspace layout (`crate::` is rooted at a
  crate's `src/`, so taking the first path segment made every file a member of a unit called `crates`),
  grouped `use` (produced a unit named `{b` and missed the rest of the group), nested `use`, one-path-per-line
  (a legal reference placed first hid an illegal one after it), and a found violation reported as an absence.
  A sixth — a `use` broken across lines by rustfmt — was found by Anvil's own review of the change.
- **clean_architecture_guard**: `INTERIOR_FACES` dropped `domain`, `application` and `adapter`. `.anvil/shape.json`
  declares exactly four faces; the spec's `layer_suffixes` table maps `domain -> core` for *crate names*, not
  face directories, and `application` is not in the spec at all. A test now reads the face set out of the spec
  so the constant cannot drift from it.
- **source_scan**: added `is_cfg_test_module_file`. Twelve scanners strip test code by searching for the literal
  `#[cfg(test)]` *inside* a file, which the standard `#[cfg(test)] mod tests;` sibling-file layout defeats — the
  attribute is in the parent. Five unit tests were being read as production diff parsers.
- **diff-parsing ratchet**: `FN_INTRODUCERS` was an enumerated list of eighteen literal prefixes and did not
  contain `pub(super) fn `, so moving a function out of an impl block made a real diff parser invisible and the
  count fell from 19 to 18. A fall that is really a blind spot is worse than a rise: it is recorded as progress.
  Replaced by a closed grammar — optional visibility, then qualifiers in any order, then `fn`.

### Changed

- **clean_architecture_guard**: split from one 928-line file into `report`/`paths`/`scan`/`analyze`/
  `source_tree`/`tests` modules within the same crate, under the 300-line hand-written file budget. Behaviour
  is unchanged; the suite is the check.

### Fixed — merge-decision integrity

- **reviewer**: an unparseable AI response reported verdict `COMMENT`, which the evaluator accepts, so a
  refusal, an error string or truncated output certified the pull request and enlisted it in the merge queue.
  It now reports a new `ERRORED` verdict, which blocks. `verdict` also lost `#[serde(default)]`, so a
  response omitting the field is a parse failure rather than an implicit pass.
- **gates**: added `GateStatus::Errored` and `GateStatus::NotMeasured`. Merge admission is now gated on
  `is_admissible()` — certified **and** every gate actually measured — so a gate with no data source can no
  longer contribute to a pass.
- **doc_guard**: gate 1 of 68 was unfailable; three paths returned `is_doc_sufficient: true` (unparseable
  output, timeout or spawn failure, and the watchdog fallback). All now report an error. It also ran with no
  `current_dir`, so it evaluated Anvil's own working directory instead of the repository under review, and
  its 20s timeout could not accommodate an xhigh-effort call.
- **fixer**: a failure to *spawn* `cargo` returned `Ok(true)` "verification gate PASSED". Spawn failure is now
  an error, and a failing gate no longer commits and pushes anyway.
- **attestation**: receipts hardcoded verdict `CERTIFIED_READY` and a fixed list of verified gates while being
  stamped *before* the gate matrix ran. Verdict and gate list are now supplied from the computed result.
- **telemetry**: pass/fail counts were hardcoded as `(70, 0)` / `(69, 1)`, so every failing PR was recorded as
  exactly one failed gate — the reason accumulated telemetry showed most PRs "stuck at 69/70". Counts are now
  computed, and the previously-empty `gate_failures` sink now records which gates failed and why.
- **feature_flag**: all three rules were unmatchable, so the gate's green was unreachable by any input. Two
  were annotations no flag system uses — LaunchDarkly, Unleash, Statsig and OpenFeature all decide staleness
  on their own backend, and `ld-find-code-refs` and `piranha` locate flags by the *key* at the call site, not
  by a comment — the third was a year window ending in 2025, expired in 2026, and the dead-branch rule
  required source rustc and clippy both reject. The key scan survives as the seam a real flag API plugs into;
  staleness is now answered from a `STALE-FLAGS.md` ledger, and reports `NotMeasured` when there is none.
- **local_probe**: the conventional-commit check graded the hardcoded literal `"feat: update codebase"`, and
  `starts_with("feat")` accepts headers Conventional Commits 1.0.0 rejects. The pipeline now reads the pull
  request's real subjects with `git log <base>..<head>` and judges them against the grammar. `latency_ms` was
  the literal `18`, and a test asserted it was under 100 — the eighth assertion over a constant found here;
  it is now the call's own elapsed time. The AST-linting claim is withdrawn: there is no AST and no parser.
- **chaos_injection**: three faults were declared and handed to a simulator that never read its `fault`
  argument, so one scan produced three identical verdicts carrying a fabricated recovery time, and the
  blocking sentence named a preview sandbox that does not exist. Every real chaos tool acts on a running
  system; nothing here runs one. The surviving half is published as what it is — a lint for an `.unwrap()`
  on an awaited call — and a diff without one reports `NotMeasured`, not resilient.

### Fixed — ingress and untrusted input

- **webhook**: `gh webhook forward` was spawned without `--secret`, so its GitHub-side hook used a different
  secret than the daemon verifies with; every delivery was unsigned. HMAC verification now runs in observe
  mode, logging `signature_valid` without rejecting, pending promotion to enforcing.
- **webhook**: added `GITHUB_WEBHOOK_SECRET_PREVIOUS` so a secret rotation does not drop in-flight deliveries
  signed with the old secret.
- **api**: the manual `/api/*` handlers accepted an arbitrary `repo`, so a request could clone any repository
  and run an agent inside it. All eight repo-taking handlers now validate against `WATCHED_REPOS`.
- **git**: `get_repo_dir` accepted `"x/.."`, resolving outside the repositories directory where executable
  hooks are written. Now sanitised.
- **dashboard**: no HTML escaping existed anywhere; repository names, PR titles and activity rows were
  interpolated into HTML, and the SSE row used `innerHTML` — stored XSS on a page sharing an origin with the
  unauthenticated control surface. Added `dashboard::escape` and applied it throughout.
- **git**: fork pull requests were indistinguishable from same-repo ones, so `git push origin HEAD:<branch>`
  could push into the base repository's branch of that name. Fork PRs are now detected and their pushes
  refused; review and certification still run.
- **secrets**: `ManagedAccount` and `AddAccountPayload` derived `Debug` while holding plaintext tokens. Both
  now redact.

### Fixed — execution safety

- **exec**: new `crate::exec` module bounding subprocesses with a timeout and `kill_on_drop`. Anvil spawns 118
  subprocesses; exactly one had a timeout. `ModelExecutionConfig::print_timeout_secs` was set in 23 places and
  read nowhere — it now bounds the call it is named for.
- **ci_triager**: a log tail sliced at a byte offset into UTF-8 output panicked whenever the offset landed
  mid-character, killing the triage task. Now slices on character boundaries.
- **server**: webhook forwarder children inherited the operator's terminal and left it in raw mode, which
  staircased log output and disabled Ctrl-C. Their stdin is now detached.

### Known limitations

- HMAC verification is in **observe mode** and does not yet reject unsigned deliveries.
- The webhook secret is passed to `gh webhook forward` via `--secret`, so it is visible in `ps`. The extension
  offers no environment binding; this is one reason the ingress is slated for replacement.
- `kill_on_drop` reaps only the direct child. `gh`, `agy`, `cursor-agent` and `cargo` fork helpers that
  survive; full containment needs a process group.
- Documentation parity runs only inside the PR pipeline and cannot amend existing documents.

### Added

- **webhook**: Added `GET /healthz` liveness probe endpoint returning HTTP `200 OK` (`"ok"`) for container orchestrators and Kubernetes liveness checks ([#1](https://github.com/oyatie/anvil/pull/1)).
- **openapi**: Updated OpenAPI 3.0.3 specification in `openapi/openapi.yaml` declaring route definition and response schema for `GET /healthz`.
- **tests**: Added asynchronous unit test suite `test_healthz_handler` in `src/webhook/mod.rs` verifying deterministic `200 OK` response payload with 100% differential branch coverage.

## [0.1.0] - 2026-08-19

### Added

- Initial release of **Anvil: Hyperscale Autonomous Engineering Delivery Fabric**.
- Comprehensive 60-Gate pre-merge certification engine (`PreMergeGuard`, `DocGuard`, `ApiContractGuard`, `CedarGuard`, `ComplianceGuard`, `KaniGuard`, `SloCanaryGuard`, `GhostMigrationHarness`, `ChaosMutationGuard`, `AttestationGuard`, etc.).
- Webhook listener daemon and CLI operations (`serve`, `review`, `fix`, `certify`, `triage`, `enlist`, `heal-queue`, `reconcile`).
- Formal verification with Kani and SMT solver harnesses.
- Autonomous PR review, triaging, and merge queue self-healing pipelines.
