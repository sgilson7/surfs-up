#!/usr/bin/env bash
# The full local pre-deploy check. CI runs this same file.
#
#   ./scripts/check.sh                 everything
#   BUILDER_ENGINES=chromium ./scripts/check.sh   one engine while iterating
#
# Each step prints its name first, so a failure names the step that failed.
set -euo pipefail
cd "$(dirname "$0")/.."

PY="${BUILDER_PYTHON:-}"
if [ -z "$PY" ]; then
  if [ -x .venv/bin/python ]; then PY=.venv/bin/python; else PY=python3; fi
fi

step() { printf '\n\033[1m== %s\033[0m\n' "$*"; }

step "1/8 format (cargo fmt --check)"
cargo fmt --all -- --check

step "2/8 lint (cargo clippy, warnings are errors)"
cargo clippy --workspace --all-targets --quiet -- -D warnings

step "3/8 core and boundary tests (cargo test)"
cargo test --workspace --quiet

step "4/8 surface sweep: every floor, ramp and trigger on every course (crates/core/tests/sweep.rs)"
cargo test --release --quiet -p surf_core --test sweep -- --ignored every_surface_and_trigger_on_every_course

step "5/8 release WebAssembly build (scripts/build.sh)"
./scripts/build.sh >/dev/null
echo "dist/ assembled"

step "6/8 WebAssembly import audit (scripts/wasm_imports.py)"
"$PY" scripts/wasm_imports.py dist

step "7/8 browser check (tests/browser/check.py)"
"$PY" tests/browser/check.py

step "8/8 online: eight browsers in one room, by pasted codes (tests/browser/online.py)"
"$PY" tests/browser/online.py ${BUILDER_ENGINES:-chromium firefox webkit}

printf '\n\033[1mBUILDER CHECK PASSED\033[0m\n'
