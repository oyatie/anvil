#!/usr/bin/env bash
# Materialise the agent tool-call policy into the location each harness reads.
#
# WHY A SCRIPT. The file a harness enforces lives under its agent directory
# (.claude/, .codex/, ...), and pre-commit refuses every staged path there --
# that refusal is what keeps agent scratch state out of the tree. So the policy
# is tracked under policies/agents/ and installed from there. Same shape as the
# git hooks: templates in src/git_manager/hooks/, installed into .git/hooks/.
#
# WHY A TABLE. More than one harness runs against this repository. A policy that
# covers the one harness whose schema we happen to know, while reporting
# success, is a policy that publishes confidence it did not earn. Every harness
# named by pre-commit's own refusal regex is declared below; one with no
# template is reported as UNPOLICED rather than skipped silently. Invariant I1:
# absent evidence is never a pass.
#
# Adding a harness is a row plus a template. It is data, not code.
#
# Mirrors src/git_manager/hook_liveness.rs: absent is not a defect (a fresh
# clone legitimately has none); drifted is (a policy edited in place is a policy
# nobody reviewed); reported as findings, never as a bool.
set -eu

repo="$(git rev-parse --show-toplevel)"

# binary : target relative to repo : template relative to policies/agents/ ("-" = none yet)
HARNESSES="
claude:.claude/settings.json:claude-settings.json
codex:.codex/config.toml:-
cursor:.cursor/cli-config.json:-
grok:.grok/config.json:-
agy:.agents/policy.json:-
"

installed=0; uptodate=0; drifted=0; unpoliced=0; absent=0

for row in $HARNESSES; do
  bin="${row%%:*}"; rest="${row#*:}"
  target="$repo/${rest%%:*}"; tmpl_name="${rest##*:}"

  if ! command -v "$bin" >/dev/null 2>&1; then
    absent=$((absent + 1)); continue
  fi

  if [ "$tmpl_name" = "-" ]; then
    echo "UNPOLICED  $bin is installed and this repository ships no policy for it."
    unpoliced=$((unpoliced + 1)); continue
  fi

  tmpl="$repo/policies/agents/$tmpl_name"
  if [ ! -f "$tmpl" ]; then
    echo "ERROR      $bin: template declared but absent: $tmpl" >&2
    exit 1
  fi
  case "$tmpl" in
    *.json) python3 -c "import json,sys; json.load(open(sys.argv[1]))" "$tmpl" \
              || { echo "ERROR      $bin: template is not valid JSON" >&2; exit 1; } ;;
  esac

  if [ ! -e "$target" ]; then
    mkdir -p "$(dirname "$target")"; cp "$tmpl" "$target"
    echo "INSTALLED  $bin -> ${target#"$repo"/}"
    installed=$((installed + 1))
  elif cmp -s "$tmpl" "$target"; then
    echo "OK         $bin"
    uptodate=$((uptodate + 1))
  else
    echo "DRIFTED    $bin: ${target#"$repo"/} differs from policies/agents/$tmpl_name" >&2
    diff -u "$target" "$tmpl" >&2 || true
    drifted=$((drifted + 1))
  fi
done

echo "installed=$installed up-to-date=$uptodate drifted=$drifted unpoliced=$unpoliced not-installed=$absent"
[ "$drifted" -eq 0 ] || { echo "Nothing was overwritten. Reconcile, then re-run." >&2; exit 1; }
