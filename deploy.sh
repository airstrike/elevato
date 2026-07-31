#!/bin/bash
# Build the release bundle and deploy it to Cloudflare Pages.
#
# Prerequisites: trunk, the wasm32-unknown-unknown target, and a
# wrangler login with access to the `elevato` Pages project. See
# PUBLISHING.md for the full ship checklist.
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

echo "Building WASM release..."
trunk build --release

echo "Deploying dist/ to Cloudflare Pages..."
npx wrangler pages deploy dist --project-name elevato
