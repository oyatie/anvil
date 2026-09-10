# The canonical shape for oyatie, measured

Status: proposal with measurements. Three rulings below are Jason's, not
Anvil's — ADR-0006 §3 puts registry changes in the tenant's hands.

Everything numbered here was measured on 2026-09-10 against
`oyatie/oyatie@b4556b57fbbee53d7f09c519fa200d230bf85323`. Where two
instruments were available they were both run and both are quoted. Inputs
came from this session plus the `buck2` and `oci` sessions, each of which
separated what it had measured from what it had not.

## 1. The grammar already exists, is Accepted, and matches nothing

`docs/standards/naming-convention-bnf-v4.md` in oyatie carries
`status: Accepted`, `enforced_by: governance-naming-convention`, and (via its
companion `crate-naming-convention.md`) `Severity = BLOCKER`. It is a full BNF
for crate names, contract files, policies, events, SLOs and ADR filenames.

It describes a repository that does not exist.

| The standard scopes | Entries in oyatie's tree |
| --- | --- |
| `crates/` | 0 |
| `microservices/` | 0 |
| `contracts/` | 0 |
| `registry/` | 0 |
| `specs/` | 0 |
| `.omc/` | 0 |

The lane declared to enforce it, `governance-naming-convention`, appears
nowhere in the tree. The registry its slot-2 rule requires,
`[workspace.metadata.oyatie.microservices]`, is not in `Cargo.toml`.

Conformance of the 480 crates that do exist:

| Rule | Conformant |
| --- | --- |
| N-002, `oyatie-` prefix | **0 / 480** |
| BNF v4.1, `oya-` prefix | **0 / 480** |
| N-005, ends in an ADR-0105 layer token | 266 / 480 |
| N-003, `check-` crates | 1 |

The prefix figure was measured twice, by different instruments that agree:
`awk` over the git tree, and Anvil's own engine reporting
`crate_name_prefix: 480` (§4). This is not drift. Nothing ever measured it.

## 2. What oyatie actually is

The unit is a **capability**, not a `src/` module:

```
<capability>/<face>/<crate>/          411 crates
app/<product>/<face>/<crate>/          61 crates
*/{ports,adapters}/draft/<crate>/       8 crates
                                     ---
                                      481 Cargo.toml (incl. the root manifest)
```

- **21 capabilities**, each carrying `OWNERS` at its root — 21 of 21, no
  exceptions. Faces are drawn from `core/ ports/ adapters/ facade/`.
- **7 products** under `app/`, each also carrying `OWNERS` — 7 of 7.
- **496 BUCK files**, one per crate, beside each `Cargo.toml`.
- **16 root files.** Anvil's proposed allowlist admits exactly these and
  refuses nothing legitimate: measured `root_file_unallowlisted: 1`.
- Satellites under capabilities: `observability` (16), `cedar` (14),
  `iac` (12).
- No capability carries a file at a fixed depth inside a face, so unit
  discovery cannot be driven off a marker deeper than the unit root.

Two constraints that bound anything built on this:

- `Cargo.toml` states that **gates must not parse the members array**
  (ADR-0538); the canonical resolver is `pipeline/core/workspace-members-kernel`.
- oyatie's own `layout.rs` `FORBIDDEN_NAMES` refuses a root `governance/`,
  `specs/`, `tools/`, `libs/`, `infra/`, `tests/` and others. Any shape
  declared has to live inside that.

## 3. Anvil's proposal for oyatie had never produced a number

`tests/fixtures/shape/oyatie.shape.json` has shipped since ADR-0006 with 15
rules, 11 of them `baseline-block-on-new`. Measuring it against oyatie did
not return a large number or a small one. It refused:

```
Command failed: unit registry could not be used: unit kind "capability"
enumerates members from governance/capability-registry.json but no registry
document was supplied
```

`governance/capability-registry.json` does not exist in oyatie, and cannot —
`governance/` is a forbidden root there. Its `unit_marker`, `manifest.json`,
is also absent from every capability and product root; the four files by that
name in the repo are a frontend client manifest, a test fixture and two
sovereignty packs.

So both halves of the proposal were unmoored, and the failure mode of the
second is the quiet one: a marker matching nothing yields a clean zero that
reads as conformance.

`tests/a_shipped_proposal_can_be_measured_test.rs` now refuses both. It was
proved by seeding each defect exactly as it shipped: the `manifest.json`
marker fails with `the app unit kind discovered []`, and removing the
companion registry fails the other test. Restored, both pass.

## 4. The corrected proposal, measured

Three changes, each grounded in §2: marker `manifest.json` → `OWNERS`
(21/21 and 7/7), the `app` kind discovers on `OWNERS`, and the registry moves
to `.anvil/capability-registry.json` beside the spec, out of the forbidden
root. `tests/fixtures/shape/oyatie.capability-registry.json` ships the
proposed registry; `meta_directories` is empty because no `harness/`
directory exists in oyatie.

It now measures:

```
shape: oyatie/oyatie @ b4556b57fbbe (PROPOSED)
  units: 28 (4 conformant)
  findings: 1523 (228 misplaced files, 419 denied edges)
    crate_name_prefix                480
    cross_unit_non_facade            340
    satellite_alias_used             228
    crate_layer_suffix               170
    face_edge_denied                 112
    suffix_face_disagree              82
    unit_root_file_unallowlisted      44
    adapter_not_port_plus_technology  28
    unit_missing_face                 25
    port_defined_in_adapter            8
    placement_ambiguous                5
    root_file_unallowlisted            1
  not measured: 5 rules, each with its reason
```

The `not measured` list is the engine honouring I1 — five capabilities
declare no crate under `ports`, so the port half of `<port>-<technology>`
cannot be checked. Those are reported as unmeasured, never as clean.

## 5. Who should enforce the face matrix — buck2, not a grep

From the `buck2` session, measured: `visibility` is declared in **496 of 496**
BUCK files and constrains nothing. `visibility = ["PUBLIC"]` appears 1437
times; exactly 5 targets carry a scoped visibility.

That makes buck2 `visibility` the right long-term home for the face
dependency matrix — it fails at analysis naming the offending label, and a
rename cannot launder it — and makes it useless as a check today. The trap to
avoid: "does every BUCK declare visibility?" passes 496/496 while the matrix
is wide open. The honest form, "no library target outside its face's allowed
consumers is PUBLIC", would refuse roughly 1400 targets.

That is a migration, not a gate. It must also be sequenced per capability
rather than swept: oyatie's `occupancy` claims whole files across open PRs
with no ordering, so a sweep touching many BUCK files deadlocks against every
open lane holding one.

Also measured there: buck2 gates **none** of the merge path today. Every
merge-blocking job is cargo; the single buck2 workflow is a non-blocking
weekly smoke.

## 6. Images and signing

From the `oci` session, measured: oyatie's only Dockerfile is
`build/images/Dockerfile.distroless`. The root allowlist needs no entry for
it and should not be widened.

Digest pinning is real and test-backed —
`build/iac/nativelink/tests/chart_test.rb:57` asserts `/@sha256:[0-9a-f]{64}\z/`
for every container in the pod, alongside `runAsNonRoot`,
`readOnlyRootFilesystem` and `capabilities.drop: [ALL]`. A second posture
coexists: the substrate mirror addresses by tag, not digest. Enforce digests
where an image is *consumed*, and leave the mirror's tag grammar alone.

Signing is **declared, not wired**. `cosign` appears as a Cedar policy input
(`context.cosign_signature_verified`), which is a policy expecting a verified
signature to be supplied by something — not evidence that anything supplies
it. 176 of 448 Cedar fragments in that repo are nullified by a bare `forbid`
and latent only because nothing loads them, so the policy's existence is not
a control.

The pipeline position of image build and publish is **unmeasured**. Neither
session could say whether the input is buck2 or cargo, what gates a publish,
or whether promotion is build-once-promote-by-digest. The hyperscaler answer
is presumably the latter; nobody has measured it and it is not written down
here on the strength of an expectation.

## 7. What needs Jason's ruling

1. **Adopting the spec in oyatie.** The corrected proposal is one PR away, but
   `.anvil/shape.json` in oyatie is oyatie's data (ADR-0006 §3). Adopting it
   as written starts reporting 1523 findings. Every rule should land
   `advisory-until-infra` first, because 11 of the 15 are currently
   `baseline-block-on-new` and oyatie has no baseline — meaning every new
   finding blocks from day one.
2. **The BNF standard.** 0 of 480 conformant on its central rule, scoping six
   directories that do not exist, enforced by a lane that does not exist.
   Retiring or rewriting it is a decision; leaving it Accepted means the
   repository's canonical naming authority describes another repository.
3. **Where the capability registry lives.** ADR-0562 named
   `governance/capability-registry.json`; oyatie's layout gate forbids a root
   `governance/`. This proposal uses `.anvil/capability-registry.json`.

## 8. Open, not closed

- `console.shape.json` uses the same `discover:manifest.json` marker that
  found nothing in oyatie. Whether console puts `manifest.json` at a unit root
  is unmeasured — it was not checked rather than checked and found fine.
- Anvil's own tree measures **559 findings, 3 of 97 units conformant** against
  its own spec, and `src/shape/core/measure.rs` violates the core-may-not-
  import-adapters rule that the shape program exists to enforce.
