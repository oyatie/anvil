# SETUP — the steps only a human can do

Roughly ten minutes. **Steps 2 and 3 need org-owner rights on `oyatie`.** Everything else
is a paste.

Nothing here is optional-order: step 3 cannot run before step 2, and `RULESET.md` cannot
run before all of this. The redirect `code` in step 2 is single-use and expires in one
hour, which is why the catcher goes up first.

---

## Step 0 — precondition, and where to work

```console
$ gh api "orgs/oyatie/memberships/$(gh api user --jq .login)" --jq '{role, state}'
{"role":"admin","state":"active"}
```

`role: admin` is org-owner. (Verified 2026-09-01 for `jason931225`; the org has exactly
one member.) If it says `member`, stop — steps 2 and 3 will not be offered to you.

**Work outside the repository.** The private key must never sit in a git working tree.

```console
$ mkdir -p ~/.anvil && cd ~/.anvil
```

Everything below writes into `~/.anvil`. No file created here belongs in the repo, so
nothing needs adding to `.gitignore`.

---

## Step 1 — start the redirect catcher (no special rights)

Paste this into a terminal and leave it running. It listens on `127.0.0.1:8721`, catches
GitHub's redirect, exchanges the one-shot `code`, and writes the credentials. Browser
noise (`/favicon.ico`) does not consume it.

```sh
cat > ~/.anvil/catch.py <<'PY'
import http.server, json, urllib.request, pathlib
from urllib.parse import urlparse, parse_qs
box = {}
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        code = (parse_qs(urlparse(self.path).query).get("code") or [None])[0]
        if not code:
            self.send_response(404); self.end_headers(); return
        req = urllib.request.Request(
            "https://api.github.com/app-manifests/%s/conversions" % code, method="POST",
            headers={"Accept": "application/vnd.github+json",
                     "User-Agent": "anvil-app-setup",
                     "X-GitHub-Api-Version": "2022-11-28"})
        try:
            app = json.load(urllib.request.urlopen(req))
        except Exception as e:
            self.send_response(500); self.end_headers()
            self.wfile.write(("conversion failed: %s" % e).encode())
            box["err"] = str(e); return
        pem = pathlib.Path("anvil-app.private-key.pem")
        pem.write_text(app["pem"]); pem.chmod(0o600)
        box["app"] = {"app_id": app["id"], "slug": app["slug"],
                      "html_url": app["html_url"],
                      "webhook_secret": app.get("webhook_secret")}
        pathlib.Path("anvil-app.json").write_text(json.dumps(box["app"], indent=2))
        self.send_response(200); self.send_header("Content-Type", "text/plain"); self.end_headers()
        self.wfile.write(b"App created. Return to the terminal.")
    def log_message(self, *a): pass
srv = http.server.HTTPServer(("127.0.0.1", 8721), H)
while "app" not in box and "err" not in box:
    srv.handle_request()
print(json.dumps(box, indent=2))
PY
cd ~/.anvil && python3 catch.py
```

It prints the App id and slug and then exits. If it prints `err`, the `code` was already
spent or expired — redo step 2, which mints a fresh one.

---

## Step 2 — create the App under the org  ⟵ **org-owner**

GitHub's manifest flow is a form POST; there is no API that starts it (`README.md` §5
shows the two 404s that establish this). This writes the form, fills it from
`app-manifest.json`, and opens it. Run it in a **second** terminal, with the catcher still
running in the first.

```sh
cd ~/.anvil
python3 - <<'PY'
import json, pathlib
SRC = "<path-to-your-anvil-checkout>/docs/plan/h1-6-machine-identity/app-manifest.json"
m = json.load(open(SRC))
html = ("<!doctype html><meta charset=utf-8><title>Create the Anvil App</title>"
        "<form method=post "
        "action='https://github.com/organizations/oyatie/settings/apps/new?state=h1-6'>"
        "<input type=hidden name=manifest id=m>"
        "<button type=submit style='font:16px system-ui;padding:12px 20px'>"
        "Create the App under oyatie</button></form>"
        "<script>document.getElementById('m').value="
        + json.dumps(json.dumps(m)) + ";</script>")
p = pathlib.Path("create-anvil-app.html"); p.write_text(html)
print(p.resolve())
PY
open ~/.anvil/create-anvil-app.html      # linux: xdg-open
```

Then, in the browser:

1. Click **Create the App under oyatie**.
2. GitHub shows a confirmation page listing the permissions and events being requested.
   **Read them.** They must match `README.md` Appendix A — 7 permissions, 7 events, and
   *no* `Workflows` and *no* `Administration`. If the page shows something the appendix
   does not name, stop and find out why before confirming.
3. Confirm. GitHub redirects to `127.0.0.1:8721`; the browser says *"App created. Return
   to the terminal."* and the catcher in terminal 1 prints:

```json
{ "app": { "app_id": 1234567, "slug": "oyatie-anvil",
           "html_url": "https://github.com/apps/oyatie-anvil",
           "webhook_secret": null } }
```

`~/.anvil/anvil-app.private-key.pem` (mode 0600) and `~/.anvil/anvil-app.json` now exist.
**The PEM is shown exactly once.** If you lose it, generate a new one in App settings; the
App itself survives.

**If the form is rejected**, the first suspect is `merge_queues` (`README.md` Appendix A,
last section). Delete `"merge_queues": "read"` from `default_permissions` *and*
`"merge_group"` from `default_events`, re-run this step, and add both afterwards in the
App's settings page. Stated cost of leaving them off: `merge_group`/`destroyed` never
arrives, so the queue healer stops seeing ejections
(`src/webhook/webhook_handlers.rs:382`).

**If the name is taken** — App names are globally unique — change `"name"` to anything
free. Nothing else changes; no code keys on the slug.

---

## Step 3 — install it on the org  ⟵ **org-owner**

Open the `html_url` from step 2, or
`https://github.com/organizations/oyatie/settings/apps/<slug>/installations`.

1. **Install** → **Only select repositories**.
2. Select `anvil`. Add `oyatie` only if the daemon watches it — the point of an
   installation token is that it reaches what Anvil watches and nothing else.
3. After install the browser lands on
   `https://github.com/organizations/oyatie/settings/installations/<INSTALLATION_ID>`.
   That number is the installation id. Copy it.

Scripted alternative, which also proves the PEM works:

```sh
cat > ~/.anvil/jwt.sh <<'SH'
#!/bin/sh
# usage: jwt.sh <app_id> <private-key.pem>   -> a 10-minute App JWT on stdout
set -eu
b64() { openssl base64 -A | tr '+/' '-_' | tr -d '='; }
now=$(date +%s)
hdr=$(printf '{"alg":"RS256","typ":"JWT"}' | b64)
pl=$(printf '{"iat":%d,"exp":%d,"iss":"%s"}' "$((now-60))" "$((now+540))" "$1" | b64)
sig=$(printf '%s' "$hdr.$pl" | openssl dgst -sha256 -sign "$2" -binary | b64)
printf '%s.%s.%s\n' "$hdr" "$pl" "$sig"
SH
chmod +x ~/.anvil/jwt.sh

cd ~/.anvil
APP_ID=$(python3 -c 'import json;print(json.load(open("anvil-app.json"))["app_id"])')
JWT=$(./jwt.sh "$APP_ID" anvil-app.private-key.pem)
curl -sS -H "Authorization: Bearer $JWT" -H "Accept: application/vnd.github+json" \
  https://api.github.com/app/installations \
  | python3 -c 'import json,sys; print([{"id":i["id"],"account":i["account"]["login"],"repos":i["repository_selection"]} for i in json.load(sys.stdin)])'
```

---

## Step 4 — prove the credential before anything depends on it

```sh
cd ~/.anvil
APP_ID=$(python3 -c 'import json;print(json.load(open("anvil-app.json"))["app_id"])')
SLUG=$(python3 -c 'import json;print(json.load(open("anvil-app.json"))["slug"])')
INSTALLATION_ID=<from step 3>

JWT=$(./jwt.sh "$APP_ID" anvil-app.private-key.pem)
TOKEN=$(curl -sS -X POST \
  -H "Authorization: Bearer $JWT" -H "Accept: application/vnd.github+json" \
  "https://api.github.com/app/installations/$INSTALLATION_ID/access_tokens" \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["token"])')

# what the token can reach — should be exactly the repositories selected in step 3
curl -sS -H "Authorization: Bearer $TOKEN" -H "Accept: application/vnd.github+json" \
  https://api.github.com/installation/repositories \
  | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["total_count"], [r["full_name"] for r in d["repositories"]])'

# the principal that will author Anvil's comments
gh api "users/${SLUG}%5Bbot%5D" --jq '{login, id, type}'
```

The last command must print `"type": "Bot"` and a numeric `id`. **Record that id** —
`CODE-CHANGES.md` §2 uses it as the identity the loop-guard compares against.

**One honest correction to the roadmap.** `docs/plan/anvil-roadmap.md:167` states the
H1-6 exit criterion as `gh api user --jq .type` = `Bot`. An installation access token is
a server-to-server token with **no user behind it**, so `GET /user` is not expected to
answer under it. The two commands above measure the same property — the token resolves an
installation, and the authoring principal is of type `Bot` — and should replace it. Raise
that as a one-line amendment on the H1-6 ticket rather than making the criterion pass by
weakening it.

---

## Step 5 — put the values where they are read

**On the machine running `anvil server`** (daemon env / launchd plist / systemd unit):

| name | value |
|---|---|
| `GITHUB_APP_ID` | from `anvil-app.json` |
| `GITHUB_APP_INSTALLATION_ID` | from step 3 |
| `GITHUB_APP_PRIVATE_KEY` | the PEM contents (or `GITHUB_APP_PRIVATE_KEY_PATH` — `CODE-CHANGES.md` §1 specifies both) |

`GITHUB_APP_PRIVATE_KEY` is that exact name on purpose: it is already on the
never-hand-over list at all three outbound seams, so no subprocess can ever see it.

```console
$ grep -rn 'GITHUB_APP_PRIVATE_KEY' src/exec/
src/exec/gh.rs:84:    "GITHUB_APP_PRIVATE_KEY",
src/exec/net.rs:79:    "GITHUB_APP_PRIVATE_KEY",
src/exec/inherited.rs:95:            "GITHUB_APP_PRIVATE_KEY",
```

**For GitHub Actions** (this is what unblocks `promotion-open-next`):

```console
$ gh variable set ANVIL_APP_ID --repo oyatie/anvil --body "<app id>"
$ gh secret set ANVIL_APP_PRIVATE_KEY --repo oyatie/anvil < ~/.anvil/anvil-app.private-key.pem
```

The workflow edit that consumes them is specified in `CODE-CHANGES.md` §5 and is **not**
applied — the secrets can be set now and sit unused, which is the right order: the
workflow change is reviewable once the credential it names exists.

---

## Step 6 — what must NOT happen yet

- **Do not change the `dev` ruleset.** See `RULESET.md`. Raising the approval count while
  the daemon still runs as `jason931225` makes the gate worse, not better.
- **Do not revoke or narrow `jason931225`'s token** until the daemon has completed one
  full review → certify → enlist cycle authenticated as the App. Keep it as the rollback.
- **Do not set `PROMOTION_PAT`.** `README.md` §4.3 gives the reasoning and the narrow
  exception.

## Step 7 — cleanup and handover

`~/.anvil/create-anvil-app.html` and `~/.anvil/catch.py` can be deleted. Keep
`anvil-app.private-key.pem` (0600) and `anvil-app.json`; they are the only copies.

Post the App id, slug, installation id and the `<slug>[bot]` numeric id on the H1-6
ticket. Those four values are the entire input to `CODE-CHANGES.md`. Issue #171 stays
open until that lands — the App is the precondition, not the fix.
