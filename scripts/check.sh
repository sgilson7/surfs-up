#!/usr/bin/env bash
# The full local pre-deploy check. CI runs this same file.
#
#   ./scripts/check.sh                 everything
#   BUILDER_ENGINES=chromium ./scripts/check.sh   one engine while iterating
#   BUILDER_ONLINE=0 ./scripts/check.sh           without the online rooms (CI)
#
# The online rooms (a room of four on every course) take about 24 minutes
# here and about an hour on CI's two-core runner, so CI skips them and they
# are run locally, in full, before each push.
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

if [ "${BUILDER_ONLINE:-1}" = 0 ]; then
  step "8/8 online: skipped (BUILDER_ONLINE=0; run locally before each push)"
else
  step "8/8 online: eight browsers in one room, then four on every course (tests/browser/online.py)"
  "$PY" -u tests/browser/online.py ${BUILDER_ENGINES:-chromium firefox webkit}
fi

printf '\n\033[1mBUILDER CHECK PASSED\033[0m\n'
