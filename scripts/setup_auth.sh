#!/usr/bin/env bash
set -e

echo "======================================================"
echo "🔐 Setting up GitHub CLI Authentication & Extensions"
echo "======================================================"

# 1. Check gh installation
if ! command -v gh &> /dev/null; then
    echo "❌ GitHub CLI ('gh') is not installed. Please install it with: brew install gh"
    exit 1
fi

echo "✅ GitHub CLI found: $(gh --version | head -n 1)"

# 2. Login to GitHub with required scopes
echo ""
echo "👉 Authenticating GitHub CLI with 'repo' and 'admin:repo_hook' scopes..."
gh auth login --scopes "repo,admin:repo_hook,read:org" -h github.com

# 3. Install gh-webhook extension
echo ""
echo "👉 Installing / Updating 'cli/gh-webhook' extension..."
gh extension install cli/gh-webhook || gh extension upgrade gh-webhook || true

echo ""
echo "✅ Authentication and webhook extension setup complete!"
echo "Run './scripts/start.sh' or 'cargo run -- serve' to start the review daemon."
