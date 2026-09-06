# ABI signoff binding

The ABI gate accepts only exact supported declaration-line transitions. Its
repository-reviewed signoff file retains the shared `anvil/ratchet-signoff/v1`
envelope and signing vocabulary. ABI keys are `abi-change/v2:` followed by a
compact JSON tuple: `[repo, kind, before_path, after_path, symbol, before_line,
after_line]`. Removal has kind `REMOVAL` and null after fields; a paired change
has kind `SIGNATURE_CHANGE`. Paths come from the corresponding diff headers.
Authority accepts only a complete raw path with the expected `a/` or `b/`
prefix; whitespace, controls, quotes and escapes are unsupported. The detector
may still report a finding for that syntax, but its partial path token cannot
authorize a waiver. No path is trimmed, decoded or split into authority.

Declaration lines retain internal whitespace and omit only outer whitespace.
Hunk coordinates and surrounding context are not identity, so moving a line
does not invalidate a decision. A different path, repository, declaration or
kind does. Only unambiguous scanner observations with a closed parameter list
and a line ending in `{` or `;` supply authority; unsupported findings stay
unsigned. This is not resolved Rust ABI identity or a more complete scanner.

Legacy `symbol@file` keys authorize nothing. A matched decision remains an
observed break: it is retained in the report and disclosed as a warning, not
reported as ABI stability. Unsigned failures and unmeasured layouts retain
precedence. An entry is not automatically consumed or expired: an identical
transition can match again. Decisions do not compose transitively. A signing
record is not cryptographic authentication; repository review remains its
trust boundary.

## Historical narrowing

The two keys signed in `47df1e7e96025b75b28ce0c528f3d1b767a33136` are narrowed
without adding or rewriting its signing object:

- `body`: `String` to `Published`, with `judged: Judged` already present.
  Transition `52e435dd8274eaae629ed996986b909911ffe258`, parent
  `590cfc595c8836e91485f3f05cc363435b0e5450`; source blobs
  `4462059be414f682daa41f8f85b6223eeec14cb7` to
  `526f7f73951328c7cedc9e3be9ee359bc2b0e9ad`.
- `sweep_repo`: `Result<String, String>` to `Result<Swept, String>`.
  Transition `426df61f58d65586aca6940adf2502709d6b6e58`, parent
  `330a017cecbc6d5ddfcf5a3b64da2ee0c6c6f9e7`; source blobs
  `cdb912e8bdf7188703b13ac00a41aa57c1e91039` to
  `9b4d6b9ea68eb87b7878703b65e3d46308322f50`.

Both transitions precede the recorded signing. Its original expiry sentence
is retained as historical text, not the implemented contract. The cumulative
staging change from `body(action, content) -> String` to
`body(action, content, judged) -> Published` is **not** authorized by these
entries. It requires its own explicit owner decision, never a new signing
attributed to the historical signer by inference.
