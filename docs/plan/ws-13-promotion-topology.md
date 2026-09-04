# WS-13 — Promotion fabric, trunk topology, and the post-merge loop

**Measured state:** `promotion-open-next` has failed on **every** push to dev in the measured window
(13/13; `gh run list --branch dev --limit 40`) with a 403 — GitHub Actions may not create PRs and no
`PROMOTION_PAT` exists — so a `promote(staging): 211 commits` PR never opens and
`staging/canary/production` starve behind active rulesets. `main` is **45 ahead / 300 behind** dev
(`git rev-list --count origin/dev..origin/main` / reverse): diverged, not merely stale. Branch
sprawl: 315 local (`git branch | wc -l`) / 167 origin / 78 `pr/*` refs (both via
`git branch -r --format='%(refname:short)' | grep -c '^origin/'` and `… '^pr/'` — the `--format` is
load-bearing: bare `git branch -r` indents every line, so the anchored greps return **0**; an
earlier revision cited exactly that broken form and external review caught it, the instrument-
verification class this plan's own WS-12 polices), 50 worktrees (`git worktree list | wc -l`).

**The malpractice class:** a standing red that gates nothing. A permanently-failing job is an alarm
nobody hears; it normalizes red and buries real signal. The class-level fix is a tripwire on
*standing state*, not another fix for this one workflow.

## Milestones

| ID | Milestone | Exit criterion | Owner |
|---|---|---|---|
| H1-11 | Promotion PRs open under the App identity (consumes WS-11); `promotion-predecessor` actually runs on them | last 5 `promotion-open-next` runs `success`; a promotion PR exists with anvil-ci + predecessor checks executed | Builder tools |
| H1-11b | `main` disposition decided by registry ticket: reconcile the 45 unique commits (merge, cherry-pick, or archive), then `main` tracks promotions or is demoted to legacy-read-only — decided, not drifted | registry row; after execution `git rev-list --count origin/dev..origin/main` = 0 or `main` is ruleset-frozen with its role documented | Human ticket queue |
| H1-11c | Standing-red tripwire live: any required or scheduled workflow red > 7 days on dev auto-opens a ticket with owner; weekly standing-red count published (target 0) | seeded always-red workflow produces a ticket within the window | Observability |
| H1-11d | Sprawl drain — **report-and-ticket, no automated deletion**: branch deletion on shared refs is in WS-06's destructive class (highest tier at every rung, human-ticketed until R4), so the TTL sweep *enumerates* stale branches/worktrees and opens a weekly batch ticket; deletions execute only on the ticket's ratification (registry row). GitHub's native delete-head-branch-on-merge is a platform setting Jason may enable via that same ticket — a human flipping a setting, not an agent deleting refs. `pr/*` refspec reviewed in the same ticket | remote branch count baselined (**167**, `git branch -r --format='%(refname:short)' \| grep -c '^origin/'`, 2026-08-31) and ratcheted downward; the ratchet's instrument is itself proven before trust — seeded with a fake branch ref and shown counting it (WS-12 rule 3) — because the first cited instrument here was anchored wrong and read 0 forever; weekly sweep report in CI artifacts; a deletion without a ratified ticket is refused pre-action (seeded) | Builder tools (report) + Human ticket queue (deletion) |
| H2-7 | Post-merge loop on anvil itself: deploy-observe-revert — canary analysis keyed to health metrics, auto-revert on breach (research gap 1: every hyperscaler pairs land authority with this loop) | seeded bad deploy auto-reverts inside the drill budget (default pin: 15 minutes detection-to-healthy; registry row at milestone start); quarterly drill logged in registry | Release |
| H3-5 | Promotion rungs agent-advanced on health evidence (feeds WS-06 R4) | seeded failed-canary halts promotion with no human in the loop; latency + rollback drills measured | Release |

## Ratchets

- Standing-red tripwire (H1-11c) is the class ratchet: the *next* permanently-red anything becomes a
  ticketed, owned defect within 7 days by construction.
- Branch-count ratchet: one baseline authority — the H1-11d row above (167 @ 2026-08-31, with
  its `--format` instrument); growth past it is a red in the weekly sweep.
- Promotion rungs never skip: `promotion-predecessor` fails a base whose head is not its
  predecessor, and the staging/canary/production ruleset **already requires it**
  (`gh api repos/oyatie/anvil/rulesets/21064983` → `required_status_checks:
  [promotion-predecessor]`). The former PR-body text claimed "no branch here is protected, so that
  check is advisory until one requires it" — stale prose that contradicted the measured ruleset.
  H1-11 removes that mutable configuration claim: the template now defers merge eligibility to the
  repository rules and required checks in force when eligibility is evaluated. The ratchet here:
  the requirement stays, and `strict_required_status_checks_policy`
  (currently `false` on that ruleset) is revisited by registry ticket once promotion PRs flow.

## Non-goals

No deleting the promotion ladder to clear the red (the rungs are the deployment model; the fix is
identity + requiredness); no history rewrite on `main`; no automated deletion of *any* shared ref
or local worktree — remote branches included — until R4 per WS-06's destructive tier (until then:
report + ticket + human ratification, as H1-11d specifies).
