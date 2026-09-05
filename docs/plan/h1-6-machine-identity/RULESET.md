# RULESET — H1-9, specified and NOT applied

> ## DECISION, 2026-09-01 (Jason): NOT YET, AND NOT ON A SCHEDULE
>
> The ruleset change is **not** the step after `SETUP.md`. It is gated on an
> observation: that the App demonstrably leaves reviews under **its own identity**,
> watched over time rather than declared once.
>
> The reason is a genuine unknown, and this document should not pretend otherwise.
> GitHub's ruleset docs describe `required_approving_review_count` in terms of
> "people with write permissions" and code owners, and say nothing about whether a
> GitHub App's `APPROVE` counts toward it. That question is not answerable from the
> documentation, and guessing it wrong in either direction is expensive: guess that
> App approvals count and the gate may be unsatisfiable; guess that they do not and
> the gate may be satisfiable by the very actor it exists to constrain.
>
> So it is measured instead. This is the trust ratchet the plan already specifies,
> applied to the *mechanism* rather than to an agent: earn the rung with evidence,
> do not declare it.
>
> **The observation, and what discharges it — see §2a.**

> ## PRECONDITION: THE APP MUST EXIST AND THE DAEMON MUST RUN AS IT
>
> Do not run the command in §3 until `SETUP.md` is complete **and**
> `CODE-CHANGES.md` §1, §2 and §6 have landed.
>
> Raising `required_approving_review_count` to `1` today makes the gate **worse than
> zero**. `MergeEnlister::ensure_approving_review` (`src/merge_enlister/mod.rs:361`)
> submits a formal `APPROVE` at `:488` and arms auto-merge at `:234`, authenticated as
> `jason931225`. GitHub cannot distinguish that approval from Jason's, because it is
> Jason's credential. The result renders as *"1 approval required — 1 received"* and
> gates nothing.
>
> Zero at least tells the truth about itself. A gate that reports satisfied because the
> thing it gates satisfied it is the exact failure this repository's doctrine is written
> against, and shipping one would make the roadmap's own tripwire ("identity migration
> stalls", `docs/plan/anvil-roadmap.md:280`) fire on a green board.

---

## 1. Current state, for reference

```console
$ gh api repos/oyatie/anvil --jq .default_branch
dev
```

The ruleset targets `~DEFAULT_BRANCH`, which resolves to `dev`.

```console
$ gh api repos/oyatie/anvil/rulesets/21064279
```

```json
{
  "id": 21064279,
  "name": "Hyperscaler Merge Queue & Quality Gate",
  "target": "branch",
  "source_type": "Repository",
  "source": "oyatie/anvil",
  "enforcement": "active",
  "conditions": { "ref_name": { "exclude": [], "include": ["~DEFAULT_BRANCH"] } },
  "rules": [
    { "type": "pull_request",
      "parameters": {
        "required_approving_review_count": 0,
        "dismiss_stale_reviews_on_push": true,
        "required_reviewers": [],
        "require_code_owner_review": false,
        "dismissal_restriction": { "enabled": false, "allowed_actors": [] },
        "require_last_push_approval": false,
        "required_review_thread_resolution": true,
        "require_extra_approval_for_unattributed_changes": false,
        "allowed_merge_methods": ["merge", "squash", "rebase"] } },
    { "type": "merge_queue",
      "parameters": {
        "merge_method": "MERGE", "max_entries_to_build": 5,
        "min_entries_to_merge": 1, "max_entries_to_merge": 5,
        "min_entries_to_merge_wait_minutes": 0,
        "grouping_strategy": "ALLGREEN", "check_response_timeout_minutes": 60 } },
    { "type": "required_status_checks",
      "parameters": {
        "strict_required_status_checks_policy": true,
        "do_not_enforce_on_create": false,
        "required_status_checks": [{ "context": "fast-checks" }] } }
  ],
  "bypass_actors": [],
  "current_user_can_bypass": "never"
}
```

Every ruleset that applies to `dev`, and `main` for contrast:

```console
$ gh api repos/oyatie/anvil/rulesets --jq '.[].id' | while read -r id; do
    gh api "repos/oyatie/anvil/rulesets/$id" --jq '{id, name,
      include: .conditions.ref_name.include,
      approvals: ([.rules[]|select(.type=="pull_request")|.parameters.required_approving_review_count]|first)}'
  done
{"approvals":0,"id":21064983,"include":["refs/heads/staging","refs/heads/canary","refs/heads/production"],"name":"Hyperscaler Environment Promotion Guard"}
{"approvals":0,"id":21064279,"include":["~DEFAULT_BRANCH"],"name":"Hyperscaler Merge Queue & Quality Gate"}
{"approvals":1,"id":21230025,"include":["refs/heads/main"],"name":"main-not-agent-merge"}
```

`main` already requires one. The mechanism exists; it is simply not applied to the branch
the work lands on.

---

## 2. Two changes, not one — and why the count alone is not enough

`required_approving_review_count: 1` says *someone with write access approved*. It does
**not** say *a human approved*.

Whether a GitHub App's approving review counts toward that number could not be settled
from documentation: GitHub's ruleset page describes required approvals in terms of
"people with write permissions" and "a designated code owner", and says nothing about
Apps either way. **Treat it as counting.** A design that only works if GitHub declines to
count App approvals is a design resting on an unverified negative.

The mechanism that does not depend on that question is `require_code_owner_review`:

```console
$ cat .github/CODEOWNERS
* @jason931225
```

Code owners are **users, teams, or email addresses** — GitHub's documentation lists no
other kind, and an App is not among them. So a code-owner requirement cannot be satisfied
by the App under any answer to the question above. That is the belt; the count is the
braces; and `CODE-CHANGES.md` §6 (the enlister stops approving at all) is the actual fix.
Three independent reasons, because the cost of being wrong is a merge gate that isn't.

### The consequence that must be understood before running this

The org has exactly one human member:

```console
$ gh api orgs/oyatie/members --jq '[.[].login]'
["jason931225"]
$ gh api "orgs/oyatie/memberships/jason931225" --jq .role
admin
```

GitHub does not let an author approve their own pull request. So after this change:

- **Pull request authored by the App** → Jason approves → merges. This is the intended
  path and it works. Anvil-originated work must therefore be opened by the App, which is
  `CODE-CHANGES.md` §1 anyway.
- **Pull request authored by `jason931225`** → Jason cannot approve it, the App must not,
  and no third principal exists → **it cannot merge.** All 100 pull requests sampled today
  are in this category:

```console
$ gh pr list --repo oyatie/anvil --state all --limit 100 --json author \
    --jq 'group_by(.author.login)|map({author:.[0].author.login, n:length})'
[{"author":"jason931225","n":100}]
```

This is not a reason to skip the change; it is the change working. But it must be
sequenced: **agent-authored pull requests must be opening as the App before this lands**,
or the trunk locks. If a hand-authored pull request genuinely needs to land in the
interim, add a scoped `bypass_actors` entry (repository admin, `bypass_mode: pull_request`)
rather than lowering the count — a bypass is visible in the ruleset and in the merge's
provenance; a lowered count is not. `bypass_actors` is `[]` today and should return to
`[]`.

---

## 2a. The observation that gates §3

Nothing in §3 is run until every line below holds. None of it requires the ruleset
to be enabled — that is the point: the property is observable **before** it is
relied upon.

**O1 — the App's review is attributed to the App.** After the App submits a review,
its author is the App's own principal, not a human's:

```
gh api repos/oyatie/anvil/pulls/<N>/reviews \
  --jq '.[] | {user: .user.login, type: .user.type, state}'
```

Every Anvil-submitted review must show `type: "Bot"` and a login ending `[bot]`.
A single row reading `jason931225` means the migration is incomplete, whatever
`SETUP.md` reported.

**O2 — it holds over time, not once.** At least **10 reviews across at least 14
days**, on real pull requests, with zero misattributions. One successful review
proves the call worked; it does not prove the identity is stable across token
refresh, re-installation, or the paths in `CODE-CHANGES.md` that were not exercised
that day.

**O3 — the reviewer and the reviewed are different principals.** Over the same
window, a review Jason leaves and a review Anvil leaves are distinguishable by
principal alone, and the fixer answers the first and not the second — the seeded
pair in `CODE-CHANGES.md` §2, re-run at the end of the window rather than only at
the start.

**O4 — the open question is answered by observation, not by reading.** Whether an
App approval counts toward `required_approving_review_count` is settled empirically
on a throwaway branch with its own ruleset, never by first enabling it on `dev`.

Note that by then **no in-tree path submits an App approval at all** — `CODE-CHANGES.md`
§6 deletes the enlister's `APPROVE` arm — so this one is exercised by hand, with an
installation token:

```
gh api repos/oyatie/anvil/pulls/<N>/reviews -f event=APPROVE -f body='O4 probe'
gh api repos/oyatie/anvil/pulls/<N> --jq '.mergeable_state'   # on the probe branch,
                                                              # with its own 1-approval ruleset
```

**Pass:** the probe branch reports mergeable with the App's approval as the only one.
**Fail:** it does not. Either answer discharges O4; only an unrun probe leaves it open.

**If O1–O4 hold**, §3 is a decision Jason takes with the evidence in hand, recorded
in the decision registry. **If any fails**, §3 stays unapplied and the failure is the
finding — the mechanism in §2 (`require_code_owner_review`, which an App cannot
satisfy because code owners must be users or teams) exists precisely so the gate does
not depend on O4's answer.

Until then `required_approving_review_count` stays **0**. Zero tells the truth about
itself; a gate satisfied by the actor it gates does not.

---

## 3. The change

Fetch, transform exactly two fields, put it back. Not hand-written JSON: everything not
named below must survive byte-for-byte, and the merge-queue and status-check rules in this
ruleset are load-bearing.

```sh
# 1. capture the current state (also your rollback)
gh api repos/oyatie/anvil/rulesets/21064279 > /tmp/ruleset-21064279.before.json

# 2. transform
python3 - <<'PY'
import json
d = json.load(open("/tmp/ruleset-21064279.before.json"))
body = {k: d[k] for k in ("name", "target", "enforcement", "conditions", "rules", "bypass_actors")}
for r in body["rules"]:
    if r["type"] == "pull_request":
        r["parameters"]["required_approving_review_count"] = 1
        r["parameters"]["require_code_owner_review"] = True
json.dump(body, open("/tmp/ruleset-21064279.after.json", "w"), indent=2)
PY

# 3. show exactly what changes, and change nothing else
python3 - <<'PY'
import json
a = json.load(open("/tmp/ruleset-21064279.before.json"))
b = json.load(open("/tmp/ruleset-21064279.after.json"))
pa = [r for r in a["rules"] if r["type"] == "pull_request"][0]["parameters"]
pb = [r for r in b["rules"] if r["type"] == "pull_request"][0]["parameters"]
print("changed:", {k: (pa[k], pb.get(k)) for k in pa if pa[k] != pb.get(k)})
print("rule types unchanged:", [r["type"] for r in a["rules"]] == [r["type"] for r in b["rules"]])
PY
```

Expected output of step 3, verified against the live ruleset on 2026-09-01 (the transform
was run; the `PUT` was not):

```
changed: {'required_approving_review_count': (0, 1), 'require_code_owner_review': (False, True)}
rule types unchanged: True
```

Then, and only then:

```sh
gh api --method PUT repos/oyatie/anvil/rulesets/21064279 \
  --input /tmp/ruleset-21064279.after.json
```

**Rollback**, if the trunk locks:

```sh
python3 -c "import json;d=json.load(open('/tmp/ruleset-21064279.before.json'));json.dump({k:d[k] for k in ('name','target','enforcement','conditions','rules','bypass_actors')},open('/tmp/rb.json','w'))"
gh api --method PUT repos/oyatie/anvil/rulesets/21064279 --input /tmp/rb.json
```

Note the `PUT` requires repository-admin rights (`jason931225` has them:
`gh api repos/oyatie/anvil/collaborators --jq '[.[]|{login,admin:.permissions.admin}]'` →
`[{"admin":true,"login":"jason931225"}]`). It is also exactly the capability
`app-manifest.json` deliberately withholds from the App — Anvil cannot run this command,
by construction.

---

## 4. Verify afterwards — that it is on, and that Anvil cannot satisfy it

### 4a. Effective state, not a ruleset id

The roadmap states the H1-9 criterion as effective state over every ruleset applying to
`dev`, with id `21064279` named as evidence and never as the predicate
(`docs/plan/anvil-roadmap.md:170`). Use this, which is that criterion:

```sh
gh api repos/oyatie/anvil/rulesets --jq '.[].id' | while read -r id; do
  gh api "repos/oyatie/anvil/rulesets/$id" --jq \
    'select(.conditions.ref_name.include | index("~DEFAULT_BRANCH") or index("refs/heads/dev"))
     | {id, name, enforcement,
        approvals: ([.rules[]|select(.type=="pull_request")|.parameters.required_approving_review_count]|first),
        codeowner: ([.rules[]|select(.type=="pull_request")|.parameters.require_code_owner_review]|first),
        bypass: (.bypass_actors|length)}'
done
```

Passes when every row applying to `dev` shows `approvals >= 1`, `codeowner: true`,
`enforcement: "active"`, and `bypass: 0`.

> `GET /repos/{owner}/{repo}/rules/branch/{branch}` would express this in one call and
> would be the better predicate. It returned `404` for both `dev` and `main` on
> 2026-09-01 under an `admin:org`+`repo` token, with the generic
> `documentation_url: https://docs.github.com/rest` that GitHub returns for an unmatched
> route. **Why is unknown** — do not build the check on it without establishing why it
> 404s. The loop above is the working substitute.

### 4b. That Anvil can no longer satisfy it

Three checks. The first two are the real ones; the third is the observation that proves
the first two are describing reality.

1. **The enlister no longer approves.** From `CODE-CHANGES.md` §6 — no non-test path under
   `src/` constructs `verdict: "APPROVE"`. Assert it as a meta-test, and seed one to prove
   the meta-test can fail. Grep alone is a proxy; a test that has never been red has not
   been shown to work.

2. **The App is not a code owner and cannot become one.** `.github/CODEOWNERS` contains
   only `* @jason931225`; code owners are users, teams or emails. Pin it:

   ```sh
   grep -v '^\s*#' .github/CODEOWNERS | grep -o '@[A-Za-z0-9/_-]*' | sort -u
   # must not contain the App slug, and must contain at least one human
   ```

3. **A probe pull request cannot merge on Anvil's approval alone.** Open a trivial
   docs-only pull request **authored by the App**, let Anvil review and certify it
   normally, and confirm with no human approval recorded:

   ```sh
   gh pr view <n> --repo oyatie/anvil --json reviewDecision,reviews,mergeStateStatus \
     --jq '{reviewDecision, mergeStateStatus, reviewers: [.reviews[]|{a:.author.login, s:.state}]}'
   ```

   Passes when `reviewDecision` is `REVIEW_REQUIRED` (not `APPROVED`) and
   `mergeStateStatus` is `BLOCKED`. Then approve as Jason and confirm it flips. A gate
   that has never refused a real merge attempt has not been shown to be a gate — the same
   rule as §4b(1), applied to the forge instead of to the code.

---

## 5. Not in scope here

- **`staging` / `canary` / `production` (ruleset 21064983)** also require zero approvals.
  Raising them is `ws-13` / rung R4 territory and depends on promotion pull requests
  actually opening first (`README.md` §4). Do not batch it with this change: this one has
  a rollback that is understood, and that one does not yet.
- **`require_last_push_approval`** (currently `false` on both `dev` and `main`) is the
  natural next ratchet — it stops an approval from surviving a later push by the author.
  It belongs to R1, with its own evidence, not to this change.
- **`main` (ruleset 21230025)** is already at 1 and stays as it is.
