# The canonical shape for oyatie, measured

Status: proposal with measurements. Three rulings in §7 are Jason's, not
Anvil's — ADR-0006 §3 puts a tenant's spec in the tenant's hands.

Every figure below carries the command that produced it, because the first
draft did not and an independent review found eight numbers wrong: a depth
histogram labelled as a pattern breakdown, a target count that was an
occurrence count, and a figure restated from another session as measured that
reproduces under no instrument at all. A number without a command is a claim.

Unless stated otherwise, `$OY` is a checkout of
`oyatie/oyatie@b4556b57fbbee53d7f09c519fa200d230bf85323`, anvil commands run
from anvil's root, and `leaves.txt` is the leaf directory name of every
non-root `Cargo.toml`. Measured 2026-09-10.

## 1. The grammar already exists, is Accepted, and matches nothing

`docs/standards/naming-convention-bnf-v4.md` in oyatie carries
`status: Accepted`, `enforced_by: governance-naming-convention`, and (via its
companion `crate-naming-convention.md`) `Severity = BLOCKER`. It is a full BNF
for crate names, contract files, policies, events, SLOs and ADR filenames.

It describes a repository that does not exist.

```
cd $OY && git ls-tree -r --name-only HEAD | awk -F/ 'NF>1{print $1}' | sort -u
# crates/ 0 · microservices/ 0 · contracts/ 0 · registry/ 0 · specs/ 0
# .omc/ 0 · governance/ 0
```

The lane declared to enforce it is not missing — it is cited and unwritten:

```
cd $OY && rg -l 'governance-naming-convention' .
  docs/standards/naming-convention-bnf-v4.md
  docs/standards/crate-naming-convention.md
  docs/standards/clean-architecture.md
  docs/standards/layer-enum-adr-0105.md
cd $OY && rg -c 'governance-naming-convention' .github/   # 0
cd $OY && rg -c -t rust 'governance-naming-convention' .  # 0
```

Four standards defer to an enforcer that has never been written. There is
nothing to restore, and writing it would refuse the whole repository on its
first run. The registry its slot-2 rule requires,
`[workspace.metadata.oyatie.microservices]`, is absent from `Cargo.toml`.

Conformance of the 480 crates that exist (N-007 requires the directory name to
equal the package name, so leaf names are the right subject):

| Rule | Conformant | Command |
| --- | --- | --- |
| N-002, `oyatie-` prefix | **0 / 480** | `rg -c '^oyatie-' leaves.txt` |
| BNF v4.1, `oya-` prefix | **0 / 480** | `rg -c '^oya-' leaves.txt` |
| N-005, ends in an ADR-0105 layer token | 266 / 480 | `rg -c -- '-(kernel\|domain\|usecase\|app\|adapter\|infrastructure\|cli\|rest\|grpc\|worker\|sdk\|api)$' leaves.txt` |
| N-003, `check-` crates | 1 | `rg -c '^check-' leaves.txt` |

The prefix figure is the load-bearing one and was measured twice by
instruments that agree: the table above, and Anvil's own engine reporting
`crate_name_prefix: 480` in §4. This is not drift. Nothing ever measured it.

## 2. What oyatie actually is

The unit is a **capability**, not a `src/` module. The real decomposition of
the 481 `Cargo.toml` files — the first draft gave a depth histogram and
labelled its buckets as patterns, merging three different shapes into one row:

| pattern | crates |
| --- | --- |
| `<capability>/<face>/<crate>/` | 411 |
| `app/<product>/<face>/<crate>/` | 33 |
| `build/{port-engine,dependency-declarations}/…` | 20 |
| `<capability>/{ports,adapters}/draft/<crate>/` | 8 |
| `app/foundry/{ports,adapters}/draft/<crate>/` | 8 |
| root workspace manifest | 1 |
| **total** | **481** |

- **21 capabilities**, each carrying `OWNERS` at its root — 21 of 21, no
  exceptions. Faces are drawn from `core/ ports/ adapters/ facade/`.
- **7 products** under `app/`, each also carrying `OWNERS` — 7 of 7.
- **`build/` is not a capability.** oyatie's own
  `pipeline/core/admission/src/layout.rs:131` declares
  `META_ROOTS = ["app", "build", "docs", "templates", "third-party"]`, and its
  20 crates are a named ADR-0538 exception in `Cargo.toml:28-31`. They are
  enrolled as meta directories in the proposed registry, not silently dropped.
- **496 BUCK files.** Not one per crate: 16 have no sibling `Cargo.toml`
  (`third-party/fixups/…`, `docs/decisions`, proto trees, toolchains), and the
  root manifest has no BUCK.
  `comm -23 <(BUCK dirs) <(Cargo dirs) | wc -l` → 16.
- **16 root files**, all admitted by the proposed allowlist once `bacon.toml`
  was added — the first draft refused it while claiming it refused nothing
  legitimate. `root_file_unallowlisted` is now 0.
- Satellites under capabilities: `observability` (16), `cedar` (14), `iac` (12).
- No capability carries a file at a *uniform* depth inside a face. 411 crates
  sit at `<cap>/<face>/<crate>/`, but the app and draft rings do not, so unit
  discovery cannot key off a marker deeper than the unit root.

Two constraints that bound anything built on this:

- `Cargo.toml:5` states that **gates must not parse the members array**
  (ADR-0538); the canonical resolver is `pipeline/core/workspace-members-kernel`.
- `layout.rs:115-129` `FORBIDDEN_NAMES` refuses a root named `plan`, `tasks`,
  `contracts`, `specs`, `libs`, `tools`, `infra`, `kernel`, `os`, `governance`,
  `console`, `tests`, `e2e`.

## 3. Anvil's proposal for oyatie had never produced a number

`tests/fixtures/shape/oyatie.shape.json` has shipped since ADR-0006 with 15
rules, 12 of them `baseline-block-on-new`. Measuring it against oyatie did not
return a large number or a small one. It refused:

```
Command failed: unit registry could not be used: unit kind "capability"
enumerates members from governance/capability-registry.json but no registry
document was supplied
```

`governance/capability-registry.json` does not exist in oyatie, and cannot —
`governance` is in `FORBIDDEN_NAMES` above. Its `unit_marker`, `manifest.json`,
is also absent from every capability and product root: three files in the repo
carry that exact name (a test fixture and two sovereignty packs), and a fourth,
`client-manifest.json`, only ends with it.

Both halves were unmoored, and the second fails quietly: a marker matching
nothing discovers no units and yields a clean zero that reads as conformance.

The spec also declared three placement destinations — `kernel/`, `os/` and
`governance/` — all in the same `FORBIDDEN_NAMES` list this document cites to
justify moving the registry. Fixing one instance and leaving three siblings in
the same file is the defect this repository keeps producing. They are now
`build/ docs/ templates/ third-party/` (oyatie's own `META_ROOTS`) and
`docs/decisions/`, which exists.

## 4. The corrected proposal, measured

Corrections, each grounded in §2 rather than chosen: marker `manifest.json` →
`OWNERS` (21/21 and 7/7); the `app` kind discovers on the same; the registry
moves to `.anvil/capability-registry.json`, beside the spec and out of the
forbidden root; `meta_directories` carries oyatie's `META_ROOTS`; three
forbidden placement destinations replaced; `bacon.toml` admitted.

```
anvil shape measure --repo-dir $OY --rev b4556b57fbbe… --repo oyatie/oyatie \
  --spec-override tests/fixtures/shape/oyatie.shape.json \
  --registry tests/fixtures/shape/oyatie.capability-registry.json

shape: oyatie/oyatie @ b4556b57fbbe (PROPOSED spec …/oyatie.shape.json)
  units: 32 (8 conformant)
  findings: 1523 (228 misplaced files, 420 denied edges)
    adapter_not_port_plus_technology 28
    crate_layer_suffix              170
    crate_name_prefix               480
    cross_unit_non_facade           341
    face_edge_denied                112
    placement_ambiguous               5
    port_defined_in_adapter           8
    satellite_alias_used            228
    suffix_face_disagree             82
    unit_missing_face                25
    unit_root_file_unallowlisted     44
  not measured: 5 units of one rule, each with its reason
```

**"Conformant" means no missing face and no aliased satellite**
(`src/shape/core/report.rs:118-122`). It does not mean zero findings — no unit
has zero findings. The 8 are `community, hr, payroll, build, docs, pipeline,
templates, third-party`.

The `not measured` list is the engine honouring I1: five capabilities (`bus`,
`gateway`, `intelligence`, `observability`, `policy`) declare no crate under
`ports`, so the port half of `<port>-<technology>` cannot be checked for the
adapters they do declare. Reported unmeasured, never as clean.

## 5. Who should enforce the face matrix — buck2, not a grep

`visibility` is declared in all 496 BUCK files and constrains almost nothing:
`visibility = ["PUBLIC"]` occurs about 1438 times, and roughly ten lines carry
a narrower scope.

Those two figures are deliberately approximate. Two sessions grepping the same
tree got 1438 and 1439, and 10 and 9. A BUCK file is Starlark: visibility can
be written across lines or built from a variable, and no line count is a target
count. **475 `rust_library` targets exist** (`rg -c '^rust_library\(' -g BUCK`),
and the figure that actually matters — how many library targets would need
narrowing — requires `buck2 uquery`, which has not been run here. The order of
magnitude is "most of 475". The precision is not available from grep and this
document will not invent it.

That makes buck2 `visibility` the right long-term home for the face dependency
matrix — it fails at analysis naming the offending label, and a rename cannot
launder it — and useless as a check today. The trap to avoid: "does every BUCK
declare visibility?" passes 496/496 while the matrix is wide open.

It is a migration, not a gate, and must be sequenced per capability rather than
swept: oyatie's `occupancy` claims whole files across open pull requests with
no ordering, so a sweep touching many BUCK files deadlocks against every open
lane holding one.

buck2 gates **none** of the merge path today. Every merge-blocking job is
cargo; the single buck2 workflow is a non-blocking weekly smoke.

## 6. Images and signing

oyatie's only Dockerfile is `build/images/Dockerfile.distroless`. The root
allowlist needs no entry for it and was not widened for one.

Digest pinning is real and test-backed:
`build/iac/nativelink/tests/chart_test.rb:57` asserts `/@sha256:[0-9a-f]{64}\z/`
for every container in the pod, alongside `runAsNonRoot`,
`readOnlyRootFilesystem` and `capabilities.drop: [ALL]`. A second posture
coexists: the substrate mirror addresses by tag, not digest. The judgement —
not a measurement — is to enforce digests where an image is *consumed* and
leave the mirror's tag grammar alone.

Signing is **declared, not wired**: `cosign` appears as a Cedar policy input
(`context.cosign_signature_verified`), which is a policy expecting a signature
to be supplied by something, not evidence that anything supplies it.

An earlier draft also carried "176 of 448 Cedar fragments are nullified by a
bare `forbid`", restated from the `oci` session as though measured here. It
reproduces under no instrument: the repository has 36 `.cedar` files and 215
`permit`/`forbid` statements, and neither 448 nor 176 appears at any
granularity. **That claim is withdrawn.** What survives is the part with a
citation behind it: a convention named in a comment is not a control, and a
Cedar policy is not evidence that anything loads it.

The pipeline position of image build and publish is **unmeasured**. Neither the
`buck2` nor the `oci` session could say whether the input is buck2 or cargo,
what gates a publish, or whether promotion is build-once-promote-by-digest. The
hyperscaler answer is presumably the last of those; nobody measured it and it
is not written down here on the strength of an expectation.

## 7. What needs Jason's ruling

1. **Adopting the spec in oyatie.** The corrected proposal is one PR away, but
   `.anvil/shape.json` in oyatie is oyatie's data (ADR-0006 §3). Adopting it as
   written starts reporting 1523 findings, and **12 of its 15 rules are
   `baseline-block-on-new` against no baseline**, so they block from day one.
   Every rule should land `advisory-until-infra` first, with the number
   published before anything is enforced.
2. **The BNF standard.** 0 of 480 conformant on its central rule, scoping six
   directories that do not exist, deferred to by four documents and enforced by
   nothing. Retiring or rewriting it is a decision; leaving it Accepted means
   the repository's canonical naming authority describes another repository.
3. **Where the capability registry lives.** ADR-0562 named
   `governance/capability-registry.json`; oyatie's own layout gate forbids a
   root `governance/`. This proposal uses `.anvil/capability-registry.json`.

## 8. Open, and named rather than quiet

- **`console.shape.json` discovers zero units, measured.** It uses
  `discover:manifest.json` at `<name>/`; console's 51 files of that name sit at
  depths 4 to 7 (`backend/crates/<x>/rest/openapi/manifest.json`), never at a
  unit root. Its real shape is `backend/crates/<name>/<face>/`, which the spec
  does not describe. Measured at `console@83b92700`. Correcting it needs
  console's unit model decided, so this document does not invent one;
  `tests/a_shipped_proposal_can_be_measured_test.rs` names console in
  `MARKER_NOT_DEMONSTRATED` so the next unverified spec cannot ship quietly.
- Anvil's own tree measures **559 findings, 3 of 97 units conformant** against
  its own spec, and `src/shape/core/measure.rs` violates the
  core-may-not-import-adapters rule the shape program exists to enforce.
- One `harness/` directory does exist in oyatie
  (`build/dependency-declarations/adapters/generation-reindeer/tests/harness/`);
  `meta_directories` is about root-level meta directories, and the earlier
  claim that none existed anywhere was wrong.
- `oyatie.shape.json` is reformatted by this change. The semantic diff is six
  values — marker, app members, registry path, two placement destinations, one
  allowlist entry — and the rest is `json.dump` indentation.
