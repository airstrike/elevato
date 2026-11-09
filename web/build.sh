#!/bin/sh
# Build the web page into DIR (dist/ by default), ready to serve:
#   sh web/build.sh && python3 -m http.server -d dist
# The page runs the player's program on rawr's universe: web/universe built
# for wasm32 by rawr-import as a universe runner, whose bindings compile the
# program in the page. This follows rawr's play/universe/web.sh and
# web/build.sh. wasm-bindgen-cli must match the wasm-bindgen of the
# runner's Cargo.lock, so the version it takes is installed under
# target/web/tools/<version>.
set -eu
cd "$(dirname "$0")/.."
out="${1:-dist}"
universe=web/universe

sh "$universe/programs.sh"

# `run` builds the runner and then starts it, which a wasm32 module can't
# be here: once the runner is built, that start is the one failure left.
log=target/web/universe.log
mkdir -p target/web
status=0
RUSTUP_TOOLCHAIN=nightly-2026-07-11 \
  CARGO_BUILD_TARGET=wasm32-unknown-unknown \
  RUSTFLAGS='--cfg getrandom_backend="wasm_js"' \
  CARGO_PROFILE_DEV_OPT_LEVEL=s \
  CARGO_PROFILE_DEV_DEBUG_ASSERTIONS=false \
  CARGO_PROFILE_DEV_OVERFLOW_CHECKS=false \
  RAWR_UNIVERSE_RUNNER=1 \
  "${RAWR_IMPORT:-rawr-import}" run "$universe" 2> "$log" || status=$?
if [ "$status" -ne 0 ] && ! grep -q "runner exited before connecting its protocol socket" "$log"; then
  cat "$log" >&2
  exit "$status"
fi
runner=$(find target/rawr -path "*/runner/target/wasm32-unknown-unknown/debug/universe.wasm" | head -n 1)
if [ -z "$runner" ]; then
  echo "web/build.sh: no runner was built (see $log)" >&2
  exit 1
fi
lock="${runner%/target/wasm32-unknown-unknown/debug/universe.wasm}/Cargo.lock"

version=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/"/, "", $3); print $3 }' "$lock")
tools="target/web/tools/$version"
if ! "$tools/bin/wasm-bindgen" --version 2>/dev/null | grep -q "$version"; then
  CARGO_TARGET_DIR=target/web/tools-build cargo +stable install wasm-bindgen-cli \
    --version "$version" --locked --root "$tools" >&2
  rm -rf target/web/tools-build
fi

rm -rf "$out"
mkdir -p "$out"
"$tools/bin/wasm-bindgen" --target web --no-typescript --remove-name-section \
  --out-name universe --out-dir "$out" "$runner"
cp "$universe/Cargo.toml" "$out/universe.toml"
cp tests/solutions/starter.rs "$out/starter.rs"
cp web/index.html web/elevato.js "$out/"
cp assets/og.png assets/favicon.svg assets/favicon-32.png assets/apple-touch-icon.png \
  assets/fonts/GeistMono-Variable.ttf assets/fonts/OFL.txt "$out/"
ls -l "$out"
