#!/usr/bin/env bash
# Runs the schema_evolution gate over the last N commits of this repo.
# Usage: scripts/schema_regression.sh [N]
set -euo pipefail
N="${1:-10}"
for i in $(seq 1 "$N"); do
  sha=$(git rev-parse --short "HEAD~$((i - 1))")
  subj=$(git log -1 --format=%s "HEAD~$((i - 1))" | cut -c1-52)
  out=$(git diff "HEAD~$i" "HEAD~$((i - 1))" | ./target/debug/examples/schema_repro)
  printf '%-10s %-55s %s\n' "$sha" "$subj" "$out"
done
