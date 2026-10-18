#!/usr/bin/env bash
# Builds the vl.prc the mod ships for Pikachu, from his own vanilla one.
#
# WHY HE NEEDS A MODIFIED ONE AT ALL. Alolan Whip's victim side is common engine code -- the seven
# FIGHTER_STATUS_KIND_SWING_GAOGAEN_* statuses, which is exactly why the move is portable at all --
# and that code reads its numbers off the GRABBER's `param_special_s`:
#
#   special_s_through_target_speed           11.2   how fast the victim is whipped at the ropes
#   special_s_through_target_return_speed     2.8   how fast they rebound
#   special_s_through_target_cancel_frame      30   how long the outbound leg lasts
#   special_s_through_target_speed_no_rope    1.6   the same throw with no ropes to bounce off
#
# Pikachu ALREADY HAS a `param_special_s`. It is Skull Bash's -- charge rates, flick thresholds --
# and it contains none of those keys, so every lookup the victim's status makes returns 0 and the
# whipped opponent goes nowhere. The object has to be REPLACED with Incineroar's, not merged
# alongside it, which is what vlmerge's --replace is for.
#
# WHAT IS DELIBERATELY NOT COPIED
#   param_special_lw   Revenge is implemented entirely plugin-side and reads nothing from vl.prc,
#                      so overwriting Thunder's params would buy nothing and risk something.
#   param_special_hi   same: the G&W up-B port hardcodes its two multipliers from his own status
#                      code, and Quick Attack's params are not consulted by anything left running.
#   param_spikeball    an ARTICLE param, and article params must track articles/mod.rs
#                      CLONE_ENABLED exactly -- see tools/check.sh step 3b. Adding one with no
#                      weapon behind it takes Pikachu from 20 article params to 21, and Training
#                      Modpack's save state walks article indices calling get_article and
#                      remove_exist_object_id.
#
# Regenerate with:  bash tools/gen-vlprc.sh
#
# ============================================================================================
# DISABLED, AND THIS IS WHY. Shipping this crashed Pikachu on FIGHTER LOAD.
#
# The v0.24 crash report (work/crash/01789067844_*.log) is a null dereference on TaskWorker2 with
# `smashline + 0x1769c (load_fighter_effects + 0x48)` as the top frame. That is the SAME signature
# as the article-clone crash documented in src/articles/mod.rs, and the cause there was also a
# vl.prc disagreement: smashline's params.rs (init_fighter_p_object) reads the HOST fighter's own
# vl.prc to build its param object list, and a layout it does not expect null-derefs during
# fighter init.
#
# And the diff is unambiguous. Between the v0.23 boot and the v0.24 boot, the only param file that
# changed is this one -- `git diff --stat v0.23 v0.24 -- pikachu-moveset/` reports exactly
# `fighter/pikachu/param/vl.prc | Bin 0 -> 7293 bytes`. Incineroar's param_special_s is a list[4]
# of ~50 entries against Pikachu's Skull Bash object; same shape, different size.
#
# NOTHING IS LOST BY TURNING IT OFF ANY MORE, and this file is now kept for the crash it records
# rather than for the numbers it was written to ship.
#
# The numbers reach the engine a better way. `plugin/src/params/mod.rs` hooks
# `WorkModule::get_param_int` and `get_param_float` and answers the twelve `param_special_s`
# lookups the VICTIM's own status makes through the grabber's accessor -- the list is
# disassembled out of lua2cpp_common.nro and checked against this same prc by tools/check.sh step
# 3e2. That is the "plugin-side reader" the previous note asked for, except it needs no
# differently-named object either: it answers for the real key, so the engine's code needs no
# cooperation from us. No file ships, no param layout changes, and there is nothing to load at the
# wrong moment.
#
# The one thing the hook cannot do is give Pikachu an ARTICLE. If the rope is ever wanted, that is
# a weapon clone and a vl.prc article param, and this generator is where that would start.
SHIP_PARAM_SPECIAL_S=0
# ============================================================================================
# THE WHIP'S LEDGE BOX (2026-10-12), the one other thing a vl.prc can give that the hook cannot.
#
# Incineroar's SPECIAL_S main calls `GroundModule::select_cliff_hangdata(boma,
# FIGHTER_GAOGAEN_CLIFF_HANG_DATA_SPECIAL_S)`, and that constant is 1: an INDEX into the fighter's
# own `cliff_hang_data` list in vl.prc, which native GroundModule code reads -- so the param hook
# cannot answer it. Incineroar's list has four boxes; Pikachu's has one, his ordinary ledge grab.
# `vlappend` adds Incineroar's [1] as Pikachu's [1], scaled by 1.00 / 0.96 (tools/scales.tsv) like
# a top-anchored offset, and keeps Pikachu's [0] untouched.
#
# NOT DERIVED FROM A FLAG HERE: it follows `WHIP_CLIFF_HANGDATA` in plugin/src/specials/whip/mod.rs,
# so the plugin's call and the box it selects cannot disagree. Off by default, because the last
# vl.prc this mod shipped crashed on fighter load (above) and that cause is not settled -- see
# check.sh 3b's comment, which reads the same crash differently. Arming it is a single-variable
# experiment for a watched boot, like CLONE_ENABLED.
# ============================================================================================
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VAN="$ROOT/extracted/fighter/pikachu/param/vl.prc"
OUT="$ROOT/pikachu-moveset/mod/fighter/pikachu/param/vl.prc"
VLMERGE="$ROOT/tools/prc-tool/target/release/vlmerge.exe"
VLAPPEND="$ROOT/tools/prc-tool/target/release/vlappend.exe"
WHIP_SRC="$ROOT/pikachu-moveset/plugin/src/specials/whip/mod.rs"
SHIP_WHIP_CLIFF_HANG=0
grep -q '^pub const WHIP_CLIFF_HANGDATA: bool = true;' "$WHIP_SRC" && SHIP_WHIP_CLIFF_HANG=1

if [ "$SHIP_PARAM_SPECIAL_S" = "0" ] && [ "$SHIP_WHIP_CLIFF_HANG" = "0" ]; then
    rm -f "$OUT"
    echo "  param_special_s override DISABLED -- Pikachu keeps his vanilla vl.prc"
    echo "  (it null-dereferenced in smashline's load_fighter_effects on fighter load)"
    echo "  whip ledge box DISABLED (WHIP_CLIFF_HANGDATA = false in specials/whip/mod.rs)"
    exit 0
fi

[ -f "$VAN" ] || { echo "no extracted vanilla vl.prc at $VAN" >&2; exit 1; }

mkdir -p "$(dirname "$OUT")"
# Always rebuild from the VANILLA file, never from the previous output. Rerunning against our own
# product is how topscale ended up double-scaling every offset.
TMP="$OUT.tmp"
cp "$VAN" "$TMP"
if [ "$SHIP_PARAM_SPECIAL_S" = "1" ]; then
    "$VLMERGE" --replace \
      "$TMP" "$ROOT/extracted/fighter/gaogaen/param/vl.prc" param_special_s "$TMP" | sed 's/^/  /'
fi
if [ "$SHIP_WHIP_CLIFF_HANG" = "1" ]; then
    # Incineroar's scale over Pikachu's, from the game's own fighter_param.prc via tools/scales.tsv.
    ratio=$(awk '$1=="gaogaen"{g=$3} $1=="pikachu"{p=$3} END{printf "%.7f", g/p}' "$ROOT/tools/scales.tsv")
    "$VLAPPEND" "$TMP" "$ROOT/extracted/fighter/gaogaen/param/vl.prc" cliff_hang_data 1 1 "$TMP" \
      --scale "$ratio" | sed 's/^/  /'
fi
mv "$TMP" "$OUT"
