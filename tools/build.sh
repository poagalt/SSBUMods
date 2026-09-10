#!/usr/bin/env bash
# Full reproducible build. Run this rather than the individual passes -- order matters.
#
# The architecture is ADR 0003: Pikachu plays the SOURCE fighter's own animation, so a ported
# ACMD's original bone anchors are correct by construction. Two passes correct the one thing an
# animation drop-in cannot carry -- the fighter `scale` param, 0.96 on Pikachu against up to
# 1.15 on a source:
#   anim-dropin  scales the animation's `Rot` node, which inherits down every bone and so fixes
#                bone-anchored hitboxes and their bone-local offsets in one place
#   topscale     scales `top`-anchored offsets, which bypass the skeleton entirely
#
# Two passes that used to live here are gone: `reanchor` (re-expressed limb hitboxes relative to
# `top`, obsolete and actively harmful on top of a dropped-in animation) and `lunge` (synthesised
# Little Mac's root motion, which his animation now supplies natively via Trans).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export PATH="/c/Users/poaga/.cargo/bin:/c/Users/poaga/w64devkit/bin:$PATH"

echo "== 1. generate move scripts from vanilla ACMD (verbatim, original bone anchors) =="
bash "$ROOT/tools/gen-moves.sh"

echo; echo "== 1b. strip the source fighters' own namespace constants =="
"$ROOT/tools/reach/target/release/deforeign.exe" "$ROOT/pikachu-moveset/plugin/src" | tail -1

echo; echo "== 2. graft on weapon bones the ported hitboxes need =="
bash "$ROOT/tools/add-bones.sh"

echo; echo "== 3. ship the source animations, scale-compensated =="
bash "$ROOT/tools/anim-dropin.sh" 2>&1 | grep -v "^Parse:" | tail -3

echo; echo "== 4. scale top-anchored hitbox offsets =="
"$ROOT/tools/reach/target/release/topscale.exe" "$ROOT/pikachu-moveset/plugin/src" | tail -1

echo; echo "== 5. pre-flight the motion entries against the dropped-in animations =="
"$ROOT/tools/motion-patch/target/release/dropin.exe" "$ROOT/extracted" \
  "$ROOT/pikachu-moveset/mod/fighter/pikachu/motion/body/c00/motion_list.bin" \
  chrom:attack_11 elight:attack_dash elight:attack_s3_s falco:attack_hi3 littlemac:attack_s4_s \
  fox:attack_hi4 palutena:attack_air_n demon:attack_air_f cloud:attack_air_b \
  kamui:attack_air_hi krool:attack_air_lw | tail -1

echo; echo "== 6. compile =="
cd "$ROOT/pikachu-moveset/plugin"
cargo skyline build --release 2>&1 | grep -E "^error|error\[|Finished" | head -8
