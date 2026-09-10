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
# WHAT IS LOST BY TURNING IT OFF: the whipped victim's engine-side statuses read
# special_s_through_target_speed off the grabber, and Pikachu's own param_special_s has no such
# key, so those lookups return 0. Alolan Whip's rebound distance may therefore be wrong or absent.
# That is a worse move, not a broken game, and it trades correctly against a crash on load.
#
# TO TRY IT AGAIN, the thing to establish first is whether smashline tolerates a replaced param
# object at all, or only an ADDED one. If only added, the way to get these numbers to the engine is
# a differently-named object plus a plugin-side reader, not a replacement.
SHIP_PARAM_SPECIAL_S=0
# ============================================================================================
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VAN="$ROOT/extracted/fighter/pikachu/param/vl.prc"
OUT="$ROOT/pikachu-moveset/mod/fighter/pikachu/param/vl.prc"
VLMERGE="$ROOT/tools/prc-tool/target/release/vlmerge.exe"

if [ "$SHIP_PARAM_SPECIAL_S" = "0" ]; then
    rm -f "$OUT"
    echo "  param_special_s override DISABLED -- Pikachu keeps his vanilla vl.prc"
    echo "  (it null-dereferenced in smashline's load_fighter_effects on fighter load)"
    exit 0
fi

[ -f "$VAN" ] || { echo "no extracted vanilla vl.prc at $VAN" >&2; exit 1; }

mkdir -p "$(dirname "$OUT")"
# Always rebuild from the VANILLA file, never from the previous output. Rerunning against our own
# product is how topscale ended up double-scaling every offset.
"$VLMERGE" --replace \
  "$VAN" "$ROOT/extracted/fighter/gaogaen/param/vl.prc" param_special_s "$OUT" | sed 's/^/  /'
