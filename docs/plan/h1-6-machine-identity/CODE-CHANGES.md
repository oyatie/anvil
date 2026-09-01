# CODE-CHANGES — specified, not applied

The change the App unblocks, at `file:line` on `origin/dev` = `65f71fd`. Nothing here is
applied; `SETUP.md` must complete first, because §2 needs the `<slug>[bot]` numeric id and
§1 needs the App id, installation id and private key.

Order matters and is not the order of the sections: **§3 must land in the same change as
§2, or before it.** §2 makes `answerable` start returning `true` for Jason's comments;
§3 is the second door that today lets Anvil's own comments through unfiltered. Landing §2
alone converts "human feedback is dropped" into "Anvil answers itself, clones, runs a
model turn and pushes". That is the loop the guard exists to prevent.

---

## §1 — the daemon authenticates as the App

### 1a. New module: `src/github/app_auth.rs`

Mint an installation access token, cache it, refresh it before expiry.

- Read `GITHUB_APP_ID`, `GITHUB_APP_INSTALLATION_ID`, and `GITHUB_APP_PRIVATE_KEY` (or
  `GITHUB_APP_PRIVATE_KEY_PATH`). Configuration goes wherever `Config::from_env` already
  lives; three optional fields, and **all-or-nothing**: two of three set is a
  misconfiguration and must refuse at boot, not silently fall back to the ambient
  credential. Silent fallback is how a migration reports success while still running as
  `jason931225`.
- Build a ≤10-minute RS256 JWT (`iss` = app id) and
  `POST /app/installations/{id}/access_tokens`.
- Cache the token with its `expires_at`; refresh when under 5 minutes remain. Tokens live
  1 hour, so this is one network call an hour.
- The private key never reaches a subprocess. It already cannot:
  `GITHUB_APP_PRIVATE_KEY` is on `NEVER_HANDED_OVER` at `src/exec/gh.rs:84`,
  `src/exec/net.rs:79` and `src/exec/inherited.rs:95`, each asserted by a test beside it.

### 1b. Injection point: `src/exec/gh.rs:94` / `:101`

`command()` and `apply()` are the one place a `gh` invocation is built — the seam exists
for exactly this ("installation token, not ambient", `src/exec/gh.rs:3-6`). Add the token
after `apply()` clears and repopulates the environment:

```rust
// src/exec/gh.rs, in apply(), after the GH_INHERITED loop
if let Some(token) = app_auth::current_installation_token() {
    cmd.env("GH_TOKEN", token);   // overrides whatever GH_INHERITED copied
}
```

`GH_TOKEN` is already on `GH_INHERITED` (`src/exec/gh.rs:46`), so no list changes and the
`gh_spawns_go_through_one_seam_test` invariants are untouched.

`command()` must stay **synchronous** — 35 call sites take a `Command` and several are not
in async context at construction. So the mint runs in a background refresher, not inline:
start it in `src/cli/server.rs` alongside the existing boot checks (the `gh --version`
probe is at `:354`), and have it write into a process-wide cell that
`current_installation_token()` reads. Fail-closed: if the App is configured and the
refresher has no valid token, `apply()` must refuse rather than fall through to the
ambient credential — a `gh` call that silently runs as Jason is the defect returning.

### 1c. `git` is a separate credential path and is easy to miss

`git clone` / `git push` do **not** go through `exec::gh`. They use plain `git` over
`https://github.com/{repo}.git` and resolve the ambient credential helper
(`git config --get credential.helper` → `osxkeychain` on the current host):

| site | command |
|---|---|
| `src/git_manager/mod.rs:103-105` | `git clone https://github.com/{repo}.git` |
| `src/fixer/mod.rs:238-241` | `git push origin HEAD:<branch>` |
| `src/pr_self_healer.rs:127` | `git push origin <branch>` |
| `src/queue_healer.rs:443` | `git push origin <target>` |
| `src/lockfile_reconciler.rs:189` | `git push origin <target>` |

Under the App these must push as the App or they will keep pushing as Jason — and the
attribution half of #171 would survive the fix. Two options:

- **Preferred:** a `crate::exec::git()` constructor mirroring `exec::gh()`, injecting
  `-c http.https://github.com/.extraheader=Authorization: Basic <base64("x-access-token:"+token)>`
  and a bounded environment. It gives `git` the same single seam `gh` already has, which
  is the same class of fix `src/exec/gh.rs:11-16` describes for the 34 `gh` sites.
- **Cheaper:** rewrite the remote to
  `https://x-access-token:<token>@github.com/{repo}.git` at clone time. Rejected — it
  writes a credential into `.git/config` on disk, where the token outlives its hour and
  every subsequent `git` in that clone leaks it into `git remote -v`.

Whichever is chosen, `contents: write` in `app-manifest.json` is what makes the push
succeed, and a fix touching `.github/workflows/**` will be refused because `workflows` is
deliberately not granted (`README.md` Appendix A).

---

## §2 — `identity.rs` decides on a principal

### 2a. Replace `authenticated_login`, `src/github/identity.rs:52`

```rust
pub async fn authenticated_login() -> Option<String>   // gh api user --jq .login
```

`GET /user` does not answer under an installation token — it is a server-to-server token
with no user behind it — so this function does not merely become wrong, it becomes
unanswerable, and it fails **open into the cached `None`** which `answerable_by` reads as
"refuse everything". The fixer would go from answering nothing to answering nothing, and
nothing in CI would notice.

Replace it with a value that comes from configuration, not from a probe:

```rust
pub struct Principal { pub id: u64, pub kind: ActorKind }   // ActorKind::Bot for the App
pub fn authenticated_principal() -> Option<&'static Principal>
```

sourced from `GITHUB_APP_BOT_USER_ID` — the numeric id of `<slug>[bot]`, recorded in
`SETUP.md` step 4. Configuration rather than a probe because it must be knowable at boot
and must not change under the process.

### 2b. Rewrite the predicate, `src/github/identity.rs:85-93`

```rust
pub fn answerable_by(me: Option<&Principal>, author: Option<&Actor>) -> bool {
    match (me, author) {
        (Some(me), Some(author)) => match author.id {
            Some(author_id) =>
                author_id != me.id                                    // not mine
                && author.kind.as_deref() == Some(HUMAN_ACTOR_TYPE),   // and a person
            None => false,                                             // unknown refuses
        },
        _ => false,
    }
}
```

Three things this buys, and one it deliberately keeps:

- `author_id != me.id` is **exact**. A rename does not move it; a login lookalike does not
  match it. `Actor.id` already exists and is already deserialized on both payload shapes
  (`src/webhook/payload.rs:90`, `src/github/mod.rs:56`) — the field was landed in `86d0188`
  for exactly this moment.
- Jason (`id 56489493`, `type User` — `gh api user --jq '{login,id,type}'`) now differs
  from Anvil (`<slug>[bot]`, `type Bot`) in **both** terms. Anvil's own comments are
  refused twice over, by id and by type. Two independent reasons is the design: a
  regression in either still fails closed.
- `author.id == None` refuses. That is new — today the id is not consulted at all — and it
  is the same fail-closed rule the type already follows (`src/github/identity.rs:71-82`).
- The `kind == "User"` term is **kept**, not replaced. It answers a different question
  (`dependabot[bot]` is not a person; `abbott` is), and deleting it because the id now
  handles self would let every other bot's comments into the fixer.

The login field stays on `Actor` for logging and for the fixer's prompt. It is no longer
consulted by any decision.

---

## §3 — the second door into the fixer

`src/webhook/webhook_handlers.rs:256-271` asks the guard.
`src/webhook/pipelines/fix.rs:18-30` **does not ask anything**:

```rust
// src/webhook/pipelines/fix.rs:18
let feedback_items: Vec<ReviewFeedbackItem> = comments
    .into_iter()
    .map(|c| ReviewFeedbackItem {
        ...
        author: c.user.map(|u| u.login)            // :27
                 .unwrap_or_else(|| "reviewer".to_string()),   // :28
    })
    .collect();
```

`fetch_review_comments` (`src/github/mod.rs:354`) returns every review comment on the pull
request, Anvil's own included. Reachable from `src/webhook/pipelines/review.rs:412` (the
REQUEST_CHANGES arm — automatic), `src/webhook/manual_handlers.rs:143`, and
`src/cli/handlers.rs:52`.

Required change:

```rust
let mut feedback_items = Vec::new();
for c in comments {
    let actor = c.user.as_ref().map(GitHubUser::actor);
    if !crate::github::identity::answerable(actor.as_ref()).await { continue; }
    let Some(actor) = actor else { continue };
    feedback_items.push(ReviewFeedbackItem { ..., author: actor.login });
}
```

Note `:28` separately: `unwrap_or_else(|| "reviewer".to_string())` **manufactures an
identity** when the payload carries no user, and that fabricated string flows into the
fixer's prompt as the author of the feedback. `webhook_handlers.rs:253-255` already
refuses this case on purpose, with the reasoning written down. The two doors should agree.

---

## §4 — closing the class, not the instance

### 4a. Census — every string-identity predicate now in the tree

The instance #171 names is already dead:

```console
$ git grep -n 'contains("bot")\|contains("antigravity")' origin/dev -- src/ tests/
(no output)
$ git log --oneline origin/dev -1 -- src/github/identity.rs
86d0188 feat(identity): decide the loop-guard on the actor type GitHub sends
```

What remains, in order of authority:

| # | site | what the string stands in for | disposition |
|---|---|---|---|
| 1 | `src/github/identity.rs:88` `me != author.login` | "is this comment mine" | replaced by the id comparison, §2b |
| 2 | `src/github/mod.rs:317` `c.body.contains(marker)` in `upsert_pr_comment` | "did **I** write this comment" — a body marker standing in for authorship | **fix:** `c.user.id == me.id && body.contains(marker)`. The marker says *which* comment; the principal says *whose*. Today any account can post a comment containing `<!-- ANVIL_SCORECARD_RECEIPT -->` (`src/webhook/pipelines/review.rs:178`, `src/pre_merge_guard/matrix.rs:26`, `src/publish/mod.rs:54`) and Anvil will edit it in place |
| 3 | `src/webhook/pipelines/fix.rs:28` `unwrap_or_else(\|\| "reviewer")` | an absent identity | **fix:** refuse, §3 |
| 4 | `src/unresolved_review_guard/mod.rs:141-144` `c["author"]["login"] … unwrap_or("")` | an absent identity | advisory (reporting only), but the same shape: an empty-string login is not a login. Carry an `Option`, render "(author not returned)" |
| 5 | `src/merge_enlister/mod.rs:493` (comment) GitHub's `"own pull request"` / `"Can not approve"` | an API outcome | not identity, same proxy class; recorded so the census is complete. Current handling (both fatal) is correct |

Sites 2 and 3 are the ones that would otherwise be "fixed the instance, left the class".

### 4b. The ratchet (`ws-11` H1-6c)

A meta-test over the authority modules — `src/github/`, `src/webhook/`,
`src/merge_enlister/`, `src/fixer/`, `src/unresolved_review_guard/` — asserting no
substring predicate is applied to a login or an author field. Pattern set at minimum:
`login.contains(`, `contains("bot"`, `contains("[bot]"`, `ends_with("[bot]")`,
`author.contains(`, `starts_with("dependabot"`.

Use `source_scan::code_only` (already used by the sibling tests) so a `//` mention of the
pattern in a doc comment — this file's own quotations included — does not trip it.

**Prove it before trusting it:** seed `&& !author.login.contains("bot")` into
`answerable_by`, run the meta-test, confirm it goes red and names the file and line, then
revert. A meta-test that has never failed has not been shown to be able to.

### 4c. A door census, not a door

`tests/anvil_does_not_answer_its_own_comments_test.rs:22-35` pins **one** door — it
locates `pull_request_review_comment` in `webhook_handlers.rs` and asserts the text up to
the `tokio::spawn(` contains `answerable(`. §3 exists because a second path was written
beside it and no test could see it.

Replace `door()` with a census: enumerate every call site of `fixer::resolve_and_fix(`
under `src/`, and assert each one is preceded, in its own function, by an `answerable`
decision. A new path into the fixer then fails the test by existing. This is the
"make the next one unwritable" form; the current test is the "fix the instance" form.

---

## §5 — seed tests (`ws-11` H1-6b exit criterion)

Four fixtures. Numbers below are the real ones — Jason's from
`gh api user --jq '{login,id,type}'`, the bot's from `SETUP.md` step 4.

| # | fixture | expected after the change |
|---|---|---|
| A | `pull_request_review_comment` payload, `user = {"login":"jason931225","id":56489493,"type":"User"}` | **reaches the fixer** |
| B | same payload, `user = {"login":"<slug>[bot]","id":<bot id>,"type":"Bot"}` | **does not** |
| C | same payload, `user = {"login":"jason931225","type":"User"}` (no `id`) | **does not** — unknown refuses |
| D | `execute_pr_fix` over a mixed comment list (one Jason, one self) | exactly one `ReviewFeedbackItem`, Jason's |

Assert at the **door**, not only on `answerable_by`. A unit test on the pure function
passes whether or not the door calls it — which is the gap §4c exists to close.

**Prove the tests before trusting them** (this repository's rule, and the reason to run
them in this order):

1. Write A-D. Run them against **unfixed** code.
2. **A must fail** — today `me == "jason931225" == author.login`, so it is dropped. If A
   passes red, it is not exercising the real door.
3. **D must fail** — today `execute_pr_fix` applies no filter, so the self-comment gets
   through. If D passes red, the fixture's comment list is not reaching the mapped path.
4. B and C are expected to pass before and after (B by login equality today, by id and
   type after; C by the type term). They are regression pins, not defect detectors, and
   should be labelled as such so nobody reads their green as evidence.
5. Only then apply §2 and §3, and confirm A and D go green.

---

## §6 — the enlister stops approving

`ws-06`: *"at R0/R1 the **human's** approval is the human's — the enlister never approves
under a shared identity again"*. Once the App exists, the enlister approving under the
App identity is the same defect with a different principal, and `RULESET.md` explains why
the ruleset alone does not stop it.

`src/merge_enlister/mod.rs:361` `ensure_approving_review` currently:

- reads `reviewDecision` (`:374`, fail-closed on an unreadable read — keep all of that),
- bails on `CHANGES_REQUESTED` (`:422`, `:427` — keep),
- and when nothing has approved, **submits a formal `APPROVE`** (`:488` verdict,
  `:499` `submit_pr_review`).

Change: delete the submission arm (`:481-505`) and rename to `require_human_approval`.
Absent approval becomes a refusal to enlist, not an approval to manufacture. The
`approval_summary` / `report` plumbing that exists only to feed that arm goes with it.

**Ratchet:** a test asserting no non-test path under `src/` constructs a review with
`verdict: "APPROVE"`. Scope it with `source_scan::code_only` minus `#[cfg(test)]` —
`src/github/reviews.rs:736` is a test fixture and must stay legal.

---

## §7 — `promotion-open-next` and its sibling

### 7a. `.github/workflows/promotion-open-next.yml`

Line 57 today:

```yaml
github-token: ${{ secrets.PROMOTION_PAT || secrets.GITHUB_TOKEN }}
```

Add a token-minting step before the script step and consume its output:

```yaml
      - id: app-token
        uses: actions/create-github-app-token@<pin-by-sha>   # every action here is SHA-pinned
        with:
          app-id: ${{ vars.ANVIL_APP_ID }}
          private-key: ${{ secrets.ANVIL_APP_PRIVATE_KEY }}
      - name: Open the next rung
        uses: actions/github-script@60a0d83039c74a4aee543508d2ffcb1c3799cdea # v7.0.1
        with:
          github-token: ${{ steps.app-token.outputs.token }}
```

Then delete the `HAS_PAT` env (`:53`) and the `patWarning` text and its two uses
(`:88-102`). That warning exists because a `GITHUB_TOKEN`-opened pull request triggers no
workflow runs; an App-opened one does, so leaving the warning in would publish a false
statement on every promotion pull request — the class this repository calls a gate
claiming more than it checks.

**Do not add `contents: write`.** `tests/promotion_ladder_test.rs:163-173` asserts its
absence, and the reasoning there is right: opening a pull request needs only
`pull-requests: write`.

**The allow-list is unaffected.** `tests/promotion_ladder_test.rs:181-196` allow-lists
exactly `github.rest.pulls.list`, `github.rest.pulls.create`,
`github.rest.repos.compareCommitsWithBasehead` inside the inline script. A new *step*
adds no `github.` call, and `inline_script` (`:90`) asserts there is exactly one
`script: |` block — so do not add a second inline script.

### 7b. Same defect, not yet observed: `.github/workflows/toolchain-weekly.yml`

`:60` sets `GH_TOKEN: ${{ github.token }}` and `:76` runs `gh pr create`. That is the same
403 waiting to happen, on the same org setting:

```console
$ gh api repos/oyatie/anvil/actions/permissions/workflow
{"default_workflow_permissions":"read","can_approve_pull_request_reviews":false}
```

It has never fired — no `chore/toolchain-*` branch or pull request has ever existed
(`git ls-remote --heads origin 'chore/toolchain-*'` → empty) — so there is no failing run
to point at, only the same shape. Apply the same App-token step. Fixing 7a alone leaves
the sibling, which is the instance-not-class pattern this plan is about.

---

## §8 — dependency ratchet: a real, measured obstacle

RS256 signing needs a crate this tree does not have:

```console
$ grep -n '^name = "\(rsa\|ring\|jsonwebtoken\|openssl\|aws-lc-rs\)"' Cargo.lock
(no output)

$ python3 -c "import tomllib;print(len(tomllib.load(open('Cargo.toml','rb'))['dependencies']))"
22
$ python3 -c "print(open('Cargo.lock').read().count('\nname = '))"
162
```

`tests/dependency_admission_test.rs` sets `DIRECT_DEPENDENCY_CEILING = 22` and
`LOCKFILE_CEILING = 170`. **Direct dependencies are exactly at the ceiling.** Adding
`jsonwebtoken` fails `direct_dependency_count_only_falls` on the first `cargo nextest run`.

That test is a ratchet, not a ban, and says so in its own doc comment: *"raising it
requires editing this file, which makes the decision visible in review rather than
invisible in a lockfile diff."* So there are two honest paths, and one dishonest one:

- **(a) Raise the ceilings, deliberately.** Edit both constants in the same commit as the
  dependency, with the justification in the commit body. Measure the transitive cost first
  — `cargo add --dry-run jsonwebtoken` and count the lockfile delta — because
  `LOCKFILE_CEILING` has only **8** crates of headroom (162 of 170) and `jsonwebtoken`
  plausibly exceeds that. Exact figure: **unknown until measured**; do not guess it.
- **(b) No new crate.** Sign through the existing `exec` seam with
  `openssl dgst -sha256 -sign <key-path>` — the same construction `SETUP.md` step 3 uses
  and which was checked to produce a JWT GitHub accepts as well-formed (it returns 401 for
  an unregistered key rather than 400). Pass the key by **path**, never on argv and never
  in the child's environment. Cost: `openssl` becomes a runtime dependency of the daemon,
  and one subprocess per hour.
- **(c) Do not** bump only `DIRECT_DEPENDENCY_CEILING` and leave `LOCKFILE_CEILING`
  passing by luck, and do not vendor a hand-rolled RS256. Rolling signature code to keep a
  count down is the count winning over the thing it was protecting.

Recommendation: **(a)**, measured first. `jsonwebtoken` is the reviewed, standard choice,
and the ratchet exists to make this a visible decision — which is what taking it is.

---

## Summary of touched sites

| file:line | change |
|---|---|
| `src/github/app_auth.rs` | new — JWT, installation token, hourly refresh |
| `src/exec/gh.rs:101` | inject the minted `GH_TOKEN` in `apply()` |
| `src/exec/git.rs` | new — the same seam for `git clone` / `git push` |
| `src/git_manager/mod.rs:104`, `src/fixer/mod.rs:238`, `src/pr_self_healer.rs:127`, `src/queue_healer.rs:443`, `src/lockfile_reconciler.rs:189` | route through `exec::git()` |
| `src/cli/server.rs` (boot, near `:354`) | start the token refresher; refuse boot on partial App config |
| `src/github/identity.rs:52` | `authenticated_login` → `authenticated_principal` |
| `src/github/identity.rs:85-93` | decide on `Actor.id`; `id: None` refuses |
| `src/webhook/pipelines/fix.rs:18-30` | filter through `identity::answerable`; stop manufacturing `"reviewer"` |
| `src/github/mod.rs:317` | marker **and** principal, not marker alone |
| `src/unresolved_review_guard/mod.rs:141-144` | `Option` instead of an empty-string login |
| `src/merge_enlister/mod.rs:481-505` | delete the self-`APPROVE`; refuse instead |
| `tests/anvil_does_not_answer_its_own_comments_test.rs:22-35` | door → door census |
| new meta-test | zero substring identity predicates in authority modules (seed-proved) |
| new meta-test | no `verdict: "APPROVE"` constructed outside tests |
| `.github/workflows/promotion-open-next.yml:53,57,88-102` | App token; delete the PAT warning |
| `.github/workflows/toolchain-weekly.yml:60,76` | App token |
| `tests/dependency_admission_test.rs:16,19` | ceilings, if path (a) in §8 |
