#!/usr/bin/env bash
# Full reproducible build: generate move scripts from vanilla ACMD, apply the reach and lunge
# passes, then compile. Run this rather than the individual passes -- order matters.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export PATH="/c/Users/poaga/.cargo/bin:/c/Users/poaga/w64devkit/bin:$PATH"

echo "== 1. generate move scripts from vanilla ACMD =="
bash "$ROOT/tools/gen-moves.sh"

echo; echo "== 2. re-anchor limb hitboxes to their true world reach =="
"$ROOT/tools/reach/target/release/reanchor.exe" "$ROOT/pikachu-moveset/plugin/src" "$ROOT/work/fk" | tail -4

echo; echo "== 3. synthesise Little Mac's f-smash lunge =="
bash "$ROOT/tools/lunge.sh"

echo; echo "== 4. compile =="
cd "$ROOT/pikachu-moveset/plugin"
cargo skyline build --release 2>&1 | grep -E "^error|error\[|Finished" | head -8
