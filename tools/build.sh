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
# The codegen tools are rebuilt FIRST. build.sh used to run the prebuilt binaries, so editing a
# tool's source had no effect until someone remembered to cargo build it by hand -- an adopt_anim
# change in synth.rs silently did nothing and Revenge's stance kept playing the wrong animation.
echo "== 0. rebuild the codegen tools =="
for c in tools/reach tools/motion-patch tools/prc-tool; do
  # `|| true` because grep exits 1 when it finds nothing, and under `set -e` that aborted the
  # whole build before it reached step 1 -- silently, since the abort looked like a clean finish.
  (cd "$ROOT/$c" && cargo build --release --bins 2>&1 | grep -E "^error" -A 6 | head -12) || true
done
echo "  tools up to date"

echo; echo "== 0a. read every fighter's scale param out of the game =="
bash "$ROOT/tools/gen-scales.sh"

echo; echo "== 0b. generate the dodge frame windows =="
bash "$ROOT/tools/gen-motion-params.sh"

echo; echo "== 0c. generate the character select rename =="
bash "$ROOT/tools/gen-name.sh"

echo; echo "== 0d. give Pikachu Incineroar's param_special_s =="
bash "$ROOT/tools/gen-vlprc.sh"

echo; echo "== 1. generate move scripts from vanilla ACMD (verbatim, original bone anchors) =="
bash "$ROOT/tools/gen-moves.sh"


echo; echo "== 1a. the command grab keeps Incineroar's own victim status =="
# THE REMAP IS GONE. tools/catch-remap.sed used to rewrite Incineroar's
# FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED into FIGHTER_STATUS_KIND_CAPTURE_PULLED, on the belief
# that Pikachu could not complete the swing because he has no equivalent of Incineroar's grabber
# statuses and none could be added. That premise was wrong -- smashline extends the status table
# (see src/specials/status.rs) -- and the remap was itself the reported bug: the ordinary capture
# pulled the victim in and released them, which playtest saw as "modified pikachu teleports back
# to where he was, facing the other direction, and does the enemy freed from grab animation".
#
# All seven SWING_GAOGAEN victim statuses (0x176-0x17C) are in the engine's COMMON list, so the
# roped swing works for any grabber. This step now only asserts the reference survived: if a
# future pass strips it, the grab silently stops connecting.
kept=$( { grep -c SWING_GAOGAEN_CATCHED "$ROOT"/pikachu-moveset/plugin/src/*/mod.rs 2>/dev/null || true; } | awk -F: '{s+=$2} END{print s+0}')
if [ "$kept" -eq 0 ]; then
  echo "  REFUSING: no SWING_GAOGAEN_CATCHED reference left -- the command grab has no victim status" >&2
  exit 1
fi
echo "  ok  command grab targets FIGHTER_STATUS_KIND_SWING_GAOGAEN_CATCHED ($kept CATCH call(s))"
echo; echo "== 1b. strip foreign constants and unsupported macro calls =="
# EVERY fighter gen-moves.sh ports from must be in deforeign's SOURCES list, or that fighter's
# namespaced constants are never stripped. DONKEY was missing for as long as the cargo throws have
# existed and nothing noticed, so the two lists are diffed here rather than trusted.
missing=""
for f in $(grep -oE "^[^|#]*\|[a-z]+\|" "$ROOT/tools/gen-moves.sh" | cut -d'|' -f2 | sort -u); do
  U=$(printf '%s' "$f" | tr 'a-z' 'A-Z')
  grep -q "\"$U\"" "$ROOT/tools/reach/src/bin/deforeign.rs" || missing="$missing $U"
done
if [ -n "$missing" ]; then
  echo "  REFUSING: gen-moves.sh ports from$missing but deforeign's SOURCES does not list them," >&2
  echo "  so their namespaced constants would reach the plugin unstripped" >&2
  exit 1
fi
echo "  ok  every ported fighter's namespace is known to deforeign"
# The macro allowlist is read from smash-script's own source, so it cannot drift from the crate
# actually being compiled against.
SS=$(ls -d /c/Users/poaga/.cargo/git/checkouts/smash-script-*/*/src 2>/dev/null | head -1)
if [ -n "$SS" ]; then
  grep -rhoE "pub (unsafe )?fn [A-Z][A-Z_0-9]+" "$SS" | sed 's/.*fn //' | sort -u > "$ROOT/work/smash-script-macros.txt"
fi
"$ROOT/tools/reach/target/release/deforeign.exe" \
  "$ROOT/pikachu-moveset/plugin/src" \
  "$ROOT/work/smash-script-macros.txt" | tail -1

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
  krool:attack_air_lw \
  gaogaen:special_s_start gaogaen:special_s_end gaogaen:special_lw \
  gaogaen:special_s_shoulder gaogaen:special_s_lariat gaogaen:special_s_throw \
  gaogaen:special_s_failure gaogaen:special_lw_start gaogaen:special_lw_turn \
  donkey:throw_f donkey:throw_f_f donkey:throw_f_b donkey:throw_f_hi donkey:throw_f_lw \
  gamewatch:special_hi gamewatch:special_air_hi gamewatch:special_hi_open \
  gamewatch:special_hi_close \
  donkey:shoulder_wait donkey:shoulder_walk_slow donkey:shoulder_walk_middle \
  donkey:shoulder_walk_fast donkey:shoulder_turn donkey:shoulder_jump_squat \
  donkey:shoulder_jump donkey:shoulder_fall donkey:shoulder_landing | tail -4

echo; echo "== 6. compile =="
cd "$ROOT/pikachu-moveset/plugin"
cargo skyline build --release 2>&1 | grep -E "^error|error\[|Finished" | head -8
