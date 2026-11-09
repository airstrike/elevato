#!/bin/bash
# Build the page and deploy it to Cloudflare Pages.
#
# Prerequisites: rawr-import, the nightly-2026-07-11 toolchain with the
# wasm32-unknown-unknown target, and a wrangler login with access to the
# `elevato` Pages project. See PUBLISHING.md for the full ship checklist.
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

echo "Building the page..."
sh web/build.sh dist

echo "Deploying dist/ to Cloudflare Pages..."
npx wrangler pages deploy dist --project-name elevato --branch main
