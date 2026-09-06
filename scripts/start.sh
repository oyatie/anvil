#!/usr/bin/env bash
set -euo pipefail

# Launches the Anvil daemon.
#
# stdin is redirected from /dev/null deliberately. The daemon never reads stdin,
# but inheriting the terminal means any stray input reaches it -- and with tmux
# `mouse off`, a scroll wheel is translated into arrow-key escape sequences and
# delivered as input, which floods the pane and looks like a crash. Detaching
# stdin makes the daemon immune to that regardless of terminal configuration.
#
# It also prevents a child process from putting the operator's tty into raw mode
# via an inherited stdin, which previously disabled both ONLCR (staircased log
# output) and INTR (Ctrl-C stopped working).

cd "$(dirname "$0")/.."

if [ ! -x ./target/release/anvil ]; then
  echo "🔨 Release binary not found; building..."
  cargo build --release
fi

echo "🚀 Starting Anvil Delivery Fabric Daemon..."
exec ./target/release/anvil serve < /dev/null
