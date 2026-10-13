# Measurements

Stage 4, recorded by the agent. `B2 Require step-by-step justification`.

The starting state, measured rather than recalled, so later changes can be compared with it.

| What | Command | Result | When |
| --- | --- | --- | --- |
| Toolchain | `rustc --version; wasm-bindgen --version` | rustc 1.99.0; wasm-bindgen 0.2.127 | 13 Oct 2026, before any code |
| Starting point | `ls` | an empty folder; Builder's template and Raiders of the Lost Mart (same request, a 3D Builder game) one directory up | 13 Oct 2026 |
| The map | `python3 -I tools/extract_map.py surf_utopia_v3.bsp` | 4,407 brushes, 105 trigger brushes, 841 KB | 13 Oct 2026 |
| Core tests | `cargo test --workspace` | see HANDOFF.md | at handoff |
| Full check | `./scripts/check.sh` | see HANDOFF.md | at handoff |
