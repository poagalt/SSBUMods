#!/usr/bin/env bash
# Full reproducible build: generate move scripts verbatim from vanilla ACMD, ship the source
# animations that make those scripts correct, then compile. Run this rather than the individual
# passes -- order matters.
#
# ADR 0003 removed two passes that used to live here:
#   - reanchor: re-expressed limb hitboxes relative to `top` to compensate for Pikachu playing his
#     own animation. Obsolete now that he plays the source's, and actively harmful on top of it.
#   - lunge: synthesised Little Mac's f-smash root motion with SET_SPEED_EX. His animation's
#     `Trans` track supplies it natively, so keeping this would double-count the movement.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export PATH="/c/Users/poaga/.cargo/bin:/c/Users/poaga/w64devkit/bin:$PATH"

echo "== 1. generate move scripts from vanilla ACMD (verbatim, original bone anchors) =="
bash "$ROOT/tools/gen-moves.sh"

echo; echo "== 2. ship the source animations those scripts assume =="
bash "$ROOT/tools/anim-dropin.sh"

echo; echo "== 3. pre-flight the motion entries against the dropped-in animations =="
"$ROOT/tools/motion-patch/target/release/dropin.exe" "$ROOT/extracted" \
  "$ROOT/pikachu-moveset/mod/fighter/pikachu/motion/body/c00/motion_list.bin" \
  chrom:attack_11 elight:attack_dash elight:attack_s3_s falco:attack_hi3 littlemac:attack_s4_s \
  fox:attack_hi4 palutena:attack_air_n demon:attack_air_f cloud:attack_air_b \
  kamui:attack_air_hi krool:attack_air_lw | tail -3

echo; echo "== 4. compile =="
cd "$ROOT/pikachu-moveset/plugin"
cargo skyline build --release 2>&1 | grep -E "^error|error\[|Finished" | head -8
