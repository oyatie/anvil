#!/usr/bin/env bash
set -e

echo "🚀 Starting Anvil Delivery Fabric Daemon..."
cargo run --release -- serve
