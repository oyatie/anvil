# H1-6 / H1-9 — machine identity: what is broken, measured

Preparation for two milestones that sit at the top of the roadmap and block every rung
below them:

- **H1-6** (`docs/plan/ws-11-actor-identity.md`) — Anvil runs as a GitHub App; the
  loop-guard decides on a principal, not a string.
- **H1-9** (`docs/plan/ws-06-autonomy-ladder.md`) — rung R0 made mechanically true:
  `dev` requires a human approval that Anvil cannot supply.

Nothing in this directory is applied. It is the material a human needs to apply it, in
the one order that is safe. See `SETUP.md` (human steps), `app-manifest.json` (the App),
`CODE-CHANGES.md` (the daemon), `RULESET.md` (the gate).

**Every claim below carries the command that produced it.** Commands were run on
2026-09-01 against `oyatie/anvil` at `origin/dev` = `65f71fd`, authenticated as the
account the finding is about.

---

## 1. Anvil and its reviewer are one principal

```console
$ gh api user --jq '{login, id, type}'
{"id":56489493,"login":"jason931225","type":"User"}

$ gh auth status
github.com
  ✓ Logged in to github.com account jason931225 (keyring)
  - Token scopes: 'admin:org', 'admin:repo_hook', 'gist', 'repo', 'workflow'
```

That token is the daemon's forge credential — `src/exec/gh.rs:46-47` inherits `GH_TOKEN`
/ `GITHUB_TOKEN`, and `GH_CONFIG_DIR` / `XDG_CONFIG_HOME` / `HOME` so `gh`'s own keyring
credential resolves. Whichever route it arrives by, it is this account.

Two properties follow, and both are the defect rather than a consequence of it:

- The principal that authors Anvil's reviews, comments, approvals and branch pushes is
  the principal that reviews Anvil.
- The credential is a long-lived user token with `repo` (every repository this human can
  reach, not the ones Anvil watches) and `admin:org`. An installation token would be
  scoped to selected repositories and expire in an hour.

**The App does not exist yet.**

```console
$ gh api orgs/oyatie/installations --jq '.total_count, [.installations[] | {app_slug, app_id}]'
1
[{"app_id":1210556,"app_slug":"cursor"}]
```

---

## 2. The loop-guard: correct code, unanswerable question

### 2.1 The predicate as it stands today

`src/github/identity.rs:85-93`:

```rust
pub fn answerable_by(me: Option<&str>, author: Option<&Actor>) -> bool {
    match (me, author) {
        (Some(me), Some(author)) => {
            me != author.login && author.kind.as_deref() == Some(HUMAN_ACTOR_TYPE)
        }
        _ => false,
    }
}
```

`me` comes from `authenticated_login()` (`src/github/identity.rs:52`), which is
`gh api user --jq .login`, cached in a `OnceLock` for the life of the process.
`HUMAN_ACTOR_TYPE` is `"User"` (`src/github/identity.rs:45`).

**Correction to issue #171 and to `ws-11`, verified.** Both quote the predicate as
`me != author && !author.contains("bot") && !author.contains("antigravity")`. That form
is gone:

```console
$ git grep -n 'contains("bot")\|contains("antigravity")' origin/dev -- src/ tests/
(no output)

$ git log --oneline origin/dev -3 -- src/github/identity.rs
86d0188 feat(identity): decide the loop-guard on the actor type GitHub sends
2457d78 feat(exec): every gh invocation goes through one seam, with a bounded environment
0d26361 fix(webhook): Anvil stops answering its own review comments
```

Step 1 and step 2 of #171's own fix ordering — one `gh` seam, typed identity — have
landed. Step 3, the App, has not, and step 3 is the one that closes the issue. #171 says
so itself: *"Step 2 does not fix this issue on its own: with one shared account, an exact
id comparison still cannot tell Anvil from its reviewer."*

### 2.2 Why the class survives the fix to the instance

The remaining term is `me != author.login`. With `me == "jason931225"`:

| comment author | `me != author.login` | `kind == "User"` | reaches the fixer |
|---|---|---|---|
| Anvil (posting as `jason931225`) | false | true | **no** — correct |
| Jason reviewing (as `jason931225`) | false | true | **no** — wrong |

The two rows are the same row. The module says this in its own doc comment
(`src/github/identity.rs:15-22`), and it is the analysis #171 was filed on:

> The identity check is correct as written. The defect is upstream of it: Anvil and its
> reviewer are the same GitHub principal, so "is this comment mine?" and "is this comment
> my reviewer's?" are the same question, and the loop-guard needs them to be different
> questions. **No predicate over a single shared login can separate them.**

This is not a claim about `!=` being the wrong operator. It is a claim about the domain.
`answerable_by` is a function of `(me, author)`; both range over logins; `me` is a
constant. Any total function of one login can partition authors into "equal to
`jason931225`" and "not". Anvil's comments and Jason's comments land in the same cell of
that partition, so **no** function of that signature separates them — including the exact
numeric-id comparison the `Actor.id` field was added for. The separation has to come from
the domain gaining a second value, which is what an App installation is. Substituting a
better proxy (id for login, login for substring) improves precision within the cell and
does not split it.

The `contains("bot")` deletion was worth doing and is not this. It fixed the *type*
question (`abbott` is a person, `dependabot[bot]` is not). The *self* question is still
unanswerable.

### 2.3 A second path into the fixer asks nothing at all

`src/webhook/webhook_handlers.rs:256-271` is the only site that consults the guard, and
`tests/anvil_does_not_answer_its_own_comments_test.rs:22-35` pins exactly that one door.

`src/webhook/pipelines/fix.rs:18-30` reaches `fixer::resolve_and_fix` by another route
and applies **no identity filter at all**:

```rust
let feedback_items: Vec<ReviewFeedbackItem> = comments
    .into_iter()
    .map(|c| ReviewFeedbackItem {
        ...
        author: c.user.map(|u| u.login).unwrap_or_else(|| "reviewer".to_string()),
    })
    .collect();
```

`fetch_review_comments` (`src/github/mod.rs:354`) returns every review comment on the
pull request, Anvil's own included. Callers:

```console
$ grep -rn 'execute_pr_fix' src/ --include='*.rs'
src/webhook/pipelines/review.rs:412       # the REQUEST_CHANGES arm — automatic
src/webhook/manual_handlers.rs:143        # manual HTTP
src/cli/handlers.rs:52                    # CLI
```

So the guard is one door on a two-door room, and the meta-test measures the door that has
a guard. Today the missing filter is masked: with one principal, the review pipeline's own
comments are also "not answerable" nowhere, because nothing asks. Once the App exists and
`answerable` starts returning `true` for Jason, this path starts feeding Anvil's own
comments to the fixer. **Closing the identity class without closing this path converts a
dropped-feedback bug into a push loop.** Ordering is in `CODE-CHANGES.md` §3.

---

## 3. The `dev` gate is a convention, not a rule

```console
$ gh api repos/oyatie/anvil --jq .default_branch
dev

$ gh api repos/oyatie/anvil/rulesets --jq '.[].id' | while read -r id; do
    gh api "repos/oyatie/anvil/rulesets/$id" --jq '{id, name,
      include: .conditions.ref_name.include,
      approvals: ([.rules[]|select(.type=="pull_request")|.parameters.required_approving_review_count]|first),
      codeowner: ([.rules[]|select(.type=="pull_request")|.parameters.require_code_owner_review]|first)}'
  done
{"approvals":0,"codeowner":false,"id":21064983,"include":["refs/heads/staging","refs/heads/canary","refs/heads/production"],"name":"Hyperscaler Environment Promotion Guard"}
{"approvals":0,"codeowner":false,"id":21064279,"include":["~DEFAULT_BRANCH"],"name":"Hyperscaler Merge Queue & Quality Gate"}
{"approvals":1,"codeowner":false,"id":21230025,"include":["refs/heads/main"],"name":"main-not-agent-merge"}
```

`~DEFAULT_BRANCH` resolves to `dev`. Every ruleset that applies to `dev` requires **zero**
approving reviews. `main` requires one — the mechanism exists and is simply not applied to
the branch the work actually lands on, which is the branch this repository merges to
(`docs/plan/anvil-roadmap.md` and the memory note both: PRs target `dev`).

Meanwhile `MergeEnlister::ensure_approving_review` (`src/merge_enlister/mod.rs:361`)
submits a formal `APPROVE` (`:488`, via `submit_pr_review` at `:499`) and then arms
auto-merge (`gh pr merge --auto --squash`, `:234`). Both under `jason931225`.

So "green is not merge authority; Jason reviews first" is enforced by Jason remembering to
look, not by GitHub. `ws-06` names this precisely: the bottom rung is a convention.

**Why the ruleset must not be raised first.** `required_approving_review_count: 1` today
would be satisfied by Anvil's own `APPROVE`, because that approval is authored by
`jason931225` and GitHub cannot tell it from Jason's. The result is a gate that renders as
"1 approval required, 1 received" and gates nothing — strictly worse than `0`, which at
least does not claim to be a gate. The ordering constraint is in `RULESET.md`, and it is
the whole reason this directory is a plan rather than a patch.

---

## 4. `promotion-open-next` has never opened a promotion pull request

```console
$ gh run list --workflow promotion-open-next --limit 100 --json conclusion \
    --jq 'group_by(.conclusion)|map({c:.[0].conclusion,n:length})'
[{"c":"failure","n":52},{"c":"success","n":13}]

$ gh run list --workflow promotion-open-next --limit 100 --json createdAt --jq '[.[].createdAt]|[min,max]'
["2026-08-24T23:58:29Z","2026-09-01T08:54:13Z"]

$ gh run list --workflow promotion-open-next --limit 100 --json conclusion,createdAt \
    --jq '[.[]|select(.conclusion=="success")][0]'
{"conclusion":"success","createdAt":"2026-08-25T14:47:45Z", ...}   # run 32861748088
```

52 consecutive failures; the newest success is the 53rd run back. The failure:

```console
$ gh run view 33489424260 --log-failed | grep -iE '403|not permitted|##\[error\]'
RequestError [HttpError]: GitHub Actions is not permitted to create or approve pull requests.
  status: 403,
    message: 'GitHub Actions is not permitted to create or approve pull requests.',
##[error]Unhandled error: HttpError: GitHub Actions is not permitted to create or approve pull requests.
```

**The 13 "successes" opened nothing.** They are the script's early-return arms:

```console
$ gh run view 32861748088 --log | grep '##\[notice\]'
##[notice]#83 is already open for dev -> staging; nothing to do.
```

Confirmed against the forge: exactly one pull request has ever targeted a rung, and a
human opened it.

```console
$ gh pr list --repo oyatie/anvil --state all --base staging --json number,author,state,title
[{"number":83,"author":{"login":"jason931225"},"state":"MERGED","title":"promote(staging): seed rung 1 of the ladder from dev"}]
$ gh pr list --repo oyatie/anvil --state all --base canary --json number      # []
$ gh pr list --repo oyatie/anvil --state all --base production --json number  # []
```

The cost, in commits:

```console
$ git rev-list --count origin/staging..origin/dev          # 219
$ git rev-list --count origin/canary..origin/staging       #  90
$ git rev-list --count origin/production..origin/canary    #   0
```

`staging` is 219 commits behind `dev`; `canary` 90 behind `staging`. (`zsh` does not
word-split an unquoted parameter — run these three as three commands, not a loop over a
variable holding one.)

### 4.1 Why it fails

Two independent causes, and the App answers both.

**Cause A — the org forbids the Actions token from creating pull requests.**

```console
$ gh api repos/oyatie/anvil/actions/permissions/workflow
{"default_workflow_permissions":"read","can_approve_pull_request_reviews":false}
$ gh api orgs/oyatie/actions/permissions/workflow
{"default_workflow_permissions":"read","can_approve_pull_request_reviews":false}
```

That flag is the literal source of the 403 string. `.github/workflows/promotion-open-next.yml:57`
selects `secrets.PROMOTION_PAT || secrets.GITHUB_TOKEN`, and there is no such secret:

```console
$ gh api repos/oyatie/anvil/actions/secrets --jq '[.secrets[].name]'   # []
$ gh api orgs/oyatie/actions/secrets  --jq '[.secrets[].name]'         # []
```

so every run falls through to `GITHUB_TOKEN` and every run 403s at `pulls.create`.

**Cause B — workflow runs from a pull request opened by `GITHUB_TOKEN` wait for approval.**
For the `opened`, `synchronize`, and `reopened` events, GitHub creates workflow runs in an
approval-required state. Until manual approval, `promotion-predecessor` cannot produce the
result required by ruleset 21064983 on `staging` / `canary` / `production`. Flipping only the
org toggle in Cause A therefore produces promotion pull requests whose checks wait for a
person before they execute — a second broken state, not a fix.

### 4.2 What the App fixes

A pull request created with an **installation access token** is authored by the App's
principal, which is neither `GITHUB_TOKEN` nor a human:

- Cause A disappears — `can_approve_pull_request_reviews` governs the Actions token, not
  an App installation token.
- Cause B disappears — App-authored `opened`, `synchronize`, and `reopened` events let
  `presubmit` and `promotion-predecessor` execute without manual approval.
- The pull request is attributed to `anvil[bot]`, so a promotion is visibly a machine's
  proposal awaiting a human, which is what `ws-13` H1-11 asks for.

The workflow edit is specified in `CODE-CHANGES.md` §7 and implemented by H1-11. It fails
closed until the repository variable and secret described in `SETUP.md` are present.

### 4.3 Is a PAT a valid interim?

Mechanically yes, and it is what the workflow was written to accept — set `PROMOTION_PAT`
and both causes clear at once. It is a bad trade here, and the reason is the subject of
this whole directory:

- **A PAT reproduces the class one principal over.** A PAT belongs to a *user*. Issued
  from Jason's account it is the shared identity again, wearing a second name — and it
  would make promotion pull requests appear as Jason's, so `require_code_owner_review`
  (see `RULESET.md`) would be satisfiable by the same credential that opened the pull
  request. `ws-11`'s non-goals say this outright: *"no shared bot account as the 'fix'
  (that reproduces the class one principal over)"*.
- **Scope.** The narrowest classic PAT that can open a pull request carries `repo`, which
  is every repository the issuing user can reach. A fine-grained PAT can be narrowed to
  `Pull requests: write` on `oyatie/anvil` — better, and still a *user's* credential with
  a manual expiry, versus an installation token that is repository-scoped and expires in
  an hour by construction.
- **It buys days, not weeks.** The App is one browser form plus `SETUP.md`. The interim
  costs a secret to create, rotate, and later revoke, and every day it exists is a day the
  identity work looks less urgent than it is.

**Recommendation:** do not set `PROMOTION_PAT`. The ladder has been stalled since
2026-08-25 and 219 commits of drift is a number, not an outage; the App is the same
afternoon's work and it is the thing the roadmap is actually blocked on. If the ladder
must move before the App exists, use a **fine-grained** PAT scoped to `oyatie/anvil` with
`Pull requests: write` + `Contents: read` only, with a ≤30-day expiry, and delete it in
the same change that installs the App.

---

## 5. The App cannot be created from here, and this was checked

`ws-11` and this note both assert that App creation is a human action. Verified rather
than assumed:

```console
$ gh api --method POST /apps
{"message":"Not Found","documentation_url":"https://docs.github.com/rest","status":"404"}

$ gh api --method POST /app-manifests/thiscodeisnotreal000/conversions
{"message":"Not Found",
 "documentation_url":"https://docs.github.com/rest/apps/apps#create-a-github-app-from-a-manifest",
 "status":"404"}
```

There is no create route. The only programmatic path is
`POST /app-manifests/{code}/conversions`, and `{code}` is a single-use value GitHub issues
**only** as a redirect parameter after a human submits the manifest form in a browser. The
route exists; the input to it does not, without a person. That is why `app-manifest.json`
ships as data and `SETUP.md` ships as a checklist.

(The conversion call itself is unauthenticated, which is what makes the redirect catcher
in `SETUP.md` a paste-and-run step rather than a credential dance.)

---

## 6. What the class looks like when it is closed

From `ws-11` and `docs/plan/anvil-roadmap.md:167`, restated as the acceptance test:

1. The daemon's authenticated principal is of type App, and is **disjoint** from every
   human reviewer principal on the repository. Stated as a type and a disjointness, never
   as `≠ jason931225` — a negative string predicate inside the milestone that exists to
   delete negative string predicates is the defect surviving in its own cure.
2. A seeded Jason-comment fixture reaches the fixer. A seeded self-comment fixture does
   not. Both in CI, both proved red against unfixed code first.
3. No substring identity predicate exists in an authority path, enforced by a meta-test
   that was itself proved by seeding one.
4. `dev` requires ≥1 approving review, and that requirement is not satisfiable by the
   principal Anvil runs as.

Items 1-3 are `CODE-CHANGES.md`. Item 4 is `RULESET.md`, and it is ordered strictly after
item 1.

---

## Appendix A — permission derivation for `app-manifest.json`

Every permission below is justified by a call site in this tree. Nothing is granted
because it is usual. The routes were enumerated from the one `gh` seam:

```console
$ grep -rn 'crate::exec::gh()' src/ --include='*.rs'      # 35 sites
$ grep -rn 'format!("repos/' src/ --include='*.rs'        # REST routes
$ grep -rn 'Command::new("git")' src/ --include='*.rs'     # push/clone paths
```

### Granted

| permission | why | call sites |
|---|---|---|
| `metadata: read` | mandatory; implied by every other permission | — |
| `contents: write` | the fixer and the healers commit and push to branches | `src/fixer/mod.rs:238` `git push origin HEAD:<branch>`; `src/pr_self_healer.rs:127`; `src/queue_healer.rs:443`; `src/lockfile_reconciler.rs:189`; `.github/workflows/toolchain-weekly.yml:75` |
| ” (read half) | clone and commit reads | `src/git_manager/mod.rs:103` `git clone https://github.com/{repo}.git`; `src/github/mod.rs:448` `GET repos/{}/commits/{}`; `promotion-open-next.yml` `repos.compareCommitsWithBasehead` |
| `pull_requests: write` | publishes reviews, replies in threads, edits bodies, and opens pull requests | `src/github/reviews.rs:95` `POST repos/{}/pulls/{}/reviews`; `src/merge_enlister/mod.rs:499` (the `APPROVE`); `src/github/mod.rs:377` `POST .../comments/{}/replies`; `src/merge_enlister/mod.rs:541` `gh pr edit --body`; `promotion-open-next.yml` `pulls.create`; `toolchain-weekly.yml:76` `gh pr create` |
| ” (read half) | every merge-admission read | `src/github/mod.rs:172,354,408,467,510`; `src/merge_enlister/mod.rs:374`; `src/recovery/reconciliation_sweep.rs:159`; `src/unresolved_review_guard/mod.rs:210` (GraphQL `reviewThreads`) |
| `issues: write` | **pull-request conversation comments are issue comments in GitHub's model** — this permission is what backs `gh pr comment` and the scorecard upsert, not only literal issues | `src/github/mod.rs:266` `gh pr comment`; `src/github/mod.rs:300,323` `GET`/`PATCH repos/{}/issues/{}/comments`, `repos/{}/issues/comments/{id}` (scorecard upsert); `src/github/reviews.rs:148` fallback summary comment; `src/ci_triager.rs:207` `gh issue create`; `src/issue_reconciler/mod.rs:36,89` `gh issue list` / `gh issue comment` |
| `actions: read` | reads failing CI logs and run history; backs the `workflow_run` subscription | `src/ci_triager.rs:74` `gh run view --log-failed`; `src/github/mod.rs:570` `gh run list` (change-failure-rate) |
| `merge_queues: read` | **subscription only** — no REST call. GitHub: *"To subscribe to this event, a GitHub App must have at least read-level access for the 'Merge queues' repository permission."* The event is consumed at `src/webhook/webhook_handlers.rs:382` (`merge_group`/`destroyed` → queue healer) | — |
| `repository_hooks: write` | **scheduled for deletion** — see below | `src/cli/server.rs:216,307` `gh webhook forward`; `src/github/mod.rs:137,151` and `src/webhook/forwarder_supervisor.rs:164,182` list/delete stale forwarder hooks |

`repository_hooks` exists only because webhook delivery currently runs through the
`gh-webhook` extension, which creates a repository hook and then streams deliveries over
a websocket. Its own route templates, read out of the shipped binary:

```console
$ strings ~/.local/share/gh/extensions/gh-webhook/gh-webhook \
    | grep -oE '(repos|orgs)/%s[a-zA-Z0-9/%._-]*'
orgs/%s/hooks
repos/%s/hooks
```

**Deletion condition:** a GitHub App carries its own webhook endpoint
(`hook_attributes.url`). Once the daemon has a reachable URL, deliveries arrive there for
every installation, no per-repository hook is created, and this permission plus both
call-site clusters are deleted. `app-manifest.json` therefore ships **without**
`hook_attributes` — there is no public daemon endpoint today, and a manifest that names a
URL nothing listens on is a manifest that lies. Adding it later is a field edit in App
settings, not a re-creation.

### Deliberately not granted

| permission | why not |
|---|---|
| `workflows: write` | Withheld on purpose: it is the permission that lets the App rewrite `.github/workflows/**` — Anvil editing the gates that bind it. **Stated cost:** a fixer commit that touches a workflow file will be refused at push (`refusing to allow a GitHub App to create or update workflow ...`). That refusal is the correct outcome and should surface as a human ticket. (`toolchain-weekly` is unaffected: `anvil toolchain --apply` edits `rust-toolchain.toml`, not a workflow file.) |
| `administration` | No call site — and it is the permission that edits rulesets and branch protection. The App must not be able to weaken the gate `RULESET.md` creates. The most load-bearing omission on this list. |
| `checks`, `statuses` | No call site: `grep -rn 'check-runs\|check_runs\|/statuses/' src/ --include='*.rs'` returns nothing. Anvil publishes verdicts as reviews and comments. A future check-run publisher is a deliberate re-grant, not a default. |
| `secrets`, `environments`, `deployments`, `packages`, `pages`, `security_events`, `vulnerability_alerts`, `repository_projects`, `single_file` | No call site. |
| `members`, `organization_administration`, `organization_hooks`, `organization_plan` | No call site. Today's PAT carries `admin:org`; dropping it is part of the point. |
| user-level permissions (`emails`, `followers`, `gpg_keys`, …) | An installation token has no user. Meaningless here. |

### Two notes on the manifest that are decisions, not defaults

- **`"public": false`.** A private App installs only on the account that owns it.
  `oyatie/anvil` and `oyatie/oyatie` are both in the owning org, so this is correct today.
  Flipping it to `true` is a real blast-radius change and a human decision — needed only
  if Anvil must watch a repository outside `oyatie`.
- **The name is not load-bearing.** App names are globally unique, so `Oyatie Anvil` may
  be taken. Pick anything; change nothing else. Nothing in `CODE-CHANGES.md` keys on the
  slug, and the roadmap's exit criterion is deliberately *type plus disjointness* rather
  than a name — precisely so that a rename cannot silently satisfy or break it. The
  resulting principal has the shape GitHub gives every App:

```console
$ gh api 'users/dependabot%5Bbot%5D'    --jq '{login,id,type}'
{"id":49699333,"login":"dependabot[bot]","type":"Bot"}
$ gh api 'users/github-actions%5Bbot%5D' --jq '{login,id,type}'
{"id":41898282,"login":"github-actions[bot]","type":"Bot"}
```

### One key I could not confirm against a live object

`merge_queues` is the key name for the "Merge queues" permission per GitHub's REST
reference. Neither App installed on this org holds it, so it could not be read back from a
real permissions object the way the others were:

```console
$ gh api orgs/oyatie/installations --jq '.installations[0].permissions'
{"actions":"write","administration":"read","checks":"write","contents":"write", ... ,"workflows":"write"}
```

If the creation form rejects the manifest, that key is the first suspect. `SETUP.md` step
2 carries the recovery.
