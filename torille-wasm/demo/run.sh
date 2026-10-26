#!/usr/bin/env bash
# Run the torille-wasm demo locally.
#
# Builds the wasm + the demo (if either is stale), then serves the
# demo directory over HTTP. ES modules and module workers don't work
# from file://, so a real HTTP server is required.
#
# Usage:
#   ./run.sh                  # build (if needed) and serve on port 8765
#   PORT=9000 ./run.sh        # serve on a different port
#   FORCE=1 ./run.sh          # rebuild everything, ignoring mtimes
#
# Serves through `serve-demo` (the `torille-demo-server` crate), a
# CORS-enabled static server; see step 3 below.
#
# Requirements: a Rust toolchain with the wasm32-unknown-unknown
# target, wasm-pack, and node. The same toolchain that builds the
# browser extensions also runs the demo -- see
# BROWSER_EXTENSION_INSTRUCTIONS.md at the repo root.

set -euo pipefail

# Resolve the demo directory from wherever this script lives.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WASM_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
WASM_PKG="${WASM_DIR}/pkg"
DIST_MAIN="${SCRIPT_DIR}/dist/main.js"

# True when something the demo depends on is newer than the last build.
# The engine (`src/`, the root `Cargo.toml`) and the generated stem maps
# (`data/precomputed/`) are part of the wasm, not just `torille-wasm/src`
# -- checking only the latter meant an engine change left a stale wasm
# and the demo kept the old spell-checker.
wasm_needs_build() {
  [ ! -f "${WASM_PKG}/torille_wasm_bg.wasm" ] && return 0
  [ -n "$(find "${WASM_DIR}/src" "${WASM_DIR}/Cargo.toml" \
                     "${WASM_DIR}/../src" "${WASM_DIR}/../Cargo.toml" \
                     "${WASM_DIR}/../data/precomputed" \
        -newer "${WASM_PKG}/torille_wasm_bg.wasm" \
        -print -quit 2>/dev/null)" ] && return 0
  return 1
}

demo_needs_build() {
  [ ! -f "${DIST_MAIN}" ] && return 0
  [ -n "$(find "${SCRIPT_DIR}/src" "${SCRIPT_DIR}/index.html" \
                     "${SCRIPT_DIR}/build.mjs" \
        -newer "${DIST_MAIN}" \
        -print -quit 2>/dev/null)" ] && return 0
  return 1
}

# 1. wasm-pack build. Only when the Rust source is newer than the
#    `pkg/` output (or `pkg/` is missing). wasm-pack is the only
#    step that compiles Rust; everything after is JS.
if [ "${FORCE:-0}" = "1" ] || wasm_needs_build; then
  if ! command -v wasm-pack >/dev/null 2>&1; then
    echo "error: wasm-pack is not on PATH. Install with: cargo install wasm-pack" >&2
    exit 1
  fi
  echo "==> Building the wasm (this can take a couple of minutes the first time)"
  (cd "${WASM_DIR}" && wasm-pack build --release)
fi

# 2. Demo bundle: esbuild compiles the Solid.js app + the spell
#    worker, inlines the wasm as a Uint8Array, writes `dist/`.
if [ "${FORCE:-0}" = "1" ] || demo_needs_build; then
  if ! command -v node >/dev/null 2>&1; then
    echo "error: node is not on PATH." >&2
    exit 1
  fi
  echo "==> Building the demo"
  (cd "${SCRIPT_DIR}" && node build.mjs)
fi

# 3. Serve. `serve-demo` (the `torille-demo-server` crate) is a
#    CORS-enabled static server for `dist/` and the local `books/`
#    corpus; the reader needs no cross-origin fetch.
PORT="${PORT:-8765}"
REPO_ROOT="$(cd "${WASM_DIR}/.." && pwd)"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is not on PATH (needed to build the demo server)." >&2
  exit 1
fi

echo "==> Building the demo server"
(cd "${REPO_ROOT}" && cargo build --release -p torille-demo-server)

# The catalog is generated from the texts on disk, so a partial fetch or a
# stale hand-written catalog cannot hide books that are already present.
if [ -d "${SCRIPT_DIR}/books" ] && compgen -G "${SCRIPT_DIR}/books/*.txt" >/dev/null; then
  if command -v python3 >/dev/null 2>&1; then
    echo "==> Rebuilding the book catalog"
    python3 "${REPO_ROOT}/tools/rebuild_book_catalog.py" --books-dir "${SCRIPT_DIR}/books"
  else
    echo "warning: python3 not found; using the existing books/catalog.json" >&2
  fi
elif [ ! -f "${SCRIPT_DIR}/books/catalog.json" ]; then
  echo ""
  echo "note: the book corpus is not downloaded yet (books/catalog.json is missing)."
  echo "      Run:  python3 tools/fetch_finnish_books.py"
  echo "      (a few GB of text; the reader shows nothing until then.)"
fi

echo ""
echo "torille-wasm demo:  http://localhost:${PORT}/"
echo "(Ctrl+C to stop)"
echo ""

exec "${REPO_ROOT}/target/release/serve-demo" --root "${SCRIPT_DIR}" --port "${PORT}"
