#!/usr/bin/env bash
# Full reproducible build. Run this rather than the individual passes -- order matters, and
# topscale is not idempotent.
#
# The architecture is ADR 0003: Pikachu plays the SOURCE fighter's own animation, so a ported
# ACMD's original bone anchors are correct by construction. Two passes correct the one thing an
# animation drop-in cannot carry -- the fighter `scale` param, 0.96 on Pikachu against up to
# 1.11 on a source:
#   anim-dropin  scales the animation's `Rot` node, which inherits down every bone and so fixes
#                bone-anchored hitboxes and their bone-local offsets in one place
#   topscale     scales `top`-anchored offsets, which bypass the skeleton entirely
#
# Both read tools/scales.tsv, generated in step 0 from the game's own fighter_param.prc. Those
# numbers were hand-entered once and two were wrong (Cloud 1.03 vs 0.98, K. Rool 1.15 vs 0.89),
# which the hitbox trace could not detect because it used the same constant on both sides.
#
# Two passes that used to live here are gone: `reanchor` (re-expressed limb hitboxes relative to
# `top`, obsolete and actively harmful on top of a dropped-in animation) and `lunge` (synthesised
# Little Mac's root motion, which his animation now supplies natively via Trans).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# w64devkit goes LAST. Prepending it shadowed six core utilities with minimal builds, and its
# grep returns 0 for a pattern containing "/" -- so tools/check-name.sh passed standalone and
# failed inside the build, on a byte-identical file. A check whose result depends on which grep
# is first on PATH is worse than no check. cargo-skyline only needs w64devkit for the linker,
# which it still finds when appended (verified with a forced relink).
export PATH="/c/Users/poaga/.cargo/bin:$PATH:/c/Users/poaga/w64devkit/bin"

echo "== 0. read every fighter's scale param out of the game =="
bash "$ROOT/tools/gen-scales.sh"

echo; echo "== 0a. generate the dodge frame windows =="
bash "$ROOT/tools/gen-motion-params.sh"

echo; echo "== 0b. generate the character select rename =="
bash "$ROOT/tools/gen-name.sh"

echo; echo "== 1. generate move scripts from vanilla ACMD (verbatim, original bone anchors) =="
bash "$ROOT/tools/gen-moves.sh"

echo; echo "== 1b. strip the source fighters' own namespace constants =="
"$ROOT/tools/reach/target/release/deforeign.exe" "$ROOT/pikachu-moveset/plugin/src" | tail -1

echo; echo "== 1c. verify no effect Pikachu cannot load survived (ADR 0002) =="
"$ROOT/tools/reach/target/release/sysfx.exe" "$ROOT/pikachu-moveset/plugin/src" | tail -2

echo; echo "== 2. graft on weapon bones the ported hitboxes need =="
bash "$ROOT/tools/add-bones.sh" | tail -1

echo; echo "== 3. ship the source animations, scale-compensated =="
bash "$ROOT/tools/anim-dropin.sh" 2>&1 | grep -v "^Parse:" | tail -1

# The shipped motion_list was previously built by hand from RESTORE.md; regenerating it here
# means a change to the FAF/flag table or to synth cannot silently fail to reach the console.
echo; echo "== 3b. rebuild motion_list.bin (FAF, intangibility, flags, synthesised entries) =="
MP="$ROOT/tools/motion-patch/target/release"
ML="$ROOT/pikachu-moveset/mod/fighter/pikachu/motion/body/c00/motion_list.bin"
mkdir -p "$ROOT/work/ml"
"$MP/motion-patch.exe" "$ROOT/extracted" "$ROOT/work/ml/patched.bin" | tail -1
"$MP/synth.exe" "$ROOT/extracted" "$ROOT/work/ml/patched.bin" "$ML" | tail -1

# Must run exactly once, on the verbatim source produced in step 1: it multiplies offsets in place.
echo; echo "== 4. scale top-anchored hitbox offsets =="
"$ROOT/tools/reach/target/release/topscale.exe" \
  "$ROOT/pikachu-moveset/plugin/src" "$ROOT/tools/scales.tsv" | tail -1

echo; echo "== 5. pre-flight the motion entries against the dropped-in animations =="
"$ROOT/tools/motion-patch/target/release/dropin.exe" "$ROOT/extracted" \
  "$ROOT/pikachu-moveset/mod/fighter/pikachu/motion/body/c00/motion_list.bin" \
  chrom:attack_11 elight:attack_dash elight:attack_s3_s falco:attack_hi3 robot:attack_lw3 \
  littlemac:attack_s4_s fox:attack_hi4 gamewatch:attack_lw4 \
  palutena:attack_air_n demon:attack_air_f cloud:attack_air_b kamui:attack_air_hi \
  krool:attack_air_lw | tail -1

echo; echo "== 6. compile =="
cd "$ROOT/pikachu-moveset/plugin"
cargo skyline build --release 2>&1 | grep -E "^error|error\[|Finished" | head -8
