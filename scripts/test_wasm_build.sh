#!/usr/bin/env bash
# Tests that the contract Makefile's build and optimize targets produce the
# expected wasm files, and that the soroban-sdk test host can load them (#341).
#
# Requires the wasm32v1-none Rust target and the stellar CLI (see
# contracts/sorobanlard/Makefile for toolchain assumptions).
#
# Usage: scripts/test_wasm_build.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CONTRACT_DIR="$ROOT/contracts/sorobanlard"
WASM="$ROOT/target/wasm32v1-none/release/sorobanlard.wasm"
WASM_OPT="$ROOT/target/wasm32v1-none/release/sorobanlard.optimized.wasm"

fail() { echo "FAIL: $*" >&2; exit 1; }

assert_wasm() {
  [ -f "$1" ] || fail "$1 was not produced"
  [ -s "$1" ] || fail "$1 is empty"
  echo "ok: $1 ($(wc -c < "$1" | tr -d ' ') bytes)"
}

rm -f "$WASM" "$WASM_OPT"

echo "== make build"
make -C "$CONTRACT_DIR" build
assert_wasm "$WASM"

echo "== make optimize"
make -C "$CONTRACT_DIR" optimize
assert_wasm "$WASM_OPT"

echo "== load wasm in soroban-sdk test host"
for wasm in "$WASM" "$WASM_OPT"; do
  SOROBANLARD_WASM="$wasm" cargo test --manifest-path "$CONTRACT_DIR/Cargo.toml" \
    --test wasm_load -- --ignored
done

echo "All wasm build tests passed."
